// ===========================================================================
// Authentication Module — JWT-based auth for the trading platform
// ===========================================================================

use anyhow::Result;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{info, warn};

use crate::AppState;

/// JWT Claims stored in the token.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (username)
    pub sub: String,
    /// Issued at (unix timestamp)
    pub iat: u64,
    /// Expiry (unix timestamp)
    pub exp: u64,
    /// Role
    pub role: String,
}

/// Login request body.
#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response body.
#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in: u64,
    pub token_type: String,
}

/// Error response body.
#[derive(Serialize)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Token Generation & Validation
// ---------------------------------------------------------------------------

/// Create a JWT token for the given username.
pub fn create_token(username: &str, secret: &str, expiry_hours: u64) -> Result<String> {
    let now = chrono::Utc::now().timestamp() as u64;
    let exp = now + (expiry_hours * 3600);

    let claims = Claims {
        sub: username.to_string(),
        iat: now,
        exp,
        role: "admin".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Validate a JWT token and return the claims.
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

/// Hash a password with SHA-256 (for comparison with stored hash).
pub fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// Login Handler
// ---------------------------------------------------------------------------

/// POST /api/auth/login — Authenticate and return a JWT token.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let config = state.config.read().await;

    // Get credentials from config (loaded from .env)
    let expected_username = &config.auth.username;
    let expected_password_hash = &config.auth.password_hash;

    // Hash the provided password and compare
    let provided_hash = hash_password(&req.password);

    if req.username != *expected_username || provided_hash != *expected_password_hash {
        warn!("❌ Failed login attempt for user: {}", req.username);
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "unauthorized".to_string(),
                message: "Invalid username or password".to_string(),
            }),
        )
            .into_response();
    }

    // Generate JWT token
    match create_token(&req.username, &config.jwt.secret, config.jwt.expiry_hours) {
        Ok(token) => {
            info!("✅ User '{}' logged in successfully", req.username);
            (
                StatusCode::OK,
                Json(LoginResponse {
                    token,
                    expires_in: config.jwt.expiry_hours * 3600,
                    token_type: "Bearer".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!("❌ Token generation failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    error: "server_error".to_string(),
                    message: "Failed to generate authentication token".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/auth/verify — Check if the current token is valid.
pub async fn verify_token(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Response {
    let jwt_secret = state.config.read().await.jwt.secret.clone();
    match extract_and_validate_token(&req, &jwt_secret) {
        Ok(claims) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "valid": true,
                "username": claims.sub,
                "expires_at": claims.exp,
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "invalid_token".to_string(),
                message: "Token is invalid or expired".to_string(),
            }),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Auth Middleware
// ---------------------------------------------------------------------------

/// Axum middleware that validates JWT token on every request.
/// Rejects with 401 if token is missing or invalid.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let jwt_secret = state.config.read().await.jwt.secret.clone();
    match extract_and_validate_token(&req, &jwt_secret) {
        Ok(_claims) => {
            // Token is valid — proceed to the handler
            next.run(req).await
        }
        Err(msg) => {
            warn!("🔒 Auth rejected: {msg}");
            (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    error: "unauthorized".to_string(),
                    message: msg,
                }),
            )
                .into_response()
        }
    }
}

/// Extract Bearer token from the Authorization header and validate it.
fn extract_and_validate_token(req: &Request, secret: &str) -> Result<Claims, String> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing Authorization header".to_string())?;

    if !auth_header.starts_with("Bearer ") {
        return Err("Invalid Authorization header format (expected 'Bearer <token>')".to_string());
    }

    let token = &auth_header[7..];

    validate_token(token, secret)
        .map_err(|e| format!("Invalid or expired token: {e}"))
}
