use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use tower::ServiceExt;

use crate::server;
use crate::server::auth::BearerAuth;
use crate::server::state::AppState;
use crate::service::auth::AuthService;

/// Fixed test token used by all test requests.
const TEST_TOKEN: &str = "test-bearer-token-for-integration-tests";

/// Test application builder for HTTP-level testing.
pub struct TestApp {
    state: AppState,
    bearer_auth: BearerAuth,
    _temp: tempfile::TempDir,
}

impl Default for TestApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TestApp {
    /// Create a new test app with a temp directory.
    pub fn new() -> Self {
        let dir = tempfile::TempDir::new().unwrap();
        let session_path = dir.path().join("test.session");
        let auth = AuthService::with_credentials(12345, "hash123".into(), session_path);
        let state = AppState::new(dir.path().to_path_buf(), auth);
        let bearer_auth = BearerAuth::new(TEST_TOKEN);
        Self {
            state,
            bearer_auth,
            _temp: dir,
        }
    }

    /// Get a fresh router instance (each oneshot consumes the router).
    fn router(&self) -> Router {
        server::build_app(self.state.clone(), self.bearer_auth.clone())
    }

    /// GET request (authenticated).
    pub async fn get(&self, uri: &str) -> (StatusCode, serde_json::Value) {
        let req = Request::builder()
            .uri(uri)
            .header("Authorization", format!("Bearer {TEST_TOKEN}"))
            .body(Body::empty())
            .unwrap();

        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    /// GET request without authentication (for testing auth rejection).
    pub async fn get_unauthed(&self, uri: &str) -> (StatusCode, serde_json::Value) {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    /// POST request with JSON body (authenticated).
    pub async fn post(
        &self,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let req = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .header("Authorization", format!("Bearer {TEST_TOKEN}"))
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    /// DELETE request with optional JSON body (authenticated).
    pub async fn delete(
        &self,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let body_str = body
            .map(|b| serde_json::to_string(&b).unwrap())
            .unwrap_or_default();
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(uri)
            .header("Authorization", format!("Bearer {TEST_TOKEN}"));

        if !body_str.is_empty() {
            builder = builder.header("content-type", "application/json");
        }

        let req = builder.body(Body::from(body_str)).unwrap();

        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let rbody = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let json = serde_json::from_slice(&rbody).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    /// Create a volume and return its ID.
    pub async fn create_volume(&self, name: &str, password: &str) -> String {
        let (status, json) = self
            .post(
                "/api/v1/volumes",
                serde_json::json!({ "name": name, "password": password }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "create volume failed: {json}");
        json["volume_id"].as_str().unwrap().to_string()
    }

    /// Open a volume session and return the volume ID.
    pub async fn open_volume(&self, volume_id: &str, password: &str) -> String {
        let (status, json) = self
            .post(
                &format!("/api/v1/volumes/{volume_id}/open"),
                serde_json::json!({ "password": password }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "open volume failed: {json}");
        json["volume_id"].as_str().unwrap().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creates_and_serves() {
        let app = TestApp::new();
        let (status, json) = app.get("/api/v1/version").await;
        assert_eq!(status, StatusCode::OK);
        assert!(json["version"].is_string());
    }

    #[tokio::test]
    async fn unauthenticated_status_allowed() {
        let app = TestApp::new();
        let (status, _json) = app.get_unauthed("/api/v1/status").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn unauthenticated_version_allowed() {
        let app = TestApp::new();
        let (status, _json) = app.get_unauthed("/api/v1/version").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn unauthenticated_volumes_rejected() {
        let app = TestApp::new();
        let (status, json) = app.get_unauthed("/api/v1/volumes").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["code"], "AUTH_REQUIRED");
    }

    #[tokio::test]
    async fn wrong_token_rejected() {
        let app = TestApp::new();
        let req = Request::builder()
            .uri("/api/v1/volumes")
            .header("Authorization", "Bearer wrong-token")
            .body(Body::empty())
            .unwrap();

        let resp = app.router().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_bearer_prefix_rejected() {
        let app = TestApp::new();
        let req = Request::builder()
            .uri("/api/v1/volumes")
            .header("Authorization", TEST_TOKEN)
            .body(Body::empty())
            .unwrap();

        let resp = app.router().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
