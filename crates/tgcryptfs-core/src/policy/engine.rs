use super::types::{DeleteBehavior, MutabilityPolicy};
use crate::error::{CoreError, Result};

/// Policy evaluation engine.
///
/// Given a set of policies, evaluates whether operations are allowed
/// on specific paths and determines the behavior for each operation.
pub struct PolicyEngine {
    policies: Vec<MutabilityPolicy>,
}

/// Result of evaluating a policy for a given path.
#[derive(Debug)]
pub struct PolicyDecision {
    /// The policy that matched
    pub policy_id: u32,
    /// Whether the path is mutable
    pub mutable: bool,
    /// What happens on delete
    pub on_delete: DeleteBehavior,
    /// Whether changes should be recorded in snapshots
    pub record_changes: bool,
    /// Retention period in seconds (if soft delete)
    pub retention_secs: Option<u64>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// Register a policy.
    pub fn add_policy(&mut self, policy: MutabilityPolicy) {
        self.policies.push(policy);
    }

    /// Evaluate a path against a specific policy.
    pub fn evaluate(&self, policy_id: u32, path: &str) -> Result<PolicyDecision> {
        let policy = self
            .policies
            .iter()
            .find(|p| p.policy_id == policy_id)
            .ok_or_else(|| CoreError::Policy(format!("policy {policy_id} not found")))?;

        let rule = policy.match_rule(path).ok_or_else(|| {
            CoreError::Policy(format!(
                "no rule matches path '{path}' in policy '{}'",
                policy.name
            ))
        })?;

        Ok(PolicyDecision {
            policy_id,
            mutable: rule.mutable,
            on_delete: rule.on_delete,
            record_changes: rule.record_changes,
            retention_secs: rule.retention_secs,
        })
    }

    /// Check if a write operation is allowed on a path.
    pub fn can_write(&self, policy_id: u32, path: &str) -> Result<bool> {
        Ok(self.evaluate(policy_id, path)?.mutable)
    }

    /// Check if a delete operation is allowed on a path.
    pub fn can_delete(&self, policy_id: u32, path: &str) -> Result<bool> {
        let decision = self.evaluate(policy_id, path)?;
        Ok(decision.on_delete != DeleteBehavior::Reject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::types::{ExpiryBehavior, PolicyRule};

    fn test_policy() -> MutabilityPolicy {
        MutabilityPolicy {
            policy_id: 1,
            name: "test".into(),
            rules: vec![
                PolicyRule {
                    path_pattern: "readonly/*".into(),
                    on_delete: DeleteBehavior::Reject,
                    record_changes: true,
                    mutable: false,
                    retention_secs: None,
                    on_expiry: ExpiryBehavior::Hold,
                },
                PolicyRule {
                    path_pattern: "**".into(),
                    on_delete: DeleteBehavior::Soft,
                    record_changes: true,
                    mutable: true,
                    retention_secs: Some(86400 * 30),
                    on_expiry: ExpiryBehavior::Hold,
                },
            ],
        }
    }

    #[test]
    fn evaluate_mutable_path() {
        let mut engine = PolicyEngine::new();
        engine.add_policy(test_policy());
        let decision = engine.evaluate(1, "docs/readme.md").unwrap();
        assert!(decision.mutable);
        assert_eq!(decision.on_delete, DeleteBehavior::Soft);
    }

    #[test]
    fn evaluate_readonly_path() {
        let mut engine = PolicyEngine::new();
        engine.add_policy(test_policy());
        let decision = engine.evaluate(1, "readonly/config.yml").unwrap();
        assert!(!decision.mutable);
        assert_eq!(decision.on_delete, DeleteBehavior::Reject);
    }

    #[test]
    fn can_write_mutable() {
        let mut engine = PolicyEngine::new();
        engine.add_policy(test_policy());
        assert!(engine.can_write(1, "docs/file.txt").unwrap());
    }

    #[test]
    fn can_write_readonly() {
        let mut engine = PolicyEngine::new();
        engine.add_policy(test_policy());
        assert!(!engine.can_write(1, "readonly/file.txt").unwrap());
    }

    #[test]
    fn can_delete_soft() {
        let mut engine = PolicyEngine::new();
        engine.add_policy(test_policy());
        assert!(engine.can_delete(1, "docs/file.txt").unwrap());
    }

    #[test]
    fn can_delete_rejected() {
        let mut engine = PolicyEngine::new();
        engine.add_policy(test_policy());
        assert!(!engine.can_delete(1, "readonly/file.txt").unwrap());
    }

    #[test]
    fn unknown_policy_errors() {
        let engine = PolicyEngine::new();
        assert!(engine.evaluate(99, "any/path").is_err());
    }
}
