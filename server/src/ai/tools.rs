use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::backtest::{BacktestConfig, BacktestEngine, BacktestResult};
use crate::db::Database;
use crate::models::{ExchangeId, Timeframe};

use super::AiEngine;

const MAX_LIST: i64 = 50;
const MAX_BACKTEST_DAYS: i64 = 120;
const MAX_AGENT_AI_CALLS: usize = 10;
const BACKTEST_TIMEOUT: Duration = Duration::from_secs(120);

pub struct ToolCtx<'a> {
    pub db: &'a Database,
    pub ai: &'a AiEngine,
    pub backtests: &'a RwLock<Vec<BacktestResult>>,
}

pub fn definitions() -> Vec<crate::ai::grok_client::Tool> {
    vec![
        tool(
            "list_trades",
            "List stored trades. Dates filter the close time when the trade is closed, otherwise the open time. Returns at most 50 rows.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Market symbol, for example BTCUSDT"},
                    "from": {"type": "string", "description": "Inclusive start, YYYY-MM-DD or RFC3339"},
                    "to": {"type": "string", "description": "Inclusive end date, YYYY-MM-DD or RFC3339"},
                    "only_closed": {"type": "boolean", "description": "When true, only closed trades. Default true."},
                    "limit": {"type": "integer", "description": "1 to 50. Default 20."}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "get_trade",
            "Load one trade plus the signal reasoning and indicator snapshot that produced it.",
            json!({
                "type": "object",
                "properties": {
                    "trade_id": {"type": "string", "description": "Trade UUID"}
                },
                "required": ["trade_id"],
                "additionalProperties": false
            }),
        ),
        tool(
            "get_performance",
            "Win rate, average win, average loss, profit factor, and total realized P&L for closed trades. Without a symbol, also returns the top symbols.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "from": {"type": "string", "description": "Inclusive start, YYYY-MM-DD or RFC3339"},
                    "to": {"type": "string", "description": "Inclusive end, YYYY-MM-DD or RFC3339"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "list_lessons",
            "List saved lessons. A hypothesis is not a live trading rule.",
            json!({
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "description": "Symbol or global"},
                    "status": {"type": "string", "enum": ["hypothesis", "supported", "rejected"]},
                    "limit": {"type": "integer"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "save_lesson",
            "Save a hypothesis tied to trade ids. This does not change live trading or the playbook. Approval is a separate step.",
            json!({
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "description": "Symbol or global"},
                    "claim": {"type": "string", "description": "What the trades show, in one or two sentences"},
                    "evidence_trade_ids": {"type": "array", "items": {"type": "string"}},
                    "sample_size": {"type": "integer", "description": "Closed trades this claim is based on. Defaults to the evidence count."}
                },
                "required": ["scope", "claim"],
                "additionalProperties": false
            }),
        ),
        tool(
            "list_playbook",
            "List rules the decision prompt is currently allowed to use.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "When set, return that symbol plus global rules"}
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "run_backtest",
            "Replay a strategy on historical candles. Date range is at most 120 days. use_grok defaults to false and is capped at 10 model calls. This does not place a live order.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "exchange": {"type": "string", "enum": ["mexc", "binance", "bybit", "alpaca", "ic_markets"]},
                    "timeframe": {"type": "string", "enum": ["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w"]},
                    "start_time": {"type": "string"},
                    "end_time": {"type": "string"},
                    "initial_balance": {"type": "number"},
                    "use_grok": {"type": "boolean"},
                    "max_ai_calls": {"type": "integer"}
                },
                "required": ["symbol", "timeframe", "start_time", "end_time"],
                "additionalProperties": false
            }),
        ),
    ]
}

pub fn tool_names() -> Vec<&'static str> {
    definitions().into_iter().map(|tool| match tool.function.name.as_str() {
        "list_trades" => "list_trades",
        "get_trade" => "get_trade",
        "get_performance" => "get_performance",
        "list_lessons" => "list_lessons",
        "save_lesson" => "save_lesson",
        "list_playbook" => "list_playbook",
        "run_backtest" => "run_backtest",
        _ => "unknown",
    }).collect()
}

