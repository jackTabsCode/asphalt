use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;

use crate::{
    asset::AssetRef,
    cli::SyncTarget,
    config::InputMap,
    lockfile::{Lockfile, LockfileEntry},
    sync::codegen::NodeSource,
};

pub struct CollectResults {
    pub new_lockfile: Lockfile,
    pub input_sources: HashMap<String, NodeSource>,
    pub new_count: u32,
}

pub async fn collect_events(
    mut rx: Receiver<super::Event>,
    target: SyncTarget,
    inputs: InputMap,
    mp: MultiProgress,
) -> anyhow::Result<CollectResults> {
    let mut new_lockfile = Lockfile::default();

    let mut input_sources: HashMap<String, NodeSource> = HashMap::new();
    for (input_name, input) in inputs {
        for (rel_path, web_asset) in &input.web {
            input_sources
                .entry(input_name.clone())
                .or_default()
                .insert(rel_path.clone(), web_asset.clone().into());
        }
    }

    let mut progress = Progress::new(mp, target);

    while let Some(super::Event {
        ty,
        input_name,
        path,
        hash,
        asset_ref,
    }) = rx.recv().await
    {
        if let Some(asset_ref) = asset_ref {
            input_sources
                .entry(input_name.clone())
                .or_default()
                .insert(path, asset_ref.clone());

            if let AssetRef::Cloud(id) = asset_ref {
                new_lockfile.insert(&input_name, &hash, LockfileEntry { asset_id: id });
            }
        }

        match ty {
            super::EventType::Synced { new } => {
                progress.synced += 1;
                if new {
                    progress.new += 1;
                    if target.write_on_sync() {
                        new_lockfile.write(None).await?;
                    }
                }
            }
            super::EventType::Duplicate => {
                progress.dupes += 1;
            }
        }

        progress.update_msg();
    }

    progress.finish();

    Ok(CollectResults {
        new_lockfile,
        input_sources,
        new_count: progress.new,
    })
}

struct Progress {
    spinner: ProgressBar,
    target: SyncTarget,
    synced: u32,
    new: u32,
    dupes: u32,
}

impl Progress {
    fn new(mp: MultiProgress, target: SyncTarget) -> Self {
        let spinner = mp.add(ProgressBar::new_spinner());
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));
        spinner.set_message("Starting sync...");

        Self {
            spinner,
            target,
            synced: 0,
            new: 0,
            dupes: 0,
        }
    }

    fn get_msg(&self) -> String {
        let mut str = format!("Synced {} files", self.synced);

        let mut parts = Vec::new();

        if self.new > 0 {
            let target_msg = match self.target {
                SyncTarget::Cloud { dry_run: true } => "uploaded",
                SyncTarget::Cloud { dry_run: false } => "checked",
                SyncTarget::Studio => "written to content folder",
                SyncTarget::Debug => "written to debug folder",
            };
            parts.push(format!("{} {}", self.new, target_msg));
        }
        let noop = self.synced - self.new;
        if noop > 0 {
            parts.push(format!("{} no-op", noop));
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

    fn update_msg(&self) {
        self.spinner.set_message(self.get_msg());
    }

    fn finish(&self) {
        self.spinner.finish_with_message(self.get_msg());
    }
}
