use std::net::SocketAddr;

use anyhow::Result;

use tgcryptfs_api::server::{self, ServerConfig};
use tgcryptfs_api::service::auth::AuthService;
use tgcryptfs_core::volume::manager;
use tgcryptfs_telegram::types::TelegramConfig;

pub async fn run(bind: &str) -> Result<()> {
    let addr: SocketAddr = bind
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid bind address '{bind}': {e}"))?;

    let volumes_dir = manager::default_volumes_dir();
    if !volumes_dir.exists() {
        std::fs::create_dir_all(&volumes_dir)?;
    }

    let auth = AuthService::new(TelegramConfig::default());

    println!("Starting tgcryptfs API server on {addr}");
    println!("  Volumes directory: {}", volumes_dir.display());
    println!();
    println!("Endpoints:");
    println!("  GET  /api/v1/status       (unauthenticated)");
    println!("  GET  /api/v1/version      (unauthenticated)");
    println!("  POST /api/v1/auth/session");
    println!("  GET  /api/v1/auth/status");
    println!("  POST /api/v1/volumes");
    println!("  GET  /api/v1/volumes");
    println!();
    println!("All endpoints except /status and /version require bearer token authentication.");
    println!("Press Ctrl+C to stop.");

    let config = ServerConfig {
        bind_addr: addr,
        volumes_dir,
        auth,
        bearer_token: None,
    };

    server::run_server(config)
        .await
        .map_err(|e| anyhow::anyhow!("server error: {e}"))?;

    Ok(())
}
