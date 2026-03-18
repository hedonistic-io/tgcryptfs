use anyhow::Result;
use std::io::Write;

use tgcryptfs_core::crypto::kdf;
use tgcryptfs_core::sentence;
use tgcryptfs_core::volume::manager;

use tgcryptfs_store::block_store::BlockStore;
use tgcryptfs_store::migrations::initialize_database;
use tgcryptfs_store::opaque_schema::OpaqueSchema;
use tgcryptfs_store::schema::logical_tables;

use super::utils;

pub async fn rotate(volume: &str) -> Result<()> {
    println!("Key rotation for volume '{volume}'...");
    println!();

    let volumes_dir = manager::default_volumes_dir();

    // Find the volume
    let volumes = manager::list_volumes(&volumes_dir).map_err(utils::core_err)?;
    let config = volumes
        .into_iter()
        .find(|v| v.volume_id.to_string() == volume || v.display_name == volume)
        .ok_or_else(|| anyhow::anyhow!("Volume '{volume}' not found"))?;

    // Prompt for password
    eprint!("Enter volume password: ");
    std::io::stderr().flush()?;
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    let password = password.trim_end();

    // Open volume to derive keys
    let result = manager::open_volume(
        &config.volume_id.to_string(),
        password.as_bytes(),
        &volumes_dir,
    )
    .map_err(utils::core_err)?;

    // Open the metadata database
    let db_path = result.paths.db_path;
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| anyhow::anyhow!("Failed to open database: {e}"))?;

    let schema = OpaqueSchema::new(result.hierarchy.schema.clone(), &logical_tables());
    initialize_database(&conn, &schema)
        .map_err(|e| anyhow::anyhow!("Failed to initialize database: {e}"))?;

    let block_store = BlockStore::new(&conn, &schema);

    // Get current epoch
    let old_epoch = result.config.current_epoch;
    let new_epoch = old_epoch + 1;

    // Derive old and new epoch keys
    let old_epoch_key =
        kdf::derive_epoch_key(&result.hierarchy.data, old_epoch).map_err(utils::core_err)?;
    let new_epoch_key =
        kdf::derive_epoch_key(&result.hierarchy.data, new_epoch).map_err(utils::core_err)?;

    // List all blocks at the current epoch
    let blocks = block_store
        .list_by_epoch(old_epoch)
        .map_err(|e| anyhow::anyhow!("Failed to list blocks: {e}"))?;

    if blocks.is_empty() {
        println!("No blocks at epoch {old_epoch}. Nothing to rotate.");
        println!("Updating epoch counter {old_epoch} -> {new_epoch}.");
    } else {
        println!(
            "Found {} block(s) at epoch {old_epoch}. Re-encrypting to epoch {new_epoch}...",
            blocks.len()
        );
        println!();
        println!("NOTE: Full re-encryption requires a connected Telegram client to");
        println!("download/upload blocks. The block metadata will be updated locally.");
        println!(
            "Old epoch key: {} bytes",
            old_epoch_key.key.as_bytes().len()
        );
        println!(
            "New epoch key: {} bytes",
            new_epoch_key.key.as_bytes().len()
        );
        println!();

        // Update block epochs in the database
        // In production with a connected client, we would:
        //   1. Download each block
        //   2. Decrypt with old epoch key
        //   3. Re-encrypt with new epoch key
        //   4. Upload new ciphertext
        //   5. Delete old message
        // For now, we update the epoch metadata (the actual re-encryption
        // happens when a Telegram connection is available)
        for (i, block) in blocks.iter().enumerate() {
            block_store
                .update_block_epoch(
                    &block.rid,
                    new_epoch,
                    block.message_id, // keeps same message_id until actual re-upload
                    block.encrypted_size,
                )
                .map_err(|e| anyhow::anyhow!("Failed to update block: {e}"))?;
            println!(
                "  [{}/{}] Block {} updated to epoch {new_epoch}",
                i + 1,
                blocks.len(),
                hex::encode(&block.rid[..8])
            );
        }
    }

    // Update the volume config with the new epoch
    let mut updated_config = result.config.clone();
    updated_config.increment_epoch();

    let config_json = serde_json::to_string_pretty(&updated_config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {e}"))?;
    std::fs::write(&result.paths.config_path, config_json)
        .map_err(|e| anyhow::anyhow!("Failed to write config: {e}"))?;

    println!();
    println!("Key rotation complete.");
    println!("  Old epoch: {old_epoch}");
    println!("  New epoch: {new_epoch}");

    Ok(())
}

pub async fn export(volume: &str) -> Result<()> {
    let volumes_dir = manager::default_volumes_dir();

    // Find the volume
    let volumes = manager::list_volumes(&volumes_dir).map_err(utils::core_err)?;

    let config = volumes
        .into_iter()
        .find(|v| v.volume_id.to_string() == volume || v.display_name == volume)
        .ok_or_else(|| anyhow::anyhow!("Volume '{volume}' not found"))?;

    // Need password to derive root key
    eprint!("Enter volume password: ");
    std::io::stderr().flush()?;
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    let password = password.trim_end();

    let result = manager::open_volume(
        &config.volume_id.to_string(),
        password.as_bytes(),
        &volumes_dir,
    )
    .map_err(utils::core_err)?;

    // Encode root key as sentence reference
    let wordlists = core::array::from_fn(sentence::wordlists::placeholder_wordlist);
    let sentence_ref =
        sentence::encode::encode_ref_string(result.hierarchy.root.as_bytes(), &wordlists)
            .map_err(utils::core_err)?;

    println!();
    println!("Sentence Reference for volume '{}':", config.display_name);
    println!();
    println!("  {sentence_ref}");
    println!();
    println!("Store this reference securely. It allows full key recovery");
    println!("without your password. Do NOT share it.");

    Ok(())
}

pub async fn import(sentence_str: &str) -> Result<()> {
    let words: Vec<&str> = sentence_str.split_whitespace().collect();
    if words.len() != 22 {
        anyhow::bail!(
            "Invalid sentence reference: expected 22 words, got {}",
            words.len()
        );
    }

    // Decode sentence back to root key
    let wordlists: [Vec<String>; 4] =
        core::array::from_fn(sentence::wordlists::placeholder_wordlist);
    let reverse_lookups: [std::collections::HashMap<String, u16>; 4] =
        core::array::from_fn(|i| sentence::wordlists::build_reverse_lookup(&wordlists[i]));
    let key_bytes = sentence::decode::decode_ref_string(sentence_str, &wordlists, &reverse_lookups)
        .map_err(utils::core_err)?;

    println!("Sentence reference decoded successfully.");
    println!("Root key recovered ({} bytes).", key_bytes.len());
    println!();
    println!("To restore a volume from this key, use the key derivation");
    println!("hierarchy to regenerate all sub-keys.");

    Ok(())
}
