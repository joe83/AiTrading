use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;

use crate::exchange::Exchange;
use crate::models::ExchangeId;

/// Manages multiple exchange connectors, routing operations to the correct one.
pub struct ExchangeManager {
    exchanges: RwLock<HashMap<ExchangeId, Arc<RwLock<Box<dyn Exchange>>>>>,
}

impl ExchangeManager {
    pub fn new() -> Self {
        Self {
            exchanges: RwLock::new(HashMap::new()),
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

    /// Check connection status for all exchanges.
    pub async fn status(&self) -> HashMap<ExchangeId, bool> {
        let exchanges = self.exchanges.read().await;
        let mut status = HashMap::new();
        for (&id, exchange) in exchanges.iter() {
            let connected = exchange.read().await.is_connected();
            status.insert(id, connected);
        }
        status
    }
}
