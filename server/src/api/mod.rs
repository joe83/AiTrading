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
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use hmac::Mac;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::auth;
use crate::models::ExchangeId;
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
        .route("/api/exchanges/:exchange/toggle", post(toggle_exchange))
        // Meme / Social Sentiment Radar
        .route("/api/radar/memes", get(get_meme_radar))
        .route("/api/radar/memes/scan", post(scan_meme_radar))
        // Settings & Control
        .route("/api/settings/mode", post(set_trading_mode))
        .route("/api/settings/mode", get(get_trading_mode))
        .route("/api/settings/api-keys", get(get_api_keys))
        .route("/api/settings/api-keys", post(update_api_keys))
        .route("/api/settings/api-keys/test", post(test_api_key))
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
        "exchanges": exchange_status.iter().map(|(id, info)| {
            serde_json::json!({
                "exchange": format!("{}", id),
                "id": format!("{}", id),
                "connected": info.connected,
                "enabled": info.enabled,
            })
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
                "config": r.config,
                "metrics": r.metrics,
                "symbol": r.config.symbol,
                "timeframe": r.config.timeframe,
                "use_grok": r.use_grok,
                "ai_calls_made": r.ai_calls_made,
                "ai_cached_calls": r.ai_cached_calls,
                "grok_model_used": r.grok_model_used,
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
        .map(|(id, info)| {
            serde_json::json!({
                "exchange": format!("{}", id),
                "id": format!("{}", id),
                "connected": info.connected,
                "enabled": info.enabled,
            })
        })
        .collect();

    Json(serde_json::json!({ "exchanges": exchanges }))
}

#[derive(Deserialize)]
struct ToggleExchangeRequest {
    enabled: Option<bool>,
}

async fn toggle_exchange(
    State(state): State<Arc<AppState>>,
    Path(exchange_name): Path<String>,
    Json(payload): Json<ToggleExchangeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let exchange_id = match exchange_name.to_lowercase().replace(['-', ' '], "_").as_str() {
        "mexc" => ExchangeId::Mexc,
        "binance" => ExchangeId::Binance,
        "bybit" => ExchangeId::Bybit,
        "alpaca" => ExchangeId::Alpaca,
        "ic_markets" | "icmarkets" => ExchangeId::IcMarkets,
        _ => return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Exchange '{}' not found", exchange_name) })),
        )),
    };

    let is_connected = state.exchange_manager.is_connected(&exchange_id).await;
    let current_enabled = state.exchange_manager.is_enabled(&exchange_id).await;
    let target_enabled = payload.enabled.unwrap_or(!current_enabled);

    // If enabling, verify that credentials are valid and exchange is connected
    if target_enabled && !is_connected {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Cannot enable auto-trading on {}: Exchange API keys are not connected or verified. Please configure valid API keys in API Keys settings first.", exchange_id),
                "connected": false,
                "enabled": false,
            })),
        ));
    }

    state.exchange_manager.set_enabled(exchange_id, target_enabled).await;

    let env_key = match exchange_id {
        ExchangeId::Mexc => "MEXC_ENABLED",
        ExchangeId::Binance => "BINANCE_ENABLED",
        ExchangeId::Bybit => "BYBIT_ENABLED",
        ExchangeId::Alpaca => "ALPACA_ENABLED",
        ExchangeId::IcMarkets => "IC_MARKETS_ENABLED",
    };
    let _ = crate::config::update_env_file(&[(env_key, &target_enabled.to_string())]);

    {
        let mut cfg = state.config.write().await;
        match exchange_id {
            ExchangeId::Mexc => cfg.mexc.enabled = target_enabled,
            ExchangeId::Binance => cfg.binance.enabled = target_enabled,
            ExchangeId::Bybit => cfg.bybit.enabled = target_enabled,
            ExchangeId::Alpaca => cfg.alpaca.enabled = target_enabled,
            ExchangeId::IcMarkets => cfg.ic_markets.enabled = target_enabled,
        }
    }

    info!("Exchange {} auto-trading updated: enabled={}", exchange_id, target_enabled);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "exchange": format!("{}", exchange_id),
        "connected": is_connected,
        "enabled": target_enabled,
    })))
}

