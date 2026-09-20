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
use crate::config::BinanceConfig;
use crate::models::*;

type HmacSha256 = Hmac<Sha256>;

/// Binance exchange connector for cryptocurrency trading.
/// Supports spot trading with WebSocket market data and REST API v3 order management.
pub struct BinanceExchange {
    config: BinanceConfig,
    http_client: Client,
    connected: Arc<RwLock<bool>>,
    event_sender: broadcast::Sender<MarketEvent>,
    /// Active WebSocket subscription handles.
    ws_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl BinanceExchange {
    pub fn new(config: BinanceConfig) -> Self {
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

    /// Generate HMAC-SHA256 signature for Binance API requests.
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

    /// Make an unauthenticated GET request to Binance REST API.
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
            .context("Failed to send Binance public request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Binance response")?;

        if !status.is_success() {
            anyhow::bail!("Binance API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated GET request to Binance REST API.
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
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await
            .context("Failed to send Binance GET request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Binance response")?;

        if !status.is_success() {
            anyhow::bail!("Binance API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated POST request to Binance REST API.
    async fn signed_post(&self, endpoint: &str, params: &mut HashMap<String, String>) -> Result<serde_json::Value> {
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
            .post(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await
            .context("Failed to send Binance POST request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Binance response")?;

        if !status.is_success() {
            anyhow::bail!("Binance API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated DELETE request to Binance REST API.
    async fn signed_delete(&self, endpoint: &str, params: &mut HashMap<String, String>) -> Result<serde_json::Value> {
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
            .delete(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await
            .context("Failed to send Binance DELETE request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Binance response")?;

        if !status.is_success() {
            anyhow::bail!("Binance API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Convert timeframe enum to Binance interval string.
    fn timeframe_to_interval(tf: &Timeframe) -> &'static str {
        match tf {
            Timeframe::Min1 => "1m",
            Timeframe::Min5 => "5m",
            Timeframe::Min15 => "15m",
            Timeframe::Min30 => "30m",
            Timeframe::Hour1 => "1h",
            Timeframe::Hour4 => "4h",
            Timeframe::Day1 => "1d",
            Timeframe::Week1 => "1w",
        }
    }

    /// Spawn a WebSocket subscription task.
    async fn spawn_ws_stream(&self, stream_param: &str) -> Result<()> {
        let stream_param = stream_param.to_string();
        let ws_url = format!("{}/{}", self.config.ws_url, stream_param);
        let sender = self.event_sender.clone();

        let handle = tokio::spawn(async move {
            loop {
                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        info!("Binance WebSocket stream connected: {}", stream_param);
                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if let Some(e) = val.get("e").and_then(|v| v.as_str()) {
                                            match e {
                                                "24hrTicker" => {
                                                    if let Ok(ticker) = Self::parse_ws_ticker(&val) {
                                                        let _ = sender.send(MarketEvent::TickerUpdate(ticker));
                                                    }
                                                }
                                                "kline" => {
                                                    if let Ok(candle) = Self::parse_ws_kline(&val) {
                                                        let _ = sender.send(MarketEvent::CandleUpdate(candle));
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                Ok(Message::Ping(_)) => {}
                                Ok(Message::Close(_)) => {
                                    warn!("Binance WebSocket closed, reconnecting in 5s...");
                                    break;
                                }
                                Err(e) => {
                                    warn!("Binance WebSocket error: {e}, reconnecting in 5s...");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to connect to Binance WebSocket ({stream_param}): {e}");
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        self.ws_handles.lock().await.push(handle);
        Ok(())
    }

    fn parse_ws_ticker(val: &serde_json::Value) -> Result<Ticker> {
        Ok(Ticker {
            symbol: val["s"].as_str().unwrap_or("").to_string(),
            exchange: ExchangeId::Binance,
            last_price: val["c"].as_str().unwrap_or("0").parse()?,
            bid: val["b"].as_str().unwrap_or("0").parse()?,
            ask: val["a"].as_str().unwrap_or("0").parse()?,
            high_24h: val["h"].as_str().unwrap_or("0").parse()?,
            low_24h: val["l"].as_str().unwrap_or("0").parse()?,
            volume_24h: val["v"].as_str().unwrap_or("0").parse()?,
            change_pct_24h: val["P"].as_str().unwrap_or("0").parse()?,
            timestamp: Utc::now(),
        })
    }

    fn parse_ws_kline(val: &serde_json::Value) -> Result<Candle> {
        let k = &val["k"];
        Ok(Candle {
            symbol: val["s"].as_str().unwrap_or("").to_string(),
            exchange: ExchangeId::Binance,
            timeframe: k["i"].as_str().unwrap_or("1m").to_string(),
            open_time: DateTime::from_timestamp(k["t"].as_i64().unwrap_or(0) / 1000, 0)
                .unwrap_or_else(Utc::now),
            close_time: DateTime::from_timestamp(k["T"].as_i64().unwrap_or(0) / 1000, 0)
                .unwrap_or_else(Utc::now),
            open: k["o"].as_str().unwrap_or("0").parse()?,
            high: k["h"].as_str().unwrap_or("0").parse()?,
            low: k["l"].as_str().unwrap_or("0").parse()?,
            close: k["c"].as_str().unwrap_or("0").parse()?,
            volume: k["v"].as_str().unwrap_or("0").parse()?,
        })
    }
}

#[async_trait]
impl Exchange for BinanceExchange {
    fn id(&self) -> ExchangeId {
        ExchangeId::Binance
    }

    fn market_type(&self) -> MarketType {
        MarketType::Crypto
    }

    async fn connect(&mut self) -> Result<()> {
        if self.config.api_key.trim().is_empty() || self.config.secret_key.trim().is_empty() {
            *self.connected.write().await = false;
            anyhow::bail!("Binance credentials not configured (API key or secret missing)");
        }

        let mut params = HashMap::new();
        self.signed_get("account", &mut params).await?;
        info!("Binance account authenticated and connected.");

        *self.connected.write().await = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.write().await = false;
        let mut handles = self.ws_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }
        info!("Binance disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn subscribe_orderbook(&self, symbol: &str) -> Result<()> {
        let stream = format!("{}@depth20@100ms", symbol.to_lowercase());
        self.spawn_ws_stream(&stream).await?;
        info!("Subscribed to Binance orderbook: {symbol}");
        Ok(())
    }

    async fn subscribe_trades(&self, symbol: &str) -> Result<()> {
        let stream = format!("{}@trade", symbol.to_lowercase());
        self.spawn_ws_stream(&stream).await?;
        info!("Subscribed to Binance trades: {symbol}");
        Ok(())
    }

    async fn subscribe_klines(&self, symbol: &str, timeframe: &Timeframe) -> Result<()> {
        let interval = Self::timeframe_to_interval(timeframe);
        let stream = format!("{}@kline_{interval}", symbol.to_lowercase());
        self.spawn_ws_stream(&stream).await?;
        info!("Subscribed to Binance klines: {symbol} {timeframe}");
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
        params.insert("symbol".to_string(), symbol.to_uppercase());
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
                    exchange: ExchangeId::Binance,
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

        debug!("Fetched {} candles from Binance for {symbol}", candles.len());
        Ok(candles)
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_uppercase());

        let result = self.public_get("ticker/24hr", &params).await?;

        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: ExchangeId::Binance,
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
        params.insert("symbol".to_string(), order.symbol.to_uppercase());
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
            params.insert("timeInForce".to_string(), "GTC".to_string());
        }

        let result = self.signed_post("order", &mut params).await?;

        info!(
            "Binance order placed: {} {} {} @ {}",
            order.symbol,
            match order.side {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            },
            order.quantity,
            order.price.map(|p| p.to_string()).unwrap_or("MARKET".to_string()),
        );

        Ok(OrderResult {
            exchange_order_id: result["orderId"]
                .as_i64()
                .map(|id| id.to_string())
                .unwrap_or_default(),
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
            average_fill_price: result["cummulativeQuoteQty"]
                .as_str()
                .and_then(|q| q.parse::<Decimal>().ok())
                .and_then(|quote_qty| {
                    let exec_qty = result["executedQty"].as_str()?.parse::<Decimal>().ok()?;
                    if exec_qty > Decimal::ZERO {
                        Some(quote_qty / exec_qty)
                    } else {
                        None
                    }
                }),
            message: None,
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()> {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_uppercase());
        params.insert("orderId".to_string(), order_id.to_string());

        self.signed_delete("order", &mut params).await?;
        info!("Binance order cancelled: {order_id}");
        Ok(())
    }

    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>> {
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_uppercase());

        let result = self.signed_get("openOrders", &mut params).await?;
        let orders: Vec<Order> = result
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|o| {
                Some(Order {
                    id: uuid::Uuid::new_v4(),
                    exchange_order_id: o["orderId"].as_i64().map(|id| id.to_string()),
                    exchange: ExchangeId::Binance,
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
            exchange: ExchangeId::Binance,
            assets,
            total_usd: None,
        })
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        let balance = self.get_balance().await?;

        Ok(AccountInfo {
            exchange: ExchangeId::Binance,
            account_type: "SPOT".to_string(),
            can_trade: true,
            can_withdraw: true,
            balance,
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        Ok(vec![])
    }
}
