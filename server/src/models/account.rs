use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::ExchangeId;

/// Account balance for a single asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBalance {
    pub asset: String,
    pub free: Decimal,
    pub locked: Decimal,
}

impl AssetBalance {
    pub fn total(&self) -> Decimal {
        self.free + self.locked
    }
}

/// Full account balance across all assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub exchange: ExchangeId,
    pub assets: Vec<AssetBalance>,
    /// Total balance converted to USD equivalent.
    pub total_usd: Option<Decimal>,
}

impl Balance {
    /// Find balance for a specific asset.
    pub fn get_asset(&self, asset_name: &str) -> Option<&AssetBalance> {
        self.assets.iter().find(|a| a.asset == asset_name)
    }

    /// Get free balance for a specific asset.
    pub fn free_balance(&self, asset_name: &str) -> Decimal {
        self.get_asset(asset_name)
            .map(|a| a.free)
            .unwrap_or(Decimal::ZERO)
    }
}

/// Account information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub exchange: ExchangeId,
    pub account_type: String,
    pub can_trade: bool,
    pub can_withdraw: bool,
    pub balance: Balance,
}
