use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::DeadmanConfig;
use crate::error::{DeadmanError, Result};
use crate::hooks::{TriggerEvaluator, TriggerResult};

/// Deadman state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum DeadmanState {
    /// Deadman is disarmed (disabled).
    Disarmed,
    /// Deadman is armed and monitoring triggers.
    Armed,
    /// A trigger has fired, waiting for grace period.
    GracePeriod { reason: String, remaining_secs: u64 },
    /// Destruction sequence is in progress.
    Destroying,
    /// Destruction complete.
    Destroyed,
}

/// Controls the deadman lifecycle.
pub struct DeadmanController {
    config: DeadmanConfig,
    armed: AtomicBool,
    state: std::sync::Mutex<DeadmanState>,
    shutdown: Arc<AtomicBool>,
}

impl DeadmanController {
    pub fn new(config: DeadmanConfig) -> Self {
        Self {
            armed: AtomicBool::new(false),
            state: std::sync::Mutex::new(DeadmanState::Disarmed),
            config,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Arm the deadman switch.
    pub fn arm(&self) -> Result<()> {
        if self.armed.load(Ordering::SeqCst) {
            return Err(DeadmanError::AlreadyArmed);
        }
        self.armed.store(true, Ordering::SeqCst);
        *self.state.lock().unwrap() = DeadmanState::Armed;
        tracing::info!("Deadman switch armed");
        Ok(())
    }

    /// Disarm the deadman switch.
    pub fn disarm(&self) -> Result<()> {
        if !self.armed.load(Ordering::SeqCst) {
            return Err(DeadmanError::NotArmed);
        }
        self.armed.store(false, Ordering::SeqCst);
        *self.state.lock().unwrap() = DeadmanState::Disarmed;
        self.shutdown.store(true, Ordering::SeqCst);
        tracing::info!("Deadman switch disarmed");
        Ok(())
    }

    /// Check if armed.
    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }

    /// Get current state.
    pub fn state(&self) -> DeadmanState {
        self.state.lock().unwrap().clone()
    }

    /// Run a single check cycle. Returns the trigger result.
    pub async fn check(&self) -> TriggerResult {
        if !self.is_armed() {
            return TriggerResult::Ok;
        }

        TriggerEvaluator::evaluate_all(&self.config.triggers).await
    }

    /// Signal the controller to stop.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Get the config.
    pub fn config(&self) -> &DeadmanConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TriggerConfig, TriggerParams, TriggerType};

    fn test_config() -> DeadmanConfig {
        DeadmanConfig {
            enabled: true,
            check_interval_secs: 1,
            grace_period_secs: 5,
            max_missed_checks: 3,
            triggers: vec![TriggerConfig {
                id: "test".into(),
                name: "test trigger".into(),
                trigger_type: TriggerType::Incoming,
                active: true,
                params: TriggerParams::Custom {
                    command: "true".into(),
                    timeout_secs: 5,
                },
            }],
            destruction: Default::default(),
        }
    }

    #[test]
    fn arm_disarm() {
        let ctrl = DeadmanController::new(test_config());
        assert!(!ctrl.is_armed());
        assert_eq!(ctrl.state(), DeadmanState::Disarmed);

        ctrl.arm().unwrap();
        assert!(ctrl.is_armed());
        assert_eq!(ctrl.state(), DeadmanState::Armed);

        ctrl.disarm().unwrap();
        assert!(!ctrl.is_armed());
        assert_eq!(ctrl.state(), DeadmanState::Disarmed);
    }

    #[test]
    fn double_arm_error() {
        let ctrl = DeadmanController::new(test_config());
        ctrl.arm().unwrap();
        assert!(ctrl.arm().is_err());
    }

    #[test]
    fn disarm_when_not_armed() {
        let ctrl = DeadmanController::new(test_config());
        assert!(ctrl.disarm().is_err());
    }

    #[tokio::test]
    async fn check_when_disarmed() {
        let ctrl = DeadmanController::new(test_config());
        let result = ctrl.check().await;
        assert_eq!(result, TriggerResult::Ok);
    }

    #[tokio::test]
    async fn check_when_armed_ok() {
        let ctrl = DeadmanController::new(test_config());
        ctrl.arm().unwrap();
        let result = ctrl.check().await;
        assert_eq!(result, TriggerResult::Ok);
    }

    #[tokio::test]
    async fn check_fires_on_failure() {
        let mut config = test_config();
        config.triggers[0].params = TriggerParams::Custom {
            command: "false".into(),
            timeout_secs: 5,
        };
        let ctrl = DeadmanController::new(config);
        ctrl.arm().unwrap();
        let result = ctrl.check().await;
        assert!(matches!(result, TriggerResult::Fired { .. }));
    }
}
