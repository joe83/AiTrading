pub mod data_loader;
pub mod metrics;
pub mod simulator;

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::ai::sentiment_analysis::{SentimentLabel, SentimentResult};
use crate::ai::technical_analysis::TrendDirection;
use crate::ai::AiEngine;
use crate::models::*;
use data_loader::DataLoader;
use metrics::{BacktestMetrics, CompletedTrade};
use simulator::{SimConfig, SimFillResult, SimulatedExchange};

/// In-memory cache for historical AI decisions to enable free reruns on the same historical data.
fn get_ai_cache() -> &'static RwLock<HashMap<String, TradingSignal>> {
    static CACHE: OnceLock<RwLock<HashMap<String, TradingSignal>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn default_exchange() -> ExchangeId {
    ExchangeId::Mexc
}

fn default_initial_balance() -> f64 {
    10000.0
}

/// Configuration for a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub symbol: String,
    #[serde(default = "default_exchange")]
    pub exchange: ExchangeId,
    pub timeframe: Timeframe,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    #[serde(default = "default_initial_balance", alias = "initial_capital")]
    pub initial_balance: f64,
    /// Maker fee fraction (default 0.1%)
    #[serde(default, alias = "fee_rate")]
    pub maker_fee: Option<f64>,
    /// Taker fee fraction (default 0.1%)
    #[serde(default)]
    pub taker_fee: Option<f64>,
    /// Slippage fraction (default 0.05%)
    pub slippage: Option<f64>,
    /// Allow short selling
    pub allow_short: Option<bool>,
    /// Optional CSV file path for data instead of API fetch
    pub csv_path: Option<String>,
    /// Minimum bars of lookback before generating signals
    pub warmup_bars: Option<usize>,
    /// Whether to evaluate candidate setups using the Grok AI model
    pub use_grok: Option<bool>,
    /// Maximum number of Grok API calls allowed for this backtest (budget safety cap)
    pub max_ai_calls: Option<usize>,
}

/// Result of a completed backtest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub id: Uuid,
    pub config: BacktestConfig,
    pub metrics: BacktestMetrics,
    pub initial_balance: f64,
    pub final_balance: f64,
    pub total_candles: usize,
    pub signals_generated: usize,
    pub signals_executed: usize,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_secs: f64,
    pub use_grok: bool,
    pub ai_calls_made: usize,
    pub ai_cached_calls: usize,
    pub grok_model_used: Option<String>,
    pub trades: Vec<CompletedTrade>,
}

/// The backtest engine orchestrates historical data replay.
pub struct BacktestEngine;

