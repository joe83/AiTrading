#![allow(dead_code, unused_imports)]

mod ai;
mod api;
mod auth;
mod backtest;
mod config;
mod db;
mod exchange;
mod execution;
mod models;
mod risk;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info};

use crate::ai::AiEngine;
use crate::api::websocket::WsBroadcast;
use crate::backtest::BacktestResult;
use crate::config::AppConfig;
use crate::db::Database;
use crate::exchange::manager::ExchangeManager;
use crate::execution::ExecutionEngine;
use crate::models::*;
use crate::risk::RiskManager;

/// Shared application state accessible by all components.
pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub ai_engine: AiEngine,
    pub risk_manager: Arc<RiskManager>,
    pub execution_engine: ExecutionEngine,
    pub exchange_manager: Arc<ExchangeManager>,
    pub db: Arc<Database>,
    /// Broadcast channel for WebSocket clients.
    pub ws_broadcast: broadcast::Sender<WsBroadcast>,
    /// Latest analysis results per symbol.
    pub latest_analyses: RwLock<HashMap<String, AnalysisResult>>,
    /// Signal history.
    pub signal_history: RwLock<Vec<TradingSignal>>,
    /// Latest X watcher status for the dashboard.
    pub watch_status: RwLock<crate::ai::watch_loop::WatchStatus>,
    /// Backtest results history.
    pub backtest_results: RwLock<Vec<BacktestResult>>,
    /// Server start time.
    pub start_time: Instant,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,ai_trading_server=debug".into()),
        )
        .with_target(true)
        .with_thread_ids(true)
        .init();

    info!("🚀 AI Trading Platform starting...");
    info!("═══════════════════════════════════════════════════════════");

    // Load configuration
    let config = AppConfig::from_env()?;
    info!("✅ Configuration loaded");
    info!("   Trading mode: {:?}", config.trading.mode);
    info!("   Grok model: {}", config.grok.model_primary);
    info!("   Max position size: {}%", config.trading.max_position_size_pct);
    info!("   Max daily loss: {}%", config.trading.max_daily_loss_pct);

    let database = Arc::new(
        Database::connect(&config.database)
            .await
            .context("Failed to connect to the trading database")?,
    );
    database
        .run_migrations()
        .await
        .context("Database migrations failed")?;
    info!("✅ Database ready");

    // Initialize risk manager
    let risk_manager = Arc::new(RiskManager::new(&config.trading));
    info!("✅ Risk manager initialized");

    // Initialize execution engine
    let execution_engine = ExecutionEngine::new(risk_manager.clone(), &config.trading, database.clone());
    info!("✅ Execution engine initialized");

    // Initialize AI engine
    let ai_engine = AiEngine::new(&config, database.clone());
    info!("✅ AI engine initialized (Grok API ready)");

    // Initialize exchange manager
    let exchange_manager = Arc::new(ExchangeManager::new());
    exchange_manager.init_enabled(&config).await;
    execution_engine.set_exchange_manager(exchange_manager.clone()).await;

    // Register exchanges
    {
        use crate::exchange::mexc::MexcExchange;
        use crate::exchange::alpaca::AlpacaExchange;
        use crate::exchange::ic_markets::IcMarketsExchange;
        use crate::exchange::binance::BinanceExchange;
        use crate::exchange::bybit::BybitExchange;

        exchange_manager
            .register(ExchangeId::Mexc, Box::new(MexcExchange::new(config.mexc.clone())))
            .await;
        exchange_manager
            .register(ExchangeId::Alpaca, Box::new(AlpacaExchange::new(config.alpaca.clone())))
            .await;
        exchange_manager
            .register(ExchangeId::IcMarkets, Box::new(IcMarketsExchange::new(config.ic_markets.clone())))
            .await;
        exchange_manager
            .register(ExchangeId::Binance, Box::new(BinanceExchange::new(config.binance.clone())))
            .await;
        exchange_manager
            .register(ExchangeId::Bybit, Box::new(BybitExchange::new(config.bybit.clone())))
            .await;

        info!("✅ Exchange manager initialized (MEXC, Alpaca, IC Markets, Binance, Bybit)");

        // Attempt to connect exchanges (non-fatal — missing keys are okay)
        let results = exchange_manager.connect_all().await;
        for (id, result) in &results {
            let is_enabled = exchange_manager.is_enabled(id).await;
            match result {
                Ok(()) => info!("   ✅ {} connected (auto-trading: {})", id, if is_enabled { "ENABLED" } else { "DISABLED" }),
                Err(e) => info!("   ⚠️  {} skipped ({})", id, e),
            }
        }
    }

    // WebSocket broadcast channel
    let (ws_broadcast, _) = broadcast::channel::<WsBroadcast>(1000);

    // Build shared application state
    let state = Arc::new(AppState {
        config: RwLock::new(config.clone()),
        ai_engine,
        risk_manager: risk_manager.clone(),
        execution_engine,
        exchange_manager: exchange_manager.clone(),
        db: database,
        ws_broadcast: ws_broadcast.clone(),
        latest_analyses: RwLock::new(HashMap::new()),
        signal_history: RwLock::new(Vec::new()),
        watch_status: RwLock::new(crate::ai::watch_loop::WatchStatus::default()),
        backtest_results: RwLock::new(Vec::new()),
        start_time: Instant::now(),
    });

    info!("═══════════════════════════════════════════════════════════");

    // Start REST API server
    let api_state = state.clone();
    let api_addr = format!("{}:{}", config.server.host, config.server.port);
    let api_router = api::create_router(api_state);

    info!("📡 REST API server starting on http://{api_addr}");

    let api_listener = TcpListener::bind(&api_addr).await?;
    let api_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(api_listener, api_router).await {
            error!("REST API server error: {e}");
        }
    });

    // Start WebSocket server
    let ws_state = state.clone();
    let ws_addr = format!("{}:{}", config.server.host, config.server.ws_port);
    let ws_router = api::websocket::create_ws_router(ws_state);

    info!("🔌 WebSocket server starting on ws://{ws_addr}");

    let ws_listener = TcpListener::bind(&ws_addr).await?;
    let ws_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(ws_listener, ws_router).await {
            error!("WebSocket server error: {e}");
        }
    });

    info!("═══════════════════════════════════════════════════════════");
    info!("🟢 AI Trading Platform is running!");
    info!("   REST API:   http://{}", format!("{}:{}", config.server.host, config.server.port));
    info!("   WebSocket:  ws://{}", format!("{}:{}", config.server.host, config.server.ws_port));
    info!("   Health:     http://{}:{}/api/health", config.server.host, config.server.port);
    info!("═══════════════════════════════════════════════════════════");

    let watch_state = state.clone();
    let watch_handle = tokio::spawn(async move {
        crate::ai::watch_loop::run(watch_state).await;
    });

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("🛑 Shutdown signal received. Cleaning up...");

    // Graceful shutdown
    watch_handle.abort();
    api_handle.abort();
    ws_handle.abort();

    info!("👋 AI Trading Platform stopped.");
    Ok(())
}
