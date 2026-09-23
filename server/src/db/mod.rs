mod journal;

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

use crate::config::DatabaseConfig;

pub use journal::{
    format_playbook, normalize_scope, PerformanceFigures, PlaybookEntry, PlaybookRule, PromoteLesson,
    TradeInsert, TradeRow,
};

/// Database connection pool and query helpers.
pub struct Database {
    pub pool: PgPool,
}

impl Database {
    /// Create a new database connection pool.
    pub async fn connect(config: &DatabaseConfig) -> Result<Self> {
        info!("Connecting to database...");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect(&config.url)
            .await?;

        info!("Database connected (max {} connections)", config.max_connections);
        Ok(Self { pool })
    }

    /// Run database migrations.
    pub async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations...");
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await?;
        info!("Migrations complete");
        Ok(())
    }
}
