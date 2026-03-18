use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::config::DeadmanConfig;
use crate::destroy::DestructionExecutor;
use crate::error::Result;
use crate::hooks::TriggerResult;
use crate::trigger::{DeadmanController, DeadmanState};

/// Background daemon that monitors deadman triggers.
pub struct DeadmanDaemon {
    controller: DeadmanController,
    check_interval: Duration,
    grace_period: Duration,
    shutdown: Arc<AtomicBool>,
}

/// Result of a daemon run (for testing/reporting).
#[derive(Debug, Clone, PartialEq)]
pub enum DaemonOutcome {
    /// Shutdown was signaled.
    Shutdown,
    /// A trigger fired and grace period expired — destruction executed.
    Destroyed { reason: String },
    /// A trigger fired but was disarmed during grace period.
    Disarmed,
}

impl DeadmanDaemon {
    pub fn new(config: DeadmanConfig) -> Self {
        let check_interval = Duration::from_secs(config.check_interval_secs);
        let grace_period = Duration::from_secs(config.grace_period_secs);
        let shutdown = Arc::new(AtomicBool::new(false));

        Self {
            controller: DeadmanController::new(config),
            check_interval,
            grace_period,
            shutdown,
        }
    }

    /// Get a handle to signal shutdown.
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    /// Arm the daemon and return a reference to the controller.
    pub fn arm(&self) -> Result<()> {
        self.controller.arm()
    }

    /// Disarm the daemon.
    pub fn disarm(&self) -> Result<()> {
        self.controller.disarm()?;
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Get the controller state.
    pub fn state(&self) -> DeadmanState {
        self.controller.state()
    }

    /// Get the config.
    pub fn config(&self) -> &DeadmanConfig {
        self.controller.config()
    }

    /// Run the daemon loop. Blocks until shutdown or destruction.
    pub async fn run(&self) -> DaemonOutcome {
        tracing::info!("deadman daemon started");

        loop {
            // Check for shutdown
            if self.shutdown.load(Ordering::SeqCst) {
                tracing::info!("deadman daemon: shutdown signaled");
                return DaemonOutcome::Shutdown;
            }

            // Skip check if not armed
            if !self.controller.is_armed() {
                tokio::time::sleep(self.check_interval).await;
                continue;
            }

            // Run trigger evaluation
            let result = self.controller.check().await;

            match result {
                TriggerResult::Ok => {
                    tracing::debug!("deadman check: all triggers OK");
                }
                TriggerResult::Fired { reason } => {
                    tracing::warn!(reason = %reason, "deadman trigger fired, entering grace period");

                    // Enter grace period
                    let grace_start = tokio::time::Instant::now();

                    while grace_start.elapsed() < self.grace_period {
                        // Check if disarmed during grace period
                        if !self.controller.is_armed() || self.shutdown.load(Ordering::SeqCst) {
                            tracing::info!("deadman disarmed during grace period");
                            return DaemonOutcome::Disarmed;
                        }

                        let remaining = self.grace_period - grace_start.elapsed();
                        tracing::warn!(
                            remaining_secs = remaining.as_secs(),
                            "grace period countdown"
                        );

                        // Sleep for a short interval during grace period
                        tokio::time::sleep(Duration::from_secs(1).min(remaining)).await;
                    }

                    // Grace period expired — execute destruction
                    tracing::error!("grace period expired, executing destruction");
                    let config = self.controller.config();
                    let progress =
                        DestructionExecutor::execute(&config.destruction, None, None).await;

                    tracing::error!(
                        phases = progress.completed_phases,
                        total = progress.total_phases,
                        errors = progress.errors.len(),
                        "destruction sequence complete"
                    );

                    return DaemonOutcome::Destroyed { reason };
                }
                TriggerResult::Error { message } => {
                    tracing::error!(error = %message, "trigger evaluation error");
                }
            }

            // Wait for next check
            tokio::time::sleep(self.check_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TriggerConfig, TriggerParams, TriggerType};

    fn fast_config(trigger_command: &str) -> DeadmanConfig {
        DeadmanConfig {
            enabled: true,
            check_interval_secs: 0, // No delay for tests
            grace_period_secs: 1,
            max_missed_checks: 3,
            triggers: vec![TriggerConfig {
                id: "test".into(),
                name: "test trigger".into(),
                trigger_type: TriggerType::Incoming,
                active: true,
                params: TriggerParams::Custom {
                    command: trigger_command.to_string(),
                    timeout_secs: 5,
                },
            }],
            destruction: Default::default(),
        }
    }

    #[tokio::test]
    async fn daemon_shutdown() {
        let daemon = DeadmanDaemon::new(fast_config("true"));
        daemon.arm().unwrap();

        let shutdown = daemon.shutdown_handle();

        // Signal shutdown after a short delay
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            shutdown.store(true, Ordering::SeqCst);
        });

        let outcome = daemon.run().await;
        assert_eq!(outcome, DaemonOutcome::Shutdown);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn daemon_trigger_fires_and_destroys() {
        let mut config = fast_config("false"); // trigger fires immediately
        config.grace_period_secs = 0; // no grace period for fast test
        config.destruction.phases = vec![]; // no actual destruction

        let daemon = DeadmanDaemon::new(config);
        daemon.arm().unwrap();

        let outcome = daemon.run().await;
        assert!(matches!(outcome, DaemonOutcome::Destroyed { .. }));
    }

    #[tokio::test]
    async fn daemon_disarm_during_grace_period() {
        let mut config = fast_config("false"); // trigger fires
        config.grace_period_secs = 5; // long grace period
        config.destruction.phases = vec![];

        let daemon = DeadmanDaemon::new(config);
        daemon.arm().unwrap();

        let shutdown = daemon.shutdown_handle();

        // Disarm after trigger fires but during grace period
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            shutdown.store(true, Ordering::SeqCst);
        });

        let outcome = daemon.run().await;
        assert_eq!(outcome, DaemonOutcome::Disarmed);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn daemon_ok_trigger_loops() {
        let daemon = DeadmanDaemon::new(fast_config("true")); // trigger OK
        daemon.arm().unwrap();

        let shutdown = daemon.shutdown_handle();

        // Let it loop a few times, then shutdown
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            shutdown.store(true, Ordering::SeqCst);
        });

        let outcome = daemon.run().await;
        assert_eq!(outcome, DaemonOutcome::Shutdown);
        handle.await.unwrap();
    }
}
