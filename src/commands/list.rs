use crate::LockFile;

pub async fn list(lockfile: LockFile) -> anyhow::Result<()> {
    for (path, entry) in lockfile.entries {
        println!("\"{}\": {}", path, entry.asset_id);
    }

    Ok(())
}
