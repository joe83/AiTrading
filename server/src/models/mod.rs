pub mod candle;
pub mod order;
pub mod signal;
pub mod account;

pub use candle::*;
pub use order::*;
pub use signal::*;
pub use account::*;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Supported market types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketType {
    Crypto,
    Stock,
    Forex,
}

/// Supported exchanges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExchangeId {
    Mexc,
    Alpaca,
    IcMarkets,
}

impl std::fmt::Display for ExchangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExchangeId::Mexc => write!(f, "MEXC"),
            ExchangeId::Alpaca => write!(f, "Alpaca"),
            ExchangeId::IcMarkets => write!(f, "IC Markets"),
        }
    }
}

/// Timeframe for candle/chart data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    #[serde(rename = "1m")]
    Min1,
    #[serde(rename = "5m")]
    Min5,
    #[serde(rename = "15m")]
    Min15,
    #[serde(rename = "30m")]
    Min30,
    #[serde(rename = "1h")]
    Hour1,
    #[serde(rename = "4h")]
    Hour4,
    #[serde(rename = "1d")]
    Day1,
    #[serde(rename = "1w")]
    Week1,
}

impl Timeframe {
    /// Duration in seconds for each timeframe.
    pub fn as_secs(&self) -> u64 {
        match self {
            Timeframe::Min1 => 60,
            Timeframe::Min5 => 300,
            Timeframe::Min15 => 900,
            Timeframe::Min30 => 1800,
            Timeframe::Hour1 => 3600,
            Timeframe::Hour4 => 14400,
            Timeframe::Day1 => 86400,
            Timeframe::Week1 => 604800,
        }
    }
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timeframe::Min1 => write!(f, "1m"),
            Timeframe::Min5 => write!(f, "5m"),
            Timeframe::Min15 => write!(f, "15m"),
            Timeframe::Min30 => write!(f, "30m"),
            Timeframe::Hour1 => write!(f, "1h"),
            Timeframe::Hour4 => write!(f, "4h"),
            Timeframe::Day1 => write!(f, "1d"),
            Timeframe::Week1 => write!(f, "1w"),
        }
    }
}

/// A trading symbol/pair (e.g., "BTCUSDT", "AAPL", "EURUSD").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub exchange: ExchangeId,
    pub market_type: MarketType,
    /// Base asset (e.g., "BTC", "AAPL", "EUR")
    pub base: String,
    /// Quote asset (e.g., "USDT", "USD")
    pub quote: String,
}

impl Symbol {
    pub fn new(name: &str, exchange: ExchangeId, market_type: MarketType, base: &str, quote: &str) -> Self {
        Self {
            name: name.to_string(),
            exchange,
            market_type,
            base: base.to_string(),
            quote: quote.to_string(),
        }
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.exchange, self.name)
    }
}

/// Order book snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub exchange: ExchangeId,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub timestamp: DateTime<Utc>,
}

/// A single price level in the order book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: Decimal,
}

/// A single trade from the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTrade {
    pub symbol: String,
    pub exchange: ExchangeId,
    pub price: Decimal,
    pub quantity: Decimal,
    pub is_buyer_maker: bool,
    pub timestamp: DateTime<Utc>,
}

/// Live ticker data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    pub exchange: ExchangeId,
    pub last_price: Decimal,
    pub bid: Decimal,
    pub ask: Decimal,
    pub high_24h: Decimal,
    pub low_24h: Decimal,
    pub volume_24h: Decimal,
    pub change_pct_24h: f64,
    pub timestamp: DateTime<Utc>,
}
