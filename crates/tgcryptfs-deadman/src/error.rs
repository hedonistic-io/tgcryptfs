use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeadmanError {
    #[error("deadman not armed")]
    NotArmed,

    #[error("deadman already armed")]
    AlreadyArmed,

    #[error("trigger evaluation failed: {0}")]
    TriggerEval(String),

    #[error("destruction failed: {0}")]
    Destruction(String),

    #[error("hook error: {0}")]
    Hook(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("custom commands not allowed: {0}")]
    CustomCommandDenied(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl DeadmanError {
    /// Returns a user-facing suggestion for how to resolve this error.
    pub fn suggestion(&self) -> &'static str {
        match self {
            DeadmanError::NotArmed => {
                "Arm the dead man's switch first with `tgcryptfs deadman arm`"
            }
            DeadmanError::AlreadyArmed => {
                "Disarm first with `tgcryptfs deadman disarm` before re-arming"
            }
            DeadmanError::TriggerEval(_) => {
                "Check trigger configuration in ~/.config/tgcryptfs/deadman.json"
            }
            DeadmanError::Destruction(_) => {
                "Destruction sequence encountered an error; check volume state manually"
            }
            DeadmanError::Hook(_) => "Verify hook scripts exist and are executable",
            DeadmanError::Config(_) => "Check ~/.config/tgcryptfs/deadman.json for syntax errors",
            DeadmanError::Store(_) => {
                "The deadman metadata store may be corrupted; check database file"
            }
            DeadmanError::CustomCommandDenied(_) => {
                "Use --allow-custom-commands to enable arbitrary shell command execution"
            }
            DeadmanError::Io(_) => "Check file permissions and available disk space",
        }
    }
}

pub type Result<T> = std::result::Result<T, DeadmanError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_have_suggestions() {
        let errors: Vec<DeadmanError> = vec![
            DeadmanError::NotArmed,
            DeadmanError::AlreadyArmed,
            DeadmanError::TriggerEval("test".into()),
            DeadmanError::Destruction("test".into()),
            DeadmanError::Hook("test".into()),
            DeadmanError::Config("test".into()),
            DeadmanError::Store("test".into()),
            DeadmanError::CustomCommandDenied("test".into()),
        ];

        for err in &errors {
            assert!(!err.suggestion().is_empty(), "Empty suggestion for: {err}");
        }
    }

    #[test]
    fn not_armed_suggests_arm_command() {
        let err = DeadmanError::NotArmed;
        assert!(err.suggestion().contains("arm"));
    }
}
