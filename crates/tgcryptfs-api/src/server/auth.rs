use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ApiError;

/// Shared bearer token state for API authentication.
///
/// On server start, a random token is generated and displayed to the user.
/// The SHA-256 hash of the token is stored here; incoming requests must
/// present the token in the `Authorization: Bearer <token>` header.
#[derive(Clone)]
pub struct BearerAuth {
    /// SHA-256 hash of the bearer token (we never store the raw token).
    token_hash: Arc<[u8; 32]>,
}

impl BearerAuth {
    /// Create a new BearerAuth from a raw token string.
    /// Stores only the hash internally.
    pub fn new(token: &str) -> Self {
        let hash = blake3::hash(token.as_bytes());
        Self {
            token_hash: Arc::new(*hash.as_bytes()),
        }
    }

    /// Verify a candidate token against the stored hash.
    pub fn verify(&self, candidate: &str) -> bool {
        let candidate_hash = blake3::hash(candidate.as_bytes());
        // Constant-time comparison to prevent timing attacks
        constant_time_eq(self.token_hash.as_ref(), candidate_hash.as_bytes())
    }
}

/// Generate a cryptographically random API token (32 bytes, hex-encoded).
pub fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Constant-time byte comparison.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Extract bearer token from Authorization header.
fn extract_bearer_token(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Axum middleware that enforces bearer token authentication.
///
/// Allows unauthenticated access to:
/// - `GET /api/v1/status` (health check)
/// - `GET /api/v1/version` (version info)
///
/// All other endpoints require a valid `Authorization: Bearer <token>` header.
pub async fn require_auth(request: Request, next: Next) -> Result<Response, ApiError> {
    let path = request.uri().path();
    let method = request.method().clone();

    // Allow unauthenticated access to health/version endpoints
    if method == http::Method::GET && (path == "/api/v1/status" || path == "/api/v1/version") {
        return Ok(next.run(request).await);
    }

    // Extract BearerAuth from extensions
    let auth = request
        .extensions()
        .get::<BearerAuth>()
        .cloned()
        .ok_or(ApiError::Internal("auth middleware misconfigured".into()))?;

    // Verify bearer token
    let token = extract_bearer_token(&request).ok_or(ApiError::AuthRequired)?;
    if !auth.verify(token) {
        return Err(ApiError::AuthRequired);
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_is_64_hex_chars() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_is_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn bearer_auth_verify_correct_token() {
        let token = "test-token-abc123";
        let auth = BearerAuth::new(token);
        assert!(auth.verify(token));
    }

    #[test]
    fn bearer_auth_reject_wrong_token() {
        let auth = BearerAuth::new("correct-token");
        assert!(!auth.verify("wrong-token"));
    }

    #[test]
    fn bearer_auth_reject_empty_token() {
        let auth = BearerAuth::new("my-token");
        assert!(!auth.verify(""));
    }

    #[test]
    fn constant_time_eq_equal() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn constant_time_eq_different() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn constant_time_eq_different_lengths() {
        assert!(!constant_time_eq(b"short", b"longer"));
    }
}
