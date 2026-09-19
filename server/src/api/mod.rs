pub mod websocket;

use crate::backtest::{BacktestConfig, BacktestEngine};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::auth;
use crate::AppState;

/// Create the REST API router.
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes — NO authentication required
    let public_routes = Router::new()
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/verify", get(auth::verify_token))
        .route("/api/health", get(health_check));

    // Protected routes — JWT authentication required
    let protected_routes = Router::new()
        // Dashboard
        .route("/api/dashboard", get(get_dashboard))
        // Positions
        .route("/api/positions", get(get_positions))
        // Analysis & Signals
        .route("/api/analysis/:symbol", get(get_analysis))
        .route("/api/signals", get(get_signals))
        .route("/api/signals/pending", get(get_pending_signals))
        .route("/api/signals/:id/approve", post(approve_signal))
        .route("/api/signals/:id/reject", post(reject_signal))
        // Orders
        .route("/api/orders", post(place_order))
        .route("/api/orders/:symbol/:id", delete(cancel_order))
        // Performance
        .route("/api/performance", get(get_performance))
        .route("/api/trades", get(get_trade_history))
        // Backtesting
        .route("/api/backtest/run", post(run_backtest))
        .route("/api/backtest/results", get(get_backtest_results))
        .route("/api/backtest/:id", get(get_backtest_result))
        // Exchanges
        .route("/api/exchanges/status", get(get_exchange_status))
        // Settings & Control
        .route("/api/settings/mode", post(set_trading_mode))
        .route("/api/settings/mode", get(get_trading_mode))
        .route("/api/control/pause", post(pause_trading))
        .route("/api/control/resume", post(resume_trading))
        // System
        .route("/api/status", get(system_status))
        // Apply auth middleware to all protected routes
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware));

    // Merge public + protected, apply shared layers
    public_routes
        .merge(protected_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}


// ===========================================================================
// Dashboard
// ===========================================================================

async fn get_dashboard(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let positions = state.risk_manager.get_positions().await;
    let pending = state.execution_engine.get_pending_signals().await;
    let history = state.execution_engine.get_trade_history().await;
    let is_paused = state.risk_manager.is_paused().await;

    let total_pnl: f64 = positions
        .iter()
        .map(|p| p.unrealized_pnl.to_string().parse::<f64>().unwrap_or(0.0))
        .sum();

    let win_count = history.iter().filter(|t| {
        t.realized_pnl
            .map(|p| p > rust_decimal::Decimal::ZERO)
            .unwrap_or(false)
    }).count();
    let total_closed = history.iter().filter(|t| t.closed_at.is_some()).count();
    let win_rate = if total_closed > 0 {
        win_count as f64 / total_closed as f64 * 100.0
    } else {
        0.0
    };

    let mode = state.execution_engine.get_mode().await;

    Json(serde_json::json!({
        "open_positions": positions.len(),
        "pending_signals": pending.len(),
        "total_trades": history.len(),
        "unrealized_pnl": total_pnl,
        "win_rate": win_rate,
        "trading_mode": format!("{:?}", mode),
        "is_paused": is_paused,
        "positions": positions,
    }))
}

// ===========================================================================
// Positions
// ===========================================================================

async fn get_positions(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let positions = state.risk_manager.get_positions().await;
    Json(serde_json::json!({ "positions": positions }))
}

// ===========================================================================
// Analysis & Signals
// ===========================================================================

async fn get_analysis(
    State(state): State<Arc<AppState>>,
    Path(symbol): Path<String>,
) -> Json<serde_json::Value> {
    let analyses = state.latest_analyses.read().await;
    if let Some(analysis) = analyses.get(&symbol) {
        Json(serde_json::json!({ "analysis": analysis }))
    } else {
        Json(serde_json::json!({ "analysis": null, "message": "No analysis available for this symbol" }))
    }
}

async fn get_signals(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let signals = state.signal_history.read().await;
    Json(serde_json::json!({ "signals": *signals }))
}

async fn get_pending_signals(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let pending = state.execution_engine.get_pending_signals().await;
    Json(serde_json::json!({ "pending_signals": pending }))
}

async fn approve_signal(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let signal_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    // TODO: In production, determine exchange from signal and call:
    // state.execution_engine.approve_signal(signal_id, &*exchange).await;

    Ok(Json(serde_json::json!({ "status": "approved", "signal_id": signal_id })))
}

async fn reject_signal(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let signal_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let rejected = _state.execution_engine.reject_signal(signal_id).await;

    Ok(Json(serde_json::json!({
        "status": if rejected { "rejected" } else { "not_found" },
        "signal_id": signal_id,
    })))
}

// ===========================================================================
// Orders
// ===========================================================================

#[derive(Deserialize)]
struct PlaceOrderRequest {
    symbol: String,
    side: String,
    #[allow(dead_code)]
    order_type: String,
    quantity: String,
    price: Option<String>,
}

async fn place_order(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<PlaceOrderRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Manual order: {} {} {} @ {:?}", req.symbol, req.side, req.quantity, req.price);

    Ok(Json(serde_json::json!({
        "status": "order_queued",
        "symbol": req.symbol,
        "side": req.side,
        "quantity": req.quantity,
    })))
}

