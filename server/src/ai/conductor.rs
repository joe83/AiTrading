use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

use crate::exchange::Exchange;
use crate::models::{Candle, CandleSeries, ExchangeId, SignalSource, Timeframe};
use crate::AppState;

const TICK: Duration = Duration::from_secs(30);
const RADAR_EVERY: Duration = Duration::from_secs(300);
const MIN_CANDLES: usize = 30;
const MEME_VELOCITY: u32 = 60;
const MEME_SENTIMENT: f64 = 0.2;
const MEME_LIMIT: usize = 3;

/// What the dashboard shows for the conductor.
#[derive(Debug, Clone, Serialize)]
pub struct ConductorStatus {
    pub running: bool,
    pub last_run_at: Option<String>,
    pub last_result: String,
    pub last_error: Option<String>,
}

impl Default for ConductorStatus {
    fn default() -> Self {
        Self {
            running: false,
            last_run_at: None,
            last_result: "Not started".to_string(),
            last_error: None,
        }
    }
}

/// Wake the analyst for watcher and meme-radar candidates.
/// Manual mode leaves the result pending. Auto mode sends it through risk and the exchange.
pub async fn run(state: Arc<AppState>) {
    info!("Conductor started");
    set_status(&state, |status| {
        status.running = true;
        status.last_result = "Waiting for the first pass".to_string();
    })
    .await;

    let mut last_radar = Utc::now() - chrono::Duration::seconds(RADAR_EVERY.as_secs() as i64);
    let mut reviewed: HashMap<Uuid, DateTime<Utc>> = HashMap::new();
    let mut meme_seen: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;
        let now = Utc::now();
        let run_radar = now - last_radar >= chrono::Duration::seconds(RADAR_EVERY.as_secs() as i64);
        if run_radar {
            last_radar = now;
        }
        if let Err(error) = pass(&state, run_radar, &mut reviewed, &mut meme_seen).await {
            warn!("Conductor pass failed: {error}");
            let message = error.to_string();
            set_status(&state, |status| {
                status.last_run_at = Some(Utc::now().to_rfc3339());
                status.last_error = Some(message);
                status.last_result = "Pass failed".to_string();
            })
            .await;
        }
    }
}

async fn pass(
    state: &AppState,
    run_radar: bool,
    reviewed: &mut HashMap<Uuid, DateTime<Utc>>,
    meme_seen: &mut HashMap<String, DateTime<Utc>>,
) -> Result<()> {
    let floor = state.config.read().await.watch.min_confidence;
    let mut analyzed = 0u32;
    let mut queued = 0u32;
    let mut executed = 0u32;

    let pending = state.execution_engine.get_pending_signals().await;
    for signal in pending.into_iter().filter(|signal| signal.source == SignalSource::Sentiment) {
        if recently(reviewed, signal.id, 600) {
            continue;
        }
        reviewed.insert(signal.id, Utc::now());
        match review_symbol(
            state,
            &signal.symbol,
            signal.exchange,
            Some(&format!("Watcher candidate: {}", signal.reasoning)),
            floor,
        )
        .await
        {
            Ok(outcome) => {
                state.execution_engine.reject_signal(signal.id).await;
                match outcome {
                    Review::LeftPending => queued += 1,
                    Review::Executed => executed += 1,
                    Review::Dropped => {}
                }
                analyzed += 1;
            }
            Err(error) => warn!("Conductor left watcher signal {} pending: {error}", signal.symbol),
        }
    }

    if run_radar {
        match state.ai_engine.meme_radar.scan().await {
            Ok(report) => {
                let symbols: Vec<String> = report
                    .tokens
                    .iter()
                    .filter(|token| {
                        meme_is_candidate(
                            token.is_tradeable_on_mexc,
                            token.viral_velocity,
                            token.sentiment_score,
                            token.risk_level == crate::ai::meme_radar::MemeRiskLevel::Extreme,
                        )
                    })
                    .take(MEME_LIMIT)
                    .map(|token| token.mexc_symbol.clone())
                    .collect();
                for symbol in symbols {
                    if recently_symbol(meme_seen, &symbol, 1800) {
                        continue;
                    }
                    meme_seen.insert(symbol.clone(), Utc::now());
                    let context = format!("Meme radar candidate {symbol}");
                    match review_symbol(state, &symbol, ExchangeId::Mexc, Some(&context), floor).await {
                        Ok(Review::LeftPending) => queued += 1,
                        Ok(Review::Executed) => executed += 1,
                        Ok(Review::Dropped) => {}
                        Err(error) => warn!("Conductor skipped meme {symbol}: {error}"),
                    }
                    analyzed += 1;
                }
            }
            Err(error) => warn!("Meme radar scan failed: {error}"),
        }
    }

    let summary = format!("{analyzed} analyzed, {queued} pending, {executed} sent");
    info!("Conductor pass: {summary}");
    set_status(state, |status| {
        status.last_run_at = Some(Utc::now().to_rfc3339());
        status.last_error = None;
        status.last_result = summary;
    })
    .await;
    Ok(())
}

enum Review {
    Dropped,
    LeftPending,
    Executed,
}

