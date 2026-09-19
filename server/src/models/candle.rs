use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::ExchangeId;

/// OHLCV candle data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub exchange: ExchangeId,
    pub timeframe: String,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
}

impl Candle {
    /// Check if candle is bullish (close > open).
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    /// Check if candle is bearish (close < open).
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// Get the candle body size (absolute difference between open and close).
    pub fn body_size(&self) -> Decimal {
        if self.close > self.open {
            self.close - self.open
        } else {
            self.open - self.close
        }
    }

    /// Get the full range (high - low).
    pub fn range(&self) -> Decimal {
        self.high - self.low
    }

    /// Upper shadow/wick size.
    pub fn upper_shadow(&self) -> Decimal {
        let body_top = std::cmp::max(self.open, self.close);
        self.high - body_top
    }

    /// Lower shadow/wick size.
    pub fn lower_shadow(&self) -> Decimal {
        let body_bottom = std::cmp::min(self.open, self.close);
        body_bottom - self.low
    }

    /// Check if it's a doji (very small body relative to range).
    pub fn is_doji(&self, threshold: Decimal) -> bool {
        let range = self.range();
        if range.is_zero() {
            return true;
        }
        self.body_size() / range < threshold
    }
}

/// A series of candles for a symbol/timeframe combination.
#[derive(Debug, Clone)]
pub struct CandleSeries {
    pub symbol: String,
    pub exchange: ExchangeId,
    pub timeframe: String,
    pub candles: Vec<Candle>,
}

impl CandleSeries {
    pub fn new(symbol: &str, exchange: ExchangeId, timeframe: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            exchange,
            timeframe: timeframe.to_string(),
            candles: Vec::new(),
        }
    }

    /// Add a candle to the series, maintaining chronological order.
    pub fn push(&mut self, candle: Candle) {
        self.candles.push(candle);
    }

    /// Get the latest N candles.
    pub fn latest(&self, n: usize) -> &[Candle] {
        let len = self.candles.len();
        if n >= len {
            &self.candles
        } else {
            &self.candles[len - n..]
        }
    }

    /// Get closing prices as f64 vector (for indicator calculations).
    pub fn closes_f64(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.close.to_string().parse::<f64>().unwrap_or(0.0))
            .collect()
    }

    /// Get high prices as f64 vector.
    pub fn highs_f64(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.high.to_string().parse::<f64>().unwrap_or(0.0))
            .collect()
    }

    /// Get low prices as f64 vector.
    pub fn lows_f64(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.low.to_string().parse::<f64>().unwrap_or(0.0))
            .collect()
    }

    /// Get volumes as f64 vector.
    pub fn volumes_f64(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.volume.to_string().parse::<f64>().unwrap_or(0.0))
            .collect()
    }
}
