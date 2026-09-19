use chrono::Utc;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::TradingConfig;
use crate::models::*;

/// In-memory risk management engine.
/// All checks are performed in-memory for zero-latency on the critical path.
pub struct RiskManager {
    config: TradingConfig,
    /// Current open positions tracked in-memory.
    positions: Arc<RwLock<Vec<Position>>>,
    /// Daily P&L tracker (reset daily).
    daily_pnl: Arc<RwLock<DailyPnl>>,
    /// Whether trading is paused due to risk limits being hit.
    trading_paused: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
struct DailyPnl {
    date: chrono::NaiveDate,
    realized_pnl: Decimal,
    unrealized_pnl: Decimal,
    starting_balance: Decimal,
}

/// Risk check result.
#[derive(Debug)]
pub struct RiskCheckResult {
    pub approved: bool,
    pub reason: Option<String>,
    pub adjusted_quantity: Option<Decimal>,
    pub warnings: Vec<String>,
}

impl RiskManager {
    pub fn new(config: &TradingConfig) -> Self {
        Self {
            config: config.clone(),
            positions: Arc::new(RwLock::new(Vec::new())),
            daily_pnl: Arc::new(RwLock::new(DailyPnl {
                date: Utc::now().date_naive(),
                realized_pnl: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                starting_balance: Decimal::ZERO,
            })),
            trading_paused: Arc::new(RwLock::new(false)),
        }
    }

    /// Check if a new trade passes all risk management rules.
    pub async fn check_trade(
        &self,
        signal: &TradingSignal,
        total_balance: Decimal,
    ) -> RiskCheckResult {
        let mut warnings = Vec::new();

        // Check if trading is paused
        if *self.trading_paused.read().await {
            return RiskCheckResult {
                approved: false,
                reason: Some("Trading is paused due to daily loss limit".to_string()),
                adjusted_quantity: None,
                warnings,
            };
        }

        // Check maximum concurrent positions
        let open_count = self.positions.read().await.len();
        if open_count >= self.config.max_concurrent_positions {
            return RiskCheckResult {
                approved: false,
                reason: Some(format!(
                    "Maximum concurrent positions ({}) reached",
                    self.config.max_concurrent_positions
                )),
                adjusted_quantity: None,
                warnings,
            };
        }

        // Check daily loss limit
        {
            let daily = self.daily_pnl.read().await;
            let today = Utc::now().date_naive();
            if daily.date == today && daily.starting_balance > Decimal::ZERO {
                let loss_pct = ((daily.realized_pnl + daily.unrealized_pnl)
                    / daily.starting_balance
                    * Decimal::new(100, 0))
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0);

                if loss_pct < -self.config.max_daily_loss_pct {
                    *self.trading_paused.write().await = true;
                    return RiskCheckResult {
                        approved: false,
                        reason: Some(format!(
                            "Daily loss limit hit: {loss_pct:.2}% (limit: {:.1}%)",
                            self.config.max_daily_loss_pct
                        )),
                        adjusted_quantity: None,
                        warnings,
                    };
                }

                if loss_pct < -(self.config.max_daily_loss_pct * 0.8) {
                    warnings.push(format!(
                        "Approaching daily loss limit: {loss_pct:.2}% (limit: {:.1}%)",
                        self.config.max_daily_loss_pct
                    ));
                }
            }
        }

        // Check position size
        let max_position_value = total_balance
            * Decimal::try_from(self.config.max_position_size_pct / 100.0).unwrap_or(Decimal::new(5, 2));

        let requested_value = signal
            .entry_price
            .unwrap_or(Decimal::ZERO)
            * signal
                .position_size_pct
                .map(|p| {
                    total_balance
                        * Decimal::try_from(p / 100.0).unwrap_or(Decimal::ZERO)
                })
                .unwrap_or(max_position_value);