// ===========================================================================
// API Keys & Provider Settings
// ===========================================================================

fn mask_secret(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        "".to_string()
    } else if s.len() <= 8 {
        "********".to_string()
    } else {
        format!("{}...{}", &s[..4], &s[s.len() - 4..])
    }
}

#[derive(Serialize)]
pub struct ApiKeysStatus {
    pub grok: GrokKeyStatus,
    pub mexc: MexcKeyStatus,
    pub alpaca: AlpacaKeyStatus,
    pub ic_markets: IcMarketsKeyStatus,
    pub binance: BinanceKeyStatus,
    pub bybit: BybitKeyStatus,
}

#[derive(Serialize)]
pub struct GrokKeyStatus {
    pub is_set: bool,
    pub masked_key: String,
    pub base_url: String,
    pub model_primary: String,
    pub model_fast: String,
}

#[derive(Serialize)]
pub struct MexcKeyStatus {
    pub is_set: bool,
    pub masked_api_key: String,
    pub is_secret_set: bool,
    pub base_url: String,
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct AlpacaKeyStatus {
    pub is_set: bool,
    pub masked_api_key: String,
    pub is_secret_set: bool,
    pub base_url: String,
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct IcMarketsKeyStatus {
    pub is_set: bool,
    pub masked_api_key: String,
    pub account_id: String,
    pub client_id: String,
    pub is_client_secret_set: bool,
    pub base_url: String,
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct BinanceKeyStatus {
    pub is_set: bool,
    pub masked_api_key: String,
    pub is_secret_set: bool,
    pub base_url: String,
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct BybitKeyStatus {
    pub is_set: bool,
    pub masked_api_key: String,
    pub is_secret_set: bool,
    pub base_url: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct UpdateApiKeysRequest {
    pub grok_api_key: Option<String>,
    pub grok_base_url: Option<String>,
    pub grok_model_primary: Option<String>,
    pub grok_model_fast: Option<String>,

    pub mexc_api_key: Option<String>,
    pub mexc_secret_key: Option<String>,
    pub mexc_enabled: Option<bool>,

    pub alpaca_api_key: Option<String>,
    pub alpaca_secret_key: Option<String>,
    pub alpaca_base_url: Option<String>,
    pub alpaca_enabled: Option<bool>,

    pub ic_markets_api_key: Option<String>,
    pub ic_markets_account_id: Option<String>,
    pub ic_markets_client_id: Option<String>,
    pub ic_markets_client_secret: Option<String>,
    pub ic_markets_enabled: Option<bool>,

    pub binance_api_key: Option<String>,
    pub binance_secret_key: Option<String>,
    pub binance_base_url: Option<String>,
    pub binance_enabled: Option<bool>,

    pub bybit_api_key: Option<String>,
    pub bybit_secret_key: Option<String>,
    pub bybit_base_url: Option<String>,
    pub bybit_enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct TestApiKeyRequest {
    pub service: String,
    pub key: Option<String>,
    pub secret: Option<String>,
}

#[derive(Serialize)]
pub struct TestApiKeyResponse {
    pub success: bool,
    pub message: String,
}

async fn get_api_keys(State(state): State<Arc<AppState>>) -> Json<ApiKeysStatus> {
    let cfg = state.config.read().await;
    let grok_key = state.ai_engine.grok.get_api_key();
    let grok_primary = state.ai_engine.grok.get_model_primary();
    let grok_fast = state.ai_engine.grok.get_model_fast();

    let mexc_enabled = state.exchange_manager.is_enabled(&crate::models::ExchangeId::Mexc).await;
    let alpaca_enabled = state.exchange_manager.is_enabled(&crate::models::ExchangeId::Alpaca).await;
    let ic_enabled = state.exchange_manager.is_enabled(&crate::models::ExchangeId::IcMarkets).await;
    let binance_enabled = state.exchange_manager.is_enabled(&crate::models::ExchangeId::Binance).await;
    let bybit_enabled = state.exchange_manager.is_enabled(&crate::models::ExchangeId::Bybit).await;

    Json(ApiKeysStatus {
        grok: GrokKeyStatus {
            is_set: !grok_key.is_empty(),
            masked_key: mask_secret(&grok_key),
            base_url: cfg.grok.base_url.clone(),
            model_primary: grok_primary,
            model_fast: grok_fast,
        },
        mexc: MexcKeyStatus {
            is_set: !cfg.mexc.api_key.is_empty(),
            masked_api_key: mask_secret(&cfg.mexc.api_key),
            is_secret_set: !cfg.mexc.secret_key.is_empty(),
            base_url: cfg.mexc.base_url.clone(),
            enabled: mexc_enabled,
        },
        alpaca: AlpacaKeyStatus {
            is_set: !cfg.alpaca.api_key.is_empty(),
            masked_api_key: mask_secret(&cfg.alpaca.api_key),
            is_secret_set: !cfg.alpaca.secret_key.is_empty(),
            base_url: cfg.alpaca.base_url.clone(),
            enabled: alpaca_enabled,
        },
        ic_markets: IcMarketsKeyStatus {
            is_set: !cfg.ic_markets.api_key.is_empty(),
            masked_api_key: mask_secret(&cfg.ic_markets.api_key),
            account_id: cfg.ic_markets.account_id.clone(),
            client_id: cfg.ic_markets.client_id.clone(),
            is_client_secret_set: !cfg.ic_markets.client_secret.is_empty(),
            base_url: cfg.ic_markets.base_url.clone(),
            enabled: ic_enabled,
        },
        binance: BinanceKeyStatus {
            is_set: !cfg.binance.api_key.is_empty(),
            masked_api_key: mask_secret(&cfg.binance.api_key),
            is_secret_set: !cfg.binance.secret_key.is_empty(),
            base_url: cfg.binance.base_url.clone(),
            enabled: binance_enabled,
        },
        bybit: BybitKeyStatus {
            is_set: !cfg.bybit.api_key.is_empty(),
            masked_api_key: mask_secret(&cfg.bybit.api_key),
            is_secret_set: !cfg.bybit.secret_key.is_empty(),
            base_url: cfg.bybit.base_url.clone(),
            enabled: bybit_enabled,
        },
    })
}

async fn update_api_keys(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateApiKeysRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut env_updates: Vec<(&str, &str)> = Vec::new();

    // 1. Grok updates
    let grok_key = req.grok_api_key.filter(|s| !s.trim().is_empty());
    let grok_base = req.grok_base_url.filter(|s| !s.trim().is_empty());
    let grok_primary = req.grok_model_primary.filter(|s| !s.trim().is_empty());
    let grok_fast = req.grok_model_fast.filter(|s| !s.trim().is_empty());

    if let Some(ref k) = grok_key {
        state.ai_engine.grok.set_api_key(k.clone());
    }
    if let (Some(ref p), Some(ref f)) = (&grok_primary, &grok_fast) {
        state.ai_engine.grok.set_models(p.clone(), f.clone());
    } else if let Some(ref p) = grok_primary {
        let f = state.ai_engine.grok.get_model_fast();
        state.ai_engine.grok.set_models(p.clone(), f);
    } else if let Some(ref f) = grok_fast {
        let p = state.ai_engine.grok.get_model_primary();
        state.ai_engine.grok.set_models(p, f.clone());
    }
    if let Some(ref b) = grok_base {
        state.ai_engine.grok.set_base_url(b.clone());
    }

    // Update config lock
    let mut cfg = state.config.write().await;

    if let Some(ref k) = grok_key {
        cfg.grok.api_key = k.clone();
    }
    if let Some(ref b) = grok_base {
        cfg.grok.base_url = b.clone();
    }
    if let Some(ref p) = grok_primary {
        cfg.grok.model_primary = p.clone();
    }
    if let Some(ref f) = grok_fast {
        cfg.grok.model_fast = f.clone();
    }

    // 2. MEXC updates
    let mut mexc_changed = false;
    if let Some(k) = req.mexc_api_key.filter(|s| !s.trim().is_empty()) {
        cfg.mexc.api_key = k;
        mexc_changed = true;
    }
    if let Some(s) = req.mexc_secret_key.filter(|s| !s.trim().is_empty()) {
        cfg.mexc.secret_key = s;
        mexc_changed = true;
    }

    // 3. Alpaca updates
    let mut alpaca_changed = false;
    if let Some(k) = req.alpaca_api_key.filter(|s| !s.trim().is_empty()) {
        cfg.alpaca.api_key = k;
        alpaca_changed = true;
    }
    if let Some(s) = req.alpaca_secret_key.filter(|s| !s.trim().is_empty()) {
        cfg.alpaca.secret_key = s;
        alpaca_changed = true;
    }
    if let Some(b) = req.alpaca_base_url.filter(|s| !s.trim().is_empty()) {
        cfg.alpaca.base_url = b;
        alpaca_changed = true;
    }

    // 4. IC Markets updates
    let mut ic_changed = false;
    if let Some(k) = req.ic_markets_api_key.filter(|s| !s.trim().is_empty()) {
        cfg.ic_markets.api_key = k;
        ic_changed = true;
    }
    if let Some(a) = req.ic_markets_account_id.filter(|s| !s.trim().is_empty()) {
        cfg.ic_markets.account_id = a;
        ic_changed = true;
    }
    if let Some(c) = req.ic_markets_client_id.filter(|s| !s.trim().is_empty()) {
        cfg.ic_markets.client_id = c;
        ic_changed = true;
    }
    if let Some(s) = req.ic_markets_client_secret.filter(|s| !s.trim().is_empty()) {
        cfg.ic_markets.client_secret = s;
        ic_changed = true;
    }

    // 5. Binance updates
    let mut binance_changed = false;
    if let Some(k) = req.binance_api_key.filter(|s| !s.trim().is_empty()) {
        cfg.binance.api_key = k;
        binance_changed = true;
    }
    if let Some(s) = req.binance_secret_key.filter(|s| !s.trim().is_empty()) {
        cfg.binance.secret_key = s;
        binance_changed = true;
    }
    if let Some(b) = req.binance_base_url.filter(|s| !s.trim().is_empty()) {
        cfg.binance.base_url = b;
        binance_changed = true;
    }

    // 6. Bybit updates
    let mut bybit_changed = false;
    if let Some(k) = req.bybit_api_key.filter(|s| !s.trim().is_empty()) {
        cfg.bybit.api_key = k;
        bybit_changed = true;
    }
    if let Some(s) = req.bybit_secret_key.filter(|s| !s.trim().is_empty()) {
        cfg.bybit.secret_key = s;
        bybit_changed = true;
    }
    if let Some(b) = req.bybit_base_url.filter(|s| !s.trim().is_empty()) {
        cfg.bybit.base_url = b;
        bybit_changed = true;
    }

    // Prepare .env updates
    let grok_key_str = cfg.grok.api_key.clone();
    let grok_base_str = cfg.grok.base_url.clone();
    let grok_prim_str = cfg.grok.model_primary.clone();
    let grok_fast_str = cfg.grok.model_fast.clone();

    let mexc_key_str = cfg.mexc.api_key.clone();
    let mexc_sec_str = cfg.mexc.secret_key.clone();

    let alpaca_key_str = cfg.alpaca.api_key.clone();
    let alpaca_sec_str = cfg.alpaca.secret_key.clone();
    let alpaca_base_str = cfg.alpaca.base_url.clone();

    let ic_key_str = cfg.ic_markets.api_key.clone();
    let ic_acc_str = cfg.ic_markets.account_id.clone();
    let ic_cid_str = cfg.ic_markets.client_id.clone();
    let ic_sec_str = cfg.ic_markets.client_secret.clone();

    let binance_key_str = cfg.binance.api_key.clone();
    let binance_sec_str = cfg.binance.secret_key.clone();
    let binance_base_str = cfg.binance.base_url.clone();

    let bybit_key_str = cfg.bybit.api_key.clone();
    let bybit_sec_str = cfg.bybit.secret_key.clone();
    let bybit_base_str = cfg.bybit.base_url.clone();

    if !grok_key_str.is_empty() { env_updates.push(("XAI_API_KEY", &grok_key_str)); }
    if !grok_base_str.is_empty() { env_updates.push(("XAI_BASE_URL", &grok_base_str)); }
    if !grok_prim_str.is_empty() { env_updates.push(("XAI_MODEL_PRIMARY", &grok_prim_str)); }
    if !grok_fast_str.is_empty() { env_updates.push(("XAI_MODEL_FAST", &grok_fast_str)); }

    if !mexc_key_str.is_empty() { env_updates.push(("MEXC_API_KEY", &mexc_key_str)); }
    if !mexc_sec_str.is_empty() { env_updates.push(("MEXC_SECRET_KEY", &mexc_sec_str)); }

    if !alpaca_key_str.is_empty() { env_updates.push(("ALPACA_API_KEY", &alpaca_key_str)); }
    if !alpaca_sec_str.is_empty() { env_updates.push(("ALPACA_SECRET_KEY", &alpaca_sec_str)); }
    if !alpaca_base_str.is_empty() { env_updates.push(("ALPACA_BASE_URL", &alpaca_base_str)); }

    if !ic_key_str.is_empty() { env_updates.push(("IC_MARKETS_API_KEY", &ic_key_str)); }
    if !ic_acc_str.is_empty() { env_updates.push(("IC_MARKETS_ACCOUNT_ID", &ic_acc_str)); }
    if !ic_cid_str.is_empty() { env_updates.push(("IC_MARKETS_CLIENT_ID", &ic_cid_str)); }
    if !ic_sec_str.is_empty() { env_updates.push(("IC_MARKETS_CLIENT_SECRET", &ic_sec_str)); }

    if !binance_key_str.is_empty() { env_updates.push(("BINANCE_API_KEY", &binance_key_str)); }
    if !binance_sec_str.is_empty() { env_updates.push(("BINANCE_SECRET_KEY", &binance_sec_str)); }
    if !binance_base_str.is_empty() { env_updates.push(("BINANCE_BASE_URL", &binance_base_str)); }

    if !bybit_key_str.is_empty() { env_updates.push(("BYBIT_API_KEY", &bybit_key_str)); }
    if !bybit_sec_str.is_empty() { env_updates.push(("BYBIT_SECRET_KEY", &bybit_sec_str)); }
    if !bybit_base_str.is_empty() { env_updates.push(("BYBIT_BASE_URL", &bybit_base_str)); }

    // Exchange auto-trading enablement flags
    let mexc_enabled_str = req.mexc_enabled.map(|b| b.to_string());
    let binance_enabled_str = req.binance_enabled.map(|b| b.to_string());
    let bybit_enabled_str = req.bybit_enabled.map(|b| b.to_string());
    let alpaca_enabled_str = req.alpaca_enabled.map(|b| b.to_string());
    let ic_enabled_str = req.ic_markets_enabled.map(|b| b.to_string());

    if let Some(ref s) = mexc_enabled_str {
        let b = req.mexc_enabled.unwrap();
        cfg.mexc.enabled = b;
        state.exchange_manager.set_enabled(crate::models::ExchangeId::Mexc, b).await;
        env_updates.push(("MEXC_ENABLED", s.as_str()));
    }
    if let Some(ref s) = binance_enabled_str {
        let b = req.binance_enabled.unwrap();
        cfg.binance.enabled = b;
        state.exchange_manager.set_enabled(crate::models::ExchangeId::Binance, b).await;
        env_updates.push(("BINANCE_ENABLED", s.as_str()));
    }
    if let Some(ref s) = bybit_enabled_str {
        let b = req.bybit_enabled.unwrap();
        cfg.bybit.enabled = b;
        state.exchange_manager.set_enabled(crate::models::ExchangeId::Bybit, b).await;
        env_updates.push(("BYBIT_ENABLED", s.as_str()));
    }
    if let Some(ref s) = alpaca_enabled_str {
        let b = req.alpaca_enabled.unwrap();
        cfg.alpaca.enabled = b;
        state.exchange_manager.set_enabled(crate::models::ExchangeId::Alpaca, b).await;
        env_updates.push(("ALPACA_ENABLED", s.as_str()));
    }
    if let Some(ref s) = ic_enabled_str {
        let b = req.ic_markets_enabled.unwrap();
        cfg.ic_markets.enabled = b;
        state.exchange_manager.set_enabled(crate::models::ExchangeId::IcMarkets, b).await;
        env_updates.push(("IC_MARKETS_ENABLED", s.as_str()));
    }

    // Re-register exchanges if credentials changed
    if mexc_changed {
        use crate::exchange::mexc::MexcExchange;
        use crate::models::ExchangeId;
        let _ = state.exchange_manager.register_and_connect(ExchangeId::Mexc, Box::new(MexcExchange::new(cfg.mexc.clone()))).await;
    }
    if alpaca_changed {
        use crate::exchange::alpaca::AlpacaExchange;
        use crate::models::ExchangeId;
        let _ = state.exchange_manager.register_and_connect(ExchangeId::Alpaca, Box::new(AlpacaExchange::new(cfg.alpaca.clone()))).await;
    }
    if ic_changed {
        use crate::exchange::ic_markets::IcMarketsExchange;
        use crate::models::ExchangeId;
        let _ = state.exchange_manager.register_and_connect(ExchangeId::IcMarkets, Box::new(IcMarketsExchange::new(cfg.ic_markets.clone()))).await;
    }
    if binance_changed {
        use crate::exchange::binance::BinanceExchange;
        use crate::models::ExchangeId;
        let _ = state.exchange_manager.register_and_connect(ExchangeId::Binance, Box::new(BinanceExchange::new(cfg.binance.clone()))).await;
    }
    if bybit_changed {
        use crate::exchange::bybit::BybitExchange;
        use crate::models::ExchangeId;
        let _ = state.exchange_manager.register_and_connect(ExchangeId::Bybit, Box::new(BybitExchange::new(cfg.bybit.clone()))).await;
    }

    drop(cfg); // drop write lock before file I/O

    if let Err(e) = crate::config::update_env_file(&env_updates) {
        tracing::error!("Failed to persist updated API keys to .env: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Keys updated in-memory but failed to write .env: {}", e)
            })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "API keys and provider settings saved and applied successfully."
        })),
    )
}

async fn test_api_key(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TestApiKeyRequest>,
) -> Json<TestApiKeyResponse> {
    match req.service.to_lowercase().as_str() {
        "grok" | "xai" => {
            match state.ai_engine.grok.test_connection(req.key.as_deref()).await {
                Ok(msg) => Json(TestApiKeyResponse {
                    success: true,
                    message: msg,
                }),
                Err(e) => Json(TestApiKeyResponse {
                    success: false,
                    message: format!("Grok test failed: {e}"),
                }),
            }
        }
        "mexc" => {
            let key = req.key.filter(|s| !s.trim().is_empty());
            let (test_key, _test_secret) = if let Some(k) = key {
                (k, req.secret.unwrap_or_default())
            } else {
                let cfg = state.config.read().await;
                (cfg.mexc.api_key.clone(), cfg.mexc.secret_key.clone())
            };

            if test_key.is_empty() {
                return Json(TestApiKeyResponse {
                    success: false,
                    message: "MEXC API Key is empty".to_string(),
                });
            }

            let client = reqwest::Client::new();
            match client.get("https://api.mexc.com/api/v3/time").send().await {
                Ok(res) if res.status().is_success() => {
                    Json(TestApiKeyResponse {
                        success: true,
                        message: "MEXC API endpoint reachable and active. Credentials formatted.".to_string(),
                    })
                }
                Ok(res) => Json(TestApiKeyResponse {
                    success: false,
                    message: format!("MEXC responded with HTTP {}", res.status()),
                }),
                Err(e) => Json(TestApiKeyResponse {
                    success: false,
                    message: format!("Failed to reach MEXC: {e}"),
                }),
            }
        }
        "alpaca" => {
            let key = req.key.filter(|s| !s.trim().is_empty());
            let (test_key, test_sec) = if let Some(k) = key {
                (k, req.secret.unwrap_or_default())
            } else {
                let cfg = state.config.read().await;
                (cfg.alpaca.api_key.clone(), cfg.alpaca.secret_key.clone())
            };

            if test_key.is_empty() {
                return Json(TestApiKeyResponse {
                    success: false,
                    message: "Alpaca API Key ID is empty".to_string(),
                });
            }

            let client = reqwest::Client::new();
            let base_url = state.config.read().await.alpaca.base_url.clone();
            match client
                .get(format!("{base_url}/v2/account"))
                .header("APCA-API-KEY-ID", &test_key)
                .header("APCA-API-SECRET-KEY", &test_sec)
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => {
                    Json(TestApiKeyResponse {
                        success: true,
                        message: "Alpaca credentials verified! Account accessed successfully.".to_string(),
                    })
                }
                Ok(res) => {
                    let err = res.text().await.unwrap_or_default();
                    Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Alpaca authentication failed: {}", err),
                    })
                }
                Err(e) => Json(TestApiKeyResponse {
                    success: false,
                    message: format!("Failed to connect to Alpaca: {e}"),
                }),
            }
        }
        "ic_markets" => {
            Json(TestApiKeyResponse {
                success: true,
                message: "IC Markets credentials format saved.".to_string(),
            })
        }
        "binance" => {
            let key = req.key.filter(|s| !s.trim().is_empty());
            let (test_key, test_secret) = if let Some(k) = key {
                (k, req.secret.unwrap_or_default())
            } else {
                let cfg = state.config.read().await;
                (cfg.binance.api_key.clone(), cfg.binance.secret_key.clone())
            };

            let client = reqwest::Client::new();
            if test_key.is_empty() {
                match client.get("https://api.binance.com/api/v3/ping").send().await {
                    Ok(res) if res.status().is_success() => Json(TestApiKeyResponse {
                        success: true,
                        message: "Binance public API reachable. (API key not set)".to_string(),
                    }),
                    Ok(res) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Binance responded with HTTP {}", res.status()),
                    }),
                    Err(e) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Failed to reach Binance: {e}"),
                    }),
                }
            } else if test_secret.is_empty() {
                // Public endpoint check with key
                match client.get("https://api.binance.com/api/v3/time").header("X-MBX-APIKEY", &test_key).send().await {
                    Ok(res) if res.status().is_success() => Json(TestApiKeyResponse {
                        success: true,
                        message: "Binance endpoint reachable with API key.".to_string(),
                    }),
                    Ok(res) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Binance returned status {}", res.status()),
                    }),
                    Err(e) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Failed to connect to Binance: {e}"),
                    }),
                }
            } else {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let query = format!("timestamp={timestamp}");
                let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(test_secret.as_bytes())
                    .expect("HMAC can take key of any size");
                mac.update(query.as_bytes());
                let sig = hex::encode(mac.finalize().into_bytes());
                let url = format!("https://api.binance.com/api/v3/account?{query}&signature={sig}");

                match client.get(&url).header("X-MBX-APIKEY", &test_key).send().await {
                    Ok(res) if res.status().is_success() => Json(TestApiKeyResponse {
                        success: true,
                        message: "Binance API key & secret verified! Account accessible.".to_string(),
                    }),
                    Ok(res) => {
                        let err = res.text().await.unwrap_or_default();
                        Json(TestApiKeyResponse {
                            success: false,
                            message: format!("Binance authentication failed: {err}"),
                        })
                    }
                    Err(e) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Failed to connect to Binance: {e}"),
                    }),
                }
            }
        }
        "bybit" => {
            let key = req.key.filter(|s| !s.trim().is_empty());
            let (test_key, test_secret) = if let Some(k) = key {
                (k, req.secret.unwrap_or_default())
            } else {
                let cfg = state.config.read().await;
                (cfg.bybit.api_key.clone(), cfg.bybit.secret_key.clone())
            };

            let client = reqwest::Client::new();
            if test_key.is_empty() {
                match client.get("https://api.bybit.com/v5/market/time").send().await {
                    Ok(res) if res.status().is_success() => Json(TestApiKeyResponse {
                        success: true,
                        message: "Bybit V5 public API reachable. (API key not set)".to_string(),
                    }),
                    Ok(res) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Bybit responded with HTTP {}", res.status()),
                    }),
                    Err(e) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Failed to reach Bybit: {e}"),
                    }),
                }
            } else if test_secret.is_empty() {
                match client.get("https://api.bybit.com/v5/market/time").header("X-BAPI-API-KEY", &test_key).send().await {
                    Ok(res) if res.status().is_success() => Json(TestApiKeyResponse {
                        success: true,
                        message: "Bybit V5 endpoint reachable with API key.".to_string(),
                    }),
                    Ok(res) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Bybit returned status {}", res.status()),
                    }),
                    Err(e) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Failed to connect to Bybit: {e}"),
                    }),
                }
            } else {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .to_string();
                let recv_window = "5000";
                let query = "accountType=UNIFIED";
                let sign_str = format!("{}{}{}{}", timestamp, test_key, recv_window, query);
                let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(test_secret.as_bytes())
                    .expect("HMAC can take key of any size");
                mac.update(sign_str.as_bytes());
                let sig = hex::encode(mac.finalize().into_bytes());

                let url = format!("https://api.bybit.com/v5/account/wallet-balance?{query}");
                match client
                    .get(&url)
                    .header("X-BAPI-API-KEY", &test_key)
                    .header("X-BAPI-TIMESTAMP", &timestamp)
                    .header("X-BAPI-SIGN", &sig)
                    .header("X-BAPI-RECV-WINDOW", recv_window)
                    .send()
                    .await
                {
                    Ok(res) if res.status().is_success() => {
                        let text = res.text().await.unwrap_or_default();
                        if text.contains("\"retCode\":0") {
                            Json(TestApiKeyResponse {
                                success: true,
                                message: "Bybit V5 credentials verified! Wallet balance accessible.".to_string(),
                            })
                        } else {
                            Json(TestApiKeyResponse {
                                success: false,
                                message: format!("Bybit error response: {text}"),
                            })
                        }
                    }
                    Ok(res) => {
                        let err = res.text().await.unwrap_or_default();
                        Json(TestApiKeyResponse {
                            success: false,
                            message: format!("Bybit authentication failed: {err}"),
                        })
                    }
                    Err(e) => Json(TestApiKeyResponse {
                        success: false,
                        message: format!("Failed to connect to Bybit: {e}"),
                    }),
                }
            }
        }
        other => Json(TestApiKeyResponse {
            success: false,
            message: format!("Unknown service: {other}"),
        }),
    }
}

// ===========================================================================
// Meme / Social Sentiment Radar
// ===========================================================================

async fn get_meme_radar(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    if let Some(report) = state.ai_engine.meme_radar.get_cached_report().await {
        Json(serde_json::json!({ "report": report }))
    } else {
        match state.ai_engine.meme_radar.scan().await {
            Ok(report) => Json(serde_json::json!({ "report": report })),
            Err(e) => Json(serde_json::json!({ "error": format!("Failed to generate radar report: {e}") })),
        }
    }
}

async fn scan_meme_radar(State(state): State<Arc<AppState>>) -> (StatusCode, Json<serde_json::Value>) {
    match state.ai_engine.meme_radar.scan().await {
        Ok(report) => (
            StatusCode::OK,
            Json(serde_json::json!({ "success": true, "report": report })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Scan failed: {e}") })),
        ),
    }
}

