use super::types::{DeleteBehavior, ExpiryBehavior, MutabilityPolicy, PolicyRule};
use crate::error::{CoreError, Result};

/// Parse a policy definition from a TOML-like DSL string.
///
/// Format:
/// ```text
/// policy "my-policy" {
///   rule "*.log" {
///     mutable = true
///     on_delete = immediate
///     record_changes = false
///   }
///   rule "**" {
///     mutable = true
///     on_delete = soft
///     record_changes = true
///     retention = 30d
///     on_expiry = hold
///   }
/// }
/// ```
pub fn parse_policy(input: &str, policy_id: u32) -> Result<MutabilityPolicy> {
    let input = input.trim();

    // Parse policy header
    let rest = input
        .strip_prefix("policy")
        .ok_or_else(|| CoreError::Policy("expected 'policy' keyword".into()))?
        .trim();

    let (name, rest) = parse_quoted_string(rest)?;

    let rest = rest
        .trim()
        .strip_prefix('{')
        .ok_or_else(|| CoreError::Policy("expected '{' after policy name".into()))?;

    let rest = rest
        .trim()
        .strip_suffix('}')
        .ok_or_else(|| CoreError::Policy("expected '}' at end of policy".into()))?;

    let rules = parse_rules(rest)?;

    Ok(MutabilityPolicy {
        policy_id,
        name,
        rules,
    })
}

fn parse_quoted_string(input: &str) -> Result<(String, &str)> {
    let input = input.trim();
    if !input.starts_with('"') {
        return Err(CoreError::Policy("expected quoted string".into()));
    }
    let end = input[1..]
        .find('"')
        .ok_or_else(|| CoreError::Policy("unterminated string".into()))?;
    Ok((input[1..=end].to_string(), &input[end + 2..]))
}

fn parse_rules(input: &str) -> Result<Vec<PolicyRule>> {
    let mut rules = Vec::new();
    let mut rest = input.trim();

    while !rest.is_empty() {
        rest = rest.trim();
        if rest.is_empty() {
            break;
        }

        rest = rest
            .strip_prefix("rule")
            .ok_or_else(|| {
                CoreError::Policy(format!(
                    "expected 'rule', got: {}",
                    &rest[..rest.len().min(20)]
                ))
            })?
            .trim();

        let (pattern, after_pattern) = parse_quoted_string(rest)?;

        let after_brace = after_pattern
            .trim()
            .strip_prefix('{')
            .ok_or_else(|| CoreError::Policy("expected '{' after rule pattern".into()))?;

        let brace_end = after_brace
            .find('}')
            .ok_or_else(|| CoreError::Policy("expected '}' to close rule".into()))?;

        let rule_body = &after_brace[..brace_end];
        rest = &after_brace[brace_end + 1..];

        let rule = parse_rule_body(&pattern, rule_body)?;
        rules.push(rule);
    }

    Ok(rules)
}

fn parse_rule_body(pattern: &str, body: &str) -> Result<PolicyRule> {
    let mut mutable = true;
    let mut on_delete = DeleteBehavior::Soft;
    let mut record_changes = true;
    let mut retention_secs = None;
    let mut on_expiry = ExpiryBehavior::Hold;

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| CoreError::Policy(format!("invalid rule line: {line}")))?;

        let key = key.trim();
        let value = value.trim();

        match key {
            "mutable" => mutable = value == "true",
            "on_delete" => {
                on_delete = match value {
                    "immediate" => DeleteBehavior::Immediate,
                    "soft" => DeleteBehavior::Soft,
                    "reject" => DeleteBehavior::Reject,
                    _ => return Err(CoreError::Policy(format!("invalid on_delete: {value}"))),
                }
            }
            "record_changes" => record_changes = value == "true",
            "retention" => retention_secs = Some(parse_duration(value)?),
            "on_expiry" => {
                on_expiry = match value {
                    "purge" => ExpiryBehavior::Purge,
                    "hold" => ExpiryBehavior::Hold,
                    _ => return Err(CoreError::Policy(format!("invalid on_expiry: {value}"))),
                }
            }
            _ => return Err(CoreError::Policy(format!("unknown rule key: {key}"))),
        }
    }

    Ok(PolicyRule {
        path_pattern: pattern.to_string(),
        on_delete,
        record_changes,
        mutable,
        retention_secs,
        on_expiry,
    })
}

fn parse_duration(s: &str) -> Result<u64> {
    let s = s.trim();
    if let Some(days) = s.strip_suffix('d') {
        let n: u64 = days
            .parse()
            .map_err(|_| CoreError::Policy(format!("invalid duration: {s}")))?;
        return Ok(n * 86400);
    }
    if let Some(hours) = s.strip_suffix('h') {
        let n: u64 = hours
            .parse()
            .map_err(|_| CoreError::Policy(format!("invalid duration: {s}")))?;
        return Ok(n * 3600);
    }
    if let Some(secs) = s.strip_suffix('s') {
        let n: u64 = secs
            .parse()
            .map_err(|_| CoreError::Policy(format!("invalid duration: {s}")))?;
        return Ok(n);
    }
    s.parse()
        .map_err(|_| CoreError::Policy(format!("invalid duration: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_policy() {
        let input = r#"
policy "default" {
  rule "**" {
    mutable = true
    on_delete = soft
    record_changes = true
    retention = 30d
    on_expiry = hold
  }
}
"#;
        let p = parse_policy(input, 1).unwrap();
        assert_eq!(p.name, "default");
        assert_eq!(p.rules.len(), 1);
        assert_eq!(p.rules[0].path_pattern, "**");
        assert_eq!(p.rules[0].on_delete, DeleteBehavior::Soft);
        assert_eq!(p.rules[0].retention_secs, Some(30 * 86400));
    }

    #[test]
    fn parse_multi_rule_policy() {
        let input = r#"
policy "strict" {
  rule "*.log" {
    mutable = true
    on_delete = immediate
    record_changes = false
  }
  rule "config/*" {
    mutable = false
    on_delete = reject
    record_changes = true
  }
  rule "**" {
    mutable = true
    on_delete = soft
    record_changes = true
    retention = 7d
    on_expiry = purge
  }
}
"#;
        let p = parse_policy(input, 2).unwrap();
        assert_eq!(p.name, "strict");
        assert_eq!(p.rules.len(), 3);
        assert_eq!(p.rules[0].on_delete, DeleteBehavior::Immediate);
        assert!(!p.rules[1].mutable);
        assert_eq!(p.rules[2].on_expiry, ExpiryBehavior::Purge);
    }

    #[test]
    fn parse_duration_days() {
        assert_eq!(parse_duration("30d").unwrap(), 30 * 86400);
    }

    #[test]
    fn parse_duration_hours() {
        assert_eq!(parse_duration("24h").unwrap(), 86400);
    }

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration("3600s").unwrap(), 3600);
    }
}
