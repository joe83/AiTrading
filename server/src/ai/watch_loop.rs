use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use super::grok_client::ChatMessage;
use crate::config::WatchConfig;
use crate::models::{ExchangeId, SignalAction, SignalSource, TradingSignal};
use crate::AppState;

const MAX_POSTS_PER_TICK: usize = 5;

/// What the dashboard shows for the sleepless watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStatus {
    pub enabled: bool,
    pub provider: String,
    pub handles: Vec<String>,
    pub interval_secs: u64,
    pub running: bool,
    pub last_tick_at: Option<String>,
    pub last_result: String,
    pub last_error: Option<String>,
}

impl Default for WatchStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "webhook".to_string(),
            handles: Vec::new(),
            interval_secs: 300,
            running: false,
            last_tick_at: None,
            last_result: "Not started".to_string(),
            last_error: None,
        }
    }
}

/// An incoming post from any ingestion channel (Webhook, Scraper API, or Grok search).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingPost {
    pub handle: String,
    pub text: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Result of evaluating an incoming post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResult {
    pub post_id: String,
    pub handle: String,
    pub already_seen: bool,
    pub is_stale: bool,
    pub queued: bool,
    pub symbol: Option<String>,
    pub side: Option<String>,
    pub confidence: Option<f64>,
    pub reason: String,
}

