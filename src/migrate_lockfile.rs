use crate::{cli::MigrateLockfileArgs, config::Config, lockfile::RawLockfile};

pub async fn migrate_lockfile(args: MigrateLockfileArgs) -> anyhow::Result<()> {
    let config = Config::read_from(args.project).await?;
    let file = RawLockfile::read_from(&config.project_dir).await?;
    let migrated = file.migrate(args.input_name.as_deref()).await?;
    migrated.write_to(&config.project_dir).await?;

    Ok(())
}
