use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
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

pub async fn get_watch_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let config = state.config.read().await.watch.clone();
    Json(serde_json::to_value(&config).unwrap_or(json!({})))
}

#[derive(Debug, Deserialize)]
pub struct UpdateWatchConfigRequest {
    pub enabled: Option<bool>,
    pub interval_secs: Option<u64>,
    pub provider: Option<String>,
    pub handles: Option<Vec<String>>,
    pub min_confidence: Option<f64>,
    pub max_post_age_secs: Option<i64>,
    pub scraper_api_key: Option<String>,
    pub scraper_provider: Option<String>,
    pub custom_feed_url: Option<String>,
    pub webhook_secret: Option<String>,
}

pub async fn update_watch_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateWatchConfigRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut config = state.config.write().await;
    if let Some(enabled) = payload.enabled {
        config.watch.enabled = enabled;
    }
    if let Some(interval) = payload.interval_secs {
        config.watch.interval_secs = interval.max(30);
    }
    if let Some(provider) = payload.provider {
        config.watch.provider = provider;
    }
    if let Some(handles) = payload.handles {
        config.watch.handles = handles
            .into_iter()
            .map(|h| h.trim().trim_start_matches('@').to_string())
            .filter(|h| !h.is_empty())
            .take(20)
            .collect();
    }
    if let Some(confidence) = payload.min_confidence {
        config.watch.min_confidence = confidence.clamp(0.1, 1.0);
    }
    if let Some(max_age) = payload.max_post_age_secs {
        config.watch.max_post_age_secs = max_age.max(60);
    }
    if let Some(key) = payload.scraper_api_key {
        config.watch.scraper_api_key = key;
    }
    if let Some(scraper_provider) = payload.scraper_provider {
        config.watch.scraper_provider = scraper_provider;
    }
    if let Some(feed_url) = payload.custom_feed_url {
        config.watch.custom_feed_url = feed_url;
    }
    if let Some(secret) = payload.webhook_secret {
        config.watch.webhook_secret = secret;
    }

    let updated_watch = config.watch.clone();
    drop(config);

    // Update watch status to reflect immediately
    {
        let mut status = state.watch_status.write().await;
        status.enabled = updated_watch.enabled;
        status.provider = updated_watch.provider.clone();
        status.handles = updated_watch.handles.clone();
        status.interval_secs = updated_watch.interval_secs;
    }

    Ok(Json(json!({
        "status": "ok",
        "config": updated_watch,
    })))
}

pub async fn watch_scan(State(state): State<Arc<AppState>>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match crate::ai::watch_loop::tick_now(&state).await {
        Ok(result) => Ok(Json(json!({ "status": "ok", "result": result }))),
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "status": "error", "error": error.to_string() })),
        )),
    }
}

pub async fn watch_ingest(
    State(state): State<Arc<AppState>>,
    Json(post): Json<crate::ai::watch_loop::IncomingPost>,
) -> Result<Json<crate::ai::watch_loop::IngestResult>, (StatusCode, Json<Value>)> {
    let config = state.config.read().await.watch.clone();
    match crate::ai::watch_loop::process_incoming_post(&state, &config, &post).await {
        Ok(result) => Ok(Json(result)),
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "status": "error", "error": error.to_string() })),
        )),
    }
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
