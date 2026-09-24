use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn proxy_base_url() -> String {
    std::env::var("GROK_PROXY_URL").unwrap_or_else(|_| "http://grok-proxy:8585/v1".to_string())
}

pub fn is_proxy_url(url: &str) -> bool {
    url.contains("grok-proxy")
}

pub async fn current_account() -> Option<String> {
    control_get("/account")
        .await
        .ok()
        .and_then(|value| value.get("account").and_then(|item| item.as_str()).map(str::to_string))
        .filter(|account| !account.is_empty())
}

fn control_url() -> String {
    std::env::var("GROK_CONTROL_URL").unwrap_or_else(|_| "http://grok-proxy:8586".to_string())
}

pub async fn watch_status(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let status = state.watch_status.read().await.clone();
    let conductor = state.conductor_status.read().await.clone();
    let posts = match state.db.recent_watched_posts(30).await {
        Ok(posts) => posts,
        Err(error) => {
            tracing::error!("Failed to list watched posts: {error}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(Json(json!({
        "status": status,
        "conductor": conductor,
        "posts": posts.into_iter().map(|post| json!({
            "post_id": post.post_id,
            "handle": post.handle,
            "body": post.body,
            "queued": post.queued,
            "seen_at": post.seen_at,
        })).collect::<Vec<_>>(),
    })))
}

pub async fn grok_account(State(_state): State<Arc<AppState>>) -> Json<Value> {
    match control_get("/account").await {
        Ok(value) => Json(value),
        Err(error) => Json(json!({
            "logged_in": false,
            "account": null,
            "error": error,
        })),
    }
}

pub async fn grok_login_status(State(_state): State<Arc<AppState>>) -> Json<Value> {
    match control_get("/login").await {
        Ok(value) => Json(value),
        Err(error) => Json(json!({ "status": "error", "error": error })),
    }
}

pub async fn grok_login_start(State(_state): State<Arc<AppState>>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match control_post("/login").await {
        Ok(value) => Ok(Json(value)),
        Err(error) => Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": error })),
        )),
    }
}

async fn control_get(path: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .get(format!("{}{path}", control_url()))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    response.json().await.map_err(|error| error.to_string())
}

async fn control_post(path: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(format!("{}{path}", control_url()))
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body: Value = response.json().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(body.get("error").and_then(|value| value.as_str()).unwrap_or("login failed").to_string());
    }
    Ok(body)
}