impl BacktestEngine {
    /// Run a full backtest.
    pub async fn run(
        config: BacktestConfig,
        ai_engine: &AiEngine,
    ) -> Result<BacktestResult> {
        let started_at = Utc::now();
        let use_grok = config.use_grok.unwrap_or(false);
        let max_ai_calls = config.max_ai_calls.unwrap_or(50);
        let mut ai_calls_made = 0usize;
        let mut ai_cached_calls = 0usize;

        info!(
            "Starting backtest: {} {} from {} to {} (Mode: {})",
            config.symbol,
            config.timeframe,
            config.start_time,
            config.end_time,
            if use_grok { "Grok AI Evaluated" } else { "Fast Quantitative" }
        );

        // 1. Load historical data
        let candles = if let Some(ref csv_path) = config.csv_path {
            DataLoader::load_csv(
                std::path::Path::new(csv_path),
                &config.symbol,
                config.exchange,
                &config.timeframe,
            )?
        } else {
            DataLoader::fetch_mexc_candles(
                &config.symbol,
                &config.timeframe,
                config.start_time,
                config.end_time,
            )
            .await?
        };

        if candles.is_empty() {
            anyhow::bail!("No candle data available for the specified range");
        }

        info!("Loaded {} candles for backtest", candles.len());

        // 2. Initialize simulator
        let initial_balance = Decimal::try_from(config.initial_balance)
            .context("Invalid initial balance")?;

        let sim_config = SimConfig {
            maker_fee: config.maker_fee.or(config.taker_fee).unwrap_or(0.001),
            taker_fee: config.taker_fee.or(config.maker_fee).unwrap_or(0.001),
            slippage: config.slippage.unwrap_or(0.0005),
            allow_short: config.allow_short.unwrap_or(false),
        };

        let mut simulator = SimulatedExchange::new(initial_balance, sim_config);

        // 3. Replay candles
        let warmup = config.warmup_bars.unwrap_or(50);
        let mut signals_generated = 0usize;
        let mut signals_executed = 0usize;

        for (i, candle) in candles.iter().enumerate() {
            // Tick the simulator (check SL/TP)
            simulator.tick(candle);

            // Skip warmup period — need enough history for indicators
            if i < warmup {
                continue;
            }

            // Get historical window for AI analysis (last `warmup` candles)
            let window_start = i.saturating_sub(warmup);
            let window = &candles[window_start..=i];

            let signal_res = if use_grok {
                Self::generate_grok_signal(
                    ai_engine,
                    &config,
                    window,
                    candle,
                    &mut ai_calls_made,
                    &mut ai_cached_calls,
                    max_ai_calls,
                )
                .await
            } else {
                Self::generate_fast_signal(
                    &config.symbol,
                    config.exchange,
                    &config.timeframe,
                    window,
                    candle,
                )
            };

            match signal_res {
                Ok(Some(signal)) => {
                    signals_generated += 1;

                    if signal.is_actionable() && signal.confidence >= 0.6 {
                        match simulator.execute_signal(&signal, candle) {
                            SimFillResult::Filled { .. } => {
                                signals_executed += 1;
                            }
                            SimFillResult::Rejected(reason) => {
                                warn!("Signal rejected at bar {}: {}", i, reason);
                            }
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("Signal generation error at bar {}: {}", i, e);
                }
            }
        }

        // 4. Close any remaining positions at the last candle
        if let Some(last_candle) = candles.last() {
            simulator.close_all(last_candle);
        }

        // 5. Compute metrics
        let bars_per_year = Self::bars_per_year(&config.timeframe);
        let completed_trades = simulator.completed_trades().to_vec();
        let metrics = BacktestMetrics::compute(
            &completed_trades,
            simulator.equity_curve(),
            config.initial_balance,
            simulator.total_bars(),
            bars_per_year,
        );

        let completed_at = Utc::now();
        let duration_secs = (completed_at - started_at).num_milliseconds() as f64 / 1000.0;

        let result = BacktestResult {
            id: Uuid::new_v4(),
            config: config.clone(),
            metrics,
            initial_balance: config.initial_balance,
            final_balance: simulator.final_balance().to_f64().unwrap_or(0.0),
            total_candles: candles.len(),
            signals_generated,
            signals_executed,
            started_at,
            completed_at,
            duration_secs,
            use_grok,
            ai_calls_made,
            ai_cached_calls,
            grok_model_used: if use_grok {
                Some(ai_engine.grok.get_model_primary())
            } else {
                None
            },
            trades: completed_trades,
        };

        info!(
            "Backtest complete: {} trades, {:.2}% return, {:.2} Sharpe, {:.1}% win rate (AI calls: {}/cached {}) in {:.1}s",
            result.metrics.total_trades,
            result.metrics.total_return_pct,
            result.metrics.sharpe_ratio,
            result.metrics.win_rate_pct,
            ai_calls_made,
            ai_cached_calls,
            duration_secs,
        );

        Ok(result)
    }

    /// Evaluates a candidate setup using Grok AI LLM with caching and budget protection.
    async fn generate_grok_signal(
        ai_engine: &AiEngine,
        config: &BacktestConfig,
        window: &[Candle],
        current: &Candle,
        ai_calls_made: &mut usize,
        ai_cached_calls: &mut usize,
        max_ai_calls: usize,
    ) -> Result<Option<TradingSignal>> {
        if window.len() < 20 {
            return Ok(None);
        }

        let series = CandleSeries {
            symbol: config.symbol.clone(),
            exchange: config.exchange,
            timeframe: config.timeframe.to_string(),
            candles: window.to_vec(),
        };

        // 1. Run local technical and pattern analyzers (sub-millisecond)
        let tech_result = match ai_engine.technical.analyze(&series) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };

        let patterns = ai_engine.pattern.detect(&series).unwrap_or_default();

        // 2. Screener Gate: Determine if this candle represents a candidate setup
        let is_candidate = {
            let rsi_extreme = tech_result.rsi.map(|r| r <= 35.0 || r >= 65.0).unwrap_or(false);
            let has_pattern = !patterns.is_empty();
            let closes = series.closes_f64();
            let momentum_active = if closes.len() >= 5 {
                let diff = (closes[closes.len() - 1] - closes[closes.len() - 5]) / closes[closes.len() - 5] * 100.0;
                diff.abs() >= 1.5
            } else {
                false
            };

            rsi_extreme || has_pattern || momentum_active
        };

        if !is_candidate {
            return Ok(None);
        }

        // 3. Cache check
        let cache_key = format!(
            "grok_bt:{}:{}:{}",
            config.symbol,
            config.timeframe,
            current.open_time.timestamp()
        );

        if let Ok(cache) = get_ai_cache().read() {
            if let Some(cached) = cache.get(&cache_key) {
                *ai_cached_calls += 1;
                return Ok(Some(cached.clone()));
            }
        }

        // 4. Budget check: if limit reached, fallback to rule-based decision
        if *ai_calls_made >= max_ai_calls {
            let fallback = Self::generate_fast_signal(
                &config.symbol,
                config.exchange,
                &config.timeframe,
                window,
                current,
            )?;
            return Ok(fallback.map(|mut s| {
                s.reasoning = format!("(Budget capped at {} calls) {}", max_ai_calls, s.reasoning);
                s
            }));
        }

        // 5. Construct historical sentiment context for Grok
        let sentiment = SentimentResult {
            score: match tech_result.trend {
                TrendDirection::StrongBullish => 0.6,
                TrendDirection::Bullish => 0.3,
                TrendDirection::Bearish => -0.3,
                TrendDirection::StrongBearish => -0.6,
                TrendDirection::Neutral => 0.0,
            },
            label: match tech_result.trend {
                TrendDirection::StrongBullish => SentimentLabel::VeryBullish,
                TrendDirection::Bullish => SentimentLabel::Bullish,
                TrendDirection::Bearish => SentimentLabel::Bearish,
                TrendDirection::StrongBearish => SentimentLabel::VeryBearish,
                TrendDirection::Neutral => SentimentLabel::Neutral,
            },
            summary: format!(
                "Historical backtest context at {}: Trend is {} (strength: {:.0}%)",
                current.open_time.to_rfc3339(),
                tech_result.trend,
                tech_result.strength * 100.0
            ),
            themes: vec!["backtest_replay".to_string()],
            news_score: None,
            social_score: None,
            tokens_used: 0,
        };

        // 6. Call Grok AI decision maker!
        info!(
            "Invoking Grok AI decision ({}/{} budget) on {} bar {}",
            *ai_calls_made + 1,
            max_ai_calls,
            config.symbol,
            current.open_time
        );

        match ai_engine.decision_maker.decide(
            &config.symbol,
            config.exchange,
            &tech_result,
            &patterns,
            &sentiment,
            &series,
        ).await {
            Ok((signal, _, _)) => {
                *ai_calls_made += 1;
                // Store in cache for free reruns
                if let Ok(mut cache) = get_ai_cache().write() {
                    cache.insert(cache_key, signal.clone());
                }
                Ok(Some(signal))
            }
            Err(e) => {
                warn!("Grok decision call failed in backtest: {e}. Using fallback.");
                Self::generate_fast_signal(
                    &config.symbol,
                    config.exchange,
                    &config.timeframe,
                    window,
                    current,
                )
            }
        }
    }

    /// Fast rule-based signal generation for free, instant backtesting without LLM costs.
    fn generate_fast_signal(
        symbol: &str,
        exchange: ExchangeId,
        timeframe: &Timeframe,
        window: &[Candle],
        current: &Candle,
    ) -> Result<Option<TradingSignal>> {
        if window.len() < 20 {
            return Ok(None);
        }

        // Extract close prices for indicator calculation
        let closes: Vec<f64> = window
            .iter()
            .filter_map(|c| c.close.to_f64())
            .collect();

        if closes.len() < 20 {
            return Ok(None);
        }

        let rsi = Self::calculate_rsi(&closes, 14);
        let (ema_short, ema_long) = Self::calculate_emas(&closes);

        // Simple signal logic based on technical indicators
        let mut score: f64 = 0.0;
        let mut reasons = Vec::new();

        // RSI signal
        if let Some(rsi_val) = rsi {
            if rsi_val < 30.0 {
                score += 0.3;
                reasons.push(format!("RSI oversold ({:.1})", rsi_val));
            } else if rsi_val > 70.0 {
                score -= 0.3;
                reasons.push(format!("RSI overbought ({:.1})", rsi_val));
            }
        }

        // EMA crossover signal
        if let (Some(short), Some(long)) = (ema_short, ema_long) {
            if short > long {
                score += 0.3;
                reasons.push("EMA bullish crossover".to_string());
            } else {
                score -= 0.3;
                reasons.push("EMA bearish crossover".to_string());
            }
        }

        // Price momentum (last 5 bars)
        if closes.len() >= 5 {
            let momentum = (closes[closes.len() - 1] - closes[closes.len() - 5])
                / closes[closes.len() - 5]
                * 100.0;
            if momentum > 2.0 {
                score += 0.2;
                reasons.push(format!("Strong momentum (+{:.1}%)", momentum));
            } else if momentum < -2.0 {
                score -= 0.2;
                reasons.push(format!("Weak momentum ({:.1}%)", momentum));
            }
        }

        // Determine action
        let (action, confidence) = if score >= 0.5 {
            (SignalAction::StrongBuy, score.abs().min(0.95))
        } else if score >= 0.2 {
            (SignalAction::Buy, 0.5 + score * 0.5)
        } else if score <= -0.5 {
            (SignalAction::StrongSell, score.abs().min(0.95))
        } else if score <= -0.2 {
            (SignalAction::Sell, 0.5 + score.abs() * 0.5)
        } else {
            return Ok(None); // Hold — no signal
        };

        let current_dec = current.close;
        let atr_estimate = Self::estimate_atr(window);

        let mut signal = TradingSignal::new(
            exchange,
            symbol,
            &timeframe.to_string(),
            action,
            SignalSource::Technical,
            confidence,
            &reasons.join("; "),
        );

        signal.entry_price = Some(current_dec);
        signal.position_size_pct = Some(5.0);

        // Set SL/TP based on ATR
        if let Some(atr) = atr_estimate {
            let atr_dec = Decimal::try_from(atr).unwrap_or(Decimal::ZERO);
            match action {
                SignalAction::Buy | SignalAction::StrongBuy => {
                    signal.stop_loss = Some(current_dec - atr_dec * Decimal::new(2, 0));
                    signal.take_profit = Some(current_dec + atr_dec * Decimal::new(3, 0));
                }
                SignalAction::Sell | SignalAction::StrongSell => {
                    signal.stop_loss = Some(current_dec + atr_dec * Decimal::new(2, 0));
                    signal.take_profit = Some(current_dec - atr_dec * Decimal::new(3, 0));
                }
                _ => {}
            }
        }

        Ok(Some(signal))
    }

    /// Calculate RSI (Relative Strength Index).
    fn calculate_rsi(prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period + 1 {
            return None;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (prices.len() - period)..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    /// Calculate short (12) and long (26) EMAs.
    fn calculate_emas(prices: &[f64]) -> (Option<f64>, Option<f64>) {
        let short_ema = Self::ema(prices, 12);
        let long_ema = Self::ema(prices, 26);
        (short_ema, long_ema)
    }

    /// Calculate Exponential Moving Average.
    fn ema(prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = prices[..period].iter().sum::<f64>() / period as f64;

        for &price in &prices[period..] {
            ema = (price - ema) * multiplier + ema;
        }

        Some(ema)
    }

    /// Estimate ATR (Average True Range) from candles.
    fn estimate_atr(candles: &[Candle]) -> Option<f64> {
        if candles.len() < 14 {
            return None;
        }

        let period = 14;
        let recent = &candles[candles.len() - period..];

        let mut tr_sum = 0.0;
        for i in 1..recent.len() {
            let high = recent[i].high.to_f64().unwrap_or(0.0);
            let low = recent[i].low.to_f64().unwrap_or(0.0);
            let prev_close = recent[i - 1].close.to_f64().unwrap_or(0.0);

            let tr = (high - low)
                .max((high - prev_close).abs())
                .max((low - prev_close).abs());
            tr_sum += tr;
        }

        Some(tr_sum / (period as f64 - 1.0))
    }

    /// Approximate number of bars per year for a given timeframe.
    fn bars_per_year(tf: &Timeframe) -> f64 {
        match tf {
            Timeframe::Min1 => 525600.0,
            Timeframe::Min5 => 105120.0,
            Timeframe::Min15 => 35040.0,
            Timeframe::Min30 => 17520.0,
            Timeframe::Hour1 => 8760.0,
            Timeframe::Hour4 => 2190.0,
            Timeframe::Day1 => 365.0,
            Timeframe::Week1 => 52.0,
        }
    }
}
