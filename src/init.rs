use crate::config::{self, Config, CreatorType};
use anyhow::bail;
use fs_err::tokio as fs;

pub async fn init() -> anyhow::Result<()> {
    if fs::metadata(config::FILE_NAME).await.is_ok() {
        bail!("Configuration file already exists");
    }

    let config = inquire_config();

    todo!()
}

fn inquire_config() -> Config {
    let creator_type =
        inquire::Select::new("Creator Type", vec![CreatorType::User, CreatorType::Group])
            .prompt()
            .unwrap();

    todo!()
}
