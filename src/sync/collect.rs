use indicatif::ProgressBar;
use relative_path::RelativePathBuf;
use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;

use crate::{
    asset::AssetRef,
    config::Config,
    lockfile::{Lockfile, LockfileEntry},
    sync::codegen::NodeSource,
};

pub struct CollectResults {
    pub new_lockfile: Lockfile,
    pub input_sources: HashMap<String, NodeSource>,
    pub dupe_count: u32,
    pub new_count: u32,
}

pub async fn collect_events(
    mut rx: Receiver<super::Event>,
    config: Config,
    dry_run: bool,
    spinner: ProgressBar,
) -> anyhow::Result<CollectResults> {
    let mut new_lockfile = Lockfile::default();

    let mut input_sources: HashMap<String, NodeSource> = HashMap::new();
    for (input_name, input) in &config.inputs {
        for (rel_path, web_asset) in &input.web {
            input_sources
                .entry(input_name.clone())
                .or_default()
                .insert(rel_path.clone(), web_asset.clone().into());
        }
    }

    struct Progress {
        spinner: ProgressBar,
        new: u32,
        noop: u32,
        dupes: u32,
    }

    impl Progress {
        fn msg(&self) -> String {
            let mut str = format!("Synced {} files", self.new + self.noop + self.dupes);

            let mut parts = Vec::new();

            if self.new > 0 {
                parts.push(format!("{} new", self.new));
            }
            if self.noop > 0 {
                parts.push(format!("{} no-op", self.noop));
            }
            if self.dupes > 0 {
                parts.push(format!("{} duplicates", self.dupes));
            }

            if parts.is_empty() {
                return str;
            }

            str.push_str(" (");
            str.push_str(&parts.join(", "));
            str.push(')');
            str
        }

        fn update(&self) {
            self.spinner.set_message(self.msg());
        }

        fn finish(&self) {
            self.spinner.finish_with_message(self.msg());
        }
    }

    let mut progress = Progress {
        spinner,
        new: 0,
        noop: 0,
        dupes: 0,
    };

    struct Duplicate {
        input_name: String,
        path: RelativePathBuf,
        original_path: RelativePathBuf,
    }

    let mut duplicates = Vec::<Duplicate>::new();

    while let Some(event) = rx.recv().await {
        match event {
            super::Event::Process {
                new,
                input_name,
                path,
                hash,
                asset_ref,
            } => {
                if let Some(asset_ref) = asset_ref {
                    input_sources
                        .entry(input_name.clone())
                        .or_default()
                        .insert(path, asset_ref.clone());

                    if let AssetRef::Cloud(id) = asset_ref {
                        new_lockfile.insert(&input_name, &hash, LockfileEntry { asset_id: id });
                    }
                }

                if new {
                    progress.new += 1;

                    if !dry_run {
                        new_lockfile.write(None).await?;
                    }
                } else {
                    progress.noop += 1;
                }

                progress.update();
            }
            super::Event::Duplicate {
                input_name,
                path,
                original_path,
            } => {
                progress.dupes += 1;
                progress.update();

                duplicates.push(Duplicate {
                    input_name,
                    path,
                    original_path,
                });
            }
        }
    }

    for dupe in duplicates {
        // If it's a duplicate, then it exists in the map.
        let source = input_sources.get_mut(&dupe.input_name).unwrap();
        let original = source
            .get(&dupe.original_path)
            .expect("We marked a duplicate, but there was no source");

        source.insert(dupe.path, original.clone());
    }

    progress.finish();

    Ok(CollectResults {
        new_lockfile,
        input_sources,
        dupe_count: progress.dupes,
        new_count: progress.new,
    })
}
