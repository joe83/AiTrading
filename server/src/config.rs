use anyhow::{Context, Result};
use std::env;

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub grok: GrokConfig,
    pub mexc: ExchangeConfig,
    pub alpaca: AlpacaConfig,
    pub ic_markets: IcMarketsConfig,
    pub binance: BinanceConfig,
    pub bybit: BybitConfig,
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub jwt: JwtConfig,
    pub auth: AuthConfig,
    pub trading: TradingConfig,
    pub watch: WatchConfig,
}

#[derive(Debug, Clone)]
pub struct GrokConfig {
    pub api_key: String,
    pub base_url: String,
    pub model_primary: String,
    pub model_fast: String,
}

#[derive(Debug, Clone)]
pub struct ExchangeConfig {
    pub api_key: String,
    pub secret_key: String,
    pub base_url: String,
    pub ws_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct BinanceConfig {
    pub api_key: String,
    pub secret_key: String,
    pub base_url: String,
    pub ws_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct BybitConfig {
    pub api_key: String,
    pub secret_key: String,
    pub base_url: String,
    pub ws_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct AlpacaConfig {
    pub api_key: String,
    pub secret_key: String,
    pub base_url: String,
    pub data_url: String,
    pub ws_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct IcMarketsConfig {
    pub api_key: String,
    pub account_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub base_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub ws_port: u16,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry_hours: u64,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct TradingConfig {
    pub mode: TradingMode,
    pub max_position_size_pct: f64,
    pub max_daily_loss_pct: f64,
    pub max_concurrent_positions: usize,
    pub default_stop_loss_pct: f64,
    pub default_take_profit_pct: f64,
    pub analysis_interval_secs: u64,
    pub sentiment_check_interval_secs: u64,
}

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub enabled: bool,
    pub interval_secs: u64,
    /// X handles without the @ sign.
    pub handles: Vec<String>,
    pub min_confidence: f64,
    /// Posts older than this are marked seen and not queued.
    pub max_post_age_secs: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TradingMode {
    Auto,
    Manual,
}

impl AppConfig {
    /// Load configuration from environment variables.
    /// Call `dotenvy::dotenv()` before this to load from .env file.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            grok: GrokConfig {
                api_key: env_var("XAI_API_KEY")?,
                base_url: env_var_or("XAI_BASE_URL", "https://api.x.ai/v1"),
                model_primary: env_var_or("XAI_MODEL_PRIMARY", "grok-4.6"),
                model_fast: env_var_or("XAI_MODEL_FAST", "grok-4.5"),
            },
            mexc: ExchangeConfig {
                api_key: env_var_or("MEXC_API_KEY", ""),
                secret_key: env_var_or("MEXC_SECRET_KEY", ""),
                base_url: env_var_or("MEXC_BASE_URL", "https://api.mexc.com"),
                ws_url: env_var_or("MEXC_WS_URL", "wss://wbs.mexc.com/ws"),
                enabled: env_var_or("MEXC_ENABLED", "false").parse().unwrap_or(false),
            },
            alpaca: AlpacaConfig {
                api_key: env_var_or("ALPACA_API_KEY", ""),
                secret_key: env_var_or("ALPACA_SECRET_KEY", ""),
                base_url: env_var_or("ALPACA_BASE_URL", "https://paper-api.alpaca.markets"),
                data_url: env_var_or("ALPACA_DATA_URL", "https://data.alpaca.markets"),
                ws_url: env_var_or("ALPACA_WS_URL", "wss://stream.data.alpaca.markets"),
                enabled: env_var_or("ALPACA_ENABLED", "false").parse().unwrap_or(false),
            },
            ic_markets: IcMarketsConfig {
                api_key: env_var_or("IC_MARKETS_API_KEY", ""),
                account_id: env_var_or("IC_MARKETS_ACCOUNT_ID", ""),
                client_id: env_var_or("IC_MARKETS_CLIENT_ID", ""),
                client_secret: env_var_or("IC_MARKETS_CLIENT_SECRET", ""),
                base_url: env_var_or("IC_MARKETS_BASE_URL", "https://openapi.ctrader.com"),
                enabled: env_var_or("IC_MARKETS_ENABLED", "false").parse().unwrap_or(false),
            },
            binance: BinanceConfig {
                api_key: env_var_or("BINANCE_API_KEY", ""),
                secret_key: env_var_or("BINANCE_SECRET_KEY", ""),
                base_url: env_var_or("BINANCE_BASE_URL", "https://api.binance.com"),
                ws_url: env_var_or("BINANCE_WS_URL", "wss://stream.binance.com:9443/ws"),
                enabled: env_var_or("BINANCE_ENABLED", "false").parse().unwrap_or(false),
            },
            bybit: BybitConfig {
                api_key: env_var_or("BYBIT_API_KEY", ""),
                secret_key: env_var_or("BYBIT_SECRET_KEY", ""),
                base_url: env_var_or("BYBIT_BASE_URL", "https://api.bybit.com"),
                ws_url: env_var_or("BYBIT_WS_URL", "wss://stream.bybit.com/v5/public/spot"),
                enabled: env_var_or("BYBIT_ENABLED", "false").parse().unwrap_or(false),
            },
            database: DatabaseConfig {
                url: env_var("DATABASE_URL")?,
                max_connections: env_var_or("DATABASE_MAX_CONNECTIONS", "10")
                    .parse()
                    .context("Invalid DATABASE_MAX_CONNECTIONS")?,
            },
            server: ServerConfig {
                host: env_var_or("SERVER_HOST", "0.0.0.0"),
                port: env_var_or("SERVER_PORT", "8080")
                    .parse()
                    .context("Invalid SERVER_PORT")?,
                ws_port: env_var_or("SERVER_WS_PORT", "8081")
                    .parse()
                    .context("Invalid SERVER_WS_PORT")?,
            },
            jwt: JwtConfig {
                secret: env_var("JWT_SECRET")?,
                expiry_hours: env_var_or("JWT_EXPIRY_HOURS", "720")
                    .parse()
                    .context("Invalid JWT_EXPIRY_HOURS")?,
            },
            auth: AuthConfig {
                username: env_var_or("ADMIN_USERNAME", "admin"),
                password_hash: env_var("ADMIN_PASSWORD_HASH")?,
            },
            trading: TradingConfig {
                mode: match env_var_or("TRADING_MODE", "manual").as_str() {
                    "auto" => TradingMode::Auto,
                    _ => TradingMode::Manual,
                },
                max_position_size_pct: env_var_or("MAX_POSITION_SIZE_PCT", "5.0")
                    .parse()
                    .context("Invalid MAX_POSITION_SIZE_PCT")?,
                max_daily_loss_pct: env_var_or("MAX_DAILY_LOSS_PCT", "3.0")
                    .parse()
                    .context("Invalid MAX_DAILY_LOSS_PCT")?,
                max_concurrent_positions: env_var_or("MAX_CONCURRENT_POSITIONS", "5")
                    .parse()
                    .context("Invalid MAX_CONCURRENT_POSITIONS")?,
                default_stop_loss_pct: env_var_or("DEFAULT_STOP_LOSS_PCT", "2.0")
                    .parse()
                    .context("Invalid DEFAULT_STOP_LOSS_PCT")?,
                default_take_profit_pct: env_var_or("DEFAULT_TAKE_PROFIT_PCT", "4.0")
                    .parse()
                    .context("Invalid DEFAULT_TAKE_PROFIT_PCT")?,
                analysis_interval_secs: env_var_or("ANALYSIS_INTERVAL_SECS", "30")
                    .parse()
                    .context("Invalid ANALYSIS_INTERVAL_SECS")?,
                sentiment_check_interval_secs: env_var_or("SENTIMENT_CHECK_INTERVAL_SECS", "300")
                    .parse()
                    .context("Invalid SENTIMENT_CHECK_INTERVAL_SECS")?,
            },
            watch: WatchConfig {
                enabled: env_var_or("WATCH_ENABLED", "true") != "false",
                interval_secs: env_var_or("WATCH_INTERVAL_SECS", "60")
                    .parse()
                    .context("Invalid WATCH_INTERVAL_SECS")?,
                handles: env_var_or("WATCH_HANDLES", "elonmusk,realDonaldTrump")
                    .split(',')
                    .map(|handle| handle.trim().trim_start_matches('@').to_string())
                    .filter(|handle| !handle.is_empty())
                    .take(20)
                    .collect(),
                min_confidence: env_var_or("WATCH_MIN_CONFIDENCE", "0.7")
                    .parse()
                    .context("Invalid WATCH_MIN_CONFIDENCE")?,
                max_post_age_secs: env_var_or("WATCH_MAX_POST_AGE_SECS", "600")
                    .parse()
                    .context("Invalid WATCH_MAX_POST_AGE_SECS")?,
            },
        })
    }
}

/// Get required environment variable.
fn env_var(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("Missing required environment variable: {key}"))
}

