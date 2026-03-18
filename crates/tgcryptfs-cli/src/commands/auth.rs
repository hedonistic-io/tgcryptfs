use anyhow::Result;

use tgcryptfs_telegram::client::TelegramClient;
use tgcryptfs_telegram::types::{AuthState, TelegramConfig};

use super::utils;

/// Compile-time API credentials. Set these via environment variables at build time:
///   TG_API_ID_BUILTIN=12345678 TG_API_HASH_BUILTIN=abcdef... cargo build --release
/// When set, users won't need to provide their own credentials.
const BUILTIN_API_ID: Option<&str> = option_env!("TG_API_ID_BUILTIN");
const BUILTIN_API_HASH: Option<&str> = option_env!("TG_API_HASH_BUILTIN");

const MAX_CODE_ATTEMPTS: u32 = 3;
const MAX_PASSWORD_ATTEMPTS: u32 = 3;

fn resolve_api_id(cli_value: Option<i32>) -> i32 {
    if let Some(id) = cli_value {
        return id;
    }
    if let Some(builtin) = BUILTIN_API_ID {
        if let Ok(id) = builtin.parse::<i32>() {
            return id;
        }
    }
    eprintln!("Telegram API ID required.");
    eprintln!();
    eprintln!("To get your API credentials:");
    eprintln!("  1. Visit https://my.telegram.org");
    eprintln!("  2. Log in and go to 'API development tools'");
    eprintln!("  3. Create an application to get your api_id and api_hash");
    eprintln!();
    eprintln!("Then either:");
    eprintln!("  export TG_API_ID=<your_id>");
    eprintln!("  export TG_API_HASH=<your_hash>");
    eprintln!("  tgcryptfs auth login");
    eprintln!();
    eprintln!("Or pass directly:");
    eprintln!("  tgcryptfs auth login --api-id <id> --api-hash <hash>");
    std::process::exit(1);
}

fn resolve_api_hash(cli_value: Option<String>) -> String {
    if let Some(hash) = cli_value {
        return hash;
    }
    if let Some(builtin) = BUILTIN_API_HASH {
        return builtin.to_string();
    }
    eprintln!("Telegram API hash required. Set TG_API_HASH or pass --api-hash");
    std::process::exit(1);
}

pub async fn login(api_id: Option<i32>, api_hash: Option<String>) -> Result<()> {
    let api_id = resolve_api_id(api_id);
    let api_hash = resolve_api_hash(api_hash);

    println!("Connecting to Telegram with API ID: {api_id}...");

    let config = TelegramConfig {
        api_id,
        api_hash,
        ..Default::default()
    };

    let client = TelegramClient::new(config);
    let state = client.connect().await.map_err(utils::telegram_err)?;

    match state {
        AuthState::Authenticated => {
            println!("Already authenticated.");
            return Ok(());
        }
        AuthState::NotAuthenticated => {
            // Continue with interactive login below
        }
        _ => {
            anyhow::bail!("unexpected auth state after connect: {state:?}");
        }
    }

    // Step 1: Get phone number and request login code
    let phone = utils::prompt("Enter your phone number (with country code, e.g. +1234567890):")?;
    if phone.is_empty() {
        anyhow::bail!("phone number is required");
    }

    println!("Requesting login code...");
    let token = client
        .request_login_code(&phone)
        .await
        .map_err(utils::telegram_err)?;
    println!("Login code sent to your Telegram app.");

    // Step 2: Enter verification code (with retry)
    for attempt in 1..=MAX_CODE_ATTEMPTS {
        let code = utils::prompt("Enter the verification code:")?;
        if code.is_empty() {
            println!("Code cannot be empty.");
            continue;
        }

        match client.sign_in(&token, &code).await {
            Ok(AuthState::Authenticated) => {
                println!("Authenticated successfully!");
                return Ok(());
            }
            Ok(AuthState::AwaitingPassword) => {
                println!("2FA password required.");
                return handle_2fa(&client).await;
            }
            Err(e) => {
                let remaining = MAX_CODE_ATTEMPTS - attempt;
                if remaining > 0 {
                    println!("Invalid code: {e}. {remaining} attempt(s) remaining.");
                } else {
                    anyhow::bail!("failed to sign in after {MAX_CODE_ATTEMPTS} attempts: {e}");
                }
            }
            Ok(other) => {
                anyhow::bail!("unexpected auth state: {other:?}");
            }
        }
    }

    anyhow::bail!("authentication failed: max code attempts exceeded");
}

async fn handle_2fa(client: &TelegramClient) -> Result<()> {
    for attempt in 1..=MAX_PASSWORD_ATTEMPTS {
        let password = utils::prompt_password("Enter your 2FA password: ")?;
        if password.is_empty() {
            println!("Password cannot be empty.");
            continue;
        }

        match client.check_password(&password).await {
            Ok(AuthState::Authenticated) => {
                println!("Authenticated successfully!");
                return Ok(());
            }
            Err(e) => {
                let remaining = MAX_PASSWORD_ATTEMPTS - attempt;
                if remaining > 0 {
                    println!("Invalid password: {e}. {remaining} attempt(s) remaining.");
                } else {
                    anyhow::bail!("failed 2FA after {MAX_PASSWORD_ATTEMPTS} attempts: {e}");
                }
            }
            Ok(other) => {
                anyhow::bail!("unexpected auth state after 2FA: {other:?}");
            }
        }
    }

    anyhow::bail!("2FA authentication failed: max password attempts exceeded");
}

/// Returns the path to the Telegram session file in the config directory.
fn session_file_path() -> std::path::PathBuf {
    dirs::config_dir()
        .expect("unable to determine config directory")
        .join("tgcryptfs")
        .join("tgcryptfs.session")
}

/// Set restrictive permissions (0600) on the session file (Unix only).
#[cfg(unix)]
fn restrict_session_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn restrict_session_permissions(_path: &std::path::Path) {}

pub async fn logout() -> Result<()> {
    println!("Removing Telegram session...");
    let session_path = session_file_path();
    if session_path.exists() {
        std::fs::remove_file(&session_path)?;
        println!("Session removed.");
    } else {
        // Also check legacy CWD location
        let legacy = std::path::Path::new("tgcryptfs.session");
        if legacy.exists() {
            std::fs::remove_file(legacy)?;
            println!("Legacy session removed.");
        } else {
            println!("No session file found.");
        }
    }
    Ok(())
}

pub async fn status() -> Result<()> {
    let session_path = session_file_path();
    if session_path.exists() {
        restrict_session_permissions(&session_path);
        println!("Session file exists: {}", session_path.display());
        println!("Status: session file present (connect to verify)");
    } else {
        println!("Not authenticated. Run `tgcryptfs auth login` to connect.");
    }
    Ok(())
}
