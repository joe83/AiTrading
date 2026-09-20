use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;

use crate::exchange::Exchange;
use crate::models::ExchangeId;

/// Status of an exchange including connection and auto-trading enabled state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExchangeStatusInfo {
    pub connected: bool,
    pub enabled: bool,
}

/// Manages multiple exchange connectors, routing operations to the correct one.
pub struct ExchangeManager {
    exchanges: RwLock<HashMap<ExchangeId, Arc<RwLock<Box<dyn Exchange>>>>>,
    enabled: RwLock<HashMap<ExchangeId, bool>>,
}

impl ExchangeManager {
    pub fn new() -> Self {
        Self {
            exchanges: RwLock::new(HashMap::new()),
            enabled: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize exchange enablement states from config.
    pub async fn init_enabled(&self, config: &crate::config::AppConfig) {
        let mut map = self.enabled.write().await;
        map.insert(ExchangeId::Mexc, config.mexc.enabled);
        map.insert(ExchangeId::Alpaca, config.alpaca.enabled);
        map.insert(ExchangeId::IcMarkets, config.ic_markets.enabled);
        map.insert(ExchangeId::Binance, config.binance.enabled);
        map.insert(ExchangeId::Bybit, config.bybit.enabled);
    }

    /// Check if an exchange is enabled for automated bot trading.
    pub async fn is_enabled(&self, id: &ExchangeId) -> bool {
        *self.enabled.read().await.get(id).unwrap_or(&false)
    }

    /// Set whether an exchange is enabled for automated bot trading.
    pub async fn set_enabled(&self, id: ExchangeId, enabled: bool) {
        self.enabled.write().await.insert(id, enabled);
        tracing::info!("Exchange {} auto-trading enabled={}", id, enabled);
    }

    /// Check whether an exchange is currently connected.
    pub async fn is_connected(&self, id: &ExchangeId) -> bool {
        if let Some(exchange) = self.exchanges.read().await.get(id) {
            exchange.read().await.is_connected()
        } else {
            false
        }
    }

    /// Register an exchange connector.
    pub async fn register(&self, id: ExchangeId, exchange: Box<dyn Exchange>) {
        self.exchanges
            .write()
            .await
            .insert(id, Arc::new(RwLock::new(exchange)));
        tracing::info!("Registered exchange: {}", id);
    }

    /// Register and attempt connection immediately.
    pub async fn register_and_connect(&self, id: ExchangeId, mut exchange: Box<dyn Exchange>) -> Result<()> {
        let conn_res = exchange.connect().await;
        if let Err(ref e) = conn_res {
            tracing::warn!("Failed to connect {}: {}", id, e);
        } else {
            tracing::info!("Connected to {}", id);
        }
        self.exchanges
            .write()
            .await
            .insert(id, Arc::new(RwLock::new(exchange)));
        tracing::info!("Registered exchange: {}", id);
        conn_res
    }

    /// Get a reference to an exchange by ID.
    pub async fn get(&self, id: &ExchangeId) -> Option<Arc<RwLock<Box<dyn Exchange>>>> {
        self.exchanges.read().await.get(id).cloned()
    }

    /// Connect all registered exchanges.
    pub async fn connect_all(&self) -> Vec<(ExchangeId, Result<()>)> {
        let exchanges = self.exchanges.read().await;
        let mut results = Vec::new();

        for (&id, exchange) in exchanges.iter() {
            let result = exchange.write().await.connect().await;
            if let Err(ref e) = result {
                tracing::warn!("Failed to connect {}: {}", id, e);
            } else {
                tracing::info!("Connected to {}", id);
            }
            results.push((id, result));
        }

        results
    }

    /// Disconnect all exchanges.
    pub async fn disconnect_all(&self) {
        let exchanges = self.exchanges.read().await;
        for (&id, exchange) in exchanges.iter() {
            if let Err(e) = exchange.write().await.disconnect().await {
                tracing::warn!("Error disconnecting {}: {}", id, e);
            } else {
                tracing::info!("Disconnected from {}", id);
            }
        }
    }

    /// List all registered exchange IDs.
    pub async fn list_exchanges(&self) -> Vec<ExchangeId> {
        self.exchanges.read().await.keys().copied().collect()
    }

    /// Check connection and enabled status for all exchanges.
    pub async fn status(&self) -> HashMap<ExchangeId, ExchangeStatusInfo> {
        let exchanges = self.exchanges.read().await;
        let enabled_map = self.enabled.read().await;
        let mut status = HashMap::new();
        for (&id, exchange) in exchanges.iter() {
            let connected = exchange.read().await.is_connected();
            let enabled = *enabled_map.get(&id).unwrap_or(&false);
            status.insert(id, ExchangeStatusInfo { connected, enabled });
        }
        status
    }
}
