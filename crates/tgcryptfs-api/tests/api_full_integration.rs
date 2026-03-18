use axum::http::StatusCode;

use tgcryptfs_api::server::test_helpers::TestApp;

// ---- System endpoints ----

#[tokio::test]
async fn system_status_reports_zero_volumes() {
    let app = TestApp::new();
    let (status, json) = app.get("/api/v1/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["total_volumes"], 0);
    assert_eq!(json["volumes_mounted"], 0);
    assert_eq!(json["deadman_armed"], false);
}

#[tokio::test]
async fn version_endpoint() {
    let app = TestApp::new();
    let (status, json) = app.get("/api/v1/version").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["version"].as_str().unwrap().contains("0.1"));
}

// ---- Auth endpoints ----

#[tokio::test]
async fn auth_status_not_authenticated() {
    let app = TestApp::new();
    let (status, json) = app.get("/api/v1/auth/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["authenticated"], false);
}

// ---- Volume CRUD ----

#[tokio::test]
async fn create_volume_success() {
    let app = TestApp::new();
    let (status, json) = app
        .post(
            "/api/v1/volumes",
            serde_json::json!({"name": "test-vol", "password": "secure-password-123"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["display_name"], "test-vol");
    assert!(json["volume_id"].is_string());
    assert!(json["sentence_ref"].is_string());
}

#[tokio::test]
async fn create_volume_short_password() {
    let app = TestApp::new();
    let (status, json) = app
        .post("/api/v1/volumes", serde_json::json!({"password": "short"}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "INVALID_ARGUMENT");
}

#[tokio::test]
async fn list_volumes_after_create() {
    let app = TestApp::new();

    app.post(
        "/api/v1/volumes",
        serde_json::json!({"name": "vol-a", "password": "password123"}),
    )
    .await;
    app.post(
        "/api/v1/volumes",
        serde_json::json!({"name": "vol-b", "password": "password456"}),
    )
    .await;

    let (status, json) = app.get("/api/v1/volumes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_volume_info() {
    let app = TestApp::new();

    let (_, create_json) = app
        .post(
            "/api/v1/volumes",
            serde_json::json!({"name": "info-test", "password": "password123"}),
        )
        .await;
    let vid = create_json["volume_id"].as_str().unwrap();

    let (status, json) = app.get(&format!("/api/v1/volumes/{vid}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["display_name"], "info-test");
}

#[tokio::test]
async fn get_nonexistent_volume() {
    let app = TestApp::new();
    let (status, json) = app.get("/api/v1/volumes/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["code"], "VOLUME_NOT_FOUND");
}

#[tokio::test]
async fn delete_volume_success() {
    let app = TestApp::new();

    let (_, create_json) = app
        .post(
            "/api/v1/volumes",
            serde_json::json!({"name": "del-test", "password": "password123"}),
        )
        .await;
    let vid = create_json["volume_id"].as_str().unwrap();

    let (status, json) = app.delete(&format!("/api/v1/volumes/{vid}"), None).await;
    assert!(status.is_success(), "delete failed: {json}");

    let (list_status, list_json) = app.get("/api/v1/volumes").await;
    assert_eq!(list_status, StatusCode::OK);
    assert!(list_json.as_array().unwrap().is_empty());
}

// ---- Session lifecycle (open/close) ----

#[tokio::test]
async fn open_and_close_volume() {
    let app = TestApp::new();
    let vid = app.create_volume("open-test", "password123").await;

    let (status, json) = app
        .post(
            &format!("/api/v1/volumes/{vid}/open"),
            serde_json::json!({"password": "password123"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "opened");

    let (status, json) = app
        .post(
            &format!("/api/v1/volumes/{vid}/close"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "closed");
}

#[tokio::test]
async fn open_twice_returns_conflict() {
    let app = TestApp::new();
    let vid = app.create_volume("dup-open", "password123").await;
    app.open_volume(&vid, "password123").await;

    let (status, json) = app
        .post(
            &format!("/api/v1/volumes/{vid}/open"),
            serde_json::json!({"password": "password123"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["code"], "SESSION_ALREADY_OPEN");
}

#[tokio::test]
async fn close_not_open_volume() {
    let app = TestApp::new();
    let (status, json) = app
        .post("/api/v1/volumes/nonexistent/close", serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

#[tokio::test]
async fn delete_open_volume_blocked() {
    let app = TestApp::new();
    let vid = app.create_volume("no-del", "password123").await;
    app.open_volume(&vid, "password123").await;

    let (status, json) = app.delete(&format!("/api/v1/volumes/{vid}"), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["code"], "VOLUME_MOUNTED");
}

// ---- Sharing via open sessions ----

#[tokio::test]
async fn sharing_requires_open_session() {
    let app = TestApp::new();
    let vid = app.create_volume("share-vol", "password123").await;

    let (status, json) = app.get(&format!("/api/v1/shares/volume/{vid}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

#[tokio::test]
async fn create_and_list_shares() {
    let app = TestApp::new();
    let vid = app.create_volume("share-vol", "password123").await;
    app.open_volume(&vid, "password123").await;

    let (status, json) = app
        .post(
            "/api/v1/shares",
            serde_json::json!({"volume_id": vid, "user_id": "alice", "access_level": "read-only"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["user_id"], "alice");

    let (status, json) = app.get(&format!("/api/v1/shares/volume/{vid}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["shares"].as_array().unwrap().len(), 1);
    assert_eq!(json["shares"][0]["user_id"], "alice");
}

#[tokio::test]
async fn revoke_share() {
    let app = TestApp::new();
    let vid = app.create_volume("revoke-vol", "password123").await;
    app.open_volume(&vid, "password123").await;

    app.post(
        "/api/v1/shares",
        serde_json::json!({"volume_id": vid, "user_id": "bob", "access_level": "read-write"}),
    )
    .await;

    let (status, json) = app
        .delete(
            "/api/v1/shares/bob",
            Some(serde_json::json!({"volume_id": vid})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "revoked");
}

// ---- Invites ----

#[tokio::test]
async fn create_and_accept_invite() {
    let app = TestApp::new();
    let vid = app.create_volume("invite-vol", "password123").await;
    app.open_volume(&vid, "password123").await;

    let (status, json) = app
        .post(
            "/api/v1/invites",
            serde_json::json!({"volume_id": vid, "access_level": "read-only", "max_uses": 5}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let invite_id = json["invite_id"].as_str().unwrap();

    let (status, json) = app
        .post(
            &format!("/api/v1/invites/{invite_id}/accept"),
            serde_json::json!({"volume_id": vid, "user_id": "charlie"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["user_id"], "charlie");
}

// ---- Deadman ----

#[tokio::test]
async fn deadman_status_not_armed() {
    let app = TestApp::new();
    let (status, json) = app.get("/api/v1/deadman/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["armed"], false);
}

#[tokio::test]
async fn deadman_arm_and_disarm() {
    let app = TestApp::new();

    let (status, json) = app.post("/api/v1/deadman/arm", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "armed");

    let (_, status_json) = app.get("/api/v1/deadman/status").await;
    assert_eq!(status_json["armed"], true);

    let (status, json) = app
        .post("/api/v1/deadman/disarm", serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "disarmed");
}

#[tokio::test]
async fn deadman_disarm_when_not_armed() {
    let app = TestApp::new();
    let (status, json) = app
        .post("/api/v1/deadman/disarm", serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "INVALID_ARGUMENT");
}

// ---- Mount/Unmount ----

#[tokio::test]
async fn mount_requires_open_session() {
    let app = TestApp::new();
    let vid = app.create_volume("mount-vol", "password123").await;

    let (status, json) = app
        .post(
            &format!("/api/v1/volumes/{vid}/mount"),
            serde_json::json!({"password": "password123", "mount_point": "/tmp/mnt"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

#[tokio::test]
async fn unmount_requires_open_session() {
    let app = TestApp::new();
    let (status, json) = app
        .post("/api/v1/volumes/nonexistent/unmount", serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "SESSION_NOT_OPEN");
}

// ---- Full CRUD workflow ----

#[tokio::test]
async fn full_volume_lifecycle() {
    let app = TestApp::new();

    // 1. Create volume
    let vid = app.create_volume("lifecycle", "password123").await;

    // 2. Open session
    app.open_volume(&vid, "password123").await;

    // 3. Create share
    let (status, _) = app
        .post(
            "/api/v1/shares",
            serde_json::json!({"volume_id": &vid, "user_id": "alice", "access_level": "read-write"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    // 4. List shares
    let (status, json) = app.get(&format!("/api/v1/shares/volume/{vid}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["shares"].as_array().unwrap().len(), 1);

    // 5. Revoke share
    let (status, _) = app
        .delete(
            "/api/v1/shares/alice",
            Some(serde_json::json!({"volume_id": &vid})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // 6. Close session
    let (status, _) = app
        .post(
            &format!("/api/v1/volumes/{vid}/close"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // 7. Delete volume
    let (status, _) = app.delete(&format!("/api/v1/volumes/{vid}"), None).await;
    assert!(status.is_success());

    // 8. Verify gone
    let (_, list) = app.get("/api/v1/volumes").await;
    assert!(list.as_array().unwrap().is_empty());
}

// ---- System status with open sessions ----

#[tokio::test]
async fn status_reflects_open_sessions() {
    let app = TestApp::new();
    let vid = app.create_volume("status-test", "password123").await;
    app.open_volume(&vid, "password123").await;

    let (_, json) = app.get("/api/v1/status").await;
    assert_eq!(json["volumes_mounted"], 1);
    assert_eq!(json["total_volumes"], 1);
}
