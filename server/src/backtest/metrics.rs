use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

/// Performance metrics computed from backtest results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    // Returns
    pub total_return_pct: f64,
    pub total_pnl: Decimal,
    pub annualized_return_pct: f64,

    // Risk
    pub max_drawdown_pct: f64,
    pub max_drawdown_duration_bars: usize,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,

    // Trade stats
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate_pct: f64,
    pub profit_factor: f64,

    // Per-trade
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub average_holding_bars: f64,

    // Fees
    pub total_fees_paid: Decimal,

    // Equity curve
    pub equity_curve: Vec<f64>,
    pub drawdown_curve: Vec<f64>,
}

/// A single completed trade for metrics computation and reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTrade {
    pub symbol: String,
    pub side: String,
    pub entry_time: chrono::DateTime<chrono::Utc>,
    pub exit_time: chrono::DateTime<chrono::Utc>,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub pnl: Decimal,
    pub pnl_pct: f64,
    pub fees: Decimal,
    pub holding_bars: usize,
    pub reasoning: String,
    pub exit_reason: String,
}

impl BacktestMetrics {
    /// Compute all metrics from completed trades and equity curve.
    pub fn compute(
        trades: &[CompletedTrade],
        equity_curve: &[f64],
        initial_balance: f64,
        total_bars: usize,
        bars_per_year: f64,
    ) -> Self {
        let total_trades = trades.len();

        let wins: Vec<&CompletedTrade> = trades.iter().filter(|t| t.pnl > Decimal::ZERO).collect();
        let losses: Vec<&CompletedTrade> = trades.iter().filter(|t| t.pnl < Decimal::ZERO).collect();

        let winning_trades = wins.len();
        let losing_trades = losses.len();

        let win_rate_pct = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64 * 100.0
        } else {
            0.0
        };

        let total_pnl: Decimal = trades.iter().map(|t| t.pnl).sum();
        let total_fees_paid: Decimal = trades.iter().map(|t| t.fees).sum();

        let final_equity = equity_curve.last().copied().unwrap_or(initial_balance);
        let total_return_pct = if initial_balance > 0.0 {
            (final_equity - initial_balance) / initial_balance * 100.0
        } else {
            0.0
        };

        // Annualized return
        let years = if bars_per_year > 0.0 {
            total_bars as f64 / bars_per_year
        } else {
            1.0
        };
        let annualized_return_pct = if years > 0.0 && initial_balance > 0.0 {
            ((final_equity / initial_balance).powf(1.0 / years) - 1.0) * 100.0
        } else {
            0.0
        };

        // Max drawdown
        let (max_drawdown_pct, max_drawdown_duration_bars, drawdown_curve) =
            Self::compute_drawdown(equity_curve);

        // Sharpe ratio (using per-bar returns)
        let returns = Self::compute_returns(equity_curve);
        let sharpe_ratio = Self::compute_sharpe(&returns, bars_per_year);
        let sortino_ratio = Self::compute_sortino(&returns, bars_per_year);

        // Profit factor
        let gross_profit: f64 = wins
            .iter()
            .map(|t| t.pnl.to_f64().unwrap_or(0.0))
            .sum();
        let gross_loss: f64 = losses
            .iter()
            .map(|t| t.pnl.to_f64().unwrap_or(0.0).abs())
            .sum();
        let profit_factor = if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else if gross_profit > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // Average win/loss
        let average_win = if winning_trades > 0 {
            wins.iter().map(|t| t.pnl).sum::<Decimal>()
                / Decimal::from(winning_trades as u64)
        } else {
            Decimal::ZERO
        };

        let average_loss = if losing_trades > 0 {
            losses.iter().map(|t| t.pnl).sum::<Decimal>()
                / Decimal::from(losing_trades as u64)
        } else {
            Decimal::ZERO
        };

        // Largest win/loss
        let largest_win = wins
            .iter()
            .map(|t| t.pnl)
            .max()
            .unwrap_or(Decimal::ZERO);
        let largest_loss = losses
            .iter()
            .map(|t| t.pnl)
            .min()
            .unwrap_or(Decimal::ZERO);

        // Average holding time
        let average_holding_bars = if total_trades > 0 {
            trades.iter().map(|t| t.holding_bars).sum::<usize>() as f64 / total_trades as f64
        } else {
            0.0
        };

