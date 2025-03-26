use std::path::Path;

use crate::{
    cli::MigrateLockfileArgs,
    lockfile::{Lockfile, LockfileEntry},
};
use anyhow::Context;
use blake3::Hasher;
use tokio::fs;

pub async fn migrate_lockfile(args: MigrateLockfileArgs) -> anyhow::Result<()> {
    let lockfile = Lockfile::read().await?;

    let entries = lockfile
        .get_all_if_v0()
        .context("Your lockfile is already up to date")?;

    let mut new_lockfile = Lockfile::default();

    for (path, entry) in entries {
        let path = Path::new(&path);
        let new_hash = read_and_hash(path)
            .await
            .context(format!("Failed to hash {}", path.display()))?;

        new_lockfile.insert(
            &args.input_name,
            path,
            LockfileEntry {
                hash: new_hash,
                asset_id: entry.asset_id,
            },
        );
    }

    new_lockfile.write(None).await?;

    Ok(())
}

async fn read_and_hash(path: &Path) -> anyhow::Result<String> {
    let file = fs::read(path).await?;

    let mut hasher = Hasher::new();
    hasher.update(&file);
    Ok(hasher.finalize().to_string())
}
