use std::path::Path;

use crate::config::{DestructionConfig, DestructionPhase};
use crate::error::{DeadmanError, Result};

/// Tracks destruction progress.
#[derive(Debug, Clone)]
pub struct DestructionProgress {
    pub total_phases: usize,
    pub completed_phases: usize,
    pub current_phase: Option<String>,
    pub errors: Vec<String>,
}

/// Options for the destruction executor.
#[derive(Debug, Clone, Default)]
pub struct DestructionOptions {
    /// Whether to allow custom shell command execution.
    /// Default: false (custom commands are rejected with a security warning).
    pub allow_custom_commands: bool,
}

/// Executes the destruction sequence.
pub struct DestructionExecutor;

impl DestructionExecutor {
    /// Execute the full destruction sequence.
    ///
    /// Custom commands require `options.allow_custom_commands` to be set.
    /// Without this flag, `CustomCommand` phases are skipped with a security error.
    pub async fn execute(
        config: &DestructionConfig,
        db_path: Option<&Path>,
        cache_dir: Option<&Path>,
    ) -> DestructionProgress {
        Self::execute_with_options(config, db_path, cache_dir, &DestructionOptions::default()).await
    }

    /// Execute the full destruction sequence with explicit options.
    pub async fn execute_with_options(
        config: &DestructionConfig,
        db_path: Option<&Path>,
        cache_dir: Option<&Path>,
        options: &DestructionOptions,
    ) -> DestructionProgress {
        let mut progress = DestructionProgress {
            total_phases: config.phases.len(),
            completed_phases: 0,
            current_phase: None,
            errors: Vec::new(),
        };

        for phase in &config.phases {
            let phase_name = format!("{:?}", phase);
            progress.current_phase = Some(phase_name.clone());
            tracing::warn!(phase = %phase_name, "executing destruction phase");

            let result = match phase {
                DestructionPhase::WipeKeys => {
                    if config.wipe_key_hierarchy {
                        Self::wipe_keys().await
                    } else {
                        Ok(())
                    }
                }
                DestructionPhase::ShredDatabase => {
                    if config.shred_metadata_db {
                        if let Some(path) = db_path {
                            Self::shred_file(path, config.shred_passes).await
                        } else {
                            Ok(())
                        }
                    } else {
                        Ok(())
                    }
                }
                DestructionPhase::WipeCache => {
                    if config.wipe_cache {
                        if let Some(dir) = cache_dir {
                            Self::wipe_directory(dir).await
                        } else {
                            Ok(())
                        }
                    } else {
                        Ok(())
                    }
                }
                DestructionPhase::DeleteTelegramMessages => {
                    if config.delete_telegram_messages {
                        // This requires a connected Telegram client
                        // In production, we'd pass a BlockTransport reference
                        tracing::warn!("Telegram message deletion requires connected client");
                        Ok(())
                    } else {
                        Ok(())
                    }
                }
                DestructionPhase::CustomCommand { command } => {
                    if options.allow_custom_commands {
                        Self::execute_command(command).await
                    } else {
                        Err(DeadmanError::CustomCommandDenied(format!(
                            "custom command execution denied (command: '{}...'). \
                             Use --allow-custom-commands to enable",
                            &command[..command.len().min(40)]
                        )))
                    }
                }
            };

            match result {
                Ok(()) => {
                    progress.completed_phases += 1;
                    tracing::info!(phase = %phase_name, "destruction phase complete");
                }
                Err(e) => {
                    progress.errors.push(format!("{phase_name}: {e}"));
                    tracing::error!(phase = %phase_name, error = %e, "destruction phase failed");
                    // Continue with remaining phases even on error
                    progress.completed_phases += 1;
                }
            }
        }

        progress.current_phase = None;
        progress
    }

    /// Wipe cryptographic keys from memory.
    async fn wipe_keys() -> Result<()> {
        // Key wiping is handled by zeroize on drop.
        // In production, we'd also:
        // 1. Signal all threads to drop their key references
        // 2. Force-zero any remaining key material
        // 3. Unmap key pages if using mlock
        tracing::info!("key hierarchy wipe triggered");
        Ok(())
    }

    /// Overwrite a file with random data multiple times, then delete.
    async fn shred_file(path: &Path, passes: u32) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let size = std::fs::metadata(path)
            .map_err(|e| DeadmanError::Destruction(format!("stat {}: {e}", path.display())))?
            .len();

        for pass in 0..passes {
            let pattern: Vec<u8> = (0..size).map(|_| rand::random::<u8>()).collect();
            std::fs::write(path, &pattern)
                .map_err(|e| DeadmanError::Destruction(format!("overwrite pass {pass}: {e}")))?;
            // Sync to ensure write is flushed to disk
            let f = std::fs::File::open(path)
                .map_err(|e| DeadmanError::Destruction(format!("open for sync: {e}")))?;
            f.sync_all()
                .map_err(|e| DeadmanError::Destruction(format!("sync pass {pass}: {e}")))?;
        }

