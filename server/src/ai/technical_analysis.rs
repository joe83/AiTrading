use anyhow::Result;
use tracing::debug;

use crate::models::CandleSeries;

/// Technical indicator analyzer.
/// Computes RSI, MACD, Bollinger Bands, EMA, SMA, Stochastic, ATR, and volume analysis.
pub struct TechnicalAnalyzer;

/// Result of technical analysis on a candle series.
#[derive(Debug, Clone)]
pub struct TechnicalResult {
    pub summary: String,
    pub rsi: Option<f64>,
    pub macd: Option<MacdResult>,
    pub bollinger: Option<BollingerResult>,
    pub ema_short: Option<f64>,
    pub ema_long: Option<f64>,
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_200: Option<f64>,
    pub atr: Option<f64>,
    pub stochastic_k: Option<f64>,
    pub stochastic_d: Option<f64>,
    pub volume_sma: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub trend: TrendDirection,
    pub strength: f64,
}

#[derive(Debug, Clone)]
pub struct MacdResult {
    pub macd_line: f64,
    pub signal_line: f64,
    pub histogram: f64,
}

#[derive(Debug, Clone)]
pub struct BollingerResult {
    pub upper: f64,
    pub middle: f64,
    pub lower: f64,
    pub bandwidth: f64,
    pub percent_b: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrendDirection {
    StrongBullish,
    Bullish,
    Neutral,
    Bearish,
    StrongBearish,
}

impl std::fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrendDirection::StrongBullish => write!(f, "Strong Bullish"),
            TrendDirection::Bullish => write!(f, "Bullish"),
            TrendDirection::Neutral => write!(f, "Neutral"),
            TrendDirection::Bearish => write!(f, "Bearish"),
            TrendDirection::StrongBearish => write!(f, "Strong Bearish"),
        }
    }
}