async fn review_symbol(
    state: &AppState,
    symbol: &str,
    exchange: ExchangeId,
    context: Option<&str>,
    floor: f64,
) -> Result<Review> {
    let candles = load_candles(state, exchange, symbol).await?;
    if candles.candles.len() < MIN_CANDLES {
        anyhow::bail!("only {} candles for {symbol}", candles.candles.len());
    }
    let analysis = state.ai_engine.analyze(symbol, exchange, &candles, context).await?;
    let signal = analysis.signal;
    if !signal.is_actionable() || signal.confidence < floor {
        info!(
            "Conductor dropped {symbol}: {:?} {:.0}%",
            signal.action,
            signal.confidence * 100.0
        );
        return Ok(Review::Dropped);
    }

    let mode = state.execution_engine.get_mode().await;
    if mode != crate::execution::TradingMode::Auto {
        state.signal_history.write().await.push(signal.clone());
        state.execution_engine.queue_for_review(signal).await;
        return Ok(Review::LeftPending);
    }

    let Some(handle) = state.exchange_manager.get(&exchange).await else {
        state.execution_engine.queue_for_review(signal).await;
        info!("Conductor queued {symbol}; exchange is not connected");
        return Ok(Review::LeftPending);
    };
    let balance = {
        let locked = handle.read().await;
        locked.get_balance().await.ok().and_then(|balance| balance.total_usd)
    };
    let Some(balance) = balance else {
        state.execution_engine.queue_for_review(signal).await;
        info!("Conductor queued {symbol}; account balance is unavailable");
        return Ok(Review::LeftPending);
    };
    let locked = handle.read().await;
    match state
        .execution_engine
        .process_signal(signal, locked.as_ref(), balance)
        .await?
    {
        crate::execution::SignalProcessResult::Executed(_) => Ok(Review::Executed),
        crate::execution::SignalProcessResult::PendingApproval => Ok(Review::LeftPending),
        crate::execution::SignalProcessResult::Rejected(reason) => {
            info!("Conductor risk rejected {symbol}: {reason}");
            Ok(Review::Dropped)
        }
        crate::execution::SignalProcessResult::Skipped(reason) => {
            info!("Conductor skipped {symbol}: {reason}");
            Ok(Review::Dropped)
        }
    }
}

async fn load_candles(state: &AppState, exchange: ExchangeId, symbol: &str) -> Result<CandleSeries> {
    if let Some(handle) = state.exchange_manager.get(&exchange).await {
        let locked = handle.read().await;
        if locked.is_connected() {
            let candles = locked.get_candles(symbol, &Timeframe::Min15, 100).await?;
            if candles.len() >= MIN_CANDLES {
                let mut series = CandleSeries::new(symbol, exchange, "15m");
                for candle in candles {
                    series.push(candle);
                }
                return Ok(series);
            }
        }
    }
    if symbol.ends_with("USDT") {
        return public_mexc_candles(symbol).await;
    }
    anyhow::bail!("no candle source for {symbol}")
}

async fn public_mexc_candles(symbol: &str) -> Result<CandleSeries> {
    let url = format!("https://api.mexc.com/api/v3/klines?symbol={symbol}&interval=15m&limit=100");
    let rows: Vec<Vec<Value>> = reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut series = CandleSeries::new(symbol, ExchangeId::Mexc, "15m");
    for row in rows {
        if let Some(candle) = parse_mexc_kline(symbol, &row) {
            series.push(candle);
        }
    }
    Ok(series)
}

fn parse_mexc_kline(symbol: &str, row: &[Value]) -> Option<Candle> {
    let open_ms = row.first()?.as_i64().or_else(|| row.first()?.as_str()?.parse().ok())?;
    let open_time = Utc.timestamp_millis_opt(open_ms).single()?;
    let num = |index: usize| -> Option<Decimal> {
        let value = row.get(index)?;
        if let Some(text) = value.as_str() {
            text.parse().ok()
        } else {
            Decimal::from_f64_retain(value.as_f64()?)
        }
    };
    Some(Candle {
        symbol: symbol.to_string(),
        exchange: ExchangeId::Mexc,
        timeframe: "15m".to_string(),
        open_time,
        close_time: open_time + chrono::Duration::minutes(15),
        open: num(1)?,
        high: num(2)?,
        low: num(3)?,
        close: num(4)?,
        volume: num(5)?,
    })
}

pub fn meme_is_candidate(tradeable: bool, velocity: u32, sentiment: f64, extreme: bool) -> bool {
    tradeable && !extreme && velocity >= MEME_VELOCITY && sentiment >= MEME_SENTIMENT
}

fn recently(seen: &HashMap<Uuid, DateTime<Utc>>, id: Uuid, secs: i64) -> bool {
    seen.get(&id)
        .is_some_and(|at| Utc::now() - *at < chrono::Duration::seconds(secs))
}

fn recently_symbol(seen: &HashMap<String, DateTime<Utc>>, symbol: &str, secs: i64) -> bool {
    seen.get(symbol)
        .is_some_and(|at| Utc::now() - *at < chrono::Duration::seconds(secs))
}

async fn set_status(state: &AppState, update: impl FnOnce(&mut ConductorStatus)) {
    let mut status = state.conductor_status.write().await;
    update(&mut status);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meme_candidate_requires_a_tradeable_liquid_token() {
        assert!(meme_is_candidate(true, 80, 0.4, false));
        assert!(!meme_is_candidate(true, 80, 0.4, true));
        assert!(!meme_is_candidate(false, 90, 0.9, false));
        assert!(!meme_is_candidate(true, 20, 0.9, false));
    }
}
