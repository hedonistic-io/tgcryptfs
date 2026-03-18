use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;

use tgcryptfs_deadman::config::DeadmanConfig;
use tgcryptfs_deadman::daemon::{DaemonOutcome, DeadmanDaemon};

use crate::error::{ApiError, Result};

/// Tracks the state of the in-process deadman daemon.
struct DaemonState {
    config: DeadmanConfig,
    shutdown_handle: Arc<AtomicBool>,
    #[allow(dead_code)]
    task_handle: tokio::task::JoinHandle<DaemonOutcome>,
}

/// Service managing the deadman daemon lifecycle.
pub struct DeadmanService {
    state: Arc<RwLock<Option<DaemonState>>>,
}

impl Default for DeadmanService {
    fn default() -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
        }
    }
}

impl DeadmanService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Arm the deadman switch with the given config, spawning a background task.
    pub async fn arm(&self, config: DeadmanConfig) -> Result<()> {
        let mut state = self.state.write().await;
        if state.is_some() {
            return Err(ApiError::InvalidArgument("deadman is already armed".into()));
        }

        let daemon = DeadmanDaemon::new(config.clone());
        daemon
            .arm()
            .map_err(|e| ApiError::Internal(format!("arm deadman: {e}")))?;

        let shutdown = daemon.shutdown_handle();
        let handle = tokio::spawn(async move { daemon.run().await });

        *state = Some(DaemonState {
            config,
            shutdown_handle: shutdown,
            task_handle: handle,
        });

        tracing::info!("deadman switch armed");
        Ok(())
    }

    /// Disarm the deadman switch by signaling shutdown.
    pub async fn disarm(&self) -> Result<()> {
        let mut state = self.state.write().await;
        let s = state
            .take()
            .ok_or_else(|| ApiError::InvalidArgument("deadman is not armed".into()))?;

        s.shutdown_handle.store(true, Ordering::SeqCst);
        // Don't await the task — it will finish on its own
        tracing::info!("deadman switch disarmed");
        Ok(())
    }

    /// Check if the deadman is currently armed.
    pub async fn is_armed(&self) -> bool {
        let state = self.state.read().await;
        state.is_some()
    }

    /// Get the deadman status including config details.
    pub async fn status(&self) -> DeadmanStatus {
        let state = self.state.read().await;
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("tgcryptfs")
            .join("deadman.json");

        match &*state {
            Some(s) => DeadmanStatus {
                configured: true,
                armed: true,
                check_interval_secs: s.config.check_interval_secs,
                grace_period_secs: s.config.grace_period_secs,
                trigger_count: s.config.triggers.len(),
                config_path: config_path.display().to_string(),
            },
            None => {
                let configured = config_path.exists();
                DeadmanStatus {
                    configured,
                    armed: false,
                    check_interval_secs: 0,
                    grace_period_secs: 0,
                    trigger_count: 0,
                    config_path: config_path.display().to_string(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeadmanStatus {
    pub configured: bool,
    pub armed: bool,
    pub check_interval_secs: u64,
    pub grace_period_secs: u64,
    pub trigger_count: usize,
    pub config_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initially_not_armed() {
        let svc = DeadmanService::new();
        assert!(!svc.is_armed().await);
    }

    #[tokio::test]
    async fn status_when_not_armed() {
        let svc = DeadmanService::new();
        let status = svc.status().await;
        assert!(!status.armed);
        assert_eq!(status.check_interval_secs, 0);
    }

    #[tokio::test]
    async fn arm_and_disarm() {
        let svc = DeadmanService::new();
        let config = DeadmanConfig::default();

        svc.arm(config).await.unwrap();
        assert!(svc.is_armed().await);

        let status = svc.status().await;
        assert!(status.armed);

        svc.disarm().await.unwrap();
        assert!(!svc.is_armed().await);
    }

    #[tokio::test]
    async fn double_arm_fails() {
        let svc = DeadmanService::new();
        svc.arm(DeadmanConfig::default()).await.unwrap();

        let err = svc.arm(DeadmanConfig::default()).await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidArgument(_)));

        svc.disarm().await.unwrap();
    }

    #[tokio::test]
    async fn disarm_when_not_armed_fails() {
        let svc = DeadmanService::new();
        let err = svc.disarm().await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidArgument(_)));
    }
}
