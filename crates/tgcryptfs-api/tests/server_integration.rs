use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use tgcryptfs_api::server::auth::BearerAuth;
use tgcryptfs_api::server::state::AppState;
use tgcryptfs_api::server::{self};
use tgcryptfs_api::service::auth::AuthService;

const TEST_TOKEN: &str = "server-integration-test-token";

fn test_app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().unwrap();
    let session_path = dir.path().join("test.session");
    let auth = AuthService::with_credentials(12345, "hash123".into(), session_path);
    let state = AppState::new(dir.path().to_path_buf(), auth);
    let bearer_auth = BearerAuth::new(TEST_TOKEN);
    let app = server::build_app(state, bearer_auth);
    (app, dir)
}

fn auth_header() -> (&'static str, String) {
    ("Authorization", format!("Bearer {TEST_TOKEN}"))
}

#[tokio::test]
async fn get_status_returns_200() {
    let (app, _dir) = test_app();
    let req = Request::builder()
        .uri("/api/v1/status")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_version_returns_version() {
    let (app, _dir) = test_app();
    let req = Request::builder()
        .uri("/api/v1/version")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn get_auth_status_returns_not_authenticated() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .uri("/api/v1/auth/status")
        .header(hdr, val)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["authenticated"], false);
}

#[tokio::test]
async fn list_volumes_empty() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .uri("/api/v1/volumes")
        .header(hdr, val)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn create_volume_returns_201() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/volumes")
        .header("content-type", "application/json")
        .header(hdr, val)
        .body(Body::from(
            r#"{"name":"api-test","password":"secure-password-123"}"#,
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["display_name"], "api-test");
    assert!(json["volume_id"].is_string());
    assert!(json["sentence_ref"].is_string());
}

#[tokio::test]
async fn create_volume_short_password_returns_400() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/volumes")
        .header("content-type", "application/json")
        .header(hdr, val)
        .body(Body::from(r#"{"password":"short"}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "INVALID_ARGUMENT");
    assert!(json["suggestion"].is_string());
}

#[tokio::test]
async fn get_nonexistent_volume_returns_404() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .uri("/api/v1/volumes/nonexistent-id")
        .header(hdr, val)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_nonexistent_volume_returns_error() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/volumes/nonexistent-id")
        .header(hdr, val)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // Should be 500 (storage error) since deletion of non-existent volume fails
    assert!(resp.status().is_server_error() || resp.status().is_client_error());
}

#[tokio::test]
async fn deadman_status_returns_200() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .uri("/api/v1/deadman/status")
        .header(hdr, val)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn nonexistent_route_returns_404() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .uri("/api/v1/nonexistent")
        .header(hdr, val)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mount_without_password_returns_400() {
    let (app, _dir) = test_app();
    let (hdr, val) = auth_header();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/volumes/some-id/mount")
        .header("content-type", "application/json")
        .header(hdr, val)
        .body(Body::from(r#"{"mount_point":"/tmp/test"}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
