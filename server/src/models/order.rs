use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ExchangeId;

/// Order side (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    StopLimit,
    TakeProfit,
    TrailingStop,
}

/// Order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// A trading order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub exchange_order_id: Option<String>,
    pub exchange: ExchangeId,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub average_fill_price: Option<Decimal>,
    pub fees: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The signal ID that triggered this order (if AI-generated).
    pub signal_id: Option<Uuid>,
}

impl Order {
    /// Create a new market order.
    pub fn market(exchange: ExchangeId, symbol: &str, side: OrderSide, quantity: Decimal) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            exchange_order_id: None,
            exchange,
            symbol: symbol.to_string(),
            side,
            order_type: OrderType::Market,
            quantity,
            price: None,
            stop_price: None,
            status: OrderStatus::Pending,
            filled_quantity: Decimal::ZERO,
            average_fill_price: None,
            fees: Decimal::ZERO,
            created_at: now,
            updated_at: now,
            signal_id: None,
        }
    }

    /// Create a new limit order.
    pub fn limit(
        exchange: ExchangeId,
        symbol: &str,
        side: OrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            exchange_order_id: None,
            exchange,
            symbol: symbol.to_string(),
            side,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            stop_price: None,
            status: OrderStatus::Pending,
            filled_quantity: Decimal::ZERO,
            average_fill_price: None,
            fees: Decimal::ZERO,
            created_at: now,
            updated_at: now,
            signal_id: None,
        }
    }

    /// Check if order is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected | OrderStatus::Expired
        )
    }
}

/// Result of placing an order on the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResult {
    pub exchange_order_id: String,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub average_fill_price: Option<Decimal>,
    pub message: Option<String>,
}

/// An open position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: Uuid,
    pub exchange: ExchangeId,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    pub unrealized_pnl: Decimal,
    pub unrealized_pnl_pct: f64,
    pub realized_pnl: Decimal,
    pub fees_paid: Decimal,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub signal_id: Option<Uuid>,
}

impl Position {
    /// Calculate unrealized P&L based on current price.
    pub fn calculate_pnl(&mut self) {
        let price_diff = match self.side {
            OrderSide::Buy => self.current_price - self.entry_price,
            OrderSide::Sell => self.entry_price - self.current_price,
        };
        self.unrealized_pnl = price_diff * self.quantity - self.fees_paid;

        let entry_f64 = self.entry_price.to_string().parse::<f64>().unwrap_or(1.0);
        let pnl_f64 = self.unrealized_pnl.to_string().parse::<f64>().unwrap_or(0.0);
        let cost = entry_f64 * self.quantity.to_string().parse::<f64>().unwrap_or(1.0);
        self.unrealized_pnl_pct = if cost > 0.0 { (pnl_f64 / cost) * 100.0 } else { 0.0 };
    }

    /// Check if stop loss has been hit.
    pub fn is_stop_loss_hit(&self) -> bool {
        if let Some(sl) = self.stop_loss {
            match self.side {
                OrderSide::Buy => self.current_price <= sl,
                OrderSide::Sell => self.current_price >= sl,
            }
        } else {
            false
        }
    }

    /// Check if take profit has been hit.
    pub fn is_take_profit_hit(&self) -> bool {
        if let Some(tp) = self.take_profit {
            match self.side {
                OrderSide::Buy => self.current_price >= tp,
                OrderSide::Sell => self.current_price <= tp,
            }
        } else {
            false
        }
    }
}
