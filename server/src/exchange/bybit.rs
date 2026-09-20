use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use reqwest::Client;
use rust_decimal::Decimal;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use super::{ConnectionStatus, Exchange, MarketEvent};
use crate::config::BybitConfig;
use crate::models::*;

type HmacSha256 = Hmac<Sha256>;

/// Bybit exchange connector for cryptocurrency trading.
/// Supports unified V5 API for spot market data and order management.
pub struct BybitExchange {
    config: BybitConfig,
    http_client: Client,
    connected: Arc<RwLock<bool>>,
    event_sender: broadcast::Sender<MarketEvent>,
    /// Active WebSocket subscription handles.
    ws_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl BybitExchange {
    pub fn new(config: BybitConfig) -> Self {
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

    /// Generate HMAC-SHA256 signature for Bybit V5 API requests.
    /// Bybit V5 signature: timestamp + api_key + recv_window + payload
    fn sign(&self, timestamp: &str, recv_window: &str, payload: &str) -> String {
        let sign_str = format!("{}{}{}{}", timestamp, self.config.api_key, recv_window, payload);
        let mut mac = HmacSha256::new_from_slice(self.config.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(sign_str.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Get current server timestamp in milliseconds.
    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    /// Make an unauthenticated GET request to Bybit V5 REST API.
    async fn public_get(&self, endpoint: &str, params: &HashMap<String, String>) -> Result<serde_json::Value> {
        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let url = if query_string.is_empty() {
            format!("{}/v5/{endpoint}", self.config.base_url)
        } else {
            format!("{}/v5/{endpoint}?{query_string}", self.config.base_url)
        };

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to send Bybit public request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Bybit response")?;

        if !status.is_success() {
            anyhow::bail!("Bybit API error ({}): {}", status, body);
        }

        if let Some(ret_code) = body.get("retCode").and_then(|r| r.as_i64()) {
            if ret_code != 0 {
                let ret_msg = body.get("retMsg").and_then(|m| m.as_str()).unwrap_or("Unknown");
                anyhow::bail!("Bybit API error code {ret_code}: {ret_msg}");
            }
        }

        Ok(body)
    }

    /// Make an authenticated GET request to Bybit V5 REST API.
    async fn signed_get(&self, endpoint: &str, params: &HashMap<String, String>) -> Result<serde_json::Value> {
        let timestamp = Self::timestamp_ms().to_string();
        let recv_window = "5000";

        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let signature = self.sign(&timestamp, recv_window, &query_string);
        let url = if query_string.is_empty() {
            format!("{}/v5/{endpoint}", self.config.base_url)
        } else {
            format!("{}/v5/{endpoint}?{query_string}", self.config.base_url)
        };

        let response = self
            .http_client
            .get(&url)
            .header("X-BAPI-API-KEY", &self.config.api_key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-SIGN", &signature)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .send()
            .await
            .context("Failed to send Bybit GET request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Bybit response")?;

        if !status.is_success() {
            anyhow::bail!("Bybit API error ({}): {}", status, body);
        }

        if let Some(ret_code) = body.get("retCode").and_then(|r| r.as_i64()) {
            if ret_code != 0 {
                let ret_msg = body.get("retMsg").and_then(|m| m.as_str()).unwrap_or("Unknown");
                anyhow::bail!("Bybit API error code {ret_code}: {ret_msg}");
            }
        }

        Ok(body)
    }

    /// Make an authenticated POST request to Bybit V5 REST API.
    async fn signed_post(&self, endpoint: &str, body_val: &serde_json::Value) -> Result<serde_json::Value> {
        let timestamp = Self::timestamp_ms().to_string();
        let recv_window = "5000";
        let body_str = serde_json::to_string(body_val)?;

        let signature = self.sign(&timestamp, recv_window, &body_str);
        let url = format!("{}/v5/{endpoint}", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("X-BAPI-API-KEY", &self.config.api_key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-SIGN", &signature)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .context("Failed to send Bybit POST request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Bybit response")?;

        if !status.is_success() {
            anyhow::bail!("Bybit API error ({}): {}", status, body);
        }

        if let Some(ret_code) = body.get("retCode").and_then(|r| r.as_i64()) {
            if ret_code != 0 {
                let ret_msg = body.get("retMsg").and_then(|m| m.as_str()).unwrap_or("Unknown");
                anyhow::bail!("Bybit API error code {ret_code}: {ret_msg}");
            }
        }

        Ok(body)
    }

    /// Convert timeframe enum to Bybit interval string.
    fn timeframe_to_interval(tf: &Timeframe) -> &'static str {
        match tf {
            Timeframe::Min1 => "1",
            Timeframe::Min5 => "5",
            Timeframe::Min15 => "15",
            Timeframe::Min30 => "30",
            Timeframe::Hour1 => "60",
            Timeframe::Hour4 => "240",
            Timeframe::Day1 => "D",
            Timeframe::Week1 => "W",
        }
    }

    /// Spawn a WebSocket subscription task.
    async fn spawn_ws_stream(&self, topic: &str) -> Result<()> {
        let ws_url = self.config.ws_url.clone();
        let topic_string = topic.to_string();
        let sender = self.event_sender.clone();

        let handle = tokio::spawn(async move {
            loop {
                match connect_async(&ws_url).await {
                    Ok((mut ws_stream, _)) => {
                        info!("Bybit WebSocket stream connected: {}", topic_string);
                        let sub_msg = serde_json::json!({
                            "op": "subscribe",
                            "args": [topic_string.clone()]
                        });
                        if let Err(e) = ws_stream.send(Message::Text(sub_msg.to_string())).await {
                            error!("Failed to subscribe to Bybit WS topic {topic_string}: {e}");
                            break;
                        }

                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if let Some(topic_name) = val.get("topic").and_then(|v| v.as_str()) {
                                            if topic_name.starts_with("tickers.") {
                                                if let Ok(ticker) = Self::parse_ws_ticker(&val) {
                                                    let _ = sender.send(MarketEvent::TickerUpdate(ticker));
                                                }
                                            } else if topic_name.starts_with("kline.") {
                                                if let Ok(candle) = Self::parse_ws_kline(&val) {
                                                    let _ = sender.send(MarketEvent::CandleUpdate(candle));
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(Message::Ping(p)) => {
                                    // Bybit ping/pong
                                    let _ = p;
                                }
                                Ok(Message::Close(_)) => {
                                    warn!("Bybit WebSocket closed, reconnecting in 5s...");
                                    break;
                                }
                                Err(e) => {
                                    warn!("Bybit WebSocket error: {e}, reconnecting in 5s...");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to connect to Bybit WebSocket ({topic_string}): {e}");
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        self.ws_handles.lock().await.push(handle);
        Ok(())
    }

    fn parse_ws_ticker(val: &serde_json::Value) -> Result<Ticker> {
        let data = &val["data"];
        Ok(Ticker {
            symbol: data["symbol"].as_str().unwrap_or("").to_string(),
            exchange: ExchangeId::Bybit,
            last_price: data["lastPrice"].as_str().unwrap_or("0").parse()?,
            bid: data["bid1Price"].as_str().unwrap_or("0").parse()?,
            ask: data["ask1Price"].as_str().unwrap_or("0").parse()?,
            high_24h: data["highPrice24h"].as_str().unwrap_or("0").parse()?,
            low_24h: data["lowPrice24h"].as_str().unwrap_or("0").parse()?,
            volume_24h: data["volume24h"].as_str().unwrap_or("0").parse()?,
            change_pct_24h: data["price24hPcnt"]
                .as_str()
                .and_then(|p| p.parse::<f64>().ok())
                .map(|p| p * 100.0)
                .unwrap_or(0.0),
            timestamp: Utc::now(),
        })
    }

    fn parse_ws_kline(val: &serde_json::Value) -> Result<Candle> {
        let topic = val["topic"].as_str().unwrap_or("");
        let parts: Vec<&str> = topic.split('.').collect();
        let interval = parts.get(1).unwrap_or(&"1");
        let symbol = parts.get(2).unwrap_or(&"");

        let k = &val["data"][0];
        Ok(Candle {
            symbol: symbol.to_string(),
            exchange: ExchangeId::Bybit,
            timeframe: interval.to_string(),
            open_time: DateTime::from_timestamp(k["start"].as_i64().unwrap_or(0) / 1000, 0)
                .unwrap_or_else(Utc::now),
            close_time: DateTime::from_timestamp(k["end"].as_i64().unwrap_or(0) / 1000, 0)
                .unwrap_or_else(Utc::now),
            open: k["open"].as_str().unwrap_or("0").parse()?,
            high: k["high"].as_str().unwrap_or("0").parse()?,
            low: k["low"].as_str().unwrap_or("0").parse()?,
            close: k["close"].as_str().unwrap_or("0").parse()?,
            volume: k["volume"].as_str().unwrap_or("0").parse()?,
        })
    }
}

#[async_trait]
impl Exchange for BybitExchange {
    fn id(&self) -> ExchangeId {
        ExchangeId::Bybit
    }

    fn market_type(&self) -> MarketType {
        MarketType::Crypto
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.api_key.trim().is_empty() || self.config.secret_key.trim().is_empty() {
            *self.connected.write().await = false;
            anyhow::bail!("Bybit credentials not configured (API key or secret missing)");
        }

        let mut params = HashMap::new();
        params.insert("accountType".to_string(), "UNIFIED".to_string());
        self.signed_get("account/wallet-balance", &params).await?;
        info!("Bybit account authenticated and connected.");

        *self.connected.write().await = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.write().await = false;
        let mut handles = self.ws_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }
        info!("Bybit disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn subscribe_orderbook(&self, symbol: &str) -> Result<()> {
        let topic = format!("orderbook.50.{}", symbol.to_uppercase());
        self.spawn_ws_stream(&topic).await?;
        info!("Subscribed to Bybit orderbook: {symbol}");
        Ok(())
    }

    async fn subscribe_trades(&self, symbol: &str) -> Result<()> {
        let topic = format!("publicTrade.{}", symbol.to_uppercase());
        self.spawn_ws_stream(&topic).await?;
        info!("Subscribed to Bybit trades: {symbol}");
        Ok(())
    }

    async fn subscribe_klines(&self, symbol: &str, timeframe: &Timeframe) -> Result<()> {
        let interval = Self::timeframe_to_interval(timeframe);
        let topic = format!("kline.{interval}.{}", symbol.to_uppercase());
        self.spawn_ws_stream(&topic).await?;
        info!("Subscribed to Bybit klines: {symbol} {timeframe}");
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
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), symbol.to_uppercase());
        params.insert("interval".to_string(), interval.to_string());
        params.insert("limit".to_string(), limit.to_string());

        let result = self.public_get("market/kline", &params).await?;

        let list = result["result"]["list"]
            .as_array()
            .context("Expected result.list array from Bybit klines")?;

        let mut candles: Vec<Candle> = list
            .iter()
            .filter_map(|k| {
                let arr = k.as_array()?;
                let start_ms: i64 = arr[0].as_str()?.parse().ok()?;
                let open_time = DateTime::from_timestamp(start_ms / 1000, 0)?;
                Some(Candle {
                    symbol: symbol.to_string(),
                    exchange: ExchangeId::Bybit,
                    timeframe: timeframe.to_string(),
                    open_time,
                    close_time: open_time, // Bybit kline has start time
                    open: arr[1].as_str()?.parse().ok()?,
                    high: arr[2].as_str()?.parse().ok()?,
                    low: arr[3].as_str()?.parse().ok()?,
                    close: arr[4].as_str()?.parse().ok()?,
                    volume: arr[5].as_str()?.parse().ok()?,
                })
            })
            .collect();

        // Bybit returns newest first, reverse so oldest is first
        candles.reverse();

        debug!("Fetched {} candles from Bybit for {symbol}", candles.len());
        Ok(candles)
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), symbol.to_uppercase());

        let result = self.public_get("market/tickers", &params).await?;
        let item = &result["result"]["list"][0];

        let change_pct: f64 = item["price24hPcnt"]
            .as_str()
            .and_then(|p| p.parse::<f64>().ok())
            .map(|p| p * 100.0)
            .unwrap_or(0.0);

        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: ExchangeId::Bybit,
            last_price: item["lastPrice"].as_str().unwrap_or("0").parse()?,
            bid: item["bid1Price"].as_str().unwrap_or("0").parse()?,
            ask: item["ask1Price"].as_str().unwrap_or("0").parse()?,
            high_24h: item["highPrice24h"].as_str().unwrap_or("0").parse()?,
            low_24h: item["lowPrice24h"].as_str().unwrap_or("0").parse()?,
            volume_24h: item["volume24h"].as_str().unwrap_or("0").parse()?,
            change_pct_24h: change_pct,
            timestamp: Utc::now(),
        })
    }

    async fn place_order(&self, order: &Order) -> Result<OrderResult> {
        let side = match order.side {
            OrderSide::Buy => "Buy",
            OrderSide::Sell => "Sell",
        };
        let order_type = match order.order_type {
            OrderType::Market => "Market",
            OrderType::Limit => "Limit",
            _ => "Market",
        };

        let mut body = serde_json::json!({
            "category": "spot",
            "symbol": order.symbol.to_uppercase(),
            "side": side,
            "orderType": order_type,
            "qty": order.quantity.to_string(),
        });

        if let Some(price) = order.price {
            body["price"] = serde_json::Value::String(price.to_string());
        }

        let result = self.signed_post("order/create", &body).await?;
        let order_id = result["result"]["orderId"].as_str().unwrap_or("").to_string();

        info!(
            "Bybit order placed: {} {} {} @ {}",
            order.symbol,
            side,
            order.quantity,
            order.price.map(|p| p.to_string()).unwrap_or("MARKET".to_string()),
        );

        Ok(OrderResult {
            exchange_order_id: order_id,
            status: OrderStatus::Open,
            filled_quantity: Decimal::ZERO,
            average_fill_price: None,
            message: None,
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()> {
        let body = serde_json::json!({
            "category": "spot",
            "symbol": symbol.to_uppercase(),
            "orderId": order_id,
        });

        self.signed_post("order/cancel", &body).await?;
        info!("Bybit order cancelled: {order_id}");
        Ok(())
    }

    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), symbol.to_uppercase());

        let result = self.signed_get("order/realtime", &params).await?;
        let orders: Vec<Order> = result["result"]["list"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|o| {
                Some(Order {
                    id: uuid::Uuid::new_v4(),
                    exchange_order_id: o["orderId"].as_str().map(|id| id.to_string()),
                    exchange: ExchangeId::Bybit,
                    symbol: o["symbol"].as_str()?.to_string(),
                    side: match o["side"].as_str()? {
                        "Buy" => OrderSide::Buy,
                        _ => OrderSide::Sell,
                    },
                    order_type: match o["orderType"].as_str()? {
                        "Limit" => OrderType::Limit,
                        _ => OrderType::Market,
                    },
                    quantity: o["qty"].as_str()?.parse().ok()?,
                    price: o["price"].as_str().and_then(|p| p.parse().ok()),
                    stop_price: None,
                    status: OrderStatus::Open,
                    filled_quantity: o["cumExecQty"].as_str()?.parse().unwrap_or(Decimal::ZERO),
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
        params.insert("accountType".to_string(), "UNIFIED".to_string());

        let result = self.signed_get("account/wallet-balance", &params).await?;
        let mut assets: Vec<AssetBalance> = Vec::new();

        if let Some(list) = result["result"]["list"].as_array() {
            for item in list {
                if let Some(coins) = item["coin"].as_array() {
                    for c in coins {
                        let asset = c["coin"].as_str().unwrap_or("").to_string();
                        let free: Decimal = c["walletBalance"].as_str().and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
                        let locked: Decimal = c["locked"].as_str().and_then(|s| s.parse().ok()).unwrap_or(Decimal::ZERO);
                        if !free.is_zero() || !locked.is_zero() {
                            assets.push(AssetBalance {
                                asset,
                                free,
                                locked,
                            });
                        }
                    }
                }
            }
        }

        Ok(Balance {
            exchange: ExchangeId::Bybit,
            assets,
            total_usd: None,
        })
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        let balance = self.get_balance().await?;

        Ok(AccountInfo {
            exchange: ExchangeId::Bybit,
            account_type: "UNIFIED".to_string(),
            can_trade: true,
            can_withdraw: true,
            balance,
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        Ok(vec![])
    }
}
