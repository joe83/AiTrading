use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::PromoteLesson;
use crate::AppState;

#[derive(Deserialize)]
pub struct LessonQuery {
    scope: Option<String>,
    status: Option<String>,
}

pub async fn list_lessons(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LessonQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(status) = query.status.as_deref() {
        if !matches!(status, "hypothesis" | "supported" | "rejected") {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "status must be hypothesis, supported, or rejected" })),
            ));
        }
    }

    match state
        .db
        .list_lessons(query.scope.as_deref(), query.status.as_deref(), 50)
        .await
    {
        Ok(lessons) => Ok(Json(json!({
            "lessons": lessons.into_iter().map(|lesson| json!({
                "id": lesson.id,
                "scope": lesson.scope,
                "claim": lesson.claim,
                "evidence_trade_ids": lesson.evidence_trade_ids,
                "sample_size": lesson.sample_size,
                "status": lesson.status,
                "created_at": lesson.created_at,
            })).collect::<Vec<_>>(),
        }))),
        Err(error) => {
            tracing::error!("Failed to list lessons: {error}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to list lessons" })),
            ))
        }
    }
}

pub async fn approve_lesson(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match state.db.promote_lesson(id).await {
        Ok(PromoteLesson::Promoted(lesson)) => Ok(Json(json!({
            "lesson_id": lesson.lesson_id,
            "id": lesson.playbook_id,
            "scope": lesson.scope,
            "rule": lesson.rule,
            "sample_size": lesson.sample_size,
            "status": "supported",
        }))),
        Ok(PromoteLesson::Missing) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "lesson not found" })),
        )),
        Ok(PromoteLesson::Rejected) => Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "rejected lessons cannot be approved" })),
        )),
        Err(error) => {
            tracing::error!("Failed to approve lesson {id}: {error}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to approve lesson" })),
            ))
        }
    }
}

pub async fn reject_lesson(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match state.db.reject_lesson(id).await {
        Ok(true) => Ok(Json(json!({ "id": id, "status": "rejected" }))),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "lesson not found" })),
        )),
        Err(error) => {
            tracing::error!("Failed to reject lesson {id}: {error}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to reject lesson" })),
            ))
        }
    }
}