/// Supervisor loop that checks config dynamically and manages the scanning cycle.
pub async fn run(state: Arc<AppState>) {
    info!("X Watch Loop supervisor started");
    loop {
        let config = state.config.read().await.watch.clone();
        if !config.enabled || config.handles.is_empty() {
            set_status(&state, |status| {
                status.enabled = false;
                status.running = false;
                status.provider = config.provider.clone();
                status.handles = config.handles.clone();
                status.interval_secs = config.interval_secs;
                status.last_result = "Watcher is paused or turned off".to_string();
            })
            .await;
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        let interval_secs = config.interval_secs.max(30);
        set_status(&state, |status| {
            status.enabled = true;
            status.running = true;
            status.provider = config.provider.clone();
            status.handles = config.handles.clone();
            status.interval_secs = interval_secs;
        })
        .await;

        if config.provider == "webhook" {
            set_status(&state, |status| {
                status.last_result = "Listening for incoming webhooks / external feed (0 polling fees)".to_string();
                status.last_error = None;
            })
            .await;
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        // Active polling (scraper or grok mode)
        if let Err(error) = tick(&state, &config).await {
            warn!("X watch tick failed: {error}");
            let message = error.to_string();
            set_status(&state, |status| {
                status.last_tick_at = Some(Utc::now().to_rfc3339());
                status.last_error = Some(message.clone());
                status.last_result = "Scan failed".to_string();
            })
            .await;
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}

/// Run an immediate on-demand scan.
pub async fn tick_now(state: &AppState) -> anyhow::Result<String> {
    let config = state.config.read().await.watch.clone();
    tick(state, &config).await?;
    let status = state.watch_status.read().await.clone();
    Ok(status.last_result)
}

async fn tick(state: &AppState, config: &WatchConfig) -> anyhow::Result<()> {
    let posts = match config.provider.as_str() {
        "scraper" => fetch_posts_from_scraper(config).await?,
        "grok" => fetch_posts_from_grok(state, config).await?,
        _ => Vec::new(),
    };

    let found = posts.len();
    let mut queued = 0u32;
    for post in posts.iter().take(MAX_POSTS_PER_TICK) {
        match process_incoming_post(state, config, post).await {
            Ok(result) => {
                if result.queued {
                    queued += 1;
                }
            }
            Err(error) => warn!("X post processing skipped: {error}"),
        }
    }

    set_status(state, |status| {
        status.last_tick_at = Some(Utc::now().to_rfc3339());
        status.last_error = None;
        status.last_result = format!("{found} posts scanned, {queued} queued for trade review");
    })
    .await;
    info!("X watch tick completed: {found} posts, {queued} queued");
    Ok(())
}

async fn set_status(state: &AppState, update: impl FnOnce(&mut WatchStatus)) {
    let mut status = state.watch_status.write().await;
    update(&mut status);
}

/// Fetch posts using a lightweight third-party scraper API or custom JSON feed.
async fn fetch_posts_from_scraper(config: &WatchConfig) -> anyhow::Result<Vec<IncomingPost>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()?;
    let mut all_posts = Vec::new();

    for handle in &config.handles {
        match config.scraper_provider.as_str() {
            "twitterapi_io" => {
                let url = format!(
                    "https://api.twitterapi.io/twitter/user/last_tweets?userName={handle}"
                );
                let mut req = client.get(&url);
                if !config.scraper_api_key.is_empty() {
                    req = req.header("X-API-Key", &config.scraper_api_key);
                }
                if let Ok(resp) = req.send().await {
                    if let Ok(body) = resp.json::<Value>().await {
                        if let Some(tweets) = body.pointer("/data/tweets").and_then(|t| t.as_array()) {
                            for t in tweets.iter().take(3) {
                                let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let text = t.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let created_at = t.get("createdAt").and_then(|v| v.as_str()).map(str::to_string);
                                if !text.is_empty() {
                                    all_posts.push(IncomingPost {
                                        handle: handle.clone(),
                                        text,
                                        id,
                                        created_at,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            "rapidapi" => {
                let url = format!(
                    "https://twitter-api45.p.rapidapi.com/timeline.php?screenname={handle}"
                );
                let mut req = client.get(&url);
                if !config.scraper_api_key.is_empty() {
                    req = req
                        .header("X-RapidAPI-Key", &config.scraper_api_key)
                        .header("X-RapidAPI-Host", "twitter-api45.p.rapidapi.com");
                }
                if let Ok(resp) = req.send().await {
                    if let Ok(body) = resp.json::<Value>().await {
                        if let Some(timeline) = body.get("timeline").and_then(|t| t.as_array()) {
                            for item in timeline.iter().take(3) {
                                let id = item.get("tweet_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let created_at = item.get("created_at").and_then(|v| v.as_str()).map(str::to_string);
                                if !text.is_empty() {
                                    all_posts.push(IncomingPost {
                                        handle: handle.clone(),
                                        text,
                                        id,
                                        created_at,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            "custom" => {
                if !config.custom_feed_url.is_empty() {
                    let url = config.custom_feed_url.replace("{handle}", handle);
                    if let Ok(resp) = client.get(&url).send().await {
                        if let Ok(body) = resp.json::<Value>().await {
                            if let Some(items) = body.get("posts").or_else(|| body.as_array().map(|_| &body)).and_then(|v| v.as_array()) {
                                for item in items.iter().take(3) {
                                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let created_at = item.get("created_at").and_then(|v| v.as_str()).map(str::to_string);
                                    if !text.is_empty() {
                                        all_posts.push(IncomingPost {
                                            handle: handle.clone(),
                                            text,
                                            id,
                                            created_at,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(all_posts)
}

/// Fetch posts using Grok Search (Safe mode with low token cap).
async fn fetch_posts_from_grok(state: &AppState, config: &WatchConfig) -> anyhow::Result<Vec<IncomingPost>> {
    let from_date = Utc::now().format("%Y-%m-%d").to_string();
    let handles = config.handles.join(", ");
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You search X and return only posts those accounts published. Reply with compact JSON.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "List latest posts by: {handles}. Return JSON {{\"posts\":[{{\"id\":\"...\",\"handle\":\"...\",\"text\":\"...\",\"created_at\":\"RFC3339\"}}]}}. Empty if none."
            ),
        },
    ];

    let response = state
        .ai_engine
        .grok
        .chat_x_accounts(messages, &config.handles, &from_date, 300)
        .await?;
    let body = json_object(&response.content).unwrap_or_else(|| json!({ "posts": [] }));
    let posts = body.get("posts").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut result = Vec::new();
    for p in posts {
        let handle = p.get("handle").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let text = p.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let created_at = p.get("created_at").and_then(|v| v.as_str()).map(str::to_string);
        if !handle.is_empty() && !text.is_empty() {
            result.push(IncomingPost {
                handle,
                text,
                id,
                created_at,
            });
        }
    }
    Ok(result)
}

/// Process an incoming post through the "Filter First, AI Second" architecture:
/// 1. Handle & text validation
/// 2. Post key deduplication ($0 LLM tokens spent if seen)
/// 3. Staleness check ($0 LLM tokens spent if stale)
/// 4. AI Reasoning (ONLY invoked when post is brand new and early!)
/// 5. Market price extreme validation
/// 6. Signal queueing
pub async fn process_incoming_post(
    state: &AppState,
    config: &WatchConfig,
    post: &IncomingPost,
) -> anyhow::Result<IngestResult> {
    let handle = post.handle.trim().trim_start_matches('@');
    let text = post.text.trim();
    if handle.is_empty() || text.is_empty() {
        return Ok(IngestResult {
            post_id: String::new(),
            handle: handle.to_string(),
            already_seen: false,
            is_stale: false,
            queued: false,
            symbol: None,
            side: None,
            confidence: None,
            reason: "Empty handle or text".to_string(),
        });
    }

    let post_id = post_key(handle, &post.id, text);

    // STEP 1 (Filter First): Check if already seen in database
    if state.db.has_seen_post(&post_id).await? {
        return Ok(IngestResult {
            post_id,
            handle: handle.to_string(),
            already_seen: true,
            is_stale: false,
            queued: false,
            symbol: None,
            side: None,
            confidence: None,
            reason: "Already seen in database".to_string(),
        });
    }

    // STEP 2 (Filter First): Check post staleness
    let created_at = post
        .created_at
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|v| v.with_timezone(&Utc));

    if post_is_stale(created_at, Utc::now(), config.max_post_age_secs) {
        state.db.remember_post(&post_id, handle, text, false).await?;
        info!("X post from @{handle} is older than {}s; not queued", config.max_post_age_secs);
        return Ok(IngestResult {
            post_id,
            handle: handle.to_string(),
            already_seen: false,
            is_stale: true,
            queued: false,
            symbol: None,
            side: None,
            confidence: None,
            reason: format!("Post is older than {}s", config.max_post_age_secs),
        });
    }

    // STEP 3 (AI Second): Trigger LLM sentiment & symbol extraction ONLY now!
    info!("🧠 Invoking AI reasoning for unseen post from @{handle} (key: {post_id})");
    let decision = match read_post(state, handle, text).await {
        Ok(d) => d,
        Err(err) => {
            warn!("AI reasoning error for @{handle}: {err}");
            return Ok(IngestResult {
                post_id,
                handle: handle.to_string(),
                already_seen: false,
                is_stale: false,
                queued: false,
                symbol: None,
                side: None,
                confidence: None,
                reason: format!("AI evaluation unavailable: {err}"),
            });
        }
    };
    info!(
        "AI evaluation for @{handle}: trade={}, symbol={}, tilt={}, conf={:.2}, reason={}",
        decision.trade, decision.symbol, decision.tilt, decision.confidence, decision.reason
    );

    if !decision.trade || decision.confidence < config.min_confidence {
        state.db.remember_post(&post_id, handle, text, false).await?;
        return Ok(IngestResult {
            post_id,
            handle: handle.to_string(),
            already_seen: false,
            is_stale: false,
            queued: false,
            symbol: Some(decision.symbol),
            side: Some(decision.side),
            confidence: Some(decision.confidence),
            reason: decision.reason,
        });
    }

    // STEP 4: Market Price & Trend validation
    let quote = fetch_quote(&decision.symbol).await;
    if let Some(quote) = quote {
        if move_already_done(&decision.side, quote.last, quote.high, quote.low) {
            state.db.remember_post(&post_id, handle, text, false).await?;
            info!(
                "X post from @{handle} on {} is already at the day's extreme; not queued",
                decision.symbol
            );
            return Ok(IngestResult {
                post_id,
                handle: handle.to_string(),
                already_seen: false,
                is_stale: false,
                queued: false,
                symbol: Some(decision.symbol),
                side: Some(decision.side),
                confidence: Some(decision.confidence),
                reason: "Market move already exhausted at day's extreme".to_string(),
            });
        }
        queue_signal(state, handle, text, &post_id, &decision, Some(quote.last)).await?;
    } else {
        queue_signal(state, handle, text, &post_id, &decision, None).await?;
    }

    Ok(IngestResult {
        post_id,
        handle: handle.to_string(),
        already_seen: false,
        is_stale: false,
        queued: true,
        symbol: Some(decision.symbol),
        side: Some(decision.side),
        confidence: Some(decision.confidence),
        reason: decision.reason,
    })
}

async fn read_post(state: &AppState, handle: &str, text: &str) -> anyhow::Result<ReadDecision> {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You judge whether one X post can move a liquid asset in the next few minutes. \
                      Reply with JSON only. trade is false for jokes, old news, politics with no asset, \
                      and vague opinions. Allowed symbols: BTCUSDT, ETHUSDT, SOLUSDT, DOGEUSDT, TSLA, NVDA, AAPL."
                .to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Account: @{handle}\nPost:\n{text}\n\n\
                 Return {{\"trade\":false,\"symbol\":\"BTCUSDT\",\"tilt\":\"pump\",\"side\":\"buy\",\"confidence\":0.0,\"reason\":\"short\"}}. \
                 tilt is pump, dump, or dip. pump and dip use side buy. dump uses side sell. \
                 confidence is 0 to 1."
            ),
        },
    ];
    let response = state
        .ai_engine
        .grok
        .chat_json(messages, true, Some(0.1), Some(300), "low", None)
        .await?;
    Ok(parse_decision(&response.content))
}

async fn queue_signal(
    state: &AppState,
    handle: &str,
    text: &str,
    post_id: &str,
    decision: &ReadDecision,
    last_price: Option<f64>,
) -> anyhow::Result<()> {
    let exchange = venue_for(&decision.symbol);
    let action = if decision.side == "sell" {
        SignalAction::Sell
    } else {
        SignalAction::Buy
    };
    let mut signal = TradingSignal::new(
        exchange,
        &decision.symbol,
        "1m",
        action,
        SignalSource::Sentiment,
        decision.confidence,
        &format!(
            "@{handle} ({}) — {}\n{}",
            decision.tilt, decision.reason, text
        ),
    );
    signal.position_size_pct = Some(config_size(state).await.min(1.0));
    signal.expires_at = Some(Utc::now() + chrono::Duration::minutes(15));
    if let Some(price) = last_price.and_then(Decimal::from_f64_retain) {
        let (stop, target) = protective_levels(price, &decision.side, 2.0, 4.0);
        signal.entry_price = Some(price);
        signal.stop_loss = Some(stop);
        signal.take_profit = Some(target);
    }

    if let Err(error) = state.db.insert_signal(&signal).await {
        warn!("Failed to persist watch signal: {error}");
    }
    state.db.remember_post(post_id, handle, text, true).await?;
    state.signal_history.write().await.push(signal.clone());
    state.execution_engine.queue_for_review(signal).await;
    Ok(())
}

async fn config_size(state: &AppState) -> f64 {
    state.config.read().await.trading.max_position_size_pct
}

async fn fetch_quote(symbol: &str) -> Option<Quote> {
    if !symbol.ends_with("USDT") {
        return None;
    }
    let url = format!("https://api.mexc.com/api/v3/ticker/24hr?symbol={symbol}");
    let body = reqwest::Client::new().get(url).send().await.ok()?.json::<Value>().await.ok()?;
    let last = body.get("lastPrice")?.as_str()?.parse().ok()?;
    let high = body.get("highPrice")?.as_str()?.parse().ok()?;
    let low = body.get("lowPrice")?.as_str()?.parse().ok()?;
    Some(Quote { last, high, low })
}

struct Quote {
    last: f64,
    high: f64,
    low: f64,
}

#[derive(Debug, Clone)]
pub struct ReadDecision {
    pub trade: bool,
    pub symbol: String,
    pub tilt: String,
    pub side: String,
    pub confidence: f64,
    pub reason: String,
}

pub fn parse_decision(content: &str) -> ReadDecision {
    let empty = ReadDecision {
        trade: false,
        symbol: String::new(),
        tilt: String::new(),
        side: String::new(),
        confidence: 0.0,
        reason: "unreadable reader response".to_string(),
    };
    let Some(value) = json_object(content) else {
        return empty;
    };
    let symbol = value.get("symbol").and_then(|item| item.as_str()).unwrap_or("").to_uppercase();
    let tilt = value.get("tilt").and_then(|item| item.as_str()).unwrap_or("").to_lowercase();
    let mut side = value.get("side").and_then(|item| item.as_str()).unwrap_or("").to_lowercase();
    side = match tilt.as_str() {
        "dump" => "sell".to_string(),
        "pump" | "dip" => "buy".to_string(),
        _ => side,
    };
    let allowed = matches!(
        symbol.as_str(),
        "BTCUSDT" | "ETHUSDT" | "SOLUSDT" | "DOGEUSDT" | "TSLA" | "NVDA" | "AAPL"
    );
    let trade = value.get("trade").and_then(|item| item.as_bool()).unwrap_or(false)
        && allowed
        && matches!(side.as_str(), "buy" | "sell");
    ReadDecision {
        trade,
        symbol,
        tilt,
        side,
        confidence: value.get("confidence").and_then(|item| item.as_f64()).unwrap_or(0.0).clamp(0.0, 1.0),
        reason: value.get("reason").and_then(|item| item.as_str()).unwrap_or("").to_string(),
    }
}

pub fn post_key(handle: &str, id: &str, text: &str) -> String {
    if !id.trim().is_empty() {
        return format!("{}:{}", handle.to_lowercase(), id.trim());
    }
    let digest = Sha256::digest(format!("{}:{}", handle.to_lowercase(), text.trim()).as_bytes());
    format!("{}:{:x}", handle.to_lowercase(), digest)
}

pub fn post_is_stale(created_at: Option<DateTime<Utc>>, now: DateTime<Utc>, max_age_secs: i64) -> bool {
    match created_at {
        Some(created) => (now - created).num_seconds() > max_age_secs,
        None => false,
    }
}

/// True when the last price is already pressed against the day's extreme
/// in the direction of the trade.
pub fn move_already_done(side: &str, last: f64, high: f64, low: f64) -> bool {
    if last <= 0.0 || high < low {
        return false;
    }
    let span = (high - low).max(last * 0.002);
    if span <= 0.0 {
        return false;
    }
    match side {
        "buy" => (high - last) / span < 0.15,
        "sell" => (last - low) / span < 0.15,
        _ => false,
    }
}

fn venue_for(symbol: &str) -> ExchangeId {
    if symbol.ends_with("USDT") {
        ExchangeId::Mexc
    } else {
        ExchangeId::Alpaca
    }
}

fn protective_levels(price: Decimal, side: &str, stop_pct: f64, target_pct: f64) -> (Decimal, Decimal) {
    let stop = Decimal::from_f64_retain(stop_pct / 100.0).unwrap_or(Decimal::new(2, 2));
    let target = Decimal::from_f64_retain(target_pct / 100.0).unwrap_or(Decimal::new(4, 2));
    let one = Decimal::ONE;
    if side == "sell" {
        (price * (one + stop), price * (one - target))
    } else {
        (price * (one - stop), price * (one + target))
    }
}

fn json_object(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    serde_json::from_str(&trimmed[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_posts_are_rejected_and_undated_posts_are_kept() {
        let now = Utc::now();
        assert!(post_is_stale(Some(now - chrono::Duration::minutes(20)), now, 600));
        assert!(!post_is_stale(Some(now - chrono::Duration::minutes(2)), now, 600));
        assert!(!post_is_stale(None, now, 600));
    }

    #[test]
    fn a_price_already_at_the_high_is_not_a_buy() {
        assert!(move_already_done("buy", 100.0, 100.1, 90.0));
        assert!(!move_already_done("buy", 95.0, 100.0, 90.0));
        assert!(move_already_done("sell", 90.1, 100.0, 90.0));
    }

    #[test]
    fn the_same_text_keeps_the_same_key() {
        assert_eq!(post_key("elonmusk", "", "hello"), post_key("elonmusk", "", "hello"));
        assert_ne!(post_key("elonmusk", "", "hello"), post_key("elonmusk", "", "hello again"));
    }
}
