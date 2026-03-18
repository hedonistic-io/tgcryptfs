use anyhow::{Context, Result};
use std::fmt;
use std::io::{self, BufRead, Write};

/// Prompt the user for text input.
pub fn prompt(question: &str) -> Result<String> {
    print!("{question} ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .context("failed to read input")?;
    Ok(input.trim().to_string())
}

/// Prompt for a password with echo suppression.
///
/// Uses rpassword to suppress terminal echo so the password is not
/// visible on screen. Falls back to plain stdin if terminal is not available.
pub fn prompt_password(msg: &str) -> Result<String> {
    rpassword::prompt_password(msg).context("failed to read password")
}

/// Format an error with its suggestion for CLI display.
pub fn cli_error<E: fmt::Display>(error: E, suggestion: &str) -> anyhow::Error {
    anyhow::anyhow!("{error}\n  Suggestion: {suggestion}")
}

/// Convert a core error to an anyhow error with suggestion.
pub fn core_err(e: tgcryptfs_core::error::CoreError) -> anyhow::Error {
    let suggestion = e.suggestion();
    cli_error(e, suggestion)
}

/// Convert a telegram error to an anyhow error with suggestion.
pub fn telegram_err(e: tgcryptfs_telegram::error::TelegramError) -> anyhow::Error {
    let suggestion = e.suggestion();
    cli_error(e, suggestion)
}

/// Convert an API error to an anyhow error with suggestion.
pub fn api_err(e: tgcryptfs_api::error::ApiError) -> anyhow::Error {
    let suggestion = e.suggestion();
    cli_error(e, suggestion)
}

/// Convert a deadman error to an anyhow error with suggestion.
pub fn deadman_err(e: tgcryptfs_deadman::error::DeadmanError) -> anyhow::Error {
    let suggestion = e.suggestion();
    cli_error(e, suggestion)
}

/// Convert a sharing error to an anyhow error with suggestion.
/// Not yet called in CLI commands (sharing ops use store errors directly),
/// but kept for consistency with other error converters.
#[cfg_attr(not(test), allow(dead_code))]
pub fn sharing_err(e: tgcryptfs_sharing::error::SharingError) -> anyhow::Error {
    let suggestion = e.suggestion();
    cli_error(e, suggestion)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_error_includes_suggestion() {
        let err = cli_error("something failed", "try again");
        let msg = format!("{err}");
        assert!(msg.contains("something failed"));
        assert!(msg.contains("Suggestion: try again"));
    }

    #[test]
    fn core_err_includes_suggestion() {
        let core_err_val = tgcryptfs_core::error::CoreError::Encryption("bad key".into());
        let err = core_err(core_err_val);
        let msg = format!("{err}");
        assert!(msg.contains("bad key"));
        assert!(msg.contains("Suggestion:"));
    }

    #[test]
    fn api_err_includes_suggestion() {
        let api_err_val = tgcryptfs_api::error::ApiError::VolumeNotFound("test".into());
        let err = api_err(api_err_val);
        let msg = format!("{err}");
        assert!(msg.contains("test"));
        assert!(msg.contains("Suggestion:"));
    }

    #[test]
    fn deadman_err_includes_suggestion() {
        let de = tgcryptfs_deadman::error::DeadmanError::NotArmed;
        let err = deadman_err(de);
        let msg = format!("{err}");
        assert!(msg.contains("Suggestion:"));
    }

    #[test]
    fn sharing_err_includes_suggestion() {
        let se = tgcryptfs_sharing::error::SharingError::InvalidInvite("expired".into());
        let err = sharing_err(se);
        let msg = format!("{err}");
        assert!(msg.contains("expired"));
        assert!(msg.contains("Suggestion:"));
    }
}
