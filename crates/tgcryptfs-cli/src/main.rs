use clap::{CommandFactory, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;

/// Load KEY=VALUE pairs from the tgcryptfs config .env file into the process
/// environment. This makes env vars available to clap's `env` attribute without
/// requiring shell integration. Only sets vars that aren't already set (shell
/// env takes precedence).
fn load_config_env() {
    let env_path = dirs::config_dir().map(|d| d.join("tgcryptfs").join(".env"));
    let Some(path) = env_path else { return };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            // Don't override existing env vars (shell takes precedence)
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }
    }
}

/// TGCryptFS v2 - Telegram Encrypted Filesystem
#[derive(Parser)]
#[command(name = "tgcryptfs", version, about)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with Telegram
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Volume management
    Volume {
        #[command(subcommand)]
        action: VolumeAction,
    },
    /// Deadman option management
    Deadman {
        #[command(subcommand)]
        action: DeadmanAction,
    },
    /// Interactive setup wizard
    Configure,
    /// Show system status
    Status,
    /// Key management
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// Share management
    Share {
        #[command(subcommand)]
        action: ShareAction,
    },
    /// Start the REST API server
    Serve {
        /// Address to bind (host:port)
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Log in to Telegram
    Login {
        /// Telegram API ID (or set TG_API_ID env var)
        #[arg(long, env = "TG_API_ID")]
        api_id: Option<i32>,
        /// Telegram API hash (or set TG_API_HASH env var)
        #[arg(long, env = "TG_API_HASH")]
        api_hash: Option<String>,
    },
    /// Log out and remove session
    Logout,
    /// Show authentication status
    Status,
}

#[derive(Subcommand)]
enum VolumeAction {
    /// Create a new encrypted volume
    Create {
        /// Volume name (auto-generated if not provided)
        #[arg(short, long)]
        name: Option<String>,
        /// Target block size in bytes
        #[arg(long, default_value = "1048576")]
        block_size: usize,
    },
    /// Mount an existing volume
    Mount {
        /// Volume name or ID
        volume: String,
        /// Mount point path
        mountpoint: String,
        /// Allow other users to access the mount (requires /etc/fuse.conf user_allow_other)
        #[arg(long)]
        allow_other: bool,
    },
    /// Unmount a mounted volume
    Unmount {
        /// Mount point or volume name
        target: String,
    },
    /// List all volumes
    List,
    /// Delete a volume
    Delete {
        /// Volume name or ID
        volume: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Show volume info
    Info {
        /// Volume name or ID
        volume: String,
    },
}

#[derive(Subcommand)]
enum DeadmanAction {
    /// Arm the deadman switch
    Arm,
    /// Disarm the deadman switch
    Disarm,
    /// Configure deadman triggers
    Configure {
        /// Path to JSON config file
        config: String,
    },
    /// Show deadman status
    Status,
}

#[derive(Subcommand)]
enum ShareAction {
    /// Share a volume with a user
    Create {
        /// Volume name or ID
        #[arg(long)]
        volume: String,
        /// User identifier (Telegram UID or username)
        #[arg(long)]
        user: String,
        /// Access level: read-only, read-write, or admin
        #[arg(long, default_value = "read-write")]
        access: String,
    },
    /// List shares for a volume
    List {
        /// Volume name or ID
        #[arg(long)]
        volume: String,
    },
    /// Revoke a user's access
    Revoke {
        /// Volume name or ID
        #[arg(long)]
        volume: String,
        /// User to revoke
        #[arg(long)]
        user: String,
    },
    /// Create an invite link
    Invite {
        /// Volume name or ID
        #[arg(long)]
        volume: String,
        /// Access level
        #[arg(long, default_value = "read-only")]
        access: String,
        /// Maximum number of uses
        #[arg(long)]
        max_uses: Option<u32>,
        /// Hours until expiry
        #[arg(long)]
        expires_in: Option<u64>,
    },
    /// Accept an invite
    Accept {
        /// Invite code
        invite_code: String,
    },
}

#[derive(Subcommand)]
enum KeyAction {
    /// Rotate encryption keys
    Rotate {
        /// Volume name or ID
        volume: String,
    },
    /// Export volume key as a sentence reference
    Export {
        /// Volume name or ID
        volume: String,
    },
    /// Import volume key from a sentence reference
    Import {
        /// The sentence reference to import
        sentence: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_config_env();
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Auth { action } => match action {
            AuthAction::Login { api_id, api_hash } => {
                commands::auth::login(api_id, api_hash).await?;
            }
            AuthAction::Logout => {
                commands::auth::logout().await?;
            }
            AuthAction::Status => {
                commands::auth::status().await?;
            }
        },
        Commands::Volume { action } => match action {
            VolumeAction::Create { name, block_size } => {
                commands::volume::create(name, block_size).await?;
            }
            VolumeAction::Mount {
                volume,
                mountpoint,
                allow_other,
            } => {
                commands::volume::mount(&volume, &mountpoint, allow_other).await?;
            }
            VolumeAction::Unmount { target } => {
                commands::volume::unmount(&target).await?;
            }
            VolumeAction::List => {
                commands::volume::list().await?;
            }
            VolumeAction::Delete { volume, force } => {
                commands::volume::delete(&volume, force).await?;
            }
            VolumeAction::Info { volume } => {
                commands::volume::info(&volume).await?;
            }
        },
        Commands::Deadman { action } => match action {
            DeadmanAction::Arm => {
                commands::deadman::arm().await?;
            }
            DeadmanAction::Disarm => {
                commands::deadman::disarm().await?;
            }
            DeadmanAction::Configure { config } => {
                commands::deadman::configure(&config).await?;
            }
            DeadmanAction::Status => {
                commands::deadman::status().await?;
            }
        },
        Commands::Configure => {
            commands::configure::run().await?;
        }
        Commands::Status => {
            commands::status::show().await?;
        }
        Commands::Share { action } => match action {
            ShareAction::Create {
                volume,
                user,
                access,
            } => {
                commands::sharing::create_share(&volume, &user, &access).await?;
            }
            ShareAction::List { volume } => {
                commands::sharing::list_shares(&volume).await?;
            }
            ShareAction::Revoke { volume, user } => {
                commands::sharing::revoke_share(&volume, &user).await?;
            }
            ShareAction::Invite {
                volume,
                access,
                max_uses,
                expires_in,
            } => {
                commands::sharing::create_invite(&volume, &access, max_uses, expires_in).await?;
            }
            ShareAction::Accept { invite_code } => {
                commands::sharing::accept_invite(&invite_code).await?;
            }
        },
        Commands::Serve { bind } => {
            commands::serve::run(&bind).await?;
        }
        Commands::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                "tgcryptfs",
                &mut std::io::stdout(),
            );
        }
        Commands::Key { action } => match action {
            KeyAction::Rotate { volume } => {
                commands::key::rotate(&volume).await?;
            }
            KeyAction::Export { volume } => {
                commands::key::export(&volume).await?;
            }
            KeyAction::Import { sentence } => {
                commands::key::import(&sentence).await?;
            }
        },
    }

    Ok(())
}
