use crate::{cli::MigrateLockfileArgs, lockfile::Lockfile};

pub async fn migrate_lockfile(args: MigrateLockfileArgs) -> anyhow::Result<()> {
    let mut file = Lockfile::read().await?;
    file.migrate(args.input_name).await?;
    file.write(None).await?;

    Ok(())
}