impl TechnicalAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Run full technical analysis on a candle series.
    pub fn analyze(&self, candles: &CandleSeries) -> Result<TechnicalResult> {
        let closes = candles.closes_f64();
        let highs = candles.highs_f64();
        let lows = candles.lows_f64();
        let volumes = candles.volumes_f64();

        if closes.len() < 20 {
            anyhow::bail!("Need at least 20 candles for technical analysis, got {}", closes.len());
        }

        let rsi = self.calculate_rsi(&closes, 14);
        let macd = self.calculate_macd(&closes, 12, 26, 9);
        let bollinger = self.calculate_bollinger(&closes, 20, 2.0);
        let ema_short = self.calculate_ema(&closes, 9);
        let ema_long = self.calculate_ema(&closes, 21);
        let sma_20 = self.calculate_sma(&closes, 20);
        let sma_50 = self.calculate_sma(&closes, 50);
        let sma_200 = self.calculate_sma(&closes, 200);
        let atr = self.calculate_atr(&highs, &lows, &closes, 14);
        let (stoch_k, stoch_d) = self.calculate_stochastic(&highs, &lows, &closes, 14, 3);
        let volume_sma = self.calculate_sma(&volumes, 20);
        let volume_ratio = volume_sma.map(|vs| {
            if vs > 0.0 {
                volumes.last().copied().unwrap_or(0.0) / vs
            } else {
                1.0
            }
        });

        // Determine overall trend
        let trend = self.determine_trend(
            rsi,
            &macd,
            &bollinger,
            ema_short,
            ema_long,
            sma_50,
            sma_200,
            &closes,
        );

        // Calculate signal strength (0.0 to 1.0)
        let strength = self.calculate_strength(rsi, &macd, &bollinger, ema_short, ema_long, &trend);

        let summary = self.generate_summary(
            rsi, &macd, &bollinger, ema_short, ema_long, atr, stoch_k, &trend, strength,
        );

        debug!("Technical analysis: trend={trend}, strength={strength:.2}");

        Ok(TechnicalResult {
            summary,
            rsi,
            macd,
            bollinger,
            ema_short,
            ema_long,
            sma_20,
            sma_50,
            sma_200,
            atr,
            stochastic_k: stoch_k,
            stochastic_d: stoch_d,
            volume_sma,
            volume_ratio,
            trend,
            strength,
        })
    }

    /// Calculate RSI (Relative Strength Index).
    fn calculate_rsi(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period + 1 {
            return None;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }

        // Initial average
        let mut avg_gain: f64 = gains[..period].iter().sum::<f64>() / period as f64;
        let mut avg_loss: f64 = losses[..period].iter().sum::<f64>() / period as f64;

        // Smoothed average (Wilder's method)
        for i in period..gains.len() {
            avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
            avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;
        }

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    /// Calculate MACD (Moving Average Convergence Divergence).
    fn calculate_macd(&self, prices: &[f64], fast: usize, slow: usize, signal: usize) -> Option<MacdResult> {
        let ema_fast = self.calculate_ema_series(prices, fast)?;
        let ema_slow = self.calculate_ema_series(prices, slow)?;

        if ema_fast.len() != ema_slow.len() {
            return None;
        }

        let macd_line: Vec<f64> = ema_fast
            .iter()
            .zip(ema_slow.iter())
            .map(|(f, s)| f - s)
            .collect();

        let signal_line = self.calculate_ema_series(&macd_line, signal)?;

        let last_macd = *macd_line.last()?;
        let last_signal = *signal_line.last()?;

        Some(MacdResult {
            macd_line: last_macd,
            signal_line: last_signal,
            histogram: last_macd - last_signal,
        })
    }

    /// Calculate Bollinger Bands.
    fn calculate_bollinger(&self, prices: &[f64], period: usize, num_std: f64) -> Option<BollingerResult> {
        if prices.len() < period {
            return None;
        }

        let recent = &prices[prices.len() - period..];
        let middle = recent.iter().sum::<f64>() / period as f64;

        let variance = recent.iter().map(|p| (p - middle).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();

        let upper = middle + num_std * std_dev;
        let lower = middle - num_std * std_dev;
        let bandwidth = (upper - lower) / middle;

        let current_price = *prices.last()?;
        let percent_b = if upper != lower {
            (current_price - lower) / (upper - lower)
        } else {
            0.5
        };

        Some(BollingerResult {
            upper,
            middle,
            lower,
            bandwidth,
            percent_b,
        })
    }

    /// Calculate EMA (Exponential Moving Average) — returns latest value.
    fn calculate_ema(&self, prices: &[f64], period: usize) -> Option<f64> {
        let series = self.calculate_ema_series(prices, period)?;
        series.last().copied()
    }

    /// Calculate EMA series.
    fn calculate_ema_series(&self, prices: &[f64], period: usize) -> Option<Vec<f64>> {
        if prices.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = Vec::with_capacity(prices.len());

        // Start with SMA for the first value
        let initial_sma: f64 = prices[..period].iter().sum::<f64>() / period as f64;
        ema.push(initial_sma);

        for i in period..prices.len() {
            let prev = *ema.last().unwrap();
            ema.push((prices[i] - prev) * multiplier + prev);
        }

        Some(ema)
    }

    /// Calculate SMA (Simple Moving Average) — returns latest value.
    fn calculate_sma(&self, data: &[f64], period: usize) -> Option<f64> {
        if data.len() < period {
            return None;
        }
        let sum: f64 = data[data.len() - period..].iter().sum();
        Some(sum / period as f64)
    }

    /// Calculate ATR (Average True Range).
    fn calculate_atr(&self, highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Option<f64> {
        if highs.len() < period + 1 || lows.len() < period + 1 || closes.len() < period + 1 {
            return None;
        }

        let mut true_ranges = Vec::new();
        for i in 1..highs.len() {
            let tr = (highs[i] - lows[i])
                .max((highs[i] - closes[i - 1]).abs())
                .max((lows[i] - closes[i - 1]).abs());
            true_ranges.push(tr);
        }

        // Initial ATR is SMA of first `period` true ranges
        if true_ranges.len() < period {
            return None;
        }

        let mut atr: f64 = true_ranges[..period].iter().sum::<f64>() / period as f64;

        // Smoothed ATR
        for i in period..true_ranges.len() {
            atr = (atr * (period - 1) as f64 + true_ranges[i]) / period as f64;
        }

        Some(atr)
    }

    /// Calculate Stochastic Oscillator (%K and %D).
    fn calculate_stochastic(
        &self,
        highs: &[f64],
        lows: &[f64],
        closes: &[f64],
        k_period: usize,
        d_period: usize,
    ) -> (Option<f64>, Option<f64>) {
        if closes.len() < k_period {
            return (None, None);
        }

        let mut k_values = Vec::new();
        for i in (k_period - 1)..closes.len() {
            let start = i + 1 - k_period;
            let period_highs = &highs[start..=i];
            let period_lows = &lows[start..=i];

            let highest = period_highs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let lowest = period_lows.iter().cloned().fold(f64::INFINITY, f64::min);

            let k = if highest != lowest {
                ((closes[i] - lowest) / (highest - lowest)) * 100.0
            } else {
                50.0
            };
            k_values.push(k);
        }

        let latest_k = k_values.last().copied();
        let latest_d = self.calculate_sma(&k_values, d_period);

        (latest_k, latest_d)
    }

    /// Determine overall trend direction from multiple indicators.
    fn determine_trend(
        &self,
        rsi: Option<f64>,
        macd: &Option<MacdResult>,
        bollinger: &Option<BollingerResult>,
        ema_short: Option<f64>,
        ema_long: Option<f64>,
        sma_50: Option<f64>,
        sma_200: Option<f64>,
        closes: &[f64],
    ) -> TrendDirection {
        let mut bullish_signals = 0i32;
        let mut bearish_signals = 0i32;

        let current_price = *closes.last().unwrap_or(&0.0);

        // RSI signals
        if let Some(rsi_val) = rsi {
            if rsi_val > 70.0 {
                bearish_signals += 1; // Overbought
            } else if rsi_val < 30.0 {
                bullish_signals += 1; // Oversold (potential reversal)
            } else if rsi_val > 50.0 {
                bullish_signals += 1;
            } else {
                bearish_signals += 1;
            }
        }

        // MACD signals
        if let Some(macd_val) = macd {
            if macd_val.histogram > 0.0 {
                bullish_signals += 1;
            } else {
                bearish_signals += 1;
            }
            if macd_val.macd_line > macd_val.signal_line {
                bullish_signals += 1;
            } else {
                bearish_signals += 1;
            }
        }

        // EMA crossover
        if let (Some(short), Some(long)) = (ema_short, ema_long) {
            if short > long {
                bullish_signals += 2; // Strong signal
            } else {
                bearish_signals += 2;
            }
        }

        // Golden/Death cross (SMA 50 vs 200)
        if let (Some(s50), Some(s200)) = (sma_50, sma_200) {
            if s50 > s200 {
                bullish_signals += 2;
            } else {
                bearish_signals += 2;
            }
        }

        // Price vs Bollinger
        if let Some(bb) = bollinger {
            if current_price > bb.upper {
                bearish_signals += 1; // Overbought
            } else if current_price < bb.lower {
                bullish_signals += 1; // Oversold
            }
        }

        let net = bullish_signals - bearish_signals;
        match net {
            n if n >= 5 => TrendDirection::StrongBullish,
            n if n >= 2 => TrendDirection::Bullish,
            n if n <= -5 => TrendDirection::StrongBearish,
            n if n <= -2 => TrendDirection::Bearish,
            _ => TrendDirection::Neutral,
        }
    }

    /// Calculate signal strength from indicators.
    fn calculate_strength(
        &self,
        rsi: Option<f64>,
        macd: &Option<MacdResult>,
        bollinger: &Option<BollingerResult>,
        ema_short: Option<f64>,
        ema_long: Option<f64>,
        trend: &TrendDirection,
    ) -> f64 {
        let mut score = 0.0;
        let mut count = 0.0;

        // RSI strength
        if let Some(rsi_val) = rsi {
            let rsi_strength = if rsi_val > 70.0 || rsi_val < 30.0 {
                0.8 // Strong extreme
            } else if rsi_val > 60.0 || rsi_val < 40.0 {
                0.6
            } else {
                0.3
            };
            score += rsi_strength;
            count += 1.0;
        }

        // MACD strength
        if let Some(macd_val) = macd {
            let hist_strength = macd_val.histogram.abs().min(1.0);
            score += hist_strength * 0.8;
            count += 1.0;
        }

        // EMA crossover strength
        if let (Some(short), Some(long)) = (ema_short, ema_long) {
            let spread = ((short - long) / long).abs();
            score += spread.min(1.0) * 0.7;
            count += 1.0;
        }

        // Bollinger position strength
        if let Some(bb) = bollinger {
            let position_strength = (bb.percent_b - 0.5).abs() * 2.0;
            score += position_strength.min(1.0) * 0.6;
            count += 1.0;
        }

        if count > 0.0 {
            (score / count).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Generate human-readable analysis summary.
    fn generate_summary(
        &self,
        rsi: Option<f64>,
        macd: &Option<MacdResult>,
        bollinger: &Option<BollingerResult>,
        ema_short: Option<f64>,
        ema_long: Option<f64>,
        atr: Option<f64>,
        stoch_k: Option<f64>,
        trend: &TrendDirection,
        strength: f64,
    ) -> String {
        let mut parts = Vec::new();

        parts.push(format!("Trend: {trend} (strength: {:.0}%)", strength * 100.0));

        if let Some(rsi_val) = rsi {
            let condition = if rsi_val > 70.0 {
                "OVERBOUGHT"
            } else if rsi_val < 30.0 {
                "OVERSOLD"
            } else {
                "neutral"
            };
            parts.push(format!("RSI(14): {rsi_val:.1} [{condition}]"));
        }

        if let Some(macd_val) = macd {
            let signal = if macd_val.histogram > 0.0 {
                "bullish"
            } else {
                "bearish"
            };
            parts.push(format!(
                "MACD: {:.4} / Signal: {:.4} / Hist: {:.4} [{signal}]",
                macd_val.macd_line, macd_val.signal_line, macd_val.histogram
            ));
        }

        if let (Some(short), Some(long)) = (ema_short, ema_long) {
            let cross = if short > long {
                "bullish crossover"
            } else {
                "bearish crossover"
            };
            parts.push(format!("EMA(9): {short:.2} / EMA(21): {long:.2} [{cross}]"));
        }

        if let Some(bb) = bollinger {
            parts.push(format!(
                "Bollinger: upper={:.2} mid={:.2} lower={:.2} (%%B={:.2})",
                bb.upper, bb.middle, bb.lower, bb.percent_b
            ));
        }

        if let Some(atr_val) = atr {
            parts.push(format!("ATR(14): {atr_val:.4}"));
        }

        if let Some(k) = stoch_k {
            let condition = if k > 80.0 {
                "overbought"
            } else if k < 20.0 {
                "oversold"
            } else {
                "neutral"
            };
            parts.push(format!("Stochastic %%K: {k:.1} [{condition}]"));
        }

        parts.join(" | ")
    }
}