pub async fn execute(name: &str, arguments: &str, ctx: &ToolCtx<'_>) -> Value {
    let args = match parse_args(arguments) {
        Ok(value) => value,
        Err(error) => return json!({ "error": error }),
    };
    let result = match name {
        "list_trades" => list_trades(&args, ctx).await,
        "get_trade" => get_trade(&args, ctx).await,
        "get_performance" => get_performance(&args, ctx).await,
        "list_lessons" => list_lessons(&args, ctx).await,
        "save_lesson" => save_lesson(&args, ctx).await,
        "list_playbook" => list_playbook(&args, ctx).await,
        "run_backtest" => run_backtest(&args, ctx).await,
        _ => Err(format!("unknown tool {name}")),
    };
    match result {
        Ok(value) => value,
        Err(error) => json!({ "error": error }),
    }
}

pub fn summarize(name: &str, value: &Value) -> String {
    if let Some(error) = value.get("error").and_then(|item| item.as_str()) {
        return error.to_string();
    }
    match name {
        "list_trades" => format!(
            "{} trades",
            value.get("trades").and_then(|item| item.as_array()).map(|rows| rows.len()).unwrap_or(0)
        ),
        "get_trade" => value.get("symbol").and_then(|item| item.as_str()).unwrap_or("trade").to_string(),
        "get_performance" => {
            let trades = value.get("trades").and_then(|item| item.as_i64()).unwrap_or(0);
            let win_rate = value.get("win_rate_pct").and_then(|item| item.as_f64()).unwrap_or(0.0);
            format!("{trades} closed trades, win rate {win_rate:.1}%")
        }
        "list_lessons" => format!(
            "{} lessons",
            value.get("lessons").and_then(|item| item.as_array()).map(|rows| rows.len()).unwrap_or(0)
        ),
        "save_lesson" => "lesson saved as a hypothesis".to_string(),
        "list_playbook" => format!(
            "{} active rules",
            value.get("rules").and_then(|item| item.as_array()).map(|rows| rows.len()).unwrap_or(0)
        ),
        "run_backtest" => {
            let ret = value.get("total_return_pct").and_then(|item| item.as_f64()).unwrap_or(0.0);
            format!("return {ret:.2}%")
        }
        _ => "done".to_string(),
    }
}

fn tool(name: &str, description: &str, parameters: Value) -> crate::ai::grok_client::Tool {
    crate::ai::grok_client::Tool {
        tool_type: "function".to_string(),
        function: crate::ai::grok_client::FunctionDef {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        },
    }
}

fn parse_args(arguments: &str) -> Result<Value, String> {
    let raw = arguments.trim();
    if raw.is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => Ok(Value::Object(map)),
        Ok(_) => Err("arguments must be a JSON object".to_string()),
        Err(error) => Err(format!("invalid arguments: {error}")),
    }
}

pub fn clamp_limit(value: Option<i64>) -> i64 {
    value.unwrap_or(20).clamp(1, MAX_LIST)
}

/// `inclusive_end` turns a date-only value into the next midnight so the named day is included.
pub fn parse_time_bound(raw: Option<&str>, inclusive_end: bool) -> Result<Option<DateTime<Utc>>, String> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
        return Ok(Some(parsed.with_timezone(&Utc)));
    }
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| format!("invalid date {raw}; use YYYY-MM-DD or RFC3339"))?;
    let start = date
        .and_hms_opt(0, 0, 0)
        .map(|naive| Utc.from_utc_datetime(&naive))
        .ok_or_else(|| format!("invalid date {raw}"))?;
    if inclusive_end {
        Ok(Some(start + chrono::Duration::days(1)))
    } else {
        Ok(Some(start))
    }
}

pub fn backtest_window_error(start: DateTime<Utc>, end: DateTime<Utc>) -> Option<String> {
    if end <= start {
        return Some("end_time must be after start_time".to_string());
    }
    if end - start > chrono::Duration::days(MAX_BACKTEST_DAYS) {
        return Some(format!("date range must be at most {MAX_BACKTEST_DAYS} days"));
    }
    None
}

