use anyhow::Result;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::TradingConfig;
use crate::exchange::Exchange;
use crate::models::*;
use crate::risk::{PositionAlert, RiskManager};

/// Trade execution engine.
/// Handles order creation, submission, tracking, and position lifecycle management.
pub struct ExecutionEngine {
    risk_manager: Arc<RiskManager>,
    trading_mode: Arc<RwLock<TradingMode>>,
    /// Pending signals awaiting user approval (manual mode).
    pending_signals: Arc<RwLock<Vec<TradingSignal>>>,
    /// Trade history log.
    trade_history: Arc<RwLock<Vec<TradeRecord>>>,
    /// Optional exchange manager reference to verify exchange auto-trading enablement.
    exchange_manager: Arc<RwLock<Option<Arc<crate::exchange::manager::ExchangeManager>>>>,
}

/// Record of an executed trade for performance tracking.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradeRecord {
    pub id: uuid::Uuid,
    pub exchange: ExchangeId,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub exit_price: Option<Decimal>,
    pub fees: Decimal,
    pub realized_pnl: Option<Decimal>,
    pub signal_id: uuid::Uuid,
    pub signal_confidence: f64,
    pub opened_at: chrono::DateTime<chrono::Utc>,
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub close_reason: Option<String>,
}

/// Mode for trading - auto or manual.
#[derive(Debug, Clone, PartialEq)]
pub enum TradingMode {
    Auto,
    Manual,
}

impl ExecutionEngine {
    pub fn new(risk_manager: Arc<RiskManager>, config: &TradingConfig) -> Self {
        let mode = match config.mode {
            crate::config::TradingMode::Auto => TradingMode::Auto,
            crate::config::TradingMode::Manual => TradingMode::Manual,
        };

        Self {
            risk_manager,
            trading_mode: Arc::new(RwLock::new(mode)),
            pending_signals: Arc::new(RwLock::new(Vec::new())),
            trade_history: Arc::new(RwLock::new(Vec::new())),
            exchange_manager: Arc::new(RwLock::new(None)),
        }
    }

    /// Set exchange manager reference to enforce exchange auto-trading toggles.
    pub async fn set_exchange_manager(&self, em: Arc<crate::exchange::manager::ExchangeManager>) {
        *self.exchange_manager.write().await = Some(em);
    }

    /// Process a trading signal — either execute immediately (auto) or queue for approval (manual).
    pub async fn process_signal(
        &self,
        signal: TradingSignal,
        exchange: &dyn Exchange,
        total_balance: Decimal,
    ) -> Result<SignalProcessResult> {
        // Skip non-actionable signals
        if !signal.is_actionable() {
            return Ok(SignalProcessResult::Skipped("Signal is HOLD — no action needed".to_string()));
        }

        // Skip expired signals
        if signal.is_expired() {
            return Ok(SignalProcessResult::Skipped("Signal has expired".to_string()));
        }

        // Run risk checks
        let risk_check = self.risk_manager.check_trade(&signal, total_balance).await;

        if !risk_check.approved {
            warn!(
                "Trade rejected by risk manager: {}",
                risk_check.reason.as_deref().unwrap_or("unknown")
            );
            return Ok(SignalProcessResult::Rejected(
                risk_check.reason.unwrap_or_default(),
            ));
        }

        // Log warnings
        for warning in &risk_check.warnings {
            warn!("Risk warning: {warning}");
        }

        let mode = self.trading_mode.read().await.clone();

        match mode {
            TradingMode::Auto => {
                // Verify if the target exchange is enabled for auto bot transactions
                if let Some(em) = self.exchange_manager.read().await.as_ref() {
                    if !em.is_enabled(&signal.exchange).await {
                        info!(
                            "Skipping auto-trade for {} on {} — exchange is disabled for auto-bot transactions",
                            signal.symbol, signal.exchange
                        );
                        return Ok(SignalProcessResult::Skipped(format!(
                            "Exchange {} is disabled for auto-bot transactions",
                            signal.exchange
                        )));
                    }
                }

                // Execute immediately
                let result = self.execute_trade(signal, exchange).await?;
                Ok(SignalProcessResult::Executed(result))
            }
            TradingMode::Manual => {
                // Queue for user approval
                info!(
                    "Signal queued for manual approval: {} {:?} (confidence: {:.1}%)",
                    signal.symbol,
                    signal.action,
                    signal.confidence * 100.0
                );
                self.pending_signals.write().await.push(signal);
                Ok(SignalProcessResult::PendingApproval)
            }
        }
    }

