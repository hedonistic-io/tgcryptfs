use tgcryptfs_core::sentence;
/// Integration tests for volume lifecycle operations.
use tgcryptfs_core::volume::manager;

/// Test: Create volume → open → verify keys match.
#[test]
fn create_and_open_volume() {
    let dir = tempfile::TempDir::new().unwrap();
    let password = b"integration-test-password";

    let created = manager::create_volume(Some("my-vault"), password, dir.path()).unwrap();
    assert_eq!(created.config.display_name, "my-vault");

    let vid = created.config.volume_id.to_string();
    let opened = manager::open_volume(&vid, password, dir.path()).unwrap();

    // Keys must be identical
    assert_eq!(
        created.hierarchy.data.as_bytes(),
        opened.hierarchy.data.as_bytes()
    );
    assert_eq!(
        created.hierarchy.meta.as_bytes(),
        opened.hierarchy.meta.as_bytes()
    );
    assert_eq!(
        created.hierarchy.schema.as_bytes(),
        opened.hierarchy.schema.as_bytes()
    );
}

/// Test: Wrong password returns a clear error instead of silently producing wrong keys.
#[test]
fn wrong_password_returns_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let created = manager::create_volume(Some("test"), b"correct-password", dir.path()).unwrap();
    let vid = created.config.volume_id.to_string();

    let result = manager::open_volume(&vid, b"wrong-password", dir.path());
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("wrong password"),
        "expected wrong password error, got: {err}"
    );
}

/// Test: Sentence reference encode → decode roundtrip.
#[test]
fn sentence_reference_roundtrip() {
    let dir = tempfile::TempDir::new().unwrap();
    let password = b"sentence-ref-test";

    let created = manager::create_volume(Some("ref-test"), password, dir.path()).unwrap();

    // Encode the root key as a sentence reference
    let wordlists: [Vec<String>; 4] =
        core::array::from_fn(sentence::wordlists::placeholder_wordlist);
    let sentence_str =
        sentence::encode::encode_ref_string(created.hierarchy.root.as_bytes(), &wordlists).unwrap();

    // Decode back
    let reverse_lookups: [std::collections::HashMap<String, u16>; 4] =
        core::array::from_fn(|i| sentence::wordlists::build_reverse_lookup(&wordlists[i]));
    let decoded =
        sentence::decode::decode_ref_string(&sentence_str, &wordlists, &reverse_lookups).unwrap();

    assert_eq!(decoded, *created.hierarchy.root.as_bytes());
}

/// Test: List and delete volumes.
#[test]
fn list_and_delete_volumes() {
    let dir = tempfile::TempDir::new().unwrap();

    // Create three volumes
    let _v1 = manager::create_volume(Some("alpha"), b"pw1", dir.path()).unwrap();
    let v2 = manager::create_volume(Some("beta"), b"pw2", dir.path()).unwrap();
    let _v3 = manager::create_volume(Some("gamma"), b"pw3", dir.path()).unwrap();

    // List shows all three
    let list = manager::list_volumes(dir.path()).unwrap();
    assert_eq!(list.len(), 3);

    // Delete one
    manager::delete_volume(&v2.config.volume_id.to_string(), dir.path()).unwrap();

    // List shows two
    let list = manager::list_volumes(dir.path()).unwrap();
    assert_eq!(list.len(), 2);
    let names: Vec<&str> = list.iter().map(|v| v.display_name.as_str()).collect();
    assert!(names.contains(&"alpha"));
    assert!(!names.contains(&"beta"));
    assert!(names.contains(&"gamma"));
}

/// Test: Auto-generated volume name.
#[test]
fn auto_generated_name() {
    let dir = tempfile::TempDir::new().unwrap();
    let created = manager::create_volume(None, b"password", dir.path()).unwrap();

    // Name should be three words separated by spaces
    let words: Vec<&str> = created.config.display_name.split_whitespace().collect();
    assert_eq!(words.len(), 3);
}

/// Test: Volume config persists to disk correctly.
#[test]
fn volume_config_persistence() {
    let dir = tempfile::TempDir::new().unwrap();
    let created = manager::create_volume(Some("persist-test"), b"pw", dir.path()).unwrap();

    // Config file exists
    assert!(created.paths.config_path.exists());

    // Read config from disk directly
    let json = std::fs::read_to_string(&created.paths.config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(config["display_name"], "persist-test");
    assert_eq!(config["volume_id"], created.config.volume_id.to_string());
}
