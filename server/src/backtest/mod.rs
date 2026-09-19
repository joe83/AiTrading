pub mod data_loader;
pub mod metrics;
pub mod simulator;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::ai::AiEngine;
use crate::models::*;
use data_loader::DataLoader;
use metrics::BacktestMetrics;
use simulator::{SimConfig, SimFillResult, SimulatedExchange};

/// Configuration for a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub symbol: String,
    pub exchange: ExchangeId,
    pub timeframe: Timeframe,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub initial_balance: f64,
    /// Maker fee fraction (default 0.1%)
    pub maker_fee: Option<f64>,
    /// Taker fee fraction (default 0.1%)
    pub taker_fee: Option<f64>,
    /// Slippage fraction (default 0.05%)
    pub slippage: Option<f64>,
    /// Allow short selling
    pub allow_short: Option<bool>,
    /// Optional CSV file path for data instead of API fetch
    pub csv_path: Option<String>,
    /// Minimum bars of lookback before generating signals
    pub warmup_bars: Option<usize>,
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
        info!(
            "Starting backtest: {} {} from {} to {}",
            config.symbol, config.timeframe, config.start_time, config.end_time
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
            maker_fee: config.maker_fee.unwrap_or(0.001),
            taker_fee: config.taker_fee.unwrap_or(0.001),
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

            // Run AI analysis on the window (technical indicators only for speed)
            match Self::generate_signal(
                ai_engine,
                &config.symbol,
                config.exchange,
                &config.timeframe,
                window,
                candle,
            )
            .await
            {
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
        let metrics = BacktestMetrics::compute(
            simulator.completed_trades(),
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
        };

        info!(
            "Backtest complete: {} trades, {:.2}% return, {:.2} Sharpe, {:.1}% win rate in {:.1}s",
            result.metrics.total_trades,
            result.metrics.total_return_pct,
            result.metrics.sharpe_ratio,
            result.metrics.win_rate_pct,
            duration_secs,
        );

        Ok(result)
    }

    /// Generate a trading signal from a window of candles using technical indicators.
    /// Uses a simplified version of the AI pipeline for backtesting speed.
    async fn generate_signal(
        _ai_engine: &AiEngine,
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

        // Calculate basic indicators inline (avoid Grok API calls during backtest for speed)
        let rsi = Self::calculate_rsi(&closes, 14);
        let (ema_short, ema_long) = Self::calculate_emas(&closes);
        let _current_price = closes.last().copied().unwrap_or(0.0);

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
