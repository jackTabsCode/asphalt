use super::sync::config::{CodegenConfig, CodegenStyle, Creator, CreatorType, SyncConfig};
use anyhow::Context;
use console::style;
use inquire::{
    validator::{ErrorMessage, Validation},
    Confirm, CustomType, CustomUserError, Select, Text,
};
use log::info;
use std::{path::Path, process::exit};

pub fn dir_validator(str: &str) -> Result<Validation, CustomUserError> {
    let path = Path::new(str);

    if path.is_dir() {
        Ok(Validation::Valid)
    } else {
        Ok(Validation::Invalid(ErrorMessage::Custom(
            "Path does not exist".to_string(),
        )))
    }
}

pub async fn init() -> anyhow::Result<()> {
    let asset_dir = Text::new("Asset source directory")
        .with_validator(dir_validator)
        .with_help_message("The directory of assets to upload to Roblox.")
        .prompt()
        .unwrap_or_else(|_| exit(1));
    let write_dir = Text::new("Output directory")
        .with_validator(dir_validator)
        .with_help_message("The directory to output the generated code to. This should probably be somewhere in your game's source folder.")
        .prompt()
    	.unwrap_or_else(|_| exit(1));

    let creator_type = Select::new("Creator Type", vec![CreatorType::User, CreatorType::Group])
        .with_help_message("The Roblox creator to upload the assets under.")
        .prompt()
        .unwrap_or_else(|_| exit(1));

    let id = CustomType::<u64>::new(format!("{} ID", creator_type).as_str())
        .with_help_message("The ID of the Creator.")
        .with_parser(&|i| match i.parse::<u64>() {
            Ok(val) => Ok(val),
            Err(_) => Err(()),
        })
        .prompt()
        .unwrap_or_else(|_| exit(1));

    let output_name = Text::new("Output name")
        .with_help_message("The name for the generated files.")
        .with_default("assets")
        .prompt_skippable()
        .unwrap_or_else(|_| exit(1));

    let typescript = Confirm::new("TypeScript support")
        .with_help_message("Generate TypeScript definition files.")
        .with_default(false)
        .prompt()
        .unwrap_or_else(|_| exit(1));

    let codegen_style = Select::new("Style", vec![CodegenStyle::Flat, CodegenStyle::Nested])
        .with_help_message("The style to use for generated code.")
        .prompt()
        .unwrap_or_else(|_| exit(1));

    let strip_extension = Confirm::new("Strip file extensions")
        .with_help_message("Strip file extensions from generated code.")
        .with_default(false)
        .prompt()
        .unwrap_or_else(|_| exit(1));

    let config: SyncConfig = SyncConfig {
        asset_dir,
        write_dir,
        exclude_assets: Vec::new(),
        creator: Creator { creator_type, id },
        codegen: CodegenConfig {
            output_name,
            typescript: Some(typescript),
            style: Some(codegen_style),
            strip_extension: Some(strip_extension),
        },
        existing: None,
    };

    config.write().await.context("Failed to write config")?;

    info!(
        "You've successfully set up Asphalt. You can now run {} to upload your assets to Roblox.",
        style("asphalt sync").green()
    );

    Ok(())
}
