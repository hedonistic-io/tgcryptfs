use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use rusqlite::Connection;

use tgcryptfs_api::service::volume::VolumeService;
use tgcryptfs_cache::block_cache::{BlockCache, CacheConfig};
use tgcryptfs_core::volume::manager;
use tgcryptfs_fuse::fs::CryptFs;
use tgcryptfs_store::migrations::initialize_database;
use tgcryptfs_store::opaque_schema::OpaqueSchema;
use tgcryptfs_store::schema::logical_tables;

use super::utils;

/// Core volume creation logic, separated for testability.
pub async fn create_with_password(
    name: Option<&str>,
    password: &str,
    volumes_dir: &std::path::Path,
) -> Result<tgcryptfs_api::types::CreateVolumeResponse> {
    let svc = VolumeService::new(volumes_dir.to_path_buf());
    svc.create(name, password).await.map_err(utils::api_err)
}

pub async fn create(name: Option<String>, _block_size: usize) -> Result<()> {
    let volumes_dir = manager::default_volumes_dir();

    // Prompt for password
    let password = utils::prompt_password("Enter volume password: ")?;
    let confirm = utils::prompt_password("Confirm password: ")?;

    if password != confirm {
        anyhow::bail!("Passwords do not match");
    }

    let resp = create_with_password(name.as_deref(), &password, &volumes_dir).await?;

    println!("Volume created successfully.");
    println!("  Name:       {}", resp.display_name);
    println!("  Volume ID:  {}", resp.volume_id);
    println!();
    println!("Sentence Reference (store securely for key recovery):");
    println!("  {}", resp.sentence_ref);
    println!();
    println!("IMPORTANT: Store your password securely. It cannot be recovered.");

    Ok(())
}

pub async fn mount(volume: &str, mountpoint: &str, allow_other: bool) -> Result<()> {
    let volumes_dir = manager::default_volumes_dir();

    // Verify mountpoint exists
    let mp = std::path::Path::new(mountpoint);
    if !mp.exists() {
        std::fs::create_dir_all(mp)?;
        println!("Created mountpoint directory: {mountpoint}");
    }

    let password = utils::prompt_password("Enter volume password: ")?;

    // Open volume and derive keys
    let result =
        manager::open_volume(volume, password.as_bytes(), &volumes_dir).map_err(utils::core_err)?;

    println!("Volume '{volume}' authenticated successfully.");

    // Open metadata database
    let conn = Connection::open(&result.paths.db_path)?;
    let schema = OpaqueSchema::new(result.hierarchy.schema.clone(), &logical_tables());
    initialize_database(&conn, &schema)?;

    // Set up block cache
    let cache_config = CacheConfig {
        cache_dir: result.paths.cache_dir.clone(),
        max_size: 512 * 1024 * 1024, // 512MB
        encrypt_at_rest: true,
    };
    let cache = BlockCache::new(cache_config, result.hierarchy.data.clone())
        .map_err(|e| anyhow::anyhow!("cache init: {e}"))?;

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    // Build CryptFs
    let fs = CryptFs::new(
        conn,
        schema,
        result.hierarchy.meta.clone(),
        result.hierarchy.data.clone(),
        uid,
        gid,
    )
    .with_cache(Arc::new(cache))
    .with_runtime(tokio::runtime::Handle::current());

    println!("Mounting at '{mountpoint}'...");
    println!("Press Ctrl+C to unmount.");

    // Mount FUSE filesystem (blocks until unmounted)
    let mut options = vec![
        fuser::MountOption::FSName("tgcryptfs".to_string()),
        fuser::MountOption::AutoUnmount,
    ];
    if allow_other {
        options.push(fuser::MountOption::AllowOther);
    }

    // Run FUSE mount in a blocking task since it blocks the thread
    let mp_owned = mountpoint.to_string();
    tokio::task::spawn_blocking(move || {
        fuser::mount2(fs, &mp_owned, &options)
            .map_err(|e| anyhow::anyhow!("FUSE mount failed: {e}"))
    })
    .await??;

    println!("Filesystem unmounted.");
    Ok(())
}

pub async fn unmount(target: &str) -> Result<()> {
    println!("Unmounting '{target}'...");

    // On macOS, use umount; on Linux, use fusermount -u
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("umount").arg(target).status();
        match status {
            Ok(s) if s.success() => println!("Unmounted successfully."),
            Ok(s) => println!("umount exited with: {s}"),
            Err(e) => println!("Failed to run umount: {e}"),
        }
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::process::Command::new("fusermount")
            .args(["-u", target])
            .status();
        match status {
            Ok(s) if s.success() => println!("Unmounted successfully."),
            Ok(s) => println!("fusermount exited with: {s}"),
            Err(e) => println!("Failed to run fusermount: {e}"),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        println!("Unmount not supported on this platform.");
    }

    Ok(())
}

pub async fn list() -> Result<()> {
    let volumes_dir = manager::default_volumes_dir();
    let svc = VolumeService::new(volumes_dir);

    let volumes = svc.list().await.map_err(utils::api_err)?;

    if volumes.is_empty() {
        println!("No volumes found.");
        println!("Use `tgcryptfs volume create` to create a new volume.");
        return Ok(());
    }

    println!("{:<38}  {:<20}  STATUS", "VOLUME ID", "NAME");
    println!("{}", "-".repeat(70));
    for v in &volumes {
        let status = if v.mounted {
            format!("mounted at {}", v.mount_point.as_deref().unwrap_or("?"))
        } else {
            "unmounted".into()
        };
        println!("{:<38}  {:<20}  {}", v.volume_id, v.display_name, status);
    }
    println!();
    println!("{} volume(s) total.", volumes.len());

    Ok(())
}

pub async fn delete(volume: &str, force: bool) -> Result<()> {
    if !force {
        print!("WARNING: This will permanently destroy volume '{volume}' and all its data. Continue? [y/N] ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let volumes_dir = manager::default_volumes_dir();
    let svc = VolumeService::new(volumes_dir);

    svc.delete(volume).await.map_err(utils::api_err)?;

    println!("Volume '{volume}' deleted.");
    Ok(())
}

pub async fn info(volume: &str) -> Result<()> {
    let volumes_dir = manager::default_volumes_dir();
    let volumes = manager::list_volumes(&volumes_dir).map_err(utils::core_err)?;

    let config = volumes
        .into_iter()
        .find(|v| v.volume_id.to_string() == volume || v.display_name == volume);

    match config {
        Some(c) => {
            println!("Volume Information");
            println!("==================");
            println!("  Volume ID:    {}", c.volume_id);
            println!("  Name:         {}", c.display_name);
            println!("  Block size:   {}", c.block_config.target_block_size);
            println!("  Compression:  {:?}", c.block_config.compression);
            println!("  KDF memory:   {} KiB", c.kdf_params.memory_kib);
            println!("  KDF iter:     {}", c.kdf_params.iterations);
        }
        None => {
            println!("Volume '{volume}' not found.");
            println!("Use `tgcryptfs volume list` to see available volumes.");
        }
    }

    Ok(())
}
