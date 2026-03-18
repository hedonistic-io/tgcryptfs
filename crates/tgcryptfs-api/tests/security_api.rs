//! Security & negative path tests for the REST API.
//!
//! Validates proper error responses for unauthorized operations,
//! missing parameters, malformed inputs, and state violations.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use tgcryptfs_api::server;
use tgcryptfs_api::server::auth::BearerAuth;
use tgcryptfs_api::server::state::AppState;
use tgcryptfs_api::service::auth::AuthService;

const TEST_TOKEN: &str = "security-api-test-token";

fn setup() -> (AppState, BearerAuth, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().unwrap();
    let session_path = dir.path().join("test.session");
    let auth = AuthService::with_credentials(12345, "hash123".into(), session_path);
    let state = AppState::new(dir.path().to_path_buf(), auth);
    let bearer_auth = BearerAuth::new(TEST_TOKEN);
    (state, bearer_auth, dir)
}

fn app(state: &AppState, bearer_auth: &BearerAuth) -> axum::Router {
    server::build_app(state.clone(), bearer_auth.clone())
}

async fn post(
    state: &AppState,
    bearer_auth: &BearerAuth,
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

    let resp = app(state, bearer_auth).oneshot(req).await.unwrap();
    let status = resp.status();
    let rbody = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let json = serde_json::from_slice(&rbody).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn get(
    state: &AppState,
    bearer_auth: &BearerAuth,
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {TEST_TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let resp = app(state, bearer_auth).oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn delete(
    state: &AppState,
    bearer_auth: &BearerAuth,
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("Authorization", format!("Bearer {TEST_TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let resp = app(state, bearer_auth).oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

// ---- Volume creation: missing/invalid password ----

#[tokio::test]
async fn create_volume_no_password_returns_error() {
    let (state, ba, _dir) = setup();
    let (status, _json) = post(
        &state,
        &ba,
        "/api/v1/volumes",
        serde_json::json!({"name": "test"}),
    )
    .await;
    // Missing required field → 422 (Unprocessable Entity) from axum's Json extractor
    assert!(status.is_client_error());
}

#[tokio::test]
async fn create_volume_empty_password_returns_400() {
    let (state, ba, _dir) = setup();
    let (status, json) = post(
        &state,
        &ba,
        "/api/v1/volumes",
        serde_json::json!({"name": "test", "password": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "INVALID_ARGUMENT");
}

#[tokio::test]
async fn create_volume_too_short_password_returns_400() {
    let (state, ba, _dir) = setup();
    let (status, json) = post(
        &state,
        &ba,
        "/api/v1/volumes",
        serde_json::json!({"name": "test", "password": "abc"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "INVALID_ARGUMENT");
}

// ---- Volume operations: nonexistent volumes ----

#[tokio::test]
async fn open_nonexistent_volume_returns_error() {
    let (state, ba, _dir) = setup();
    let (status, _json) = post(
        &state,
        &ba,
        "/api/v1/volumes/nonexistent-id/open",
        serde_json::json!({"password": "password123"}),
    )
    .await;
    // Either 404 (volume not found) or 500 (file not found)
    assert!(status.is_client_error() || status.is_server_error());
}

#[tokio::test]
async fn close_nonexistent_session_returns_400() {
    let (state, ba, _dir) = setup();
    let (status, json) = post(
        &state,
        &ba,
        "/api/v1/volumes/fake-id/close",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

#[tokio::test]
async fn delete_nonexistent_volume_returns_error() {
    let (state, ba, _dir) = setup();
    let (status, _json) = delete(&state, &ba, "/api/v1/volumes/nonexistent-id").await;
    assert!(status.is_client_error() || status.is_server_error());
}

// ---- Sharing: operations without open session ----

#[tokio::test]
async fn create_share_without_session_returns_400() {
    let (state, ba, _dir) = setup();

    // Create volume but don't open it
    let (_, create_json) = post(
        &state,
        &ba,
        "/api/v1/volumes",
        serde_json::json!({"name": "share-test", "password": "password123"}),
    )
    .await;
    let vid = create_json["volume_id"].as_str().unwrap();

    // Try to create share
    let (status, json) = post(
        &state,
        &ba,
        "/api/v1/shares",
        serde_json::json!({"volume_id": vid, "user_id": "alice", "access_level": "read-only"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

#[tokio::test]
async fn list_shares_without_session_returns_400() {
    let (state, ba, _dir) = setup();

    let (_, create_json) = post(
        &state,
        &ba,
        "/api/v1/volumes",
        serde_json::json!({"name": "list-test", "password": "password123"}),
    )
    .await;
    let vid = create_json["volume_id"].as_str().unwrap();

    let (status, json) = get(&state, &ba, &format!("/api/v1/shares/volume/{vid}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

// ---- Deadman: double arm, disarm without arm ----

#[tokio::test]
async fn deadman_double_arm_returns_400() {
    let (state, ba, _dir) = setup();

    // Arm first time
    let (status, _) = post(&state, &ba, "/api/v1/deadman/arm", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);

    // Arm second time → error
    let (status, json) = post(&state, &ba, "/api/v1/deadman/arm", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "INVALID_ARGUMENT");

    // Clean up
    post(&state, &ba, "/api/v1/deadman/disarm", serde_json::json!({})).await;
}

#[tokio::test]
async fn deadman_disarm_without_arm_returns_400() {
    let (state, ba, _dir) = setup();
    let (status, json) = post(&state, &ba, "/api/v1/deadman/disarm", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "INVALID_ARGUMENT");
}

// ---- Mount/unmount: state violations ----

#[tokio::test]
async fn mount_without_session_returns_400() {
    let (state, ba, _dir) = setup();

    let (_, create_json) = post(
        &state,
        &ba,
        "/api/v1/volumes",
        serde_json::json!({"name": "mount-test", "password": "password123"}),
    )
    .await;
    let vid = create_json["volume_id"].as_str().unwrap();

    let (status, json) = post(
        &state,
        &ba,
        &format!("/api/v1/volumes/{vid}/mount"),
        serde_json::json!({"password": "password123", "mount_point": "/tmp/test"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

#[tokio::test]
async fn unmount_without_session_returns_400() {
    let (state, ba, _dir) = setup();
    let (status, json) = post(
        &state,
        &ba,
        "/api/v1/volumes/fake/unmount",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

// ---- Malformed requests ----

#[tokio::test]
async fn malformed_json_body_returns_error() {
    let (state, ba, _dir) = setup();

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/volumes")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {TEST_TOKEN}"))
        .body(Body::from("{invalid json"))
        .unwrap();

    let resp = app(&state, &ba).oneshot(req).await.unwrap();
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn nonexistent_route_returns_404() {
    let (state, ba, _dir) = setup();
    let (status, _) = get(&state, &ba, "/api/v1/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---- Open session blocks delete ----

#[tokio::test]
async fn delete_volume_with_open_session_returns_409() {
    let (state, ba, _dir) = setup();

    let (_, create_json) = post(
        &state,
        &ba,
        "/api/v1/volumes",
        serde_json::json!({"name": "no-del", "password": "password123"}),
    )
    .await;
    let vid = create_json["volume_id"].as_str().unwrap();

    // Open session
    post(
        &state,
        &ba,
        &format!("/api/v1/volumes/{vid}/open"),
        serde_json::json!({"password": "password123"}),
    )
    .await;

    // Delete while session open → blocked
    let (status, json) = delete(&state, &ba, &format!("/api/v1/volumes/{vid}")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["code"], "VOLUME_MOUNTED");

    // Close session first, then delete works
    post(
        &state,
        &ba,
        &format!("/api/v1/volumes/{vid}/close"),
        serde_json::json!({}),
    )
    .await;
    let (status, _) = delete(&state, &ba, &format!("/api/v1/volumes/{vid}")).await;
    assert!(status.is_success());
}

// ---- Bearer token authentication ----

#[tokio::test]
async fn unauthenticated_request_to_protected_endpoint_returns_401() {
    let (state, ba, _dir) = setup();
    let req = Request::builder()
        .uri("/api/v1/volumes")
        .body(Body::empty())
        .unwrap();
    let resp = app(&state, &ba).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_bearer_token_returns_401() {
    let (state, ba, _dir) = setup();
    let req = Request::builder()
        .uri("/api/v1/volumes")
        .header("Authorization", "Bearer wrong-token-here")
        .body(Body::empty())
        .unwrap();
    let resp = app(&state, &ba).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn status_endpoint_allows_unauthenticated() {
    let (state, ba, _dir) = setup();
    let req = Request::builder()
        .uri("/api/v1/status")
        .body(Body::empty())
        .unwrap();
    let resp = app(&state, &ba).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
