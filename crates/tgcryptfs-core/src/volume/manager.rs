use std::path::{Path, PathBuf};

use crate::crypto::kdf;
use crate::crypto::keys::KeyHierarchy;
use crate::error::{CoreError, Result};
use crate::volume::config::VolumeConfig;
use crate::volume::names;

/// Derive a verification key from the key hierarchy and hash it.
/// This allows checking if a password is correct without exposing any real keys.
fn compute_verification_hash(hierarchy: &KeyHierarchy) -> String {
    let verification_key = kdf::hkdf_derive(&hierarchy.root, b"password-verification").unwrap();
    let hash = blake3::hash(verification_key.as_bytes());
    hex::encode(hash.as_bytes())
}

/// Paths associated with a volume on disk.
#[derive(Debug, Clone)]
pub struct VolumePaths {
    /// Base directory for volume data (e.g., ~/.tgcryptfs/volumes/<id>/)
    pub base_dir: PathBuf,
    /// SQLite metadata database path
    pub db_path: PathBuf,
    /// Serialized volume config path
    pub config_path: PathBuf,
    /// Cache directory
    pub cache_dir: PathBuf,
}

impl VolumePaths {
    pub fn new(base_dir: PathBuf) -> Self {
        let db_path = base_dir.join("metadata.db");
        let config_path = base_dir.join("volume.json");
        let cache_dir = base_dir.join("cache");
        Self {
            base_dir,
            db_path,
            config_path,
            cache_dir,
        }
    }

    /// Create all required directories.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.base_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        Ok(())
    }
}

/// Result of creating a new volume.
#[derive(Debug)]
pub struct CreateVolumeResult {
    pub config: VolumeConfig,
    pub paths: VolumePaths,
    pub hierarchy: KeyHierarchy,
}

/// Result of opening an existing volume.
#[derive(Debug)]
pub struct OpenVolumeResult {
    pub config: VolumeConfig,
    pub paths: VolumePaths,
    pub hierarchy: KeyHierarchy,
}

/// Compute the default base directory for volumes.
///
/// Returns the platform data directory (e.g. `~/.local/share/tgcryptfs/volumes`
/// on Linux, `~/Library/Application Support/tgcryptfs/volumes` on macOS).
///
/// # Panics
///
/// Panics if the platform data directory cannot be determined. This should
/// never happen on supported platforms (Linux, macOS).
pub fn default_volumes_dir() -> PathBuf {
    dirs::data_dir()
        .expect("unable to determine platform data directory; set TGCRYPTFS_VOLUMES_DIR")
        .join("tgcryptfs")
        .join("volumes")
}

/// Create a new volume: generate config, derive keys, persist config to disk.
pub fn create_volume(
    name: Option<&str>,
    password: &[u8],
    volumes_dir: &Path,
) -> Result<CreateVolumeResult> {
    let display_name = name
        .map(std::string::ToString::to_string)
        .unwrap_or_else(names::generate_group_name);
    let group_name = names::generate_group_name();
    let mut config = VolumeConfig::new(display_name, group_name);

    let paths = VolumePaths::new(volumes_dir.join(config.volume_id.to_string()));
    paths
        .ensure_dirs()
        .map_err(|e| CoreError::Volume(format!("create dirs: {e}")))?;

    // Derive key hierarchy
    let root_key = kdf::derive_root_key(password, &config.salt, &config.kdf_params)?;
    let hierarchy = kdf::derive_hierarchy(root_key)?;

    // Store password verification hash (allows "wrong password" detection on open)
    config.password_verification_hash = Some(compute_verification_hash(&hierarchy));

    // Save config to disk (does NOT contain keys)
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|e| CoreError::Serialization(e.to_string()))?;
    std::fs::write(&paths.config_path, config_json)
        .map_err(|e| CoreError::Volume(format!("write config: {e}")))?;

    Ok(CreateVolumeResult {
        config,
        paths,
        hierarchy,
    })
}

/// Open an existing volume: load config, derive keys from password.
pub fn open_volume(
    volume_id: &str,
    password: &[u8],
    volumes_dir: &Path,
) -> Result<OpenVolumeResult> {
    let paths = VolumePaths::new(volumes_dir.join(volume_id));

    if !paths.config_path.exists() {
        return Err(CoreError::Volume(format!(
            "volume not found: {}",
            volume_id
        )));
    }

    let config_json = std::fs::read_to_string(&paths.config_path)
        .map_err(|e| CoreError::Volume(format!("read config: {e}")))?;
    let config: VolumeConfig = serde_json::from_str(&config_json)
        .map_err(|e| CoreError::Serialization(format!("parse config: {e}")))?;

    let root_key = kdf::derive_root_key(password, &config.salt, &config.kdf_params)?;
    let hierarchy = kdf::derive_hierarchy(root_key)?;

    // Verify password if verification hash is stored (volumes created before
    // this feature won't have one, so we skip verification for those)
    if let Some(ref expected_hash) = config.password_verification_hash {
        let actual_hash = compute_verification_hash(&hierarchy);
        if &actual_hash != expected_hash {
            return Err(CoreError::Decryption(
                "wrong password: verification hash mismatch".into(),
            ));
        }
    }

    Ok(OpenVolumeResult {
        config,
        paths,
        hierarchy,
    })
}