    /// Execute a trade based on a signal.
    async fn execute_trade(
        &self,
        signal: TradingSignal,
        exchange: &dyn Exchange,
    ) -> Result<TradeRecord> {
        let side = signal.order_side().unwrap();
        let entry_price = signal.entry_price.unwrap_or(Decimal::ZERO);

        // Calculate quantity from position size percentage
        // For now, use a simple calculation
        let quantity = signal
            .position_size_pct
            .map(|_pct| Decimal::new(1, 2)) // placeholder — real calc needs balance info
            .unwrap_or(Decimal::new(1, 2));

        // Create order
        let order = match signal.entry_price {
            Some(price) => Order::limit(signal.exchange, &signal.symbol, side, quantity, price),
            None => Order::market(signal.exchange, &signal.symbol, side, quantity),
        };

        info!(
            "Executing trade: {} {} {} @ {}",
            signal.symbol,
            match side { OrderSide::Buy => "BUY", OrderSide::Sell => "SELL" },
            quantity,
            signal.entry_price.map(|p| p.to_string()).unwrap_or("MARKET".to_string())
        );

        // Place order on exchange
        let result = exchange.place_order(&order).await?;

        let fill_price = result.average_fill_price.unwrap_or(entry_price);

        // Create position for tracking
        let position = Position {
            id: uuid::Uuid::new_v4(),
            exchange: signal.exchange,
            symbol: signal.symbol.clone(),
            side,
            quantity: result.filled_quantity,
            entry_price: fill_price,
            current_price: fill_price,
            stop_loss: signal.stop_loss,
            take_profit: signal.take_profit,
            unrealized_pnl: Decimal::ZERO,
            unrealized_pnl_pct: 0.0,
            realized_pnl: Decimal::ZERO,
            fees_paid: Decimal::ZERO,
            opened_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            signal_id: Some(signal.id),
        };

        // Register with risk manager
        self.risk_manager.add_position(position).await;

        // Create trade record
        let record = TradeRecord {
            id: uuid::Uuid::new_v4(),
            exchange: signal.exchange,
            symbol: signal.symbol.clone(),
            side,
            quantity: result.filled_quantity,
            entry_price: fill_price,
            exit_price: None,
            fees: Decimal::ZERO,
            realized_pnl: None,
            signal_id: signal.id,
            signal_confidence: signal.confidence,
            opened_at: chrono::Utc::now(),
            closed_at: None,
            close_reason: None,
        };

        self.trade_history.write().await.push(record.clone());

        info!(
            "Trade executed: {} {} {} @ {} (order: {})",
            signal.symbol,
            match side { OrderSide::Buy => "BUY", OrderSide::Sell => "SELL" },
            result.filled_quantity,
            fill_price,
            result.exchange_order_id
        );

        Ok(record)
    }

    /// Handle position alerts (stop loss / take profit hits).
    pub async fn handle_alert(
        &self,
        alert: PositionAlert,
        exchange: &dyn Exchange,
    ) -> Result<()> {
        match alert {
            PositionAlert::StopLossHit(position) => {
                warn!(
                    "STOP LOSS HIT: {} {} @ {} (entry: {})",
                    position.symbol,
                    match position.side { OrderSide::Buy => "LONG", OrderSide::Sell => "SHORT" },
                    position.current_price,
                    position.entry_price
                );

                // Close position at market
                let close_side = match position.side {
                    OrderSide::Buy => OrderSide::Sell,
                    OrderSide::Sell => OrderSide::Buy,
                };

                let close_order = Order::market(
                    position.exchange,
                    &position.symbol,
                    close_side,
                    position.quantity,
                );

                exchange.place_order(&close_order).await?;
                self.risk_manager
                    .remove_position(position.id, position.unrealized_pnl)
                    .await;
            }
            PositionAlert::TakeProfitHit(position) => {
                info!(
                    "TAKE PROFIT HIT: {} {} @ {} (entry: {}, P&L: {})",
                    position.symbol,
                    match position.side { OrderSide::Buy => "LONG", OrderSide::Sell => "SHORT" },
                    position.current_price,
                    position.entry_price,
                    position.unrealized_pnl
                );

                let close_side = match position.side {
                    OrderSide::Buy => OrderSide::Sell,
                    OrderSide::Sell => OrderSide::Buy,
                };

                let close_order = Order::market(
                    position.exchange,
                    &position.symbol,
                    close_side,
                    position.quantity,
                );

                exchange.place_order(&close_order).await?;
                self.risk_manager
                    .remove_position(position.id, position.unrealized_pnl)
                    .await;
            }
            PositionAlert::TrailingStopUpdated(position) => {
                info!(
                    "Trailing stop updated for {} to {}",
                    position.symbol,
                    position.stop_loss.unwrap_or(Decimal::ZERO)
                );
            }
        }
        Ok(())
    }

    /// Approve a pending signal (manual mode).
    pub async fn approve_signal(&self, signal_id: uuid::Uuid, exchange: &dyn Exchange) -> Result<Option<TradeRecord>> {
        let mut pending = self.pending_signals.write().await;
        if let Some(pos) = pending.iter().position(|s| s.id == signal_id) {
            let signal = pending.remove(pos);
            drop(pending);
            let record = self.execute_trade(signal, exchange).await?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Reject a pending signal.
    pub async fn reject_signal(&self, signal_id: uuid::Uuid) -> bool {
        let mut pending = self.pending_signals.write().await;
        let before = pending.len();
        pending.retain(|s| s.id != signal_id);
        pending.len() < before
    }

    /// Get all pending signals.
    pub async fn get_pending_signals(&self) -> Vec<TradingSignal> {
        self.pending_signals.read().await.clone()
    }

    /// Get trade history.
    pub async fn get_trade_history(&self) -> Vec<TradeRecord> {
        self.trade_history.read().await.clone()
    }

    /// Toggle trading mode.
    pub async fn set_mode(&self, mode: TradingMode) {
        info!("Trading mode changed to: {:?}", mode);
        *self.trading_mode.write().await = mode;
    }

    /// Get current trading mode.
    pub async fn get_mode(&self) -> TradingMode {
        self.trading_mode.read().await.clone()
    }
}

/// Result of processing a signal.
#[derive(Debug)]
pub enum SignalProcessResult {
    Executed(TradeRecord),
    PendingApproval,
    Rejected(String),
    Skipped(String),
}
