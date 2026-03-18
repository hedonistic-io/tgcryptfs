use serde::{Deserialize, Serialize};

/// Top-level deadman configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadmanConfig {
    /// Whether the deadman option is enabled.
    pub enabled: bool,
    /// Check interval in seconds.
    pub check_interval_secs: u64,
    /// Grace period in seconds after trigger fires before destruction begins.
    pub grace_period_secs: u64,
    /// Maximum number of missed checks before auto-trigger.
    pub max_missed_checks: u32,
    /// List of trigger configurations.
    pub triggers: Vec<TriggerConfig>,
    /// Destruction sequence configuration.
    pub destruction: DestructionConfig,
}

impl Default for DeadmanConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            check_interval_secs: 300, // 5 minutes
            grace_period_secs: 60,
            max_missed_checks: 3,
            triggers: Vec::new(),
            destruction: DestructionConfig::default(),
        }
    }
}

/// Configuration for a single trigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    /// Unique trigger identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Type of trigger.
    pub trigger_type: TriggerType,
    /// Whether this trigger is active.
    pub active: bool,
    /// Trigger-specific parameters.
    pub params: TriggerParams,
}

/// The five hook/trigger types per the BRD.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TriggerType {
    /// Monitors incoming signals (network packets, messages, etc.).
    Incoming,
    /// Monitors outgoing signals (heartbeat failures, etc.).
    Outgoing,
    /// Monitors OS-level events (user login, process termination, etc.).
    Os,
    /// Monitors network conditions (connectivity loss, specific endpoints).
    Network,
    /// Monitors RPC endpoints (external service health checks).
    Rpc,
}

/// Trigger-specific parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerParams {
    /// Heartbeat: if no check-in within timeout, trigger fires.
    Heartbeat { timeout_secs: u64 },
    /// Network: monitor connectivity to a host.
    NetworkCheck {
        host: String,
        port: u16,
        timeout_secs: u64,
    },
    /// RPC: check an HTTP/gRPC endpoint.
    RpcCheck {
        url: String,
        expected_status: u16,
        timeout_secs: u64,
    },
    /// OS: monitor for specific process or user session.
    OsEvent { event_type: OsEventType },
    /// Custom: user-defined shell command that returns exit code 0 (ok) or non-zero (trigger).
    Custom { command: String, timeout_secs: u64 },
}

/// OS event types that can trigger the deadman.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OsEventType {
    /// Triggered when a specific user logs in.
    UserLogin { username: String },
    /// Triggered when system is shutting down.
    Shutdown,
    /// Triggered when a specific process starts.
    ProcessStart { name: String },
    /// Triggered when a USB device is inserted.
    UsbInsert,
}

/// Configuration for the destruction sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestructionConfig {
    /// Phases of destruction, executed in order.
    pub phases: Vec<DestructionPhase>,
    /// Whether to delete Telegram messages.
    pub delete_telegram_messages: bool,
    /// Whether to overwrite the metadata database.
    pub shred_metadata_db: bool,
    /// Number of overwrite passes for shredding.
    pub shred_passes: u32,
    /// Whether to wipe the key hierarchy from memory.
    pub wipe_key_hierarchy: bool,
    /// Whether to wipe the local cache.
    pub wipe_cache: bool,
}

impl Default for DestructionConfig {
    fn default() -> Self {
        Self {
            phases: vec![
                DestructionPhase::WipeKeys,
                DestructionPhase::ShredDatabase,
                DestructionPhase::WipeCache,
                DestructionPhase::DeleteTelegramMessages,
            ],
            delete_telegram_messages: true,
            shred_metadata_db: true,
            shred_passes: 3,
            wipe_key_hierarchy: true,
            wipe_cache: true,
        }
    }
}

/// Individual destruction phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DestructionPhase {
    /// Wipe all keys from memory.
    WipeKeys,
    /// Overwrite and delete the metadata SQLite database.
    ShredDatabase,
    /// Delete the local block cache.
    WipeCache,
    /// Delete messages from Telegram Saved Messages.
    DeleteTelegramMessages,
    /// Run a custom cleanup command.
    CustomCommand { command: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = DeadmanConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.check_interval_secs, 300);
        assert_eq!(config.grace_period_secs, 60);
        assert!(config.triggers.is_empty());
    }

    #[test]
    fn serialization_roundtrip() {
        let config = DeadmanConfig {
            enabled: true,
            check_interval_secs: 60,
            grace_period_secs: 30,
            max_missed_checks: 5,
            triggers: vec![TriggerConfig {
                id: "heartbeat-1".into(),
                name: "Main heartbeat".into(),
                trigger_type: TriggerType::Outgoing,
                active: true,
                params: TriggerParams::Heartbeat { timeout_secs: 120 },
            }],
            destruction: DestructionConfig::default(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DeadmanConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.triggers.len(), 1);
        assert_eq!(deserialized.triggers[0].trigger_type, TriggerType::Outgoing);
    }

    #[test]
    fn trigger_types() {
        let types = vec![
            TriggerType::Incoming,
            TriggerType::Outgoing,
            TriggerType::Os,
            TriggerType::Network,
            TriggerType::Rpc,
        ];
        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let rt: TriggerType = serde_json::from_str(&json).unwrap();
            assert_eq!(rt, t);
        }
    }

    #[test]
    fn destruction_phases() {
        let config = DestructionConfig::default();
        assert_eq!(config.phases.len(), 4);
        assert_eq!(config.phases[0], DestructionPhase::WipeKeys);
        assert_eq!(config.shred_passes, 3);
    }
}
