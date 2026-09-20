use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::path::Path;
use tracing::{info, warn};

use crate::models::*;

/// Load historical candle data from various sources.
pub struct DataLoader;

impl DataLoader {
    /// Fetch historical candles from MEXC public API with automatic pagination.
    /// MEXC limits to 1000 candles per request, so we paginate backwards.
    pub async fn fetch_mexc_candles(
        symbol: &str,
        timeframe: &Timeframe,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Candle>> {
        let http_client = reqwest::Client::new();
        let base_url = "https://api.mexc.com";
        let interval = Self::timeframe_to_mexc_interval(timeframe);

        let mut all_candles: Vec<Candle> = Vec::new();
        let mut current_start = start_time.timestamp_millis();
        let end_ms = end_time.timestamp_millis();
        let limit = 1000;

        info!(
            "Fetching MEXC candles: {} {} from {} to {}",
            symbol, timeframe, start_time, end_time
        );

        loop {
            if current_start >= end_ms {
                break;
            }

            let url = format!(
                "{}/api/v3/klines?symbol={}&interval={}&startTime={}&endTime={}&limit={}",
                base_url, symbol, interval, current_start, end_ms, limit
            );

            let response = http_client
                .get(&url)
                .send()
                .await
                .context("Failed to fetch MEXC klines")?;

            let body: serde_json::Value = response
                .json()
                .await
                .context("Failed to parse MEXC klines response")?;

            let klines = body
                .as_array()
                .context("Expected array of klines")?;

            if klines.is_empty() {
                break;
            }

            for k in klines {
                let arr = match k.as_array() {
                    Some(a) => a,
                    None => continue,
                };

                if arr.len() < 7 {
                    continue;
                }

                let open_time_ms = arr[0].as_i64().unwrap_or(0);
                let close_time_ms = arr[6].as_i64().unwrap_or(0);

                if let (Some(ot), Some(ct)) = (
                    DateTime::from_timestamp(open_time_ms / 1000, 0),
                    DateTime::from_timestamp(close_time_ms / 1000, 0),
                ) {
                    let candle = Candle {
                        symbol: symbol.to_string(),
                        exchange: ExchangeId::Mexc,
                        timeframe: timeframe.to_string(),
                        open_time: ot,
                        close_time: ct,
                        open: Self::parse_decimal(&arr[1]).unwrap_or(Decimal::ZERO),
                        high: Self::parse_decimal(&arr[2]).unwrap_or(Decimal::ZERO),
                        low: Self::parse_decimal(&arr[3]).unwrap_or(Decimal::ZERO),
                        close: Self::parse_decimal(&arr[4]).unwrap_or(Decimal::ZERO),
                        volume: Self::parse_decimal(&arr[5]).unwrap_or(Decimal::ZERO),
                    };
                    all_candles.push(candle);
                }

                // Move start forward past the last candle
                current_start = close_time_ms + 1;
            }

            // If we got fewer than limit, we've reached the end
            if klines.len() < limit {
                break;
            }

            // Rate limit: small delay between requests
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }

        // Sort by open_time ascending
        all_candles.sort_by_key(|c| c.open_time);

        // Deduplicate by open_time
        all_candles.dedup_by_key(|c| c.open_time);

        info!(
            "Loaded {} candles for {} ({})",
            all_candles.len(),
            symbol,
            timeframe
        );

        // Validate for gaps
        Self::detect_gaps(&all_candles, timeframe);

        Ok(all_candles)
    }

    /// Load candles from a CSV file.
    /// Expected CSV format: timestamp,open,high,low,close,volume
    /// Timestamp can be Unix seconds or ISO 8601.
    pub fn load_csv(
        path: &Path,
        symbol: &str,
        exchange: ExchangeId,
        timeframe: &Timeframe,
    ) -> Result<Vec<Candle>> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_path(path)
            .context("Failed to open CSV file")?;

        let mut candles = Vec::new();

        for result in reader.records() {
            let record = result.context("Failed to read CSV record")?;

            if record.len() < 6 {
                warn!("Skipping CSV row with insufficient columns: {:?}", record);
                continue;
            }

            // Parse timestamp (try Unix seconds first, then ISO 8601)
            let timestamp_str = &record[0];
            let open_time = if let Ok(ts) = timestamp_str.parse::<i64>() {
                DateTime::from_timestamp(ts, 0)
            } else {
                timestamp_str.parse::<DateTime<Utc>>().ok()
            };

            let open_time = match open_time {
                Some(t) => t,
                None => {
                    warn!("Skipping row with unparseable timestamp: {}", timestamp_str);
                    continue;
                }
            };

            let close_time = open_time
                + chrono::Duration::seconds(timeframe.as_secs() as i64);

            let candle = Candle {
                symbol: symbol.to_string(),
                exchange,
                timeframe: timeframe.to_string(),
                open_time,
                close_time,
                open: record[1].parse().unwrap_or(Decimal::ZERO),
                high: record[2].parse().unwrap_or(Decimal::ZERO),
                low: record[3].parse().unwrap_or(Decimal::ZERO),
                close: record[4].parse().unwrap_or(Decimal::ZERO),
                volume: record[5].parse().unwrap_or(Decimal::ZERO),
            };

            candles.push(candle);
        }

        candles.sort_by_key(|c| c.open_time);

        info!(
            "Loaded {} candles from CSV: {}",
            candles.len(),
            path.display()
        );

        Ok(candles)
    }

    /// Detect gaps in candle data and log warnings.
    fn detect_gaps(candles: &[Candle], timeframe: &Timeframe) {
        if candles.len() < 2 {
            return;
        }

        let expected_gap_secs = timeframe.as_secs() as i64;
        let mut gap_count = 0;

        for window in candles.windows(2) {
            let diff = (window[1].open_time - window[0].open_time).num_seconds();
            // Allow up to 2x the expected gap (some tolerance for market closures)
            if diff > expected_gap_secs * 2 {
                gap_count += 1;
            }
        }

        if gap_count > 0 {
            warn!(
                "Detected {} gaps in candle data (expected interval: {}s)",
                gap_count, expected_gap_secs
            );
        }
    }

    /// Parse a JSON value to Decimal (handles both string and number formats).
    fn parse_decimal(value: &serde_json::Value) -> Option<Decimal> {
        if let Some(s) = value.as_str() {
            s.parse().ok()
        } else if let Some(f) = value.as_f64() {
            Decimal::try_from(f).ok()
        } else {
            None
        }
    }

    /// Convert Timeframe to MEXC interval string.
    fn timeframe_to_mexc_interval(tf: &Timeframe) -> &'static str {
        match tf {
            Timeframe::Min1 => "1m",
            Timeframe::Min5 => "5m",
            Timeframe::Min15 => "15m",
            Timeframe::Min30 => "30m",
            Timeframe::Hour1 => "60m",
            Timeframe::Hour4 => "4h",
            Timeframe::Day1 => "1d",
            Timeframe::Week1 => "1W",
        }
    }
}
