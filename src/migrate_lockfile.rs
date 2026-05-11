use crate::{
    cli::MigrateLockfileArgs, config::Config, input_name::InputName, lockfile::RawLockfile,
};

pub async fn migrate_lockfile(args: MigrateLockfileArgs) -> anyhow::Result<()> {
    let config = Config::read_from(args.project).await?;
    let file = RawLockfile::read_from(&config.project_dir).await?;
    let input_name = args.input_name.map(InputName::new).transpose()?;
    let migrated = file.migrate(input_name.as_ref()).await?;
    migrated.write_to(&config.project_dir).await?;

    Ok(())
}
