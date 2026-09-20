use anyhow::Result;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

use crate::models::*;
use super::metrics::CompletedTrade;

/// Simulated exchange for backtesting.
/// Fills orders against historical candle data with configurable fees and slippage.
pub struct SimulatedExchange {
    config: SimConfig,
    balance: Decimal,
    initial_balance: Decimal,
    positions: Vec<SimPosition>,
    closed_trades: Vec<CompletedTrade>,
    equity_curve: Vec<f64>,
    current_bar: usize,
    total_fees: Decimal,
}

/// Configuration for the simulated exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    /// Maker fee as a fraction (e.g., 0.001 = 0.1%)
    pub maker_fee: f64,
    /// Taker fee as a fraction (e.g., 0.001 = 0.1%)
    pub taker_fee: f64,
    /// Slippage as a fraction (e.g., 0.0005 = 0.05%)
    pub slippage: f64,
    /// Whether to allow short selling
    pub allow_short: bool,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            maker_fee: 0.001,  // 0.1%
            taker_fee: 0.001,  // 0.1%
            slippage: 0.0005,  // 0.05%
            allow_short: false,
        }
    }
}

/// A position held during simulation.
#[derive(Debug, Clone)]
pub struct SimPosition {
    pub id: Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    pub entry_bar: usize,
    pub entry_time: chrono::DateTime<chrono::Utc>,
    pub signal_id: Uuid,
    pub reasoning: String,
}

/// Result of attempting to execute a signal in the simulator.
#[derive(Debug)]
pub enum SimFillResult {
    Filled { position_id: Uuid, fill_price: Decimal, fees: Decimal },
    Rejected(String),
}

impl SimulatedExchange {
    pub fn new(initial_balance: Decimal, config: SimConfig) -> Self {
        let balance_f64 = initial_balance.to_f64().unwrap_or(0.0);
        Self {
            config,
            balance: initial_balance,
            initial_balance,
            positions: Vec::new(),
            closed_trades: Vec::new(),
            equity_curve: vec![balance_f64],
            current_bar: 0,
            total_fees: Decimal::ZERO,
        }
    }

    /// Advance the simulation by one bar. Updates positions and checks SL/TP.
    pub fn tick(&mut self, candle: &Candle) {
        self.current_bar += 1;

        // Check stop loss / take profit for all positions
        let mut to_close: Vec<(Uuid, Decimal, String)> = Vec::new();

        for pos in &self.positions {
            // Check stop loss
            if let Some(sl) = pos.stop_loss {
                let hit = match pos.side {
                    OrderSide::Buy => candle.low <= sl,
                    OrderSide::Sell => candle.high >= sl,
                };
                if hit {
                    to_close.push((pos.id, sl, "stop_loss".to_string()));
                    continue;
                }
            }

            // Check take profit
            if let Some(tp) = pos.take_profit {
                let hit = match pos.side {
                    OrderSide::Buy => candle.high >= tp,
                    OrderSide::Sell => candle.low <= tp,
                };
                if hit {
                    to_close.push((pos.id, tp, "take_profit".to_string()));
                }
            }
        }

        // Close triggered positions
        for (id, exit_price, reason) in to_close {
            self.close_position(id, exit_price, candle.close_time, &reason);
        }

        // Update equity curve
        let unrealized = self.unrealized_pnl(candle);
        let equity = self.balance.to_f64().unwrap_or(0.0) + unrealized;
        self.equity_curve.push(equity);
    }

    /// Execute a trading signal against the current candle.
    pub fn execute_signal(
        &mut self,
        signal: &TradingSignal,
        candle: &Candle,
    ) -> SimFillResult {
        let side = match signal.order_side() {
            Some(s) => s,
            None => return SimFillResult::Rejected("Signal is HOLD".to_string()),
        };

        // Don't allow shorts if disabled
        if side == OrderSide::Sell && !self.config.allow_short && self.positions.is_empty() {
            return SimFillResult::Rejected("Short selling disabled".to_string());
        }

        // If it's a sell and we have a long position, close it instead
        if side == OrderSide::Sell {
            let long_positions: Vec<Uuid> = self.positions
                .iter()
                .filter(|p| p.side == OrderSide::Buy && p.symbol == signal.symbol)
                .map(|p| p.id)
                .collect();

            if !long_positions.is_empty() {
                for id in long_positions {
                    let exit_price = self.apply_slippage(candle.close, OrderSide::Sell);
                    self.close_position(id, exit_price, candle.close_time, "sell_signal");
                }
                return SimFillResult::Filled {
                    position_id: Uuid::nil(),
                    fill_price: candle.close,
                    fees: Decimal::ZERO,
                };
            }

            if !self.config.allow_short {
                return SimFillResult::Rejected("No position to sell".to_string());
            }
        }

        // Calculate position size
        let fill_price = self.apply_slippage(candle.close, side);
        let position_size_pct = signal.position_size_pct.unwrap_or(5.0) / 100.0;
        let position_value = self.balance * Decimal::try_from(position_size_pct).unwrap_or(Decimal::new(5, 2));

        if position_value <= Decimal::ZERO || fill_price <= Decimal::ZERO {
            return SimFillResult::Rejected("Insufficient balance".to_string());
        }

        let quantity = position_value / fill_price;

        // Calculate fees
        let fee_rate = Decimal::try_from(self.config.taker_fee).unwrap_or(Decimal::new(1, 3));
        let fees = position_value * fee_rate;

        // Check sufficient balance
        if position_value + fees > self.balance {
            return SimFillResult::Rejected("Insufficient balance after fees".to_string());
        }

        // Deduct from balance
        self.balance -= position_value + fees;
        self.total_fees += fees;

        let position_id = Uuid::new_v4();

        let position = SimPosition {
            id: position_id,
            symbol: signal.symbol.clone(),
            side,
            quantity,
            entry_price: fill_price,
            stop_loss: signal.stop_loss,
            take_profit: signal.take_profit,
            entry_bar: self.current_bar,
            entry_time: candle.open_time,
            signal_id: signal.id,
            reasoning: signal.reasoning.clone(),
        };

        debug!(
            "SIM: Opened {} {} @ {} (qty: {}, fees: {})",
            signal.symbol,
            match side { OrderSide::Buy => "LONG", OrderSide::Sell => "SHORT" },
            fill_price, quantity, fees
        );

        self.positions.push(position);

        SimFillResult::Filled {
            position_id,
            fill_price,
            fees,
        }
    }