        Self {
            total_return_pct,
            total_pnl,
            annualized_return_pct,
            max_drawdown_pct,
            max_drawdown_duration_bars,
            sharpe_ratio,
            sortino_ratio,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate_pct,
            profit_factor,
            average_win,
            average_loss,
            largest_win,
            largest_loss,
            average_holding_bars,
            total_fees_paid,
            equity_curve: equity_curve.to_vec(),
            drawdown_curve,
        }
    }

    /// Compute drawdown curve and max drawdown.
    fn compute_drawdown(equity: &[f64]) -> (f64, usize, Vec<f64>) {
        let mut max_dd = 0.0f64;
        let mut peak = equity.first().copied().unwrap_or(1.0);
        let mut max_dd_duration = 0usize;
        let mut current_dd_duration = 0usize;
        let mut drawdown_curve = Vec::with_capacity(equity.len());

        for &eq in equity {
            if eq > peak {
                peak = eq;
                current_dd_duration = 0;
            } else {
                current_dd_duration += 1;
            }

            let dd = if peak > 0.0 {
                (peak - eq) / peak * 100.0
            } else {
                0.0
            };

            drawdown_curve.push(dd);
            max_dd = max_dd.max(dd);
            max_dd_duration = max_dd_duration.max(current_dd_duration);
        }

        (max_dd, max_dd_duration, drawdown_curve)
    }

    /// Compute per-bar returns from equity curve.
    fn compute_returns(equity: &[f64]) -> Vec<f64> {
        if equity.len() < 2 {
            return vec![];
        }

        equity
            .windows(2)
            .map(|w| {
                if w[0] > 0.0 {
                    (w[1] - w[0]) / w[0]
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Compute annualized Sharpe ratio (risk-free rate = 0).
    fn compute_sharpe(returns: &[f64], bars_per_year: f64) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev > 0.0 {
            (mean / std_dev) * bars_per_year.sqrt()
        } else {
            0.0
        }
    }

    /// Compute annualized Sortino ratio (downside deviation only).
    fn compute_sortino(returns: &[f64], bars_per_year: f64) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let downside_returns: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).copied().collect();

        if downside_returns.is_empty() {
            return if mean > 0.0 { f64::INFINITY } else { 0.0 };
        }

        let downside_variance = downside_returns
            .iter()
            .map(|r| r.powi(2))
            .sum::<f64>()
            / downside_returns.len() as f64;
        let downside_dev = downside_variance.sqrt();

        if downside_dev > 0.0 {
            (mean / downside_dev) * bars_per_year.sqrt()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_metrics_basic() {
        let now = chrono::Utc::now();
        let trades = vec![
            CompletedTrade {
                symbol: "BTCUSDT".to_string(),
                side: "BUY".to_string(),
                entry_time: now,
                exit_time: now,
                entry_price: Decimal::new(50000, 0),
                exit_price: Decimal::new(51000, 0),
                quantity: Decimal::new(1, 0),
                pnl: Decimal::new(100, 0),
                pnl_pct: 2.0,
                fees: Decimal::new(1, 0),
                holding_bars: 5,
                reasoning: "Test win".to_string(),
                exit_reason: "take_profit".to_string(),
            },
            CompletedTrade {
                symbol: "BTCUSDT".to_string(),
                side: "BUY".to_string(),
                entry_time: now,
                exit_time: now,
                entry_price: Decimal::new(50000, 0),
                exit_price: Decimal::new(49500, 0),
                quantity: Decimal::new(1, 0),
                pnl: Decimal::new(-50, 0),
                pnl_pct: -1.0,
                fees: Decimal::new(1, 0),
                holding_bars: 3,
                reasoning: "Test loss".to_string(),
                exit_reason: "stop_loss".to_string(),
            },
            CompletedTrade {
                symbol: "BTCUSDT".to_string(),
                side: "BUY".to_string(),
                entry_time: now,
                exit_time: now,
                entry_price: Decimal::new(50000, 0),
                exit_price: Decimal::new(52000, 0),
                quantity: Decimal::new(1, 0),
                pnl: Decimal::new(200, 0),
                pnl_pct: 4.0,
                fees: Decimal::new(2, 0),
                holding_bars: 10,
                reasoning: "Test big win".to_string(),
                exit_reason: "take_profit".to_string(),
            },
        ];

        let equity = vec![1000.0, 1050.0, 1100.0, 1000.0, 1050.0, 1200.0, 1250.0];
        let metrics = BacktestMetrics::compute(&trades, &equity, 1000.0, 7, 365.0 * 24.0);

        assert_eq!(metrics.total_trades, 3);
        assert_eq!(metrics.winning_trades, 2);
        assert_eq!(metrics.losing_trades, 1);
        assert!(metrics.win_rate_pct > 60.0);
        assert!(metrics.max_drawdown_pct > 0.0);
        assert!(metrics.profit_factor > 1.0);
    }

    #[test]
    fn test_drawdown_computation() {
        let equity = vec![100.0, 110.0, 105.0, 90.0, 95.0, 115.0];
        let (max_dd, _, _) = BacktestMetrics::compute_drawdown(&equity);
        // Peak = 110, trough = 90 → DD = 18.18%
        assert!(max_dd > 18.0 && max_dd < 19.0);
    }
}