async fn list_trades(args: &Value, ctx: &ToolCtx<'_>) -> Result<Value, String> {
    let from = parse_time_bound(arg_str(args, "from"), false)?;
    let to = parse_time_bound(arg_str(args, "to"), true)?;
    let only_closed = arg_bool(args, "only_closed").unwrap_or(true);
    let limit = clamp_limit(arg_i64(args, "limit"));
    let rows = ctx
        .db
        .list_trades(arg_str(args, "symbol"), from, to, only_closed, limit)
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "trades": rows.iter().map(trade_json).collect::<Vec<_>>(),
    }))
}

async fn get_trade(args: &Value, ctx: &ToolCtx<'_>) -> Result<Value, String> {
    let id = required_uuid(args, "trade_id")?;
    let detail = ctx.db.get_trade(id).await.map_err(|error| error.to_string())?;
    let Some(detail) = detail else {
        return Err(format!("trade {id} was not found"));
    };
    let mut value = trade_json(&detail.trade);
    if let Some(object) = value.as_object_mut() {
        object.insert("signal_action".to_string(), json!(detail.signal_action));
        object.insert("signal_timeframe".to_string(), json!(detail.signal_timeframe));
        object.insert("reasoning".to_string(), json!(detail.signal_reasoning));
        object.insert("indicators".to_string(), json!(detail.indicators));
    }
    Ok(value)
}

async fn get_performance(args: &Value, ctx: &ToolCtx<'_>) -> Result<Value, String> {
    let from = parse_time_bound(arg_str(args, "from"), false)?;
    let to = parse_time_bound(arg_str(args, "to"), true)?;
    let report = ctx
        .db
        .trade_performance(arg_str(args, "symbol"), from, to)
        .await
        .map_err(|error| error.to_string())?;
    let mut value = figures_json(&report.figures);
    if !report.by_symbol.is_empty() {
        value["by_symbol"] = json!(report.by_symbol.iter().map(|row| {
            let mut item = figures_json(&row.figures);
            item["symbol"] = json!(row.symbol);
            item
        }).collect::<Vec<_>>());
    }
    Ok(value)
}

async fn list_lessons(args: &Value, ctx: &ToolCtx<'_>) -> Result<Value, String> {
    let status = arg_str(args, "status");
    if let Some(status) = status {
        if !matches!(status, "hypothesis" | "supported" | "rejected") {
            return Err("status must be hypothesis, supported, or rejected".to_string());
        }
    }
    let rows = ctx
        .db
        .list_lessons(arg_str(args, "scope"), status, clamp_limit(arg_i64(args, "limit")))
        .await
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "lessons": rows.iter().map(|row| json!({
            "id": row.id,
            "scope": row.scope,
            "claim": row.claim,
            "evidence_trade_ids": row.evidence_trade_ids,
            "sample_size": row.sample_size,
            "status": row.status,
            "created_at": row.created_at,
        })).collect::<Vec<_>>(),
    }))
}

async fn save_lesson(args: &Value, ctx: &ToolCtx<'_>) -> Result<Value, String> {
    let scope = arg_str(args, "scope").ok_or("scope is required")?;
    let claim = arg_str(args, "claim").ok_or("claim is required")?;
    if scope.chars().count() > 64 {
        return Err("scope must be 1-64 characters".to_string());
    }
    if claim.chars().count() > 2000 {
        return Err("claim must be 1-2000 characters".to_string());
    }
    let evidence = arg_uuids(args, "evidence_trade_ids")?;
    let sample_size = match arg_i64(args, "sample_size") {
        Some(value) if value < 0 => return Err("sample_size must be zero or greater".to_string()),
        Some(value) => i32::try_from(value).unwrap_or(i32::MAX),
        None => i32::try_from(evidence.len()).unwrap_or(i32::MAX),
    };
    let id = ctx
        .db
        .save_lesson(scope, claim, sample_size, &evidence)
        .await
        .map_err(|error| error.to_string())?;
    info!("Agent saved lesson hypothesis {id} for {scope}");
    Ok(json!({
        "id": id,
        "scope": crate::db::normalize_scope(scope),
        "claim": claim,
        "status": "hypothesis",
        "sample_size": sample_size,
        "note": "Saved as a hypothesis. It is not in the playbook and does not affect the next trade.",
    }))
}

