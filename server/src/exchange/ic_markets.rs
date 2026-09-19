use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::Exchange;
use crate::config::IcMarketsConfig;
use crate::models::*;

/// IC Markets forex connector via cTrader Open API (REST).
/// Supports forex pairs, metals, indices and CFDs.
pub struct IcMarketsExchange {
    config: IcMarketsConfig,
    http_client: Client,
    connected: Arc<RwLock<bool>>,
    access_token: Arc<RwLock<Option<String>>>,
}

/// cTrader account response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CTraderAccount {
    account_id: Option<i64>,
    balance: Option<f64>,
    equity: Option<f64>,
    currency: Option<String>,
    is_live: Option<bool>,
}

/// cTrader position response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CTraderPosition {
    position_id: Option<i64>,
    symbol_name: Option<String>,
    trade_side: Option<String>,
    volume: Option<f64>,
    entry_price: Option<f64>,
    current_price: Option<f64>,
    swap: Option<f64>,
    profit: Option<f64>,
    #[serde(default)]
    stop_loss: Option<f64>,
    #[serde(default)]
    take_profit: Option<f64>,
}

/// cTrader order response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CTraderOrder {
    order_id: Option<i64>,
    symbol_name: Option<String>,
    trade_side: Option<String>,
    order_type: Option<String>,
    volume: Option<f64>,
    execution_price: Option<f64>,
    status: Option<String>,
}

/// cTrader trendbar (candle) response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CTraderTrendbar {
    timestamp: Option<i64>,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    close: Option<f64>,
    volume: Option<f64>,
}

