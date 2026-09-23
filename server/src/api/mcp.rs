use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::ai::tools::{self, ToolCtx};
use crate::AppState;

const PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

pub fn mcp_tool_catalog() -> Vec<Value> {
    tools::definitions()
        .into_iter()
        .map(|tool| {
            json!({
                "name": tool.function.name,
                "description": tool.function.description,
                "inputSchema": tool.function.parameters,
            })
        })
        .collect()
}

pub async fn mcp_http(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Response {
    let request: RpcRequest = match serde_json::from_str(&body) {
        Ok(request) => request,
        Err(_) => {
            return with_protocol(rpc_error(Value::Null, -32700, "parse error").into_response());
        }
    };

    if request.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let id = request.id.unwrap_or(Value::Null);

    let response = match request.method.as_str() {
        "initialize" => {
            let requested = request
                .params
                .get("protocolVersion")
                .and_then(|value| value.as_str())
                .unwrap_or(PROTOCOL_VERSION);
            let version = if requested == "2024-11-05" || requested == PROTOCOL_VERSION {
                requested
            } else {
                PROTOCOL_VERSION
            };
            rpc_result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "aitrading", "version": "1.0.0" },
                }),
            )
            .into_response()
        }
        "ping" => rpc_result(id, json!({})).into_response(),
        "tools/list" => rpc_result(id, json!({ "tools": mcp_tool_catalog() })).into_response(),
        "tools/call" => {
            let name = request
                .params
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if name.is_empty() {
                return with_protocol(rpc_error(id, -32602, "tool name is required").into_response());
            }
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let encoded = serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string());
            let ctx = ToolCtx {
                db: state.db.as_ref(),
                ai: &state.ai_engine,
                backtests: &state.backtest_results,
            };
            let value = tools::execute(name, &encoded, &ctx).await;
            let is_error = value.get("error").is_some();
            let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());
            rpc_result(
                id,
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": is_error,
                }),
            )
            .into_response()
        }
        _ => rpc_error(id, -32601, "method not found").into_response(),
    };
    with_protocol(response)
}

fn with_protocol(mut response: Response) -> Response {
    response.headers_mut().insert(
        "mcp-protocol-version",
        HeaderValue::from_static(PROTOCOL_VERSION),
    );
    response
}

fn rpc_result(id: Value, result: Value) -> Json<Value> {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
}

fn rpc_error(id: Value, code: i32, message: &str) -> Json<Value> {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_exposes_every_agent_tool() {
        let tools = mcp_tool_catalog();
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(|value| value.as_str()))
            .collect();
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
        assert_eq!(tools[0]["inputSchema"]["type"].as_str(), Some("object"));
    }
}
