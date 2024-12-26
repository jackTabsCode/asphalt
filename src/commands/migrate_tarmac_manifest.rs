use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anyhow::Context;
use resvg::usvg::fontdb::Database;
use serde::{Deserialize, Serialize};

use crate::asset::Asset;

#[derive(Debug, Serialize, Deserialize)]
struct TarmacManifest {
    inputs: BTreeMap<PathBuf, TarmacEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TarmacEntry {
    id: u64,
}

pub async fn migrate_manifest(args: MigrateTarmacManifestArgs) -> anyhow::Result<()> {
    let tarmac_manifest_contents = std::fs::read_to_string(&args.manifest_path)
        .context("Failed to open tarmac-manifest.toml")?;
    let tarmac_manifest: TarmacManifest = toml::from_str(&tarmac_manifest_contents)
        .context("Failed to parse tarmac-manifest.toml")?;

    let mut lockfile = crate::LockFile::default();

    for (path, entry) in tarmac_manifest.inputs {
        let content_path = args.manifest_path.with_file_name(&path);
        let content = match std::fs::read(&content_path) {
            Ok(content) => content,
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    log::warn!(
                        "Content file {} not found, skipping",
                        content_path.display()
                    );

                    continue;
                } else {
                    return Err(error).with_context(|| {
                        format!("Failed to read content file {}", content_path.display())
                    });
                }
            }
        };

        let font_db = Arc::new(Database::new());

        let asset = Asset::new(
            path.to_string_lossy().to_string(),
            content,
            &path.extension().unwrap_or_default().to_string_lossy(),
            font_db,
        )
        .await
        .with_context(|| format!("Failed to create asset for {}", path.to_string_lossy()))?;

        lockfile.entries.insert(
            path.to_string_lossy().to_string(),
            crate::FileEntry {
                asset_id: entry.id,
                hash: asset.hash(),
            },
        );
    }

    lockfile
        .write(
            &args
                .manifest_path
                .with_file_name(crate::lockfile::FILE_NAME),
        )
        .await
        .context("Failed to write Asphalt lockfile")?;

    Ok(())
}

#[derive(clap::Args)]
pub struct MigrateTarmacManifestArgs {
    /// The path to the tarmac-manifest.toml file.
    #[clap(default_value = "tarmac-manifest.toml")]
    pub manifest_path: PathBuf,
}
