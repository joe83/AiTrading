pub mod mexc;
pub mod alpaca;
pub mod ic_markets;
pub mod binance;
pub mod bybit;
pub mod manager;

use anyhow::Result;
use async_trait::async_trait;

use crate::models::*;

/// Unified exchange interface trait.
/// All exchange connectors must implement this trait, providing a consistent API
/// regardless of the underlying exchange.
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Get the exchange identifier.
    fn id(&self) -> ExchangeId;

    /// Get the market type this exchange supports.
    fn market_type(&self) -> MarketType;

    /// Connect to the exchange (authenticate, establish WebSocket, etc.)
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the exchange.
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if connected and ready to trade.
    fn is_connected(&self) -> bool;

    // =========================================================================
    // Market Data
    // =========================================================================

    /// Subscribe to real-time orderbook updates for a symbol.
    async fn subscribe_orderbook(&self, symbol: &str) -> Result<()>;

    /// Subscribe to real-time trade stream for a symbol.
    async fn subscribe_trades(&self, symbol: &str) -> Result<()>;

    /// Subscribe to real-time kline/candle updates.
    async fn subscribe_klines(&self, symbol: &str, timeframe: &Timeframe) -> Result<()>;

    /// Fetch historical candles.
    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &Timeframe,
        limit: usize,
    ) -> Result<Vec<Candle>>;

    /// Get current ticker for a symbol.
    async fn get_ticker(&self, symbol: &str) -> Result<Ticker>;

    // =========================================================================
    // Trading
    // =========================================================================

    /// Place an order on the exchange.
    async fn place_order(&self, order: &Order) -> Result<OrderResult>;

    /// Cancel an open order.
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()>;

    /// Get all open orders for a symbol.
    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>>;

    // =========================================================================
    // Account
    // =========================================================================

    /// Get account balance.
    async fn get_balance(&self) -> Result<Balance>;

    /// Get account information.
    async fn get_account_info(&self) -> Result<AccountInfo>;

    /// Get all open positions (for futures/margin).
    async fn get_positions(&self) -> Result<Vec<Position>>;
}

/// Market data event types pushed from exchange WebSocket.
#[derive(Debug, Clone)]
pub enum MarketEvent {
    OrderBookUpdate(OrderBook),
    TradeUpdate(MarketTrade),
    CandleUpdate(Candle),
    TickerUpdate(Ticker),
    ConnectionStatus(ConnectionStatus),
}

/// Connection status events.
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected,
    Disconnected(String),
    Reconnecting(u32),
    Error(String),
}
