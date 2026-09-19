use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use super::{ConnectionStatus, Exchange, MarketEvent};
use crate::config::ExchangeConfig;
use crate::models::*;

type HmacSha256 = Hmac<Sha256>;

/// MEXC exchange connector for cryptocurrency trading.
/// Supports spot trading with WebSocket market data and REST API order management.
pub struct MexcExchange {
    config: ExchangeConfig,
    http_client: Client,
    connected: Arc<RwLock<bool>>,
    event_sender: broadcast::Sender<MarketEvent>,
    /// Active WebSocket subscription handles.
    ws_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl MexcExchange {
    pub fn new(config: ExchangeConfig) -> Self {
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

    /// Generate HMAC-SHA256 signature for MEXC API requests.
    fn sign(&self, query_string: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.config.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Get current server timestamp in milliseconds.
    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    /// Make an authenticated GET request to MEXC REST API.
    async fn signed_get(&self, endpoint: &str, params: &mut HashMap<String, String>) -> Result<serde_json::Value> {
        let timestamp = Self::timestamp_ms().to_string();
        params.insert("timestamp".to_string(), timestamp);

        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let signature = self.sign(&query_string);
        let url = format!(
            "{}/api/v3/{endpoint}?{query_string}&signature={signature}",
            self.config.base_url
        );

        let response = self
            .http_client
            .get(&url)
            .header("X-MEXC-APIKEY", &self.config.api_key)
            .send()
            .await
            .context("Failed to send MEXC GET request")?;

        let status = response.status();
        let body: serde_json::Value = response.json().await
            .context("Failed to parse MEXC response")?;

        if !status.is_success() {
            anyhow::bail!("MEXC API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated POST request to MEXC REST API.
    async fn signed_post(&self, endpoint: &str, params: &mut HashMap<String, String>) -> Result<serde_json::Value> {
        let timestamp = Self::timestamp_ms().to_string();
        params.insert("timestamp".to_string(), timestamp);

        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let signature = self.sign(&query_string);

        let url = format!("{}/api/v3/{endpoint}", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("X-MEXC-APIKEY", &self.config.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("{query_string}&signature={signature}"))
            .send()
            .await
            .context("Failed to send MEXC POST request")?;

        let status = response.status();
        let body: serde_json::Value = response.json().await
            .context("Failed to parse MEXC response")?;

        if !status.is_success() {
            anyhow::bail!("MEXC API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make a public (unsigned) GET request.
    async fn public_get(&self, endpoint: &str, params: &HashMap<String, String>) -> Result<serde_json::Value> {
        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let url = if query_string.is_empty() {
            format!("{}/api/v3/{endpoint}", self.config.base_url)
        } else {
            format!("{}/api/v3/{endpoint}?{query_string}", self.config.base_url)
        };

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to send MEXC public request")?;

        let body: serde_json::Value = response.json().await
            .context("Failed to parse MEXC response")?;

        Ok(body)
    }

    /// Spawn a WebSocket connection task for market data streaming.
    async fn spawn_ws_stream(&self, subscription_msg: serde_json::Value) -> Result<()> {
        let ws_url = self.config.ws_url.clone();
        let event_sender = self.event_sender.clone();
        let connected = self.connected.clone();

        let handle = tokio::spawn(async move {
            let mut reconnect_delay = 1u64;

            loop {
                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        info!("MEXC WebSocket connected");
                        reconnect_delay = 1;
                        let (mut write, mut read) = ws_stream.split();

                        // Send subscription
                        let sub_msg = serde_json::to_string(&subscription_msg)
                            .expect("Failed to serialize subscription");
                        if let Err(e) = write.send(Message::Text(sub_msg)).await {
                            error!("Failed to send MEXC subscription: {e}");
                            continue;
                        }

                        // Read messages
                        while let Some(msg_result) = read.next().await {
                            match msg_result {
                                Ok(Message::Text(text)) => {
                                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                                        // Parse and emit market events
                                        if let Some(event) = Self::parse_ws_message(&data) {
                                            let _ = event_sender.send(event);
                                        }
                                    }
                                }
                                Ok(Message::Ping(data)) => {
                                    let _ = write.send(Message::Pong(data)).await;
                                }
                                Ok(Message::Close(_)) => {
                                    warn!("MEXC WebSocket closed by server");
                                    break;
                                }
                                Err(e) => {
                                    error!("MEXC WebSocket error: {e}");
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
                        error!("Failed to connect MEXC WebSocket: {e}");
                    }
                }

                // Reconnect with exponential backoff (max 60 seconds)
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

    /// Parse incoming WebSocket message into a MarketEvent.
    fn parse_ws_message(data: &serde_json::Value) -> Option<MarketEvent> {
        // MEXC WebSocket message format varies by channel
        let channel = data.get("c")?.as_str()?;

        if channel.contains("@kline") {
            // Kline/candle update
            if let Some(kline_data) = data.get("d") {
                let candle = Candle {
                    symbol: data.get("s")?.as_str()?.to_string(),
                    exchange: ExchangeId::Mexc,
                    timeframe: kline_data.get("i")?.as_str()?.to_string(),
                    open_time: DateTime::from_timestamp(
                        kline_data.get("t")?.as_i64()? / 1000,
                        0,
                    )?,
                    close_time: DateTime::from_timestamp(
                        kline_data.get("T")?.as_i64()? / 1000,
                        0,
                    )?,
                    open: kline_data.get("o")?.as_str()?.parse().ok()?,
                    high: kline_data.get("h")?.as_str()?.parse().ok()?,
                    low: kline_data.get("l")?.as_str()?.parse().ok()?,
                    close: kline_data.get("c")?.as_str()?.parse().ok()?,
                    volume: kline_data.get("v")?.as_str()?.parse().ok()?,
                };
                return Some(MarketEvent::CandleUpdate(candle));
            }
        } else if channel.contains("@trade") {
            // Trade update
            if let Some(trade_data) = data.get("d") {
                let trade = MarketTrade {
                    symbol: data.get("s")?.as_str()?.to_string(),
                    exchange: ExchangeId::Mexc,
                    price: trade_data.get("p")?.as_str()?.parse().ok()?,
                    quantity: trade_data.get("v")?.as_str()?.parse().ok()?,
                    is_buyer_maker: trade_data.get("S")?.as_i64()? == 2,
                    timestamp: DateTime::from_timestamp(
                        trade_data.get("t")?.as_i64()? / 1000,
                        0,
                    )?,
                };
                return Some(MarketEvent::TradeUpdate(trade));
            }
        }

        None
    }

    /// Convert MEXC interval string to timeframe.
    fn timeframe_to_interval(tf: &Timeframe) -> &'static str {
        match tf {
            Timeframe::Min1 => "Min1",
            Timeframe::Min5 => "Min5",
            Timeframe::Min15 => "Min15",
            Timeframe::Min30 => "Min30",
            Timeframe::Hour1 => "Min60",
            Timeframe::Hour4 => "Hour4",
            Timeframe::Day1 => "Day1",
            Timeframe::Week1 => "Week1",
        }
    }
}

#[async_trait]
impl Exchange for MexcExchange {
    fn id(&self) -> ExchangeId {
        ExchangeId::Mexc
    }

    fn market_type(&self) -> MarketType {
        MarketType::Crypto
    }

    async fn connect(&mut self) -> Result<()> {
        // Test connectivity with server time check
        let params = HashMap::new();
        let result = self.public_get("time", &params).await?;
        info!("MEXC connected. Server time: {}", result);

        *self.connected.write().await = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.write().await = false;
        let mut handles = self.ws_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }
        info!("MEXC disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        // Use try_read to avoid blocking
        self.connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn subscribe_orderbook(&self, symbol: &str) -> Result<()> {
        let msg = serde_json::json!({
            "method": "SUBSCRIPTION",
            "params": [format!("spot@public.bookTicker.v3.api@{symbol}")]
        });
        self.spawn_ws_stream(msg).await?;
        info!("Subscribed to MEXC orderbook: {symbol}");
        Ok(())
    }

    async fn subscribe_trades(&self, symbol: &str) -> Result<()> {
        let msg = serde_json::json!({
            "method": "SUBSCRIPTION",
            "params": [format!("spot@public.deals.v3.api@{symbol}")]
        });
        self.spawn_ws_stream(msg).await?;
        info!("Subscribed to MEXC trades: {symbol}");
        Ok(())
    }

    async fn subscribe_klines(&self, symbol: &str, timeframe: &Timeframe) -> Result<()> {
        let interval = Self::timeframe_to_interval(timeframe);
        let msg = serde_json::json!({
            "method": "SUBSCRIPTION",
            "params": [format!("spot@public.kline.v3.api@{symbol}@{interval}")]
        });
        self.spawn_ws_stream(msg).await?;
        info!("Subscribed to MEXC klines: {symbol} {timeframe}");
        Ok(())
    }

    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &Timeframe,
        limit: usize,
    ) -> Result<Vec<Candle>> {
        let interval = Self::timeframe_to_interval(timeframe);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("interval".to_string(), interval.to_string());
        params.insert("limit".to_string(), limit.to_string());

        let result = self.public_get("klines", &params).await?;

        let candles: Vec<Candle> = result
            .as_array()
            .context("Expected array of klines")?
            .iter()
            .filter_map(|k| {
                let arr = k.as_array()?;
                Some(Candle {
                    symbol: symbol.to_string(),
                    exchange: ExchangeId::Mexc,
                    timeframe: timeframe.to_string(),
                    open_time: DateTime::from_timestamp(arr[0].as_i64()? / 1000, 0)?,
                    close_time: DateTime::from_timestamp(arr[6].as_i64()? / 1000, 0)?,
                    open: arr[1].as_str()?.parse().ok()?,
                    high: arr[2].as_str()?.parse().ok()?,
                    low: arr[3].as_str()?.parse().ok()?,
                    close: arr[4].as_str()?.parse().ok()?,
                    volume: arr[5].as_str()?.parse().ok()?,
                })
            })
            .collect();

        debug!("Fetched {} candles for {symbol}", candles.len());
        Ok(candles)
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_string());

        let result = self.public_get("ticker/24hr", &params).await?;

        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: ExchangeId::Mexc,
            last_price: result["lastPrice"].as_str().unwrap_or("0").parse()?,
            bid: result["bidPrice"].as_str().unwrap_or("0").parse()?,
            ask: result["askPrice"].as_str().unwrap_or("0").parse()?,
            high_24h: result["highPrice"].as_str().unwrap_or("0").parse()?,
            low_24h: result["lowPrice"].as_str().unwrap_or("0").parse()?,
            volume_24h: result["volume"].as_str().unwrap_or("0").parse()?,
            change_pct_24h: result["priceChangePercent"]
                .as_str()
                .unwrap_or("0")
                .parse()?,
            timestamp: Utc::now(),
        })
    }

    async fn place_order(&self, order: &Order) -> Result<OrderResult> {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), order.symbol.clone());
        params.insert(
            "side".to_string(),
            match order.side {
                OrderSide::Buy => "BUY".to_string(),
                OrderSide::Sell => "SELL".to_string(),
            },
        );
        params.insert(
            "type".to_string(),
            match order.order_type {
                OrderType::Market => "MARKET".to_string(),
                OrderType::Limit => "LIMIT".to_string(),
                _ => "MARKET".to_string(),
            },
        );
        params.insert("quantity".to_string(), order.quantity.to_string());

        if let Some(price) = order.price {
            params.insert("price".to_string(), price.to_string());
        }

        let result = self.signed_post("order", &mut params).await?;

        info!(
            "MEXC order placed: {} {} {} @ {}",
            order.symbol,
            match order.side {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            },
            order.quantity,
            order.price.map(|p| p.to_string()).unwrap_or("MARKET".to_string()),
        );

        Ok(OrderResult {
            exchange_order_id: result["orderId"].as_str().unwrap_or("").to_string(),
            status: match result["status"].as_str().unwrap_or("NEW") {
                "FILLED" => OrderStatus::Filled,
                "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
                "CANCELED" => OrderStatus::Cancelled,
                _ => OrderStatus::Open,
            },
            filled_quantity: result["executedQty"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(Decimal::ZERO),
            average_fill_price: result["price"]
                .as_str()
                .and_then(|p| p.parse().ok()),
            message: result["msg"].as_str().map(|s| s.to_string()),
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()> {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("orderId".to_string(), order_id.to_string());

        self.signed_post("order", &mut params).await?;
        info!("MEXC order cancelled: {order_id}");
        Ok(())
    }

    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>> {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_string());

        let result = self.signed_get("openOrders", &mut params).await?;
        let orders: Vec<Order> = result
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|o| {
                Some(Order {
                    id: uuid::Uuid::new_v4(),
                    exchange_order_id: Some(o["orderId"].as_str()?.to_string()),
                    exchange: ExchangeId::Mexc,
                    symbol: o["symbol"].as_str()?.to_string(),
                    side: match o["side"].as_str()? {
                        "BUY" => OrderSide::Buy,
                        _ => OrderSide::Sell,
                    },
                    order_type: match o["type"].as_str()? {
                        "LIMIT" => OrderType::Limit,
                        _ => OrderType::Market,
                    },
                    quantity: o["origQty"].as_str()?.parse().ok()?,
                    price: o["price"].as_str().and_then(|p| p.parse().ok()),
                    stop_price: None,
                    status: OrderStatus::Open,
                    filled_quantity: o["executedQty"].as_str()?.parse().ok()?,
                    average_fill_price: None,
                    fees: Decimal::ZERO,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    signal_id: None,
                })
            })
            .collect();

        Ok(orders)
    }

    async fn get_balance(&self) -> Result<Balance> {
        let mut params = HashMap::new();
        let result = self.signed_get("account", &mut params).await?;

        let assets: Vec<AssetBalance> = result["balances"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|b| {
                let free: Decimal = b["free"].as_str()?.parse().ok()?;
                let locked: Decimal = b["locked"].as_str()?.parse().ok()?;
                if free.is_zero() && locked.is_zero() {
                    return None;
                }
                Some(AssetBalance {
                    asset: b["asset"].as_str()?.to_string(),
                    free,
                    locked,
                })
            })
            .collect();

        Ok(Balance {
            exchange: ExchangeId::Mexc,
            assets,
            total_usd: None,
        })
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        let balance = self.get_balance().await?;

        Ok(AccountInfo {
            exchange: ExchangeId::Mexc,
            account_type: "SPOT".to_string(),
            can_trade: true,
            can_withdraw: true,
            balance,
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        // MEXC spot doesn't have native positions — positions are tracked locally
        Ok(vec![])
    }
}
