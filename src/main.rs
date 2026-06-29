use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, Commands};
use config::Config;
use dotenvy::dotenv;
use indicatif::MultiProgress;
use log::LevelFilter;
use miette::{IntoDiagnostic, WrapErr};
use migrate_lockfile::migrate_lockfile;
use schemars::generate::SchemaSettings;
use sync::sync;
use upload::upload;

mod asset;
mod cli;
mod config;
mod glob;
mod hash;
mod lockfile;
mod migrate_lockfile;
mod pack;
mod sync;
mod upload;
mod util;
mod web_api;

#[tokio::main]
async fn main() -> miette::Result<()> {
    let _ = dotenv();

    let args = Cli::parse();

    let mut binding = env_logger::Builder::new();
    let logger = binding
        .filter_level(LevelFilter::Info)
        .filter_module("asphalt", args.verbose.log_level_filter())
        .format_timestamp(None)
        .format_module_path(false)
        .build();

    let level = logger.filter();

    let multi_progress = MultiProgress::new();
    indicatif_log_bridge::LogWrapper::new(multi_progress.clone(), logger)
        .try_init()
        .into_diagnostic()
        .wrap_err("Failed to initialize logging")?;

    log::set_max_level(level);

    match args.command {
        Commands::Sync(args) => sync(args, multi_progress).await,
        Commands::Upload(args) => upload(args).await,
        Commands::MigrateLockfile(args) => migrate_lockfile(args).await,
        Commands::GenerateSchema(args) => generate_schema(args).await,
        Commands::Completions(args) => {
            generate_completions(args);
            Ok(())
        }
        Commands::Check(args) => check_config(args).await,
        Commands::List(args) => list_assets(args).await,
        Commands::GenerateConfigSchema => generate_config_schema().await,
    }
    .map_err(|e| miette::miette!(e))
}

async fn generate_schema(args: cli::GenerateSchemaArgs) -> anyhow::Result<()> {
    use anyhow::Context;
    use fs_err::tokio as fs;
    use std::path::Path;

    let settings = SchemaSettings::draft07();
    let generator = settings.into_generator();
    let schema = generator.into_root_schema_for::<Config>();
    let schema_json =
        serde_json::to_string_pretty(&schema).context("Failed to serialize JSON schema")?;

    let output_path = Path::new(&args.output);
    if let Some(parent_dir) = output_path.parent() {
        fs::create_dir_all(parent_dir)
            .await
            .with_context(|| format!("Failed to create directory: {}", parent_dir.display()))?;
    }

    fs::write(output_path, schema_json)
        .await
        .with_context(|| format!("Failed to write schema to: {}", output_path.display()))?;

    println!("Generated JSON schema at: {}", args.output);
    Ok(())
}

async fn generate_config_schema() -> anyhow::Result<()> {
    let settings = SchemaSettings::draft07();
    let generator = settings.into_generator();
    let schema = generator.into_root_schema_for::<Config>();
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

fn generate_completions(args: cli::CompletionsArgs) {
    let mut cmd = Cli::command();
    generate(args.shell, &mut cmd, "asphalt", &mut std::io::stdout());
}

async fn check_config(args: cli::ProjectArgs) -> anyhow::Result<()> {
    use anyhow::Context;

    let config = Config::read_from(args.project)
        .await
        .context("Failed to read configuration file")?;

    println!("Configuration is valid");
    println!("  Creator: {:?} #{}", config.creator.ty, config.creator.id);
    println!("  Inputs: {}", config.inputs.len());

    for (name, input) in &config.inputs {
        println!("    - {}: {}", name, input.include.get_prefix().display());
    }

    Ok(())
}

async fn list_assets(args: cli::ProjectArgs) -> anyhow::Result<()> {
    use anyhow::Context;
    use walkdir::WalkDir;

    let config = Config::read_from(args.project)
        .await
        .context("Failed to read configuration file")?;

    println!("Assets that would be synced:\n");

    for (input_name, input) in &config.inputs {
        println!("Input: {input_name}");

        let mut count = 0;
        let input_prefix = config.project_dir.join(input.include.get_prefix());
        for entry in WalkDir::new(&input_prefix)
            .follow_links(true)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let Ok(rel_path) = entry.path().strip_prefix(&config.project_dir) else {
                continue;
            };
            if input.include.is_match(rel_path) {
                println!("  - {}", entry.path().display());
                count += 1;
            }
        }

        println!("  Total: {count} files\n");
    }

    Ok(())
}