        // Final deletion
        std::fs::remove_file(path)
            .map_err(|e| DeadmanError::Destruction(format!("delete: {e}")))?;

        tracing::info!(path = %path.display(), passes, "file shredded");
        Ok(())
    }

    /// Recursively wipe a directory.
    async fn wipe_directory(dir: &Path) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        // First overwrite all files
        for entry in std::fs::read_dir(dir)
            .map_err(|e| DeadmanError::Destruction(format!("readdir: {e}")))?
        {
            let entry = entry.map_err(|e| DeadmanError::Destruction(format!("dir entry: {e}")))?;
            let path = entry.path();
            if path.is_dir() {
                Box::pin(Self::wipe_directory(&path)).await?;
            } else {
                // Quick single-pass overwrite for cache files
                Self::shred_file(&path, 1).await?;
            }
        }

        // Remove the directory
        std::fs::remove_dir_all(dir)
            .map_err(|e| DeadmanError::Destruction(format!("rmdir: {e}")))?;

        tracing::info!(dir = %dir.display(), "directory wiped");
        Ok(())
    }

    /// Execute a custom cleanup command.
    async fn execute_command(command: &str) -> Result<()> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .map_err(|e| DeadmanError::Destruction(format!("exec {command}: {e}")))?;

        if !output.status.success() {
            return Err(DeadmanError::Destruction(format!(
                "command failed with exit code {}",
                output.status.code().unwrap_or(-1)
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DestructionConfig;

    #[tokio::test]
    async fn shred_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("secret.db");
        std::fs::write(&path, b"sensitive data here").unwrap();

        DestructionExecutor::shred_file(&path, 3).await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn wipe_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let sub = dir.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("file1"), b"data1").unwrap();
        std::fs::write(sub.join("file2"), b"data2").unwrap();

        DestructionExecutor::wipe_directory(&sub).await.unwrap();
        assert!(!sub.exists());
    }

    #[tokio::test]
    async fn full_destruction_sequence() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("meta.db");
        let cache_dir = dir.path().join("cache");

        std::fs::write(&db_path, b"database content").unwrap();
        std::fs::create_dir(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("block1"), b"cached block").unwrap();

        let config = DestructionConfig {
            phases: vec![
                DestructionPhase::WipeKeys,
                DestructionPhase::ShredDatabase,
                DestructionPhase::WipeCache,
            ],
            delete_telegram_messages: false,
            shred_metadata_db: true,
            shred_passes: 2,
            wipe_key_hierarchy: true,
            wipe_cache: true,
        };

        let progress =
            DestructionExecutor::execute(&config, Some(&db_path), Some(&cache_dir)).await;

        assert_eq!(progress.total_phases, 3);
        assert_eq!(progress.completed_phases, 3);
        assert!(progress.errors.is_empty());
        assert!(!db_path.exists());
        assert!(!cache_dir.exists());
    }

    #[tokio::test]
    async fn shred_nonexistent_file() {
        let path = Path::new("/tmp/nonexistent_deadman_test_file");
        // Should be a no-op, not an error
        DestructionExecutor::shred_file(path, 3).await.unwrap();
    }

    #[tokio::test]
    async fn custom_command_phase_allowed() {
        let dir = tempfile::TempDir::new().unwrap();
        let marker = dir.path().join("marker");
        std::fs::write(&marker, b"exists").unwrap();

        let config = DestructionConfig {
            phases: vec![DestructionPhase::CustomCommand {
                command: format!("rm {}", marker.display()),
            }],
            delete_telegram_messages: false,
            shred_metadata_db: false,
            shred_passes: 1,
            wipe_key_hierarchy: false,
            wipe_cache: false,
        };

        let options = DestructionOptions {
            allow_custom_commands: true,
        };
        let progress =
            DestructionExecutor::execute_with_options(&config, None, None, &options).await;
        assert_eq!(progress.completed_phases, 1);
        assert!(progress.errors.is_empty());
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn custom_command_denied_by_default() {
        let config = DestructionConfig {
            phases: vec![DestructionPhase::CustomCommand {
                command: "echo hello".to_string(),
            }],
            delete_telegram_messages: false,
            shred_metadata_db: false,
            shred_passes: 1,
            wipe_key_hierarchy: false,
            wipe_cache: false,
        };

        let progress = DestructionExecutor::execute(&config, None, None).await;
        assert_eq!(progress.completed_phases, 1); // Phase still counted as "completed" (attempted)
        assert_eq!(progress.errors.len(), 1);
        assert!(progress.errors[0].contains("custom command execution denied"));
    }
}
