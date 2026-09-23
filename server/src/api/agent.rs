use std::convert::Infallible;
use std::pin::Pin;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::Stream;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::ai::agent::{self, AgentEvent};
use crate::AppState;

type SseStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

#[derive(Deserialize)]
pub struct AgentChatRequest {
    messages: Vec<AgentChatMessage>,
}

#[derive(Deserialize)]
struct AgentChatMessage {
    role: String,
    content: String,
}

pub async fn agent_chat(
    State(state): State<std::sync::Arc<AppState>>,
    Json(request): Json<AgentChatRequest>,
) -> Result<Sse<SseStream>, (StatusCode, Json<Value>)> {
    let history = agent::prepare_history(
        &request
            .messages
            .into_iter()
            .map(|message| (message.role, message.content))
            .collect::<Vec<_>>(),
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))))?;

    let (tx, rx) = mpsc::channel::<AgentEvent>(32);
    tokio::spawn(async move {
        agent::run_chat(
            state.ai_engine.grok.as_ref(),
            state.db.as_ref(),
            &state.ai_engine,
            &state.backtest_results,
            history,
            tx,
        )
        .await;
    });

    let stream = futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| {
            let payload = serde_json::to_string(&item.data).unwrap_or_else(|_| "{}".to_string());
            let event = Event::default().event(item.event).data(payload);
            (Ok(event), rx)
        })
    });

    Ok(Sse::new(Box::pin(stream) as SseStream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    ))
}
