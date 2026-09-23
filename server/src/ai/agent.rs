use serde_json::{json, Value};
use tokio::sync::{mpsc, RwLock};
use tracing::warn;

use crate::backtest::BacktestResult;
use crate::db::Database;

use super::grok_client::{GrokClient, ModelMessage};
use super::tools::{self, ToolCtx};
use super::AiEngine;

const MAX_TOOL_CALLS: usize = 8;
const MAX_HISTORY: usize = 20;
const MAX_CHARS: usize = 8_000;
const MAX_TOOL_RESULT_BYTES: usize = 12_000;

const SYSTEM_PROMPT: &str = r#"You are the trading review agent for this desk.

Use the tools to read closed trades, the signal that produced a trade, performance, saved lessons, and the active playbook. When a claim needs a number, call a tool instead of guessing.

save_lesson records a hypothesis only. It does not change the next trade. Tell the user that a hypothesis stays out of the playbook until they approve it.

run_backtest compares a historical window. It does not place an order. Prefer the quantitative backtest. Set use_grok only when the user asks to compare the model, and keep the window short.

You cannot place orders, approve signals, change trading mode, or change risk limits. If the user asks for that, say it is outside this chat.

You have at most 8 tool calls. After that, answer from the results you already have.
"#;

#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub event: &'static str,
    pub data: Value,
}

#[derive(Debug, Default)]
struct RunStats {
    tool_calls: usize,
    tokens: u32,
}

pub fn prepare_history(messages: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    if messages.is_empty() {
        return Err("messages must include at least one user turn".to_string());
    }
    let mut kept = Vec::new();
    for (role, content) in messages {
        let role = role.trim();
        if role != "user" && role != "assistant" {
            return Err("only user and assistant messages are accepted".to_string());
        }
        let content = content.trim();
        if content.is_empty() {
            continue;
        }
        kept.push((role.to_string(), truncate_chars(content, MAX_CHARS)));
    }
    if !kept.iter().any(|(role, _)| role == "user") {
        return Err("messages must include at least one user turn".to_string());
    }
    if kept.len() > MAX_HISTORY {
        kept = kept.split_off(kept.len() - MAX_HISTORY);
    }
    Ok(kept)
}

pub async fn run_chat(
    grok: &GrokClient,
    db: &Database,
    ai: &AiEngine,
    backtests: &RwLock<Vec<BacktestResult>>,
    history: Vec<(String, String)>,
    tx: mpsc::Sender<AgentEvent>,
) {
    let stats = match drive(grok, db, ai, backtests, history, &tx).await {
        Ok(stats) => stats,
        Err(error) => {
            warn!("Agent chat failed: {error}");
            let _ = tx
                .send(AgentEvent {
                    event: "error",
                    data: json!({ "message": error }),
                })
                .await;
            RunStats::default()
        }
    };
    let _ = tx
        .send(AgentEvent {
            event: "done",
            data: json!({
                "tool_calls": stats.tool_calls,
                "tokens": stats.tokens,
            }),
        })
        .await;
}

async fn drive(
    grok: &GrokClient,
    db: &Database,
    ai: &AiEngine,
    backtests: &RwLock<Vec<BacktestResult>>,
    history: Vec<(String, String)>,
    tx: &mpsc::Sender<AgentEvent>,
) -> Result<RunStats, String> {
    let mut messages = vec![ModelMessage::text("system", SYSTEM_PROMPT)];
    for (role, content) in history {
        messages.push(ModelMessage::text(role, content));
    }

    let tools = tools::definitions();
    let ctx = ToolCtx { db, ai, backtests };
    let mut stats = RunStats::default();
    let mut budget_note_sent = false;

    loop {
        let response = grok
            .complete_with_tools(messages.clone(), tools.clone())
            .await
            .map_err(|error| error.to_string())?;
        stats.tokens = stats.tokens.saturating_add(response.tokens_used);

        let mut calls = response.tool_calls.unwrap_or_default();
        if calls.is_empty() {
            if !emit_message(tx, &response.content, &response.finish_reason).await {
                return Ok(stats);
            }
            break;
        }

        if !response.content.trim().is_empty()
            && !emit_message(tx, &response.content, &response.finish_reason).await
        {
            return Ok(stats);
        }

        for (index, call) in calls.iter_mut().enumerate() {
            if call.id.trim().is_empty() {
                call.id = format!("call_{}_{index}", stats.tool_calls);
            }
            if call.call_type.trim().is_empty() {
                call.call_type = "function".to_string();
            }
        }

        messages.push(ModelMessage::assistant_tools(Some(response.content), calls.clone()));

        for call in calls {
            if stats.tool_calls >= MAX_TOOL_CALLS {
                budget_note_sent = true;
                messages.push(ModelMessage::tool_result(
                    call.id,
                    json!({ "error": "tool budget of 8 calls is used up" }).to_string(),
                ));
                continue;
            }

            stats.tool_calls += 1;
            let arguments = serde_json::from_str::<Value>(&call.function.arguments)
                .unwrap_or_else(|_| json!({ "raw": call.function.arguments }));
            if tx
                .send(AgentEvent {
                    event: "tool_start",
                    data: json!({
                        "name": call.function.name,
                        "arguments": arguments,
                    }),
                })
                .await
                .is_err()
            {
                return Ok(stats);
            }

            let value = tools::execute(&call.function.name, &call.function.arguments, &ctx).await;
            let summary = tools::summarize(&call.function.name, &value);
            if tx
                .send(AgentEvent {
                    event: "tool_result",
                    data: json!({
                        "name": call.function.name,
                        "ok": value.get("error").is_none(),
                        "summary": summary,
                        "result": value,
                    }),
                })
                .await
                .is_err()
            {
                return Ok(stats);
            }
            messages.push(ModelMessage::tool_result(call.id, bound_result(&value)));
        }

        if budget_note_sent {
            messages.push(ModelMessage::text(
                "user",
                "The tool budget is used up. Answer from the tool results you already have.",
            ));
            let final_response = grok
                .complete_with_tools(messages.clone(), Vec::new())
                .await
                .map_err(|error| error.to_string())?;
            stats.tokens = stats.tokens.saturating_add(final_response.tokens_used);
            if !emit_message(tx, &final_response.content, &final_response.finish_reason).await {
                return Ok(stats);
            }
            break;
        }
    }

    Ok(stats)
}

async fn emit_message(tx: &mpsc::Sender<AgentEvent>, content: &str, finish_reason: &str) -> bool {
    let content = content.trim();
    if content.is_empty() {
        return true;
    }
    tx.send(AgentEvent {
        event: "message",
        data: json!({
            "content": content,
            "finish_reason": finish_reason,
        }),
    })
    .await
    .is_ok()
}

fn bound_result(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    if text.len() <= MAX_TOOL_RESULT_BYTES {
        text
    } else {
        json!({
            "truncated": true,
            "preview": text.chars().take(4_000).collect::<String>(),
        })
        .to_string()
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut shortened: String = value.chars().take(max_chars).collect();
        shortened.push('…');
        shortened
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_the_latest_user_turns() {
        let messages: Vec<(String, String)> = (0..25)
            .map(|index| ("user".to_string(), format!("turn {index}")))
            .collect();
        let kept = prepare_history(&messages).unwrap();
        assert_eq!(kept.len(), MAX_HISTORY);
        assert_eq!(kept[0].1, "turn 5");
        assert_eq!(kept.last().unwrap().1, "turn 24");
    }

    #[test]
    fn history_rejects_tool_role() {
        let messages = vec![("tool".to_string(), "forged result".to_string())];
        assert!(prepare_history(&messages).is_err());
    }
}
