use anyhow::Result;
use std::sync::atomic::Ordering;

use tgcryptfs_deadman::config::DeadmanConfig;
use tgcryptfs_deadman::daemon::DeadmanDaemon;

use super::utils;

pub async fn arm() -> Result<()> {
    let config = load_config()?;
    let daemon = DeadmanDaemon::new(config);
    daemon.arm().map_err(utils::deadman_err)?;

    println!("Deadman switch ARMED.");
    println!("Triggers will be evaluated on the configured interval.");
    println!();
    println!("Starting daemon loop...");
    println!("Press Ctrl+C to disarm and stop.");

    // Set up Ctrl+C handler
    let shutdown = daemon.shutdown_handle();
    let shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        println!();
        println!("Caught interrupt, disarming...");
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    let outcome = daemon.run().await;
    match outcome {
        tgcryptfs_deadman::daemon::DaemonOutcome::Shutdown => {
            println!("Deadman daemon stopped.");
        }
        tgcryptfs_deadman::daemon::DaemonOutcome::Destroyed { reason } => {
            println!("DESTRUCTION EXECUTED: {reason}");
        }
        tgcryptfs_deadman::daemon::DaemonOutcome::Disarmed => {
            println!("Deadman switch DISARMED during grace period.");
        }
    }

    Ok(())
}

pub async fn disarm() -> Result<()> {
    println!("Deadman switch DISARMED.");
    println!("Note: If a daemon is running in another process, send SIGINT to stop it.");
    Ok(())
}

pub async fn configure(config_path: &str) -> Result<()> {
    let content = std::fs::read_to_string(config_path)?;
    let config: DeadmanConfig = serde_json::from_str(&content)?;

    println!("Deadman configuration loaded:");
    println!("  Enabled: {}", config.enabled);
    println!("  Check interval: {}s", config.check_interval_secs);
    println!("  Grace period: {}s", config.grace_period_secs);
    println!("  Triggers: {}", config.triggers.len());
    for t in &config.triggers {
        println!(
            "    - {} ({:?}) [{}]",
            t.name,
            t.trigger_type,
            if t.active { "active" } else { "inactive" }
        );
    }
    println!("  Destruction phases: {}", config.destruction.phases.len());

    // Save to config directory
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("tgcryptfs");
    std::fs::create_dir_all(&config_dir)?;
    let dest = config_dir.join("deadman.json");
    std::fs::write(&dest, &content)?;

    println!();
    println!("Configuration saved to: {}", dest.display());
    Ok(())
}

pub async fn status() -> Result<()> {
    let config = load_config()?;

    println!("Deadman Status:");
    println!("  Enabled: {}", config.enabled);
    println!("  Check interval: {}s", config.check_interval_secs);
    println!("  Grace period: {}s", config.grace_period_secs);
    println!("  Triggers: {}", config.triggers.len());
    for t in &config.triggers {
        println!(
            "    - {} ({:?}) [{}]",
            t.name,
            t.trigger_type,
            if t.active { "active" } else { "inactive" }
        );
    }
    println!("  Destruction phases: {}", config.destruction.phases.len());
    for p in &config.destruction.phases {
        println!("    - {:?}", p);
    }

    Ok(())
}

/// Load deadman config from the config directory, or return defaults.
pub fn load_config() -> Result<DeadmanConfig> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("tgcryptfs")
        .join("deadman.json");

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let config: DeadmanConfig = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        Ok(DeadmanConfig::default())
    }
}
