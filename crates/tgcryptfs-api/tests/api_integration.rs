/// Integration tests for the API service layer.
/// Tests the full volume lifecycle through the service API.
use tgcryptfs_api::service::auth::AuthService;
use tgcryptfs_api::service::system::SystemService;
use tgcryptfs_api::service::volume::VolumeService;

/// Test: Create volume via service → list → open → delete.
#[tokio::test]
async fn volume_service_full_lifecycle() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = VolumeService::new(dir.path().to_path_buf());

    // Create
    let resp = svc
        .create(Some("lifecycle-test"), "secure-password-123")
        .await
        .unwrap();
    assert_eq!(resp.display_name, "lifecycle-test");
    assert!(!resp.sentence_ref.is_empty());
    let vid = resp.volume_id.clone();

    // List
    let vols = svc.list().await.unwrap();
    assert_eq!(vols.len(), 1);
    assert_eq!(vols[0].display_name, "lifecycle-test");
    assert!(!vols[0].mounted);

    // Open (verify password)
    svc.open(&vid, "secure-password-123").await.unwrap();

    // Delete
    svc.delete(&vid).await.unwrap();
    let vols = svc.list().await.unwrap();
    assert!(vols.is_empty());
}

/// Test: Multiple volumes managed concurrently.
#[tokio::test]
async fn multiple_volumes() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = VolumeService::new(dir.path().to_path_buf());

    let r1 = svc.create(Some("vol-1"), "password-one-1").await.unwrap();
    let r2 = svc.create(Some("vol-2"), "password-two-2").await.unwrap();
    let r3 = svc.create(Some("vol-3"), "password-three").await.unwrap();

    let vols = svc.list().await.unwrap();
    assert_eq!(vols.len(), 3);

    // Open each with correct password
    svc.open(&r1.volume_id, "password-one-1").await.unwrap();
    svc.open(&r2.volume_id, "password-two-2").await.unwrap();
    svc.open(&r3.volume_id, "password-three").await.unwrap();

    // Delete middle one
    svc.delete(&r2.volume_id).await.unwrap();
    let vols = svc.list().await.unwrap();
    assert_eq!(vols.len(), 2);
}

/// Test: Short password rejected.
#[tokio::test]
async fn short_password_rejected() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = VolumeService::new(dir.path().to_path_buf());
    assert!(svc.create(None, "short").await.is_err());
}

/// Test: Delete nonexistent volume.
#[tokio::test]
async fn delete_nonexistent() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = VolumeService::new(dir.path().to_path_buf());
    assert!(svc.delete("nonexistent-vol-id").await.is_err());
}

/// Test: Auth service session management.
#[test]
fn auth_service_session_lifecycle() {
    let dir = tempfile::TempDir::new().unwrap();
    let session_path = dir.path().join("test.session");

    let svc = AuthService::with_credentials(12345, "hash123".into(), session_path.clone());
    assert!(!svc.has_session());

    // Simulate session file creation
    std::fs::write(&session_path, b"session-data").unwrap();
    assert!(svc.has_session());

    // Remove session
    svc.remove_session().unwrap();
    assert!(!svc.has_session());
}

/// Test: System service status reporting.
#[test]
fn system_service_status() {
    let sys = SystemService::new();
    let status = sys.status(true, 2, 5, 100, 1048576, false);

    assert!(status.telegram_connected);
    assert_eq!(status.volumes_mounted, 2);
    assert_eq!(status.total_volumes, 5);
    assert_eq!(status.cache_entries, 100);
    assert!(!status.deadman_armed);
}

/// Test: Sentence reference included in create response.
#[tokio::test]
async fn sentence_ref_in_create_response() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = VolumeService::new(dir.path().to_path_buf());

    let resp = svc
        .create(Some("ref-check"), "password-for-sentence")
        .await
        .unwrap();

    // Sentence reference should contain 22 words
    let words: Vec<&str> = resp.sentence_ref.split_whitespace().collect();
    assert_eq!(words.len(), 22, "sentence reference should have 22 words");
}