impl IcMarketsExchange {
    pub fn new(config: IcMarketsConfig) -> Self {
        Self {
            config,
            http_client: Client::new(),
            connected: Arc::new(RwLock::new(false)),
            access_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Authenticate via OAuth2 to get access token.
    async fn authenticate(&self) -> Result<String> {
        let token_url = format!("{}/connect/token", self.config.base_url);

        let response = self
            .http_client
            .post(&token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
            ])
            .send()
            .await
            .context("Failed to authenticate with cTrader")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse cTrader auth response")?;

        if !status.is_success() {
            anyhow::bail!("cTrader auth error ({}): {}", status, body);
        }

        let token = body["access_token"]
            .as_str()
            .context("Missing access_token in auth response")?
            .to_string();

        info!("cTrader OAuth2 authentication successful");
        Ok(token)
    }

    /// Get the current access token, refreshing if necessary.
    async fn get_token(&self) -> Result<String> {
        let token = self.access_token.read().await;
        if let Some(ref t) = *token {
            return Ok(t.clone());
        }
        drop(token);

        let new_token = self.authenticate().await?;
        *self.access_token.write().await = Some(new_token.clone());
        Ok(new_token)
    }

    /// Make an authenticated GET request to the cTrader Open API.
    async fn api_get(&self, endpoint: &str) -> Result<serde_json::Value> {
        let token = self.get_token().await?;
        let url = format!("{}/{}", self.config.base_url, endpoint);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to send cTrader GET request")?;

        let status = response.status();

        // Handle token expiry
        if status.as_u16() == 401 {
            warn!("cTrader token expired, re-authenticating...");
            *self.access_token.write().await = None;
            let new_token = self.get_token().await?;

            let retry = self
                .http_client
                .get(&url)
                .header("Authorization", format!("Bearer {}", new_token))
                .header("Accept", "application/json")
                .send()
                .await?;

            let body: serde_json::Value = retry.json().await?;
            return Ok(body);
        }

        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse cTrader response")?;

        if !status.is_success() {
            anyhow::bail!("cTrader API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated POST request to the cTrader Open API.
    async fn api_post(
        &self,
        endpoint: &str,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let token = self.get_token().await?;
        let url = format!("{}/{}", self.config.base_url, endpoint);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .context("Failed to send cTrader POST request")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse cTrader response")?;

        if !status.is_success() {
            anyhow::bail!("cTrader API error ({}): {}", status, body);
        }

        Ok(body)
    }

    /// Make an authenticated DELETE request.
    async fn api_delete(&self, endpoint: &str) -> Result<()> {
        let token = self.get_token().await?;
        let url = format!("{}/{}", self.config.base_url, endpoint);

        let response = self
            .http_client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to send cTrader DELETE request")?;

        if !response.status().is_success() {
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            anyhow::bail!("cTrader DELETE error: {}", body);
        }

        Ok(())
    }

    /// Convert our Timeframe to cTrader period type.
    fn timeframe_to_ctrader(tf: &Timeframe) -> &'static str {
        match tf {
            Timeframe::Min1 => "M1",
            Timeframe::Min5 => "M5",
            Timeframe::Min15 => "M15",
            Timeframe::Min30 => "M30",
            Timeframe::Hour1 => "H1",
            Timeframe::Hour4 => "H4",
            Timeframe::Day1 => "D1",
            Timeframe::Week1 => "W1",
        }
    }

    /// Helper to safely parse optional f64 to Decimal.
    fn to_decimal(val: Option<f64>) -> Decimal {
        val.and_then(|v| Decimal::try_from(v).ok())
            .unwrap_or(Decimal::ZERO)
    }
}

#[async_trait]
impl Exchange for IcMarketsExchange {
    fn id(&self) -> ExchangeId {
        ExchangeId::IcMarkets
    }

    fn market_type(&self) -> MarketType {
        MarketType::Forex
    }

    async fn connect(&mut self) -> Result<()> {
        // Authenticate and verify account
        let token = self.authenticate().await?;
        *self.access_token.write().await = Some(token);

        // Fetch account info to verify
        let endpoint = format!("v2/trading/accounts/{}", self.config.account_id);
        let body = self.api_get(&endpoint).await?;

        let account: CTraderAccount =
            serde_json::from_value(body).context("Failed to parse cTrader account")?;

        info!(
            "IC Markets connected: account={:?}, balance={:?}, currency={:?}, live={:?}",
            account.account_id, account.balance, account.currency, account.is_live
        );

        *self.connected.write().await = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.connected.write().await = false;
        *self.access_token.write().await = None;
        info!("IC Markets disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    async fn subscribe_orderbook(&self, _symbol: &str) -> Result<()> {
        info!("IC Markets orderbook subscription requires FIX protocol — not implemented in REST mode");
        Ok(())
    }

    async fn subscribe_trades(&self, _symbol: &str) -> Result<()> {
        info!("IC Markets trade stream requires FIX protocol — not implemented in REST mode");
        Ok(())
    }

    async fn subscribe_klines(&self, _symbol: &str, _timeframe: &Timeframe) -> Result<()> {
        info!("IC Markets kline stream requires FIX protocol — polling via REST instead");
        Ok(())
    }

    async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &Timeframe,
        limit: usize,
    ) -> Result<Vec<Candle>> {
        let period = Self::timeframe_to_ctrader(timeframe);
        let endpoint = format!(
            "v2/trendbars?symbolName={}&period={}&count={}",
            symbol, period, limit
        );

        let body = self.api_get(&endpoint).await?;

        let bars: Vec<CTraderTrendbar> = body
            .get("data")
            .and_then(|d| serde_json::from_value(d.clone()).ok())
            .unwrap_or_default();

        let candles = bars
            .iter()
            .filter_map(|bar| {
                let open_time = DateTime::from_timestamp(bar.timestamp? / 1000, 0)?;
                let close_time =
                    open_time + chrono::Duration::seconds(timeframe.as_secs() as i64);

                Some(Candle {
                    symbol: symbol.to_string(),
                    exchange: ExchangeId::IcMarkets,
                    timeframe: timeframe.to_string(),
                    open_time,
                    close_time,
                    open: Self::to_decimal(bar.open),
                    high: Self::to_decimal(bar.high),
                    low: Self::to_decimal(bar.low),
                    close: Self::to_decimal(bar.close),
                    volume: Self::to_decimal(bar.volume),
                })
            })
            .collect();

        debug!("Fetched {} candles for {} from IC Markets", bars.len(), symbol);
        Ok(candles)
    }

    async fn get_ticker(&self, symbol: &str) -> Result<Ticker> {
        let endpoint = format!("v2/symbols/{}/tick", symbol);
        let body = self.api_get(&endpoint).await?;

        let bid = body
            .get("bid")
            .and_then(|v| v.as_f64())
            .and_then(|v| Decimal::try_from(v).ok())
            .unwrap_or(Decimal::ZERO);

        let ask = body
            .get("ask")
            .and_then(|v| v.as_f64())
            .and_then(|v| Decimal::try_from(v).ok())
            .unwrap_or(Decimal::ZERO);

        let last_price = (bid + ask) / Decimal::new(2, 0);

        Ok(Ticker {
            symbol: symbol.to_string(),
            exchange: ExchangeId::IcMarkets,
            last_price,
            bid,
            ask,
            high_24h: Decimal::ZERO,
            low_24h: Decimal::ZERO,
            volume_24h: Decimal::ZERO,
            change_pct_24h: 0.0,
            timestamp: Utc::now(),
        })
    }

    async fn place_order(&self, order: &Order) -> Result<OrderResult> {
        let side = match order.side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };

        let order_type = match order.order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopLoss => "STOP",
            OrderType::StopLimit => "STOP_LIMIT",
            _ => "MARKET",
        };

        let mut payload = serde_json::json!({
            "accountId": self.config.account_id,
            "symbolName": order.symbol,
            "tradeSide": side,
            "orderType": order_type,
            "volume": order.quantity.to_string(),
        });

        if let Some(price) = order.price {
            payload["price"] = serde_json::Value::String(price.to_string());
        }

        if let Some(stop) = order.stop_price {
            payload["stopPrice"] = serde_json::Value::String(stop.to_string());
        }

        let result = self.api_post("v2/trading/orders", &payload).await?;

        let ctrader_order: CTraderOrder =
            serde_json::from_value(result).context("Failed to parse cTrader order response")?;

        info!(
            "IC Markets order placed: {:?} {:?} {:?} (status: {:?})",
            ctrader_order.symbol_name, side, order.quantity, ctrader_order.status
        );

        let status = match ctrader_order.status.as_deref() {
            Some("FILLED") => OrderStatus::Filled,
            Some("PARTIALLY_FILLED") => OrderStatus::PartiallyFilled,
            Some("CANCELLED") => OrderStatus::Cancelled,
            Some("REJECTED") => OrderStatus::Rejected,
            _ => OrderStatus::Open,
        };

        Ok(OrderResult {
            exchange_order_id: ctrader_order
                .order_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            status,
            filled_quantity: Self::to_decimal(ctrader_order.volume),
            average_fill_price: ctrader_order
                .execution_price
                .and_then(|p| Decimal::try_from(p).ok()),
            message: None,
        })
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> Result<()> {
        let endpoint = format!(
            "v2/trading/orders/{}?accountId={}",
            order_id, self.config.account_id
        );
        self.api_delete(&endpoint).await?;
        info!("IC Markets order cancelled: {order_id}");
        Ok(())
    }

    async fn get_open_orders(&self, _symbol: &str) -> Result<Vec<Order>> {
        let endpoint = format!(
            "v2/trading/orders?accountId={}&status=ACTIVE",
            self.config.account_id
        );
        let body = self.api_get(&endpoint).await?;

        let ctrader_orders: Vec<CTraderOrder> = body
            .get("data")
            .and_then(|d| serde_json::from_value(d.clone()).ok())
            .unwrap_or_default();

        let orders = ctrader_orders
            .iter()
            .map(|o| {
                let side = match o.trade_side.as_deref() {
                    Some("BUY") => OrderSide::Buy,
                    _ => OrderSide::Sell,
                };

                let order_type = match o.order_type.as_deref() {
                    Some("LIMIT") => OrderType::Limit,
                    Some("STOP") => OrderType::StopLoss,
                    Some("STOP_LIMIT") => OrderType::StopLimit,
                    _ => OrderType::Market,
                };

                Order {
                    id: uuid::Uuid::new_v4(),
                    exchange_order_id: o.order_id.map(|id| id.to_string()),
                    exchange: ExchangeId::IcMarkets,
                    symbol: o.symbol_name.clone().unwrap_or_default(),
                    side,
                    order_type,
                    quantity: Self::to_decimal(o.volume),
                    price: o.execution_price.and_then(|p| Decimal::try_from(p).ok()),
                    stop_price: None,
                    status: OrderStatus::Open,
                    filled_quantity: Decimal::ZERO,
                    average_fill_price: None,
                    fees: Decimal::ZERO,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    signal_id: None,
                }
            })
            .collect();

        Ok(orders)
    }

    async fn get_balance(&self) -> Result<Balance> {
        let endpoint = format!("v2/trading/accounts/{}", self.config.account_id);
        let body = self.api_get(&endpoint).await?;

        let account: CTraderAccount =
            serde_json::from_value(body).context("Failed to parse cTrader account")?;

        let balance_val = Decimal::try_from(account.balance.unwrap_or(0.0))
            .unwrap_or(Decimal::ZERO);
        let equity_val = Decimal::try_from(account.equity.unwrap_or(0.0))
            .unwrap_or(Decimal::ZERO);
        let currency = account.currency.unwrap_or_else(|| "USD".to_string());

        Ok(Balance {
            exchange: ExchangeId::IcMarkets,
            assets: vec![AssetBalance {
                asset: currency,
                free: balance_val,
                locked: equity_val - balance_val,
            }],
            total_usd: Some(equity_val),
        })
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        let endpoint = format!("v2/trading/accounts/{}", self.config.account_id);
        let body = self.api_get(&endpoint).await?;

        let account: CTraderAccount =
            serde_json::from_value(body).context("Failed to parse cTrader account")?;

        let balance = self.get_balance().await?;
        let is_live = account.is_live.unwrap_or(false);

        Ok(AccountInfo {
            exchange: ExchangeId::IcMarkets,
            account_type: if is_live { "live" } else { "demo" }.to_string(),
            can_trade: true,
            can_withdraw: is_live,
            balance,
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        let endpoint = format!(
            "v2/trading/positions?accountId={}",
            self.config.account_id
        );
        let body = self.api_get(&endpoint).await?;

        let ctrader_positions: Vec<CTraderPosition> = body
            .get("data")
            .and_then(|d| serde_json::from_value(d.clone()).ok())
            .unwrap_or_default();

        let positions = ctrader_positions
            .iter()
            .filter_map(|p| {
                let entry_price = Decimal::try_from(p.entry_price?).ok()?;
                let current_price = Decimal::try_from(p.current_price?).ok()?;
                let quantity = Decimal::try_from(p.volume?).ok()?;
                let unrealized_pnl = Decimal::try_from(p.profit.unwrap_or(0.0)).ok()?;

                let side = match p.trade_side.as_deref() {
                    Some("BUY") => OrderSide::Buy,
                    _ => OrderSide::Sell,
                };

                let entry_f64 = entry_price.to_string().parse::<f64>().unwrap_or(1.0);
                let pnl_f64 = unrealized_pnl.to_string().parse::<f64>().unwrap_or(0.0);
                let cost = entry_f64 * quantity.to_string().parse::<f64>().unwrap_or(1.0);
                let pnl_pct = if cost > 0.0 {
                    (pnl_f64 / cost) * 100.0
                } else {
                    0.0
                };

                Some(Position {
                    id: uuid::Uuid::new_v4(),
                    exchange: ExchangeId::IcMarkets,
                    symbol: p.symbol_name.clone().unwrap_or_default(),
                    side,
                    quantity,
                    entry_price,
                    current_price,
                    stop_loss: p.stop_loss.and_then(|v| Decimal::try_from(v).ok()),
                    take_profit: p.take_profit.and_then(|v| Decimal::try_from(v).ok()),
                    unrealized_pnl,
                    unrealized_pnl_pct: pnl_pct,
                    realized_pnl: Decimal::ZERO,
                    fees_paid: Decimal::try_from(p.swap.unwrap_or(0.0))
                        .unwrap_or(Decimal::ZERO),
                    opened_at: Utc::now(),
                    updated_at: Utc::now(),
                    signal_id: None,
                })
            })
            .collect();

        Ok(positions)
    }
}
