//! Multi-user authentication module: password hashing (bcrypt), JWT issuance/validation, HTTP extractors.
//!
//! - `AuthUser`: resolves and validates the JWT from the request, yielding the currently logged-in user (any logged-in user).
//! - `AdminUser`: builds on `AuthUser` and additionally requires `role == "admin"`, otherwise 403.
//! The token is passed via `Authorization: Bearer` or the `aiapp_token` cookie.

use axum::extract::FromRequestParts;
use axum::http::{header, request::Parts, StatusCode};
use axum::response::{IntoResponse, Response};
use async_trait::async_trait;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AppState;
use crate::AppUser;

/// Default JWT lifetime: 7 days.
const TOKEN_TTL: u64 = 7 * 24 * 3600;

/// JWT claims.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// User id (sub).
    sub: String,
    /// Role: admin / user.
    role: String,
    /// Expiration time (Unix seconds).
    exp: usize,
}

/// Hash a plaintext password with bcrypt.
pub fn hash_password(pw: &str) -> String {
    bcrypt::hash(pw, bcrypt::DEFAULT_COST).unwrap_or_default()
}

/// Verify a plaintext password against a bcrypt hash.
pub fn verify_password(pw: &str, hash: &str) -> bool {
    bcrypt::verify(pw, hash).unwrap_or(false)
}

/// Issue a JWT.
pub fn make_token(secret: &str, id: &str, role: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let claims = Claims {
        sub: id.to_string(),
        role: role.to_string(),
        exp: (now + TOKEN_TTL) as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

/// Validate a JWT, returning its claims.
fn verify_token(secret: &str, token: &str) -> Result<Claims, String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| e.to_string())?;
    Ok(data.claims)
}

/// Extract the token from request headers: prefer `Authorization: Bearer`, then the `aiapp_token` cookie.
fn extract_token(parts: &Parts) -> Option<String> {
    if let Some(auth) = parts.headers.get(header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(t) = s.strip_prefix("Bearer ") {
                return Some(t.to_string());
            }
        }
    }
    if let Some(cookie) = parts.headers.get(header::COOKIE) {
        if let Ok(s) = cookie.to_str() {
            for pair in s.split(';') {
                let pair = pair.trim();
                if let Some((k, v)) = pair.split_once('=') {
                    if k.trim() == "aiapp_token" {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Validate the token in the request, returning the JWT claims.
fn authenticate(secret: &str, parts: &Parts) -> Result<Claims, AuthRejection> {
    let token = extract_token(parts).ok_or_else(AuthRejection::unauthorized)?;
    verify_token(secret, &token).map_err(|_| AuthRejection::unauthorized())
}

/// Unified response for authentication failures.
pub struct AuthRejection {
    status: StatusCode,
    message: &'static str,
}

impl AuthRejection {
    fn unauthorized() -> Self {
        Self { status: StatusCode::UNAUTHORIZED, message: "Not logged in or session expired" }
    }
    fn forbidden() -> Self {
        Self { status: StatusCode::FORBIDDEN, message: "Admin privileges required" }
    }
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        (self.status, axum::Json(serde_json::json!({ "ok": false, "error": self.message })))
            .into_response()
    }
}

/// Currently logged-in user (any role).
pub struct AuthUser {
    pub id: String,
    pub role: String,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let claims = authenticate(&state.auth_secret, parts)?;
        Ok(AuthUser { id: claims.sub, role: claims.role })
    }
}

/// Optional current user: returns None when not logged in; an invalid token is also treated as not logged in (does not block public endpoints).
pub struct OptionalAuth(pub Option<AuthUser>);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for OptionalAuth {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        match authenticate(&state.auth_secret, parts) {
            Ok(claims) => Ok(OptionalAuth(Some(AuthUser { id: claims.sub, role: claims.role }))),
            Err(_) => Ok(OptionalAuth(None)),
        }
    }
}

/// Currently logged-in user who must also be an admin.
pub struct AdminUser {
    pub id: String,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AdminUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let claims = authenticate(&state.auth_secret, parts)?;
        if claims.role != "admin" {
            return Err(AuthRejection::forbidden());
        }
        Ok(AdminUser { id: claims.sub })
    }
}

/// Build a successful-login response: write an HttpOnly cookie and return the user info.
pub fn login_response(token: &str, u: &AppUser) -> Response {
    let max_age = TOKEN_TTL;
    let cookie = format!(
        "aiapp_token={}; HttpOnly; Path=/; Max-Age={}; SameSite=Lax",
        token, max_age
    );
    let body = axum::Json(serde_json::json!({
        "ok": true,
        "token": token,
        "user": { "id": u.id, "name": u.name, "role": u.role }
    }));
    let mut resp = body.into_response();
    if let Ok(v) = header::HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    resp
}

/// Clear the auth cookie so a refresh no longer restores the session.
pub async fn logout() -> Response {
    let cookie = "aiapp_token=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax";
    let mut resp = axum::Json(serde_json::json!({ "ok": true })).into_response();
    if let Ok(v) = header::HeaderValue::from_str(cookie) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    resp
}