    /// Close a position at the given price.
    fn close_position(&mut self, position_id: Uuid, exit_price: Decimal, exit_time: chrono::DateTime<chrono::Utc>, reason: &str) {
        if let Some(idx) = self.positions.iter().position(|p| p.id == position_id) {
            let pos = self.positions.remove(idx);

            let pnl = match pos.side {
                OrderSide::Buy => (exit_price - pos.entry_price) * pos.quantity,
                OrderSide::Sell => (pos.entry_price - exit_price) * pos.quantity,
            };

            // Exit fees
            let exit_value = exit_price * pos.quantity;
            let fee_rate = Decimal::try_from(self.config.taker_fee).unwrap_or(Decimal::new(1, 3));
            let exit_fees = exit_value * fee_rate;
            self.total_fees += exit_fees;

            let net_pnl = pnl - exit_fees;

            let pnl_pct = if pos.entry_price > Decimal::ZERO {
                let diff = match pos.side {
                    OrderSide::Buy => exit_price - pos.entry_price,
                    OrderSide::Sell => pos.entry_price - exit_price,
                };
                (diff / pos.entry_price).to_f64().unwrap_or(0.0) * 100.0
            } else {
                0.0
            };

            // Return capital + PnL to balance
            let returned = pos.entry_price * pos.quantity + net_pnl;
            self.balance += returned;

            let holding_bars = self.current_bar.saturating_sub(pos.entry_bar);

            debug!(
                "SIM: Closed {} @ {} (entry: {}, PnL: {}, reason: {})",
                pos.symbol, exit_price, pos.entry_price, net_pnl, reason
            );

            self.closed_trades.push(CompletedTrade {
                symbol: pos.symbol,
                side: match pos.side {
                    OrderSide::Buy => "BUY".to_string(),
                    OrderSide::Sell => "SELL".to_string(),
                },
                entry_time: pos.entry_time,
                exit_time,
                entry_price: pos.entry_price,
                exit_price,
                quantity: pos.quantity,
                pnl: net_pnl,
                pnl_pct,
                fees: exit_fees,
                holding_bars,
                reasoning: pos.reasoning,
                exit_reason: reason.to_string(),
            });
        }
    }

    /// Close all open positions at the given candle's close price.
    pub fn close_all(&mut self, candle: &Candle) {
        let ids: Vec<Uuid> = self.positions.iter().map(|p| p.id).collect();
        for id in ids {
            let exit_price = self.apply_slippage(candle.close, OrderSide::Sell);
            self.close_position(id, exit_price, candle.close_time, "backtest_end");
        }
    }

    /// Apply slippage to a price.
    fn apply_slippage(&self, price: Decimal, side: OrderSide) -> Decimal {
        let slippage = Decimal::try_from(self.config.slippage).unwrap_or(Decimal::ZERO);
        match side {
            OrderSide::Buy => price * (Decimal::ONE + slippage),   // Pay slightly more
            OrderSide::Sell => price * (Decimal::ONE - slippage),  // Receive slightly less
        }
    }

    /// Calculate unrealized PnL for all open positions at current candle price.
    fn unrealized_pnl(&self, candle: &Candle) -> f64 {
        self.positions
            .iter()
            .map(|pos| {
                let pnl = match pos.side {
                    OrderSide::Buy => (candle.close - pos.entry_price) * pos.quantity,
                    OrderSide::Sell => (pos.entry_price - candle.close) * pos.quantity,
                };
                pnl.to_f64().unwrap_or(0.0)
            })
            .sum()
    }

    // =========================================================================
    // Getters for results
    // =========================================================================

    pub fn completed_trades(&self) -> &[CompletedTrade] {
        &self.closed_trades
    }

    pub fn equity_curve(&self) -> &[f64] {
        &self.equity_curve
    }

    pub fn final_balance(&self) -> Decimal {
        self.balance
    }

    pub fn initial_balance(&self) -> Decimal {
        self.initial_balance
    }

    pub fn total_bars(&self) -> usize {
        self.current_bar
    }

    pub fn total_fees(&self) -> Decimal {
        self.total_fees
    }

    pub fn open_position_count(&self) -> usize {
        self.positions.len()
    }
}
