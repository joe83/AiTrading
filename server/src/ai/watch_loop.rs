use chrono::{DateTime, Utc};
use serde::Serialize;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use super::grok_client::ChatMessage;
use crate::config::WatchConfig;
use crate::models::{ExchangeId, SignalAction, SignalSource, TradingSignal};
use crate::AppState;

const MAX_POSTS_PER_TICK: usize = 3;

/// What the dashboard shows for the sleepless watcher.
#[derive(Debug, Clone, Serialize)]
pub struct WatchStatus {
    pub enabled: bool,
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
            handles: Vec::new(),
            interval_secs: 60,
            running: false,
            last_tick_at: None,
            last_result: "Not started".to_string(),
            last_error: None,
        }
    }
}

/// Poll a fixed list of X accounts and queue a review signal when a new post
/// still looks early. This loop never places an order.
pub async fn run(state: Arc<AppState>) {
    let config = state.config.read().await.watch.clone();
    if !config.enabled || config.handles.is_empty() {
        info!("X watch loop is off");
        set_status(&state, |status| {
            status.enabled = false;
            status.running = false;
            status.last_result = "Watcher is turned off".to_string();
        })
        .await;
        return;
    }
    let interval_secs = config.interval_secs.max(30);
    set_status(&state, |status| {
        status.enabled = true;
        status.running = true;
        status.handles = config.handles.clone();
        status.interval_secs = interval_secs;
        status.last_result = "Waiting for the first scan".to_string();
        status.last_error = None;
    })
    .await;
    info!(
        "X watch loop started for {} every {interval_secs}s",
        config.handles.join(", ")
    );

    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
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
    }
}

async fn tick(state: &AppState, config: &WatchConfig) -> anyhow::Result<()> {
    let from_date = Utc::now().format("%Y-%m-%d").to_string();
    let handles = config.handles.join(", ");
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You search X and return only posts those accounts published. Reply with JSON.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "List posts from the last 15 minutes by these X accounts: {handles}. \
                 Return JSON {{\"posts\":[{{\"id\":\"post id\",\"handle\":\"account\",\"text\":\"exact text\",\"created_at\":\"RFC3339 or empty\"}}]}}. \
                 Use an empty posts array when there is nothing new. Do not invent posts."
            ),
        },
    ];
    let response = state
        .ai_engine
        .grok
        .chat_x_accounts(messages, &config.handles, &from_date, 800)
        .await?;
    let body = json_object(&response.content).unwrap_or_else(|| json!({ "posts": [] }));
    let posts = body.get("posts").and_then(|value| value.as_array()).cloned().unwrap_or_default();
    let mut queued = 0u32;
    for post in posts.iter().take(MAX_POSTS_PER_TICK) {
        match consider_post(state, config, post).await {
            Ok(true) => queued += 1,
            Ok(false) => {}
            Err(error) => warn!("X post skipped: {error}"),
        }
    }
    let found = posts.len();
    set_status(state, |status| {
        status.last_tick_at = Some(Utc::now().to_rfc3339());
        status.last_error = None;
        status.last_result = format!("{found} posts from X, {queued} queued for review");
    })
    .await;
    info!("X watch tick: {found} posts, {queued} queued");
    Ok(())
}

async fn set_status(state: &AppState, update: impl FnOnce(&mut WatchStatus)) {
    let mut status = state.watch_status.write().await;
    update(&mut status);
}

async fn consider_post(state: &AppState, config: &WatchConfig, post: &Value) -> anyhow::Result<bool> {
    let handle = post.get("handle").and_then(|value| value.as_str()).unwrap_or("").trim().trim_start_matches('@');
    let text = post.get("text").and_then(|value| value.as_str()).unwrap_or("").trim();
    if handle.is_empty() || text.is_empty() {
        return Ok(false);
    }
    let raw_id = post.get("id").and_then(|value| value.as_str()).unwrap_or("");
    let post_id = post_key(handle, raw_id, text);
    if state.db.has_seen_post(&post_id).await? {
        return Ok(false);
    }

    let created_at = post
        .get("created_at")
        .and_then(|value| value.as_str())
        .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
        .map(|value| value.with_timezone(&Utc));
    if post_is_stale(created_at, Utc::now(), config.max_post_age_secs) {
        state.db.remember_post(&post_id, handle, text, false).await?;
        info!("X post from @{handle} is older than {}s; not queued", config.max_post_age_secs);
        return Ok(false);
    }

    let decision = read_post(&state, handle, text).await?;
    if !decision.trade || decision.confidence < config.min_confidence {
        state.db.remember_post(&post_id, handle, text, false).await?;
        info!("X post from @{handle} is not a trade ({})", decision.reason);
        return Ok(false);
    }

    let quote = fetch_quote(&decision.symbol).await;
    if let Some(quote) = quote {
        if move_already_done(&decision.side, quote.last, quote.high, quote.low) {
            state.db.remember_post(&post_id, handle, text, false).await?;
            info!(
                "X post from @{handle} on {} is already at the day's extreme; not queued",
                decision.symbol
            );
            return Ok(false);
        }
        queue_signal(state, handle, text, &post_id, &decision, Some(quote.last)).await?;
    } else {
        queue_signal(state, handle, text, &post_id, &decision, None).await?;
    }
    Ok(true)
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
        .chat_json(messages, true, Some(0.1), Some(400), "low", None)
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
    signal.position_size_pct = Some(config_size(&state).await.min(1.0));
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

struct ReadDecision {
    trade: bool,
    symbol: String,
    tilt: String,
    side: String,
    confidence: f64,
    reason: String,
}

fn parse_decision(content: &str) -> ReadDecision {
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
