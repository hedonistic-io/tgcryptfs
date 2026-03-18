use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

fn prompt(question: &str) -> Result<String> {
    print!("{question} ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .context("failed to read input")?;
    Ok(input.trim().to_string())
}

fn prompt_default(question: &str, default: &str) -> Result<String> {
    print!("{question} [{default}]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .context("failed to read input")?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn prompt_yes_no(question: &str, default_yes: bool) -> Result<bool> {
    let hint = if default_yes { "Y/n" } else { "y/N" };
    let answer = prompt(&format!("{question} ({hint})"))?;
    if answer.is_empty() {
        return Ok(default_yes);
    }
    Ok(matches!(answer.to_lowercase().as_str(), "y" | "yes"))
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("tgcryptfs")
}

fn env_file_path() -> PathBuf {
    config_dir().join(".env")
}

pub async fn run() -> Result<()> {
    println!();
    println!("  ╔══════════════════════════════════════╗");
    println!("  ║     TGCryptFS Configuration Setup    ║");
    println!("  ╚══════════════════════════════════════╝");
    println!();

    // Step 1: Check existing config
    let env_path = env_file_path();
    if env_path.exists() {
        println!("  Existing configuration found at: {}", env_path.display());
        let existing = std::fs::read_to_string(&env_path)?;
        println!();
        for line in existing.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((key, _)) = line.split_once('=') {
                let masked = if key.contains("HASH") || key.contains("SECRET") {
                    format!("{key}=****")
                } else {
                    line.to_string()
                };
                println!("  Current: {masked}");
            }
        }
        println!();
        if !prompt_yes_no("  Overwrite existing configuration?", false)? {
            println!("  Keeping existing configuration.");
            return Ok(());
        }
        println!();
    }

    // Step 2: Telegram API credentials
    println!("  Step 1: Telegram API Credentials");
    println!("  ─────────────────────────────────");
    println!("  TGCryptFS uses Telegram as encrypted block storage.");
    println!("  You need API credentials from https://my.telegram.org");
    println!();
    println!("  How to get them:");
    println!("    1. Open https://my.telegram.org in your browser");
    println!("    2. Log in with your phone number");
    println!("    3. Click 'API development tools'");
    println!("    4. Fill in the form (app name/platform don't matter)");
    println!("    5. Copy api_id and api_hash");
    println!();

    let open_browser = prompt_yes_no("  Open https://my.telegram.org in your browser?", true)?;
    if open_browser {
        let _ = open_url("https://my.telegram.org");
        println!();
        println!("  Browser opened. Complete the steps above, then enter your credentials.");
        println!();
    }

    let api_id = loop {
        let input = prompt("  Telegram API ID (number):")?;
        if input.is_empty() {
            println!("  API ID is required.");
            continue;
        }
        match input.parse::<i32>() {
            Ok(id) if id > 0 => break id,
            _ => {
                println!("  Invalid API ID. Must be a positive number.");
                continue;
            }
        }
    };

    let api_hash = loop {
        let input = prompt("  Telegram API Hash (hex string):")?;
        if input.is_empty() {
            println!("  API hash is required.");
            continue;
        }
        if input.len() < 16 {
            println!("  API hash looks too short. It should be a 32-character hex string.");
            continue;
        }
        break input;
    };

    println!();

    // Step 3: Volumes directory
    println!("  Step 2: Storage Location");
    println!("  ────────────────────────");
    let default_volumes_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("tgcryptfs")
        .join("volumes");
    let volumes_dir = prompt_default(
        "  Volumes directory:",
        &default_volumes_dir.to_string_lossy(),
    )?;

    println!();

    // Step 4: Session file
    println!("  Step 3: Session Storage");
    println!("  ───────────────────────");
    let default_session = config_dir().join("tgcryptfs.session");
    let session_path = prompt_default(
        "  Telegram session file:",
        &default_session.to_string_lossy(),
    )?;

    println!();

    // Step 5: Deadman switch
    println!("  Step 4: Deadman Switch (Optional)");
    println!("  ──────────────────────────────────");
    println!("  The deadman switch can auto-destroy volumes if you don't check in.");
    let enable_deadman = prompt_yes_no("  Enable deadman switch?", false)?;

    let deadman_timeout = if enable_deadman {
        let timeout = prompt_default("  Heartbeat timeout (hours):", "72")?;
        Some(timeout.parse::<u64>().unwrap_or(72))
    } else {
        None
    };

    println!();

    // Step 6: Write config
    println!("  Step 5: Writing Configuration");
    println!("  ─────────────────────────────");

    let config_path = config_dir();
    std::fs::create_dir_all(&config_path)
        .with_context(|| format!("failed to create config dir: {}", config_path.display()))?;

    std::fs::create_dir_all(&volumes_dir)
        .with_context(|| format!("failed to create volumes dir: {volumes_dir}"))?;

    if let Some(parent) = Path::new(&session_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut env_content = String::new();
    env_content.push_str("# TGCryptFS Configuration\n");
    env_content.push_str("# Generated by `tgcryptfs configure`\n");
    env_content.push_str("#\n");
    env_content.push_str("# Telegram API credentials - DO NOT share these\n");
    writeln!(env_content, "TG_API_ID={api_id}").unwrap();
    writeln!(env_content, "TG_API_HASH={api_hash}").unwrap();
    env_content.push('\n');
    env_content.push_str("# Storage paths\n");
    writeln!(env_content, "TGCRYPTFS_VOLUMES_DIR={volumes_dir}").unwrap();
    writeln!(env_content, "TGCRYPTFS_SESSION_PATH={session_path}").unwrap();

    if let Some(timeout) = deadman_timeout {
        env_content.push('\n');
        env_content.push_str("# Deadman switch\n");
        env_content.push_str("TGCRYPTFS_DEADMAN_ENABLED=true\n");
        writeln!(env_content, "TGCRYPTFS_DEADMAN_TIMEOUT_HOURS={timeout}").unwrap();
    }

    std::fs::write(&env_path, &env_content)
        .with_context(|| format!("failed to write {}", env_path.display()))?;

    // Set restrictive permissions on the env file (contains secrets)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&env_path, std::fs::Permissions::from_mode(0o600))?;
    }

    println!("  Config written to: {}", env_path.display());
    println!("  Permissions: 600 (owner read/write only)");
    println!();

    // Step 7: Shell integration
    println!("  Step 6: Shell Integration");
    println!("  ─────────────────────────");
    println!("  Note: The tgcryptfs CLI auto-loads its config file, so shell");
    println!("  integration is optional. It's useful if you want the env vars");
    println!("  available to other tools.");
    println!();
    let shell_hint = detect_shell();
    let env_display = env_path.display();
    let source_snippet = if shell_hint.is_fish {
        format!(
            "# TGCryptFS credentials\n\
             for line in (string match -rv '^\\s*#|^\\s*$' < \"{env_display}\")\n    \
                 set -l kv (string split -m1 '=' -- $line)\n    \
                 set -gx $kv[1] $kv[2]\n\
             end"
        )
    } else {
        format!("# TGCryptFS credentials\nset -a; source \"{env_display}\"; set +a")
    };

    println!(
        "  To load credentials in your shell, add this to your {}:",
        shell_hint.rc_file
    );
    println!();
    for line in source_snippet.lines() {
        println!("    {line}");
    }
    println!();

    let auto_add = prompt_yes_no(
        &format!("  Add this to {} automatically?", shell_hint.rc_file),
        true,
    )?;
    if auto_add {
        let rc_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(&shell_hint.rc_file[2..]); // strip ~/
        let source_block = format!("\n{source_snippet}\n");

        // Check if already present
        let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();
        if existing.contains("TGCryptFS credentials") {
            println!("  Already present in {}. Skipping.", shell_hint.rc_file);
            println!("  You may want to update it manually with the snippet above.");
        } else {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&rc_path)?;
            file.write_all(source_block.as_bytes())?;
            println!("  Added to {}", shell_hint.rc_file);
        }
    }

    println!();
    println!("  ╔══════════════════════════════════════╗");
    println!("  ║        Configuration Complete!        ║");
    println!("  ╚══════════════════════════════════════╝");
    println!();
    println!("  Next steps:");
    if shell_hint.is_fish {
        println!("    1. Reload your shell: exec fish");
    } else {
        println!(
            "    1. Reload your shell or run: source {}",
            env_path.display()
        );
    }
    println!("    2. Authenticate: tgcryptfs auth login");
    println!("    3. Create a volume: tgcryptfs volume create --name my-vault");
    println!();

    Ok(())
}

struct ShellHint {
    rc_file: String,
    is_fish: bool,
}

fn detect_shell() -> ShellHint {
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.contains("zsh") {
        ShellHint {
            rc_file: "~/.zshrc".into(),
            is_fish: false,
        }
    } else if shell.contains("fish") {
        ShellHint {
            rc_file: "~/.config/fish/config.fish".into(),
            is_fish: true,
        }
    } else {
        ShellHint {
            rc_file: "~/.bashrc".into(),
            is_fish: false,
        }
    }
}

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()?;
    }
    Ok(())
}
