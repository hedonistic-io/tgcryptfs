use anyhow::Result;

use tgcryptfs_api::service::system::SystemService;
use tgcryptfs_core::volume::manager;

pub async fn show() -> Result<()> {
    let sys = SystemService::new();
    let volumes_dir = manager::default_volumes_dir();

    let total_volumes = manager::list_volumes(&volumes_dir).map_or(0, |v| v.len());

    let session_exists = std::path::Path::new("tgcryptfs.session").exists();

    println!("TGCryptFS v2 Status");
    println!("====================");
    println!();
    println!("Version:    {}", sys.version());
    println!("Volumes:    {} configured, 0 mounted", total_volumes);
    println!(
        "Telegram:   {}",
        if session_exists {
            "session present"
        } else {
            "not connected"
        }
    );
    println!("Cache:      not initialized");
    println!("Deadman:    disarmed");
    println!("Data dir:   {}", volumes_dir.display());
    println!();

    if total_volumes == 0 {
        println!("Get started: tgcryptfs volume create");
    } else {
        println!("Use `tgcryptfs volume list` to see volumes.");
    }

    Ok(())
}
