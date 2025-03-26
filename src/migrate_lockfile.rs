use anyhow::bail;

use crate::lockfile::Lockfile;

pub async fn migrate_lockfile() -> anyhow::Result<()> {
    let lockfile = Lockfile::read().await?;

    if lockfile.version != 0 {
        bail!("Your lockfile is already up to date");
    }

    todo!()
}
