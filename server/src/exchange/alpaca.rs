use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use super::{ConnectionStatus, Exchange, MarketEvent};
use crate::config::AlpacaConfig;
use crate::models::*;

/// Alpaca exchange connector for US stock trading.
/// Supports paper and live trading via Alpaca API v2.
pub struct AlpacaExchange {
    config: AlpacaConfig,
    http_client: Client,
    connected: Arc<RwLock<bool>>,
    event_sender: broadcast::Sender<MarketEvent>,
    ws_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

/// Alpaca account response.
#[derive(Debug, Deserialize)]
struct AlpacaAccount {
    id: String,
    status: String,
    currency: String,
    cash: String,
    portfolio_value: String,
    buying_power: String,
    equity: String,
    #[serde(default)]
    trading_blocked: bool,
    #[serde(default)]
    transfers_blocked: bool,
}

/// Alpaca order response.
#[derive(Debug, Deserialize)]
struct AlpacaOrder {
    id: String,
    status: String,
    symbol: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    qty: Option<String>,
    filled_qty: Option<String>,
    filled_avg_price: Option<String>,
    #[serde(default)]
    failed_at: Option<String>,
}

/// Alpaca position response.
#[derive(Debug, Deserialize)]
struct AlpacaPosition {
    asset_id: String,
    symbol: String,
    side: String,
    qty: String,
    avg_entry_price: String,
    current_price: String,
    unrealized_pl: String,
    unrealized_plpc: String,
    market_value: String,
}

/// Alpaca bar (candle) response.
#[derive(Debug, Deserialize)]
struct AlpacaBar {
    t: String,
    o: f64,
    h: f64,
    l: f64,
    c: f64,
    v: u64,
}

/// Alpaca bars response wrapper.
#[derive(Debug, Deserialize)]
struct AlpacaBarsResponse {
    bars: Option<Vec<AlpacaBar>>,
    next_page_token: Option<String>,
}

/// Alpaca snapshot response.
#[derive(Debug, Deserialize)]
struct AlpacaSnapshot {
    #[serde(rename = "latestTrade")]
    latest_trade: Option<AlpacaLatestTrade>,
    #[serde(rename = "latestQuote")]
    latest_quote: Option<AlpacaLatestQuote>,
    #[serde(rename = "dailyBar")]
    daily_bar: Option<AlpacaBar>,
}

#[derive(Debug, Deserialize)]
struct AlpacaLatestTrade {
    p: f64,
}

#[derive(Debug, Deserialize)]
struct AlpacaLatestQuote {
    bp: f64,
    ap: f64,
}

impl AlpacaExchange {
    pub fn new(config: AlpacaConfig) -> Self {
        let (event_sender, _) = broadcast::channel(10000);
        Self {
            config,
            http_client: Client::new(),
            connected: Arc::new(RwLock::new(false)),
            event_sender,
            ws_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get a receiver for market events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<MarketEvent> {
        self.event_sender.subscribe()
    }

    /// Make an authenticated GET request to the Alpaca Trading API.
    async fn trading_get(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.config.base_url, endpoint);

        let response = self
            .http_client
            .get(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to send Alpaca trading GET request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Alpaca response")?;

        if !status.is_success() {
            anyhow::bail!("Alpaca API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated POST request to the Alpaca Trading API.
    async fn trading_post(
        &self,
        endpoint: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.config.base_url, endpoint);

        let response = self
            .http_client
            .post(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .context("Failed to send Alpaca trading POST request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Alpaca response")?;

        if !status.is_success() {
            anyhow::bail!("Alpaca API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated DELETE request to the Alpaca Trading API.
    async fn trading_delete(&self, endpoint: &str) -> Result<()> {
        let url = format!("{}/{}", self.config.base_url, endpoint);

        let response = self
            .http_client
            .delete(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .send()
            .await
            .context("Failed to send Alpaca DELETE request")?;

        if !response.status().is_success() {
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            anyhow::bail!("Alpaca DELETE error: {}", body);
        }

        Ok(())
    }

    /// Make a GET request to the Alpaca Data API.
    async fn data_get(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.config.data_url, endpoint);

        let response = self
            .http_client
            .get(&url)
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.secret_key)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to send Alpaca data GET request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Alpaca data response")?;

        if !status.is_success() {
            anyhow::bail!("Alpaca Data API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Convert our Timeframe to Alpaca timeframe string.
    fn timeframe_to_alpaca(tf: &Timeframe) -> &'static str {
        match tf {
            Timeframe::Min1 => "1Min",
            Timeframe::Min5 => "5Min",
            Timeframe::Min15 => "15Min",
            Timeframe::Min30 => "30Min",
            Timeframe::Hour1 => "1Hour",
            Timeframe::Hour4 => "4Hour",
            Timeframe::Day1 => "1Day",
            Timeframe::Week1 => "1Week",
        }
    }

    /// Spawn a WebSocket streaming task for Alpaca market data.
    async fn spawn_ws_stream(&self, subscription_msg: serde_json::Value) -> Result<()> {
        let ws_url = format!("{}/v2/sip", self.config.ws_url);
        let api_key = self.config.api_key.clone();
        let secret_key = self.config.secret_key.clone();
        let event_sender = self.event_sender.clone();

        let handle = tokio::spawn(async move {
            let mut reconnect_delay = 1u64;

            loop {
                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        info!("Alpaca WebSocket connected");
                        reconnect_delay = 1;
                        let (mut write, mut read) = ws_stream.split();

                        // Authenticate
                        let auth_msg = serde_json::json!({
                            "action": "auth",
                            "key": api_key,
                            "secret": secret_key,
                        });
                        if let Err(e) = write
                            .send(Message::Text(serde_json::to_string(&auth_msg).unwrap()))
                            .await
                        {
                            error!("Alpaca WS auth failed: {e}");
                            continue;
                        }

                        // Send subscription
                        if let Err(e) = write
                            .send(Message::Text(
                                serde_json::to_string(&subscription_msg).unwrap(),
                            ))
                            .await
                        {
                            error!("Alpaca WS subscription failed: {e}");
                            continue;
                        }

                        // Read messages
                        while let Some(msg_result) = read.next().await {
                            match msg_result {
                                Ok(Message::Text(text)) => {
                                    if let Ok(data) =
                                        serde_json::from_str::<Vec<serde_json::Value>>(&text)
                                    {
                                        for item in &data {
                                            if let Some(event) = Self::parse_ws_message(item) {
                                                let _ = event_sender.send(event);
                                            }
                                        }
                                    }
                                }
                                Ok(Message::Ping(data)) => {
                                    let _ = write.send(Message::Pong(data)).await;
                                }
                                Ok(Message::Close(_)) => {
                                    warn!("Alpaca WebSocket closed");
                                    break;
                                }
                                Err(e) => {
                                    error!("Alpaca WebSocket error: {e}");
                                    break;
                                }
                                _ => {}
                            }
                        }

                        let _ = event_sender.send(MarketEvent::ConnectionStatus(
                            ConnectionStatus::Disconnected("WebSocket closed".to_string()),
                        ));
                    }
                    Err(e) => {
                        error!("Failed to connect Alpaca WebSocket: {e}");
                    }
                }

                let _ = event_sender.send(MarketEvent::ConnectionStatus(
                    ConnectionStatus::Reconnecting(reconnect_delay as u32),
                ));
                tokio::time::sleep(tokio::time::Duration::from_secs(reconnect_delay)).await;
                reconnect_delay = (reconnect_delay * 2).min(60);
            }
        });

        self.ws_handles.lock().await.push(handle);
        Ok(())
    }

    /// Parse Alpaca WebSocket message into MarketEvent.
    fn parse_ws_message(data: &serde_json::Value) -> Option<MarketEvent> {
        let msg_type = data.get("T")?.as_str()?;

        match msg_type {
            "t" => {
                // Trade update
                let trade = MarketTrade {
                    symbol: data.get("S")?.as_str()?.to_string(),
                    exchange: ExchangeId::Alpaca,
                    price: Decimal::try_from(data.get("p")?.as_f64()?).ok()?,
                    quantity: Decimal::try_from(data.get("s")?.as_f64().unwrap_or(0.0)).ok()?,
                    is_buyer_maker: false,
                    timestamp: data
                        .get("t")?
                        .as_str()?
                        .parse::<DateTime<Utc>>()
                        .ok()?,
                };
                Some(MarketEvent::TradeUpdate(trade))
            }
            "b" => {
                // Bar (candle) update
                let candle = Candle {
                    symbol: data.get("S")?.as_str()?.to_string(),
                    exchange: ExchangeId::Alpaca,
                    timeframe: "1m".to_string(),
                    open_time: data.get("t")?.as_str()?.parse().ok()?,
                    close_time: Utc::now(),
                    open: Decimal::try_from(data.get("o")?.as_f64()?).ok()?,
                    high: Decimal::try_from(data.get("h")?.as_f64()?).ok()?,
                    low: Decimal::try_from(data.get("l")?.as_f64()?).ok()?,
                    close: Decimal::try_from(data.get("c")?.as_f64()?).ok()?,
                    volume: Decimal::try_from(data.get("v")?.as_f64().unwrap_or(0.0)).ok()?,
                };
                Some(MarketEvent::CandleUpdate(candle))
            }
            _ => None,
        }
    }
}

#[async_trait]
impl Exchange for AlpacaExchange {
    fn id(&self) -> ExchangeId {
        ExchangeId::Alpaca
    }

    fn market_type(&self) -> MarketType {
        MarketType::Stock
    }

    async fn connect(&mut self) -> Result<()> {
        // Verify credentials by fetching account info
        let body = self.trading_get("v2/account").await?;
        let account: AlpacaAccount =
            serde_json::from_value(body).context("Failed to parse Alpaca account")?;

        info!(
            "Alpaca connected: account={}, status={}, equity={}",
            account.id, account.status, account.equity
        );

        if account.trading_blocked {
            warn!("Alpaca trading is blocked on this account");
        }

        *self.connected.write().await = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.write().await = false;
        let mut handles = self.ws_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }
        info!("Alpaca disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn subscribe_orderbook(&self, _symbol: &str) -> Result<()> {
        info!("Alpaca does not support real-time orderbook — use quotes instead");
        Ok(())
    }

    async fn subscribe_trades(&self, symbol: &str) -> Result<()> {
        let msg = serde_json::json!({
            "action": "subscribe",
            "trades": [symbol],
        });
        self.spawn_ws_stream(msg).await?;
        info!("Subscribed to Alpaca trades: {symbol}");
        Ok(())
    }

    async fn subscribe_klines(&self, symbol: &str, _timeframe: &Timeframe) -> Result<()> {
        let msg = serde_json::json!({
            "action": "subscribe",
            "bars": [symbol],
        });
        self.spawn_ws_stream(msg).await?;
        info!("Subscribed to Alpaca bars: {symbol}");
        Ok(())
    }

    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &Timeframe,
        limit: usize,
    ) -> Result<Vec<Candle>> {
        let tf = Self::timeframe_to_alpaca(timeframe);
        let endpoint = format!(
            "v2/stocks/{}/bars?timeframe={}&limit={}&feed=sip",
            symbol, tf, limit
        );

        let body = self.data_get(&endpoint).await?;
        let response: AlpacaBarsResponse =
            serde_json::from_value(body).context("Failed to parse Alpaca bars")?;

        let candles = response
            .bars
            .unwrap_or_default()
            .iter()
            .filter_map(|bar| {
                let open_time: DateTime<Utc> = bar.t.parse().ok()?;
                let close_time =
                    open_time + chrono::Duration::seconds(timeframe.as_secs() as i64);

                Some(Candle {
                    symbol: symbol.to_string(),
                    exchange: ExchangeId::Alpaca,
                    timeframe: timeframe.to_string(),
                    open_time,
                    close_time,
                    open: Decimal::try_from(bar.o).ok()?,
                    high: Decimal::try_from(bar.h).ok()?,
                    low: Decimal::try_from(bar.l).ok()?,
                    close: Decimal::try_from(bar.c).ok()?,
                    volume: Decimal::from(bar.v),
                })
            })
            .collect();

        Ok(candles)
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        let endpoint = format!("v2/stocks/{}/snapshot?feed=sip", symbol);
        let body = self.data_get(&endpoint).await?;
        let snapshot: AlpacaSnapshot =
            serde_json::from_value(body).context("Failed to parse Alpaca snapshot")?;

        let last_price = snapshot
            .latest_trade
            .map(|t| Decimal::try_from(t.p).unwrap_or(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        let bid = snapshot
            .latest_quote
            .as_ref()
            .map(|q| Decimal::try_from(q.bp).unwrap_or(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        let ask = snapshot
            .latest_quote
            .as_ref()
            .map(|q| Decimal::try_from(q.ap).unwrap_or(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        let (high_24h, low_24h, volume_24h) = if let Some(ref bar) = snapshot.daily_bar {
            (
                Decimal::try_from(bar.h).unwrap_or(Decimal::ZERO),
                Decimal::try_from(bar.l).unwrap_or(Decimal::ZERO),
                Decimal::from(bar.v),
            )
        } else {
            (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO)
        };

        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: ExchangeId::Alpaca,
            last_price,
            bid,
            ask,
            high_24h,
            low_24h,
            volume_24h,
            change_pct_24h: 0.0,
            timestamp: Utc::now(),
        })
    }

    async fn place_order(&self, order: &Order) -> Result<OrderResult> {
        let side_str = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let type_str = match order.order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::StopLoss => "stop",
            OrderType::StopLimit => "stop_limit",
            _ => "market",
        };

        let mut body = serde_json::json!({
            "symbol": order.symbol,
            "qty": order.quantity.to_string(),
            "side": side_str,
            "type": type_str,
            "time_in_force": "day",
        });

        if let Some(price) = order.price {
            body["limit_price"] = serde_json::Value::String(price.to_string());
        }

        if let Some(stop) = order.stop_price {
            body["stop_price"] = serde_json::Value::String(stop.to_string());
        }

        let result = self.trading_post("v2/orders", &body).await?;

        let alpaca_order: AlpacaOrder = serde_json::from_value(result)
            .context("Failed to parse Alpaca order response")?;

        info!(
            "Alpaca order placed: {} {} {} (status: {})",
            order.symbol, side_str, order.quantity, alpaca_order.status
        );

        let status = match alpaca_order.status.as_str() {
            "filled" => OrderStatus::Filled,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "canceled" | "cancelled" => OrderStatus::Cancelled,
            "rejected" => OrderStatus::Rejected,
            "expired" => OrderStatus::Expired,
            _ => OrderStatus::Open,
        };

        Ok(OrderResult {
            exchange_order_id: alpaca_order.id,
            status,
            filled_quantity: alpaca_order
                .filled_qty
                .and_then(|q| q.parse().ok())
                .unwrap_or(Decimal::ZERO),
            average_fill_price: alpaca_order.filled_avg_price.and_then(|p| p.parse().ok()),
            message: alpaca_order.failed_at,
        })
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> Result<()> {
        self.trading_delete(&format!("v2/orders/{}", order_id))
            .await?;
        info!("Alpaca order cancelled: {order_id}");
        Ok(())
    }

    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>> {
        let endpoint = format!("v2/orders?status=open&symbols={}", symbol);
        let body = self.trading_get(&endpoint).await?;

        let alpaca_orders: Vec<AlpacaOrder> =
            serde_json::from_value(body).unwrap_or_default();

        let orders = alpaca_orders
            .iter()
            .map(|o| Order {
                id: uuid::Uuid::new_v4(),
                exchange_order_id: Some(o.id.clone()),
                exchange: ExchangeId::Alpaca,
                symbol: o.symbol.clone(),
                side: if o.side == "buy" {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                },
                order_type: match o.order_type.as_str() {
                    "limit" => OrderType::Limit,
                    "stop" => OrderType::StopLoss,
                    "stop_limit" => OrderType::StopLimit,
                    _ => OrderType::Market,
                },
                quantity: o
                    .qty
                    .as_deref()
                    .and_then(|q| q.parse().ok())
                    .unwrap_or(Decimal::ZERO),
                price: None,
                stop_price: None,
                status: OrderStatus::Open,
                filled_quantity: o
                    .filled_qty
                    .as_deref()
                    .and_then(|q| q.parse().ok())
                    .unwrap_or(Decimal::ZERO),
                average_fill_price: o
                    .filled_avg_price
                    .as_deref()
                    .and_then(|p| p.parse().ok()),
                fees: Decimal::ZERO,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                signal_id: None,
            })
            .collect();

        Ok(orders)
    }

    async fn get_balance(&self) -> Result<Balance> {
        let body = self.trading_get("v2/account").await?;
        let account: AlpacaAccount =
            serde_json::from_value(body).context("Failed to parse Alpaca account")?;

        let cash: Decimal = account.cash.parse().unwrap_or(Decimal::ZERO);
        let equity: Decimal = account.equity.parse().unwrap_or(Decimal::ZERO);

        Ok(Balance {
            exchange: ExchangeId::Alpaca,
            assets: vec![
                AssetBalance {
                    asset: account.currency.clone(),
                    free: cash,
                    locked: equity - cash,
                },
            ],
            total_usd: Some(equity),
        })
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        let body = self.trading_get("v2/account").await?;
        let account: AlpacaAccount =
            serde_json::from_value(body).context("Failed to parse Alpaca account")?;

        let balance = self.get_balance().await?;

        Ok(AccountInfo {
            exchange: ExchangeId::Alpaca,
            account_type: account.status,
            can_trade: !account.trading_blocked,
            can_withdraw: !account.transfers_blocked,
            balance,
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        let body = self.trading_get("v2/positions").await?;
        let alpaca_positions: Vec<AlpacaPosition> =
            serde_json::from_value(body).unwrap_or_default();

        let positions = alpaca_positions
            .iter()
            .filter_map(|p| {
                let entry_price: Decimal = p.avg_entry_price.parse().ok()?;
                let current_price: Decimal = p.current_price.parse().ok()?;
                let quantity: Decimal = p.qty.parse().ok()?;
                let unrealized_pnl: Decimal = p.unrealized_pl.parse().ok()?;
                let unrealized_pnl_pct: f64 = p.unrealized_plpc.parse().ok().unwrap_or(0.0);

                Some(Position {
                    id: uuid::Uuid::new_v4(),
                    exchange: ExchangeId::Alpaca,
                    symbol: p.symbol.clone(),
                    side: if p.side == "long" {
                        OrderSide::Buy
                    } else {
                        OrderSide::Sell
                    },
                    quantity,
                    entry_price,
                    current_price,
                    stop_loss: None,
                    take_profit: None,
                    unrealized_pnl,
                    unrealized_pnl_pct: unrealized_pnl_pct * 100.0,
                    realized_pnl: Decimal::ZERO,
                    fees_paid: Decimal::ZERO,
                    opened_at: Utc::now(),
                    updated_at: Utc::now(),
                    signal_id: None,
                })
            })
            .collect();

        Ok(positions)
    }
}