/// List all volumes in the volumes directory.
pub fn list_volumes(volumes_dir: &Path) -> Result<Vec<VolumeConfig>> {
    if !volumes_dir.exists() {
        return Ok(Vec::new());
    }

    let mut volumes = Vec::new();
    let entries = std::fs::read_dir(volumes_dir)
        .map_err(|e| CoreError::Volume(format!("read volumes dir: {e}")))?;

    for entry in entries {
        let entry = entry.map_err(|e| CoreError::Volume(format!("dir entry: {e}")))?;
        let config_path = entry.path().join("volume.json");
        if config_path.exists() {
            if let Ok(json) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_json::from_str::<VolumeConfig>(&json) {
                    volumes.push(config);
                }
            }
        }
    }

    Ok(volumes)
}

/// Delete a volume and all its data securely.
///
/// Files are overwritten with random data before deletion to prevent
/// recovery of sensitive metadata.
pub fn delete_volume(volume_id: &str, volumes_dir: &Path) -> Result<()> {
    let vol_dir = volumes_dir.join(volume_id);
    if !vol_dir.exists() {
        return Err(CoreError::Volume(format!(
            "volume not found: {}",
            volume_id
        )));
    }
    // Shred all files before removing the directory
    shred_directory(&vol_dir)?;
    std::fs::remove_dir_all(&vol_dir)
        .map_err(|e| CoreError::Volume(format!("delete volume: {e}")))?;
    Ok(())
}

/// Overwrite all files in a directory with random data before deletion.
fn shred_directory(dir: &Path) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| CoreError::Volume(format!("read dir for shred: {e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| CoreError::Volume(format!("dir entry: {e}")))?;
        let path = entry.path();
        if path.is_dir() {
            shred_directory(&path)?;
        } else {
            shred_file(&path)?;
        }
    }
    Ok(())
}

/// Overwrite a single file with random data (3 passes), then delete.
fn shred_file(path: &Path) -> Result<()> {
    use rand::RngCore;

    let size = std::fs::metadata(path)
        .map_err(|e| CoreError::Volume(format!("stat for shred: {e}")))?
        .len() as usize;

    for _ in 0..3 {
        let mut buf = vec![0u8; size];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        std::fs::write(path, &buf).map_err(|e| CoreError::Volume(format!("shred write: {e}")))?;
        // Sync to ensure the overwrite hits disk
        let f = std::fs::File::open(path)
            .map_err(|e| CoreError::Volume(format!("open for sync: {e}")))?;
        f.sync_all()
            .map_err(|e| CoreError::Volume(format!("sync: {e}")))?;
    }

    std::fs::remove_file(path).map_err(|e| CoreError::Volume(format!("shred delete: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_open_volume() {
        let dir = tempfile::TempDir::new().unwrap();

        let created = create_volume(Some("test-vol"), b"password123", dir.path()).unwrap();
        assert_eq!(created.config.display_name, "test-vol");
        assert!(created.paths.config_path.exists());

        // Open with correct password
        let opened = open_volume(
            &created.config.volume_id.to_string(),
            b"password123",
            dir.path(),
        )
        .unwrap();
        assert_eq!(opened.config.volume_id, created.config.volume_id);

        // Keys should match
        assert_eq!(
            opened.hierarchy.data.as_bytes(),
            created.hierarchy.data.as_bytes()
        );
    }

    #[test]
    fn wrong_password_returns_error() {
        let dir = tempfile::TempDir::new().unwrap();

        let created = create_volume(Some("test"), b"correct", dir.path()).unwrap();
        let result = open_volume(&created.config.volume_id.to_string(), b"wrong", dir.path());

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("wrong password"),
            "expected wrong password error, got: {msg}"
        );
    }

    #[test]
    fn list_volumes() {
        let dir = tempfile::TempDir::new().unwrap();

        create_volume(Some("vol1"), b"pw1", dir.path()).unwrap();
        create_volume(Some("vol2"), b"pw2", dir.path()).unwrap();

        let volumes = super::list_volumes(dir.path()).unwrap();
        assert_eq!(volumes.len(), 2);
    }

    #[test]
    fn delete_volume() {
        let dir = tempfile::TempDir::new().unwrap();
        let created = create_volume(Some("temp"), b"pw", dir.path()).unwrap();
        let vol_id = created.config.volume_id.to_string();

        assert!(dir.path().join(&vol_id).exists());
        super::delete_volume(&vol_id, dir.path()).unwrap();
        assert!(!dir.path().join(&vol_id).exists());
    }

    #[test]
    fn open_nonexistent() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = open_volume("nonexistent", b"pw", dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn auto_generated_name() {
        let dir = tempfile::TempDir::new().unwrap();
        let created = create_volume(None, b"pw", dir.path()).unwrap();
        assert!(!created.config.display_name.is_empty());
    }
}