async fn list_playbook(args: &Value, ctx: &ToolCtx<'_>) -> Result<Value, String> {
    let rows = ctx.db.list_playbook().await.map_err(|error| error.to_string())?;
    let symbol = arg_str(args, "symbol").map(crate::db::normalize_scope);
    let rules: Vec<_> = rows
        .into_iter()
        .filter(|row| match &symbol {
            Some(scope) => row.scope == "global" || row.scope == *scope,
            None => true,
        })
        .map(|row| json!({
            "id": row.id,
            "scope": row.scope,
            "rule": row.rule,
            "sample_size": row.sample_size,
            "created_at": row.created_at,
        }))
        .collect();
    Ok(json!({ "rules": rules }))
}

async fn run_backtest(args: &Value, ctx: &ToolCtx<'_>) -> Result<Value, String> {
    let symbol = arg_str(args, "symbol").ok_or("symbol is required")?.to_string();
    let exchange = parse_exchange(arg_str(args, "exchange").unwrap_or("mexc"))?;
    let timeframe = parse_timeframe(arg_str(args, "timeframe").ok_or("timeframe is required")?)?;
    let start = parse_time_bound(arg_str(args, "start_time"), false)?.ok_or("start_time is required")?;
    let end = parse_time_bound(arg_str(args, "end_time"), true)?.ok_or("end_time is required")?;
    if let Some(error) = backtest_window_error(start, end) {
        return Err(error);
    }
    let initial_balance = args.get("initial_balance").and_then(|value| value.as_f64()).unwrap_or(10_000.0);
    if !(100.0..=1_000_000.0).contains(&initial_balance) {
        return Err("initial_balance must be between 100 and 1000000".to_string());
    }
    let use_grok = arg_bool(args, "use_grok").unwrap_or(false);
    let requested_calls = arg_i64(args, "max_ai_calls").unwrap_or(MAX_AGENT_AI_CALLS as i64);
    let max_ai_calls = usize::try_from(requested_calls.max(0)).unwrap_or(MAX_AGENT_AI_CALLS).min(MAX_AGENT_AI_CALLS);

    let config = BacktestConfig {
        symbol: symbol.clone(),
        exchange,
        timeframe,
        start_time: start,
        end_time: end,
        initial_balance,
        maker_fee: None,
        taker_fee: None,
        slippage: None,
        allow_short: None,
        csv_path: None,
        warmup_bars: None,
        use_grok: Some(use_grok),
        max_ai_calls: Some(max_ai_calls),
    };

    info!("Agent backtest {symbol} {timeframe:?} use_grok={use_grok} calls<={max_ai_calls}");
    let result = match tokio::time::timeout(BACKTEST_TIMEOUT, BacktestEngine::run(config, ctx.ai)).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => return Err(error.to_string()),
        Err(_) => return Err("backtest exceeded 120 seconds".to_string()),
    };
    let summary = backtest_summary(&result);
    ctx.backtests.write().await.push(result);
    Ok(summary)
}

fn backtest_summary(result: &BacktestResult) -> Value {
    json!({
        "id": result.id,
        "symbol": result.config.symbol,
        "exchange": result.config.exchange.as_db_str(),
        "timeframe": timeframe_label(result.config.timeframe),
        "initial_balance": result.initial_balance,
        "final_balance": result.final_balance,
        "total_return_pct": result.metrics.total_return_pct,
        "max_drawdown_pct": result.metrics.max_drawdown_pct,
        "win_rate_pct": result.metrics.win_rate_pct,
        "profit_factor": result.metrics.profit_factor,
        "total_trades": result.metrics.total_trades,
        "winning_trades": result.metrics.winning_trades,
        "losing_trades": result.metrics.losing_trades,
        "use_grok": result.use_grok,
        "ai_calls_made": result.ai_calls_made,
        "duration_secs": result.duration_secs,
    })
}

