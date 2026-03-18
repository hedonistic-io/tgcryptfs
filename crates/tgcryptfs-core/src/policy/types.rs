use serde::{Deserialize, Serialize};

/// A named mutability policy with ordered rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutabilityPolicy {
    /// Unique policy ID
    pub policy_id: u32,
    /// Human-readable name
    pub name: String,
    /// Policy rules (ordered, first match wins)
    pub rules: Vec<PolicyRule>,
}

/// A single policy rule matching paths to behaviors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Glob pattern for matching paths
    pub path_pattern: String,
    /// What happens on delete
    pub on_delete: DeleteBehavior,
    /// Whether changes are recorded in snapshots
    pub record_changes: bool,
    /// Whether the path is mutable at all
    pub mutable: bool,
    /// Retention period for soft-deleted items (None = forever)
    pub retention_secs: Option<u64>,
    /// What happens when retention expires
    pub on_expiry: ExpiryBehavior,
}

/// Behavior when a file is deleted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeleteBehavior {
    /// Blocks deleted immediately, no record
    Immediate,
    /// File removed from listing, blocks retained
    Soft,
    /// Delete rejected
    Reject,
}

/// Behavior when a soft-deleted item's retention expires.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExpiryBehavior {
    /// Purge blocks automatically
    Purge,
    /// Keep blocks, mark as expired (require manual purge)
    Hold,
}

impl MutabilityPolicy {
    /// Find the first rule that matches the given path.
    pub fn match_rule(&self, path: &str) -> Option<&PolicyRule> {
        self.rules
            .iter()
            .find(|rule| glob_matches(&rule.path_pattern, path))
    }
}

/// Simple glob matching supporting `*` and `**`.
fn glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == "**" || pattern == "**/*" {
        return true;
    }

    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();

    glob_match_parts(&pattern_parts, &path_parts)
}

fn glob_match_parts(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        // ** matches zero or more path segments
        for i in 0..=path.len() {
            if glob_match_parts(&pattern[1..], &path[i..]) {
                return true;
            }
        }
        return false;
    }
    if path.is_empty() {
        return false;
    }
    if glob_match_segment(pattern[0], path[0]) {
        return glob_match_parts(&pattern[1..], &path[1..]);
    }
    false
}

fn glob_match_segment(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return segment.ends_with(suffix);
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return segment.starts_with(prefix);
    }
    pattern == segment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_exact() {
        assert!(glob_matches("foo/bar.txt", "foo/bar.txt"));
        assert!(!glob_matches("foo/bar.txt", "foo/baz.txt"));
    }

    #[test]
    fn glob_matches_star() {
        assert!(glob_matches("foo/*", "foo/bar.txt"));
        assert!(!glob_matches("foo/*", "foo/sub/bar.txt"));
    }

    #[test]
    fn glob_matches_double_star() {
        assert!(glob_matches("**", "foo/bar/baz"));
        assert!(glob_matches("**/*.txt", "foo/bar/test.txt"));
    }

    #[test]
    fn glob_matches_extension() {
        assert!(glob_matches("**/*.log", "var/log/app.log"));
        assert!(!glob_matches("**/*.log", "var/log/app.txt"));
    }

    #[test]
    fn policy_first_match_wins() {
        let policy = MutabilityPolicy {
            policy_id: 1,
            name: "test".into(),
            rules: vec![
                PolicyRule {
                    path_pattern: "**/*.log".into(),
                    on_delete: DeleteBehavior::Immediate,
                    record_changes: false,
                    mutable: true,
                    retention_secs: None,
                    on_expiry: ExpiryBehavior::Purge,
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
        };

        let log_rule = policy.match_rule("var/app.log").unwrap();
        assert_eq!(log_rule.on_delete, DeleteBehavior::Immediate);

        let other_rule = policy.match_rule("docs/readme.md").unwrap();
        assert_eq!(other_rule.on_delete, DeleteBehavior::Soft);
    }
}
