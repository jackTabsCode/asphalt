use crate::{cli::MigrateLockfileArgs, lockfile::RawLockfile};

pub async fn migrate_lockfile(args: MigrateLockfileArgs) -> anyhow::Result<()> {
    let file = RawLockfile::read().await?;
    let migrated = file.migrate(args.input_name.as_deref()).await?;
    migrated.write(None).await?;

    Ok(())
}