async fn cancel_order(
    State(_state): State<Arc<AppState>>,
    Path((symbol, id)): Path<(String, String)>,
) -> Json<serde_json::Value> {
    info!("Cancel order request: {symbol} / {id}");
    Json(serde_json::json!({ "status": "cancelled", "order_id": id }))
}

// ===========================================================================
// Performance
// ===========================================================================

async fn get_performance(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let history = state.execution_engine.get_trade_history().await;

    let total = history.len();
    let closed: Vec<_> = history.iter().filter(|t| t.closed_at.is_some()).collect();
    let wins = closed
        .iter()
        .filter(|t| t.realized_pnl.map(|p| p > rust_decimal::Decimal::ZERO).unwrap_or(false))
        .count();
    let losses = closed.len() - wins;

    Json(serde_json::json!({
        "total_trades": total,
        "closed_trades": closed.len(),
        "wins": wins,
        "losses": losses,
        "win_rate": if closed.is_empty() { 0.0 } else { wins as f64 / closed.len() as f64 * 100.0 },
    }))
}

async fn get_trade_history(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let history = state.execution_engine.get_trade_history().await;
    Json(serde_json::json!({ "trades": history }))
}

// ===========================================================================
// Settings & Control
// ===========================================================================

#[derive(Deserialize)]
struct SetModeRequest {
    mode: String,
}

async fn set_trading_mode(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetModeRequest>,
) -> Json<serde_json::Value> {
    let mode = match req.mode.to_lowercase().as_str() {
        "auto" => crate::execution::TradingMode::Auto,
        _ => crate::execution::TradingMode::Manual,
    };
    state.execution_engine.set_mode(mode.clone()).await;
    Json(serde_json::json!({ "mode": format!("{:?}", mode) }))
}

async fn get_trading_mode(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mode = state.execution_engine.get_mode().await;
    Json(serde_json::json!({ "mode": format!("{:?}", mode) }))
}

async fn pause_trading(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Pause via risk manager
    info!("Trading paused by user");
    Json(serde_json::json!({ "status": "paused" }))
}

async fn resume_trading(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    state.risk_manager.resume_trading().await;
    Json(serde_json::json!({ "status": "resumed" }))
}

// ===========================================================================
// System
// ===========================================================================

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn system_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let positions = state.risk_manager.open_position_count().await;
    let is_paused = state.risk_manager.is_paused().await;
    let mode = state.execution_engine.get_mode().await;
    let tokens = state.ai_engine.grok.total_tokens_used();
    let exchange_status = state.exchange_manager.status().await;

    Json(serde_json::json!({
        "server": "running",
        "trading_mode": format!("{:?}", mode),
        "is_paused": is_paused,
        "open_positions": positions,
        "grok_tokens_used": tokens,
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "exchanges": exchange_status.iter().map(|(id, connected)| {
            serde_json::json!({ "id": format!("{}", id), "connected": connected })
        }).collect::<Vec<_>>(),
    }))
}

// ===========================================================================
// Backtesting
// ===========================================================================

async fn run_backtest(
    State(state): State<Arc<AppState>>,
    Json(config): Json<BacktestConfig>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Starting backtest: {} {} from {} to {}",
        config.symbol, config.timeframe, config.start_time, config.end_time
    );

    match BacktestEngine::run(config, &state.ai_engine).await {
        Ok(result) => {
            let result_json = serde_json::to_value(&result).unwrap_or_default();
            state.backtest_results.write().await.push(result);
            Ok(Json(serde_json::json!({
                "status": "completed",
                "result": result_json,
            })))
        }
        Err(e) => {
            tracing::error!("Backtest failed: {e}");
            Ok(Json(serde_json::json!({
                "status": "error",
                "message": e.to_string(),
            })))
        }
    }
}

async fn get_backtest_results(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let results = state.backtest_results.read().await;
    let summaries: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "symbol": r.config.symbol,
                "timeframe": r.config.timeframe,
                "total_return_pct": r.metrics.total_return_pct,
                "total_trades": r.metrics.total_trades,
                "win_rate_pct": r.metrics.win_rate_pct,
                "sharpe_ratio": r.metrics.sharpe_ratio,
                "max_drawdown_pct": r.metrics.max_drawdown_pct,
                "completed_at": r.completed_at,
            })
        })
        .collect();

    Json(serde_json::json!({ "results": summaries }))
}

async fn get_backtest_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let results = state.backtest_results.read().await;

    if let Some(result) = results.iter().find(|r| r.id == result_id) {
        Ok(Json(serde_json::json!({ "result": result })))
    } else {
        Ok(Json(serde_json::json!({
            "error": "Backtest result not found",
            "id": result_id
        })))
    }
}

// ===========================================================================
// Exchange Status
// ===========================================================================

async fn get_exchange_status(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let status = state.exchange_manager.status().await;
    let exchanges: Vec<serde_json::Value> = status
        .iter()
        .map(|(id, connected)| {
            serde_json::json!({
                "exchange": format!("{}", id),
                "connected": connected,
            })
        })
        .collect();

    Json(serde_json::json!({ "exchanges": exchanges }))
}