        if requested_value > max_position_value {
            warnings.push(format!(
                "Position size adjusted from {} to {} (max {:.1}% of portfolio)",
                requested_value, max_position_value, self.config.max_position_size_pct
            ));
        }

        // Check if signal confidence meets minimum threshold
        if signal.confidence < 0.5 {
            warnings.push(format!(
                "Low confidence signal: {:.1}% — consider waiting for stronger confluence",
                signal.confidence * 100.0
            ));
        }

        // Check stop loss is set
        if signal.stop_loss.is_none() {
            warnings.push("No stop loss set — will use default".to_string());
        }

        // Check risk/reward ratio
        if let Some(rr) = signal.risk_reward_ratio {
            if rr < 1.5 {
                warnings.push(format!(
                    "Low risk/reward ratio: {rr:.2} (recommended minimum: 1.5)"
                ));
            }
        }

        // Check for duplicate positions on same symbol
        let has_existing = self
            .positions
            .read()
            .await
            .iter()
            .any(|p| p.symbol == signal.symbol && p.exchange == signal.exchange);

        if has_existing {
            warnings.push(format!(
                "Already have an open position on {} — consider averaging or skipping",
                signal.symbol
            ));
        }

        RiskCheckResult {
            approved: true,
            reason: None,
            adjusted_quantity: None,
            warnings,
        }
    }

    /// Register a new position.
    pub async fn add_position(&self, position: Position) {
        info!(
            "Risk manager tracking new position: {} {} {} @ {}",
            position.exchange, position.symbol,
            match position.side { OrderSide::Buy => "LONG", OrderSide::Sell => "SHORT" },
            position.entry_price
        );
        self.positions.write().await.push(position);
    }

    /// Remove a position (closed trade).
    pub async fn remove_position(&self, position_id: uuid::Uuid, realized_pnl: Decimal) {
        let mut positions = self.positions.write().await;
        positions.retain(|p| p.id != position_id);

        // Update daily P&L
        let mut daily = self.daily_pnl.write().await;
        daily.realized_pnl += realized_pnl;
    }

    /// Update current price for all positions and check for stop-loss/take-profit.
    pub async fn update_prices(&self, symbol: &str, exchange: ExchangeId, price: Decimal) -> Vec<PositionAlert> {
        let mut alerts = Vec::new();
        let mut positions = self.positions.write().await;

        for pos in positions.iter_mut() {
            if pos.symbol == symbol && pos.exchange == exchange {
                pos.current_price = price;
                pos.calculate_pnl();

                if pos.is_stop_loss_hit() {
                    alerts.push(PositionAlert::StopLossHit(pos.clone()));
                }

                if pos.is_take_profit_hit() {
                    alerts.push(PositionAlert::TakeProfitHit(pos.clone()));
                }
            }
        }

        alerts
    }

    /// Get all current positions.
    pub async fn get_positions(&self) -> Vec<Position> {
        self.positions.read().await.clone()
    }

    /// Get current position count.
    pub async fn open_position_count(&self) -> usize {
        self.positions.read().await.len()
    }

    /// Set starting balance for daily P&L tracking.
    pub async fn set_starting_balance(&self, balance: Decimal) {
        let mut daily = self.daily_pnl.write().await;
        let today = Utc::now().date_naive();
        if daily.date != today {
            // New day — reset
            daily.date = today;
            daily.realized_pnl = Decimal::ZERO;
            daily.unrealized_pnl = Decimal::ZERO;
        }
        daily.starting_balance = balance;
    }

    /// Resume trading after manual review.
    pub async fn resume_trading(&self) {
        *self.trading_paused.write().await = false;
        info!("Trading resumed by user");
    }

    /// Check if trading is currently paused.
    pub async fn is_paused(&self) -> bool {
        *self.trading_paused.read().await
    }
}

/// Alerts generated by position monitoring.
#[derive(Debug, Clone)]
pub enum PositionAlert {
    StopLossHit(Position),
    TakeProfitHit(Position),
    TrailingStopUpdated(Position),
}