/// Get environment variable with default value.
fn env_var_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Update environment variables in .env file(s) and in the current process.
pub fn update_env_file(updates: &[(&str, &str)]) -> Result<()> {
    use std::fs;
    use std::path::Path;

    // 1. Set variables in the current running process
    for &(key, val) in updates {
        env::set_var(key, val);
    }

    // 2. Identify potential .env files
    let candidates = ["/app/.env", "./server/.env", ".env", "../.env"];
    let mut updated_any = false;

    for path_str in &candidates {
        let path = Path::new(path_str);
        if path.exists() {
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            let mut keys_found: std::collections::HashSet<String> = std::collections::HashSet::new();

            for line in lines.iter_mut() {
                let trimmed = line.trim().to_string();
                for &(key, val) in updates {
                    if trimmed.starts_with(&format!("{key}=")) {
                        *line = format!("{key}={val}");
                        keys_found.insert(key.to_string());
                    }
                }
            }

            for &(key, val) in updates {
                if !keys_found.contains(key) {
                    lines.push(format!("{key}={val}"));
                }
            }

            let new_content = lines.join("\n") + "\n";
            if let Err(e) = fs::write(path, new_content) {
                tracing::warn!("Failed to write to {}: {}", path_str, e);
            } else {
                tracing::info!("Successfully updated .env file at {}", path_str);
                updated_any = true;
            }
        }
    }

    if !updated_any {
        let fallback = if Path::new("/app").exists() { "/app/.env" } else { ".env" };
        let mut content = String::new();
        for &(key, val) in updates {
            content.push_str(&format!("{key}={val}\n"));
        }
        let _ = fs::write(fallback, content);
        tracing::info!("Created fallback .env file at {}", fallback);
    }

    Ok(())
}

