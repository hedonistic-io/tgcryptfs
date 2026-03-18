use std::time::Duration;

use crate::config::{TriggerConfig, TriggerParams};

/// Result of evaluating a trigger.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerResult {
    /// Trigger condition is satisfied (safe, no action needed).
    Ok,
    /// Trigger has fired (condition violated, destruction should begin).
    Fired { reason: String },
    /// Trigger evaluation failed (could not determine state).
    Error { message: String },
}

/// Evaluates trigger conditions.
pub struct TriggerEvaluator;

impl TriggerEvaluator {
    /// Evaluate a single trigger.
    pub async fn evaluate(config: &TriggerConfig) -> TriggerResult {
        if !config.active {
            return TriggerResult::Ok;
        }

        match &config.params {
            TriggerParams::Heartbeat { timeout_secs } => Self::eval_heartbeat(*timeout_secs).await,
            TriggerParams::NetworkCheck {
                host,
                port,
                timeout_secs,
            } => Self::eval_network(host, *port, *timeout_secs).await,
            TriggerParams::RpcCheck {
                url,
                expected_status,
                timeout_secs,
            } => Self::eval_rpc(url, *expected_status, *timeout_secs).await,
            TriggerParams::OsEvent { event_type } => Self::eval_os_event(event_type).await,
            TriggerParams::Custom {
                command,
                timeout_secs,
            } => Self::eval_custom(command, *timeout_secs).await,
        }
    }

    /// Evaluate all triggers. Returns Fired if ANY trigger fires.
    pub async fn evaluate_all(configs: &[TriggerConfig]) -> TriggerResult {
        for config in configs {
            let result = Self::evaluate(config).await;
            if let TriggerResult::Fired { .. } = &result {
                return result;
            }
        }
        TriggerResult::Ok
    }

    async fn eval_heartbeat(_timeout_secs: u64) -> TriggerResult {
        // Heartbeat evaluation is handled by the deadman daemon checking timestamps.
        // The trigger fires when the last check-in exceeds the timeout.
        // This is evaluated externally by comparing last_check against current time.
        TriggerResult::Ok
    }

    async fn eval_network(host: &str, port: u16, timeout_secs: u64) -> TriggerResult {
        let addr = format!("{host}:{port}");
        let timeout = Duration::from_secs(timeout_secs);

        match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => TriggerResult::Ok,
            Ok(Err(e)) => TriggerResult::Fired {
                reason: format!("network check failed: {addr}: {e}"),
            },
            Err(_) => TriggerResult::Fired {
                reason: format!("network check timed out: {addr}"),
            },
        }
    }

    async fn eval_rpc(_url: &str, _expected_status: u16, _timeout_secs: u64) -> TriggerResult {
        // RPC checks require an HTTP client - placeholder for now.
        // In production, this would use reqwest or hyper to check the endpoint.
        TriggerResult::Error {
            message: "RPC checks require HTTP client (not yet implemented)".into(),
        }
    }

    async fn eval_os_event(event_type: &crate::config::OsEventType) -> TriggerResult {
        // OS event monitoring is platform-specific.
        // In production, this would use OS-specific APIs.
        TriggerResult::Error {
            message: format!(
                "OS event monitoring not yet implemented for {:?}",
                event_type
            ),
        }
    }

    async fn eval_custom(command: &str, timeout_secs: u64) -> TriggerResult {
        let timeout = Duration::from_secs(timeout_secs);

        let result = tokio::time::timeout(timeout, async {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
                .await
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                if output.status.success() {
                    TriggerResult::Ok
                } else {
                    TriggerResult::Fired {
                        reason: format!(
                            "custom command exited with code {}",
                            output.status.code().unwrap_or(-1)
                        ),
                    }
                }
            }
            Ok(Err(e)) => TriggerResult::Error {
                message: format!("failed to execute command: {e}"),
            },
            Err(_) => TriggerResult::Fired {
                reason: format!("custom command timed out after {timeout_secs}s"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TriggerType;

    fn heartbeat_trigger() -> TriggerConfig {
        TriggerConfig {
            id: "hb-1".into(),
            name: "test heartbeat".into(),
            trigger_type: TriggerType::Outgoing,
            active: true,
            params: TriggerParams::Heartbeat { timeout_secs: 60 },
        }
    }

    #[tokio::test]
    async fn inactive_trigger_is_ok() {
        let mut t = heartbeat_trigger();
        t.active = false;
        let result = TriggerEvaluator::evaluate(&t).await;
        assert_eq!(result, TriggerResult::Ok);
    }

    #[tokio::test]
    async fn heartbeat_returns_ok() {
        let result = TriggerEvaluator::evaluate(&heartbeat_trigger()).await;
        assert_eq!(result, TriggerResult::Ok);
    }

    #[tokio::test]
    async fn custom_command_success() {
        let t = TriggerConfig {
            id: "custom-1".into(),
            name: "true command".into(),
            trigger_type: TriggerType::Incoming,
            active: true,
            params: TriggerParams::Custom {
                command: "true".into(),
                timeout_secs: 5,
            },
        };
        let result = TriggerEvaluator::evaluate(&t).await;
        assert_eq!(result, TriggerResult::Ok);
    }

    #[tokio::test]
    async fn custom_command_failure() {
        let t = TriggerConfig {
            id: "custom-2".into(),
            name: "false command".into(),
            trigger_type: TriggerType::Incoming,
            active: true,
            params: TriggerParams::Custom {
                command: "false".into(),
                timeout_secs: 5,
            },
        };
        let result = TriggerEvaluator::evaluate(&t).await;
        assert!(matches!(result, TriggerResult::Fired { .. }));
    }

    #[tokio::test]
    async fn evaluate_all_one_fires() {
        let triggers = vec![
            heartbeat_trigger(),
            TriggerConfig {
                id: "custom-fail".into(),
                name: "failing trigger".into(),
                trigger_type: TriggerType::Incoming,
                active: true,
                params: TriggerParams::Custom {
                    command: "false".into(),
                    timeout_secs: 5,
                },
            },
        ];
        let result = TriggerEvaluator::evaluate_all(&triggers).await;
        assert!(matches!(result, TriggerResult::Fired { .. }));
    }

    #[tokio::test]
    async fn network_check_unreachable() {
        let t = TriggerConfig {
            id: "net-1".into(),
            name: "bad host".into(),
            trigger_type: TriggerType::Network,
            active: true,
            params: TriggerParams::NetworkCheck {
                host: "192.0.2.1".into(), // TEST-NET, should be unreachable
                port: 1,
                timeout_secs: 1,
            },
        };
        let result = TriggerEvaluator::evaluate(&t).await;
        assert!(matches!(result, TriggerResult::Fired { .. }));
    }
}