fn trade_json(trade: &crate::db::TradeRow) -> Value {
    json!({
        "id": trade.id,
        "exchange": trade.exchange,
        "symbol": trade.symbol,
        "side": trade.side,
        "quantity": trade.quantity.to_string(),
        "entry_price": trade.entry_price.to_string(),
        "exit_price": trade.exit_price.map(|price| price.to_string()),
        "fees": trade.fees.to_string(),
        "realized_pnl": trade.realized_pnl.map(|pnl| pnl.to_string()),
        "signal_id": trade.signal_id,
        "signal_confidence": trade.signal_confidence,
        "opened_at": trade.opened_at,
        "closed_at": trade.closed_at,
        "close_reason": trade.close_reason,
    })
}

fn figures_json(figures: &crate::db::PerformanceFigures) -> Value {
    json!({
        "trades": figures.trades,
        "wins": figures.wins,
        "losses": figures.losses,
        "win_rate_pct": figures.win_rate_pct,
        "total_pnl": figures.total_pnl.to_string(),
        "average_win": figures.average_win.to_string(),
        "average_loss": figures.average_loss.to_string(),
        "profit_factor": figures.profit_factor,
    })
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty())
}

fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|value| value.as_bool())
}

fn arg_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|value| value.as_i64().or_else(|| value.as_f64().map(|number| number as i64)))
}

fn required_uuid(args: &Value, key: &str) -> Result<Uuid, String> {
    let raw = arg_str(args, key).ok_or_else(|| format!("{key} is required"))?;
    Uuid::parse_str(raw).map_err(|_| format!("{key} must be a UUID"))
}

fn arg_uuids(args: &Value, key: &str) -> Result<Vec<Uuid>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|error| format!("{key} must be an array of trade ids: {error}")),
    }
}

fn parse_exchange(raw: &str) -> Result<ExchangeId, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "mexc" => Ok(ExchangeId::Mexc),
        "alpaca" => Ok(ExchangeId::Alpaca),
        "ic_markets" | "icmarkets" => Ok(ExchangeId::IcMarkets),
        "binance" => Ok(ExchangeId::Binance),
        "bybit" => Ok(ExchangeId::Bybit),
        other => Err(format!("unknown exchange {other}")),
    }
}

fn parse_timeframe(raw: &str) -> Result<Timeframe, String> {
    match raw.trim() {
        "1m" => Ok(Timeframe::Min1),
        "5m" => Ok(Timeframe::Min5),
        "15m" => Ok(Timeframe::Min15),
        "30m" => Ok(Timeframe::Min30),
        "1h" => Ok(Timeframe::Hour1),
        "4h" => Ok(Timeframe::Hour4),
        "1d" => Ok(Timeframe::Day1),
        "1w" => Ok(Timeframe::Week1),
        other => Err(format!("unknown timeframe {other}")),
    }
}

fn timeframe_label(timeframe: Timeframe) -> &'static str {
    match timeframe {
        Timeframe::Min1 => "1m",
        Timeframe::Min5 => "5m",
        Timeframe::Min15 => "15m",
        Timeframe::Min30 => "30m",
        Timeframe::Hour1 => "1h",
        Timeframe::Hour4 => "4h",
        Timeframe::Day1 => "1d",
        Timeframe::Week1 => "1w",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_matches_the_agent_surface() {
        let names = tool_names();
        assert_eq!(
            names,
            vec![
                "list_trades",
                "get_trade",
                "get_performance",
                "list_lessons",
                "save_lesson",
                "list_playbook",
                "run_backtest",
            ]
        );
    }

    #[test]
    fn date_only_end_includes_that_day() {
        let bound = parse_time_bound(Some("2026-01-31"), true).unwrap().unwrap();
        assert_eq!(bound.date_naive().year(), 2026);
        assert_eq!(bound.date_naive().month(), 2);
        assert_eq!(bound.date_naive().day(), 1);
    }

    #[test]
    fn backtest_window_rejects_more_than_120_days() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        assert!(backtest_window_error(start, end).is_some());
    }

    #[test]
    fn limit_stays_inside_1_to_50() {
        assert_eq!(clamp_limit(None), 20);
        assert_eq!(clamp_limit(Some(500)), 50);
        assert_eq!(clamp_limit(Some(0)), 1);
    }
}
