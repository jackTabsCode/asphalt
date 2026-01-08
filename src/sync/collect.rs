use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use tokio::sync::mpsc::UnboundedReceiver;

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
    pub new_count: u64,
    pub any_failed: bool,
}

pub async fn collect_events(
    mut rx: UnboundedReceiver<super::Event>,
    target: SyncTarget,
    inputs: InputMap,
    mp: MultiProgress,
    base_dir: &std::path::Path,
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

    let mut seen_paths = HashSet::new();

    while let Some(event) = rx.recv().await {
        match event {
            super::Event::Discovered(path) => {
                if !seen_paths.contains(&path) {
                    progress.discovered += 1;
                }
            }
            super::Event::InFlight(path) => {
                if !seen_paths.contains(&path) {
                    progress.in_flight.insert(path.clone());
                }
            }
            super::Event::Finished {
                state,
                input_name,
                path,
                rel_path,
                hash,
                asset_ref,
            } => {
                seen_paths.insert(path.clone());

                if let Some(asset_ref) = asset_ref {
                    input_sources
                        .entry(input_name.clone())
                        .or_default()
                        .insert(rel_path.clone(), asset_ref.clone());

                    if let AssetRef::Cloud(id) = asset_ref {
                        new_lockfile.insert(&input_name, &hash, LockfileEntry { asset_id: id });
                    }
                }

                match state {
                    super::EventState::Synced { new } => {
                        progress.synced += 1;
                        if new {
                            progress.new += 1;
                            if target.write_on_sync() {
                                new_lockfile.write_to(base_dir).await?;
                            }
                        }
                    }
                    super::EventState::Duplicate => {
                        progress.dupes += 1;
                    }
                }

                progress.in_flight.remove(&path);
            }
            super::Event::Failed(path) => {
                progress.failed += 1;
                progress.in_flight.remove(&path);
            }
        }

        progress.update();
    }

    progress.finish();

    Ok(CollectResults {
        new_lockfile,
        input_sources,
        new_count: progress.new,
        any_failed: progress.failed > 0,
    })
}

struct Progress {
    inner: ProgressBar,
    target: SyncTarget,
    in_flight: HashSet<PathBuf>,
    discovered: u64,
    synced: u64,
    new: u64,
    dupes: u64,
    failed: u64,
}

impl Progress {
    fn get_style(finished: bool) -> ProgressStyle {
        ProgressStyle::default_bar()
            .template(&format!(
                "{{prefix:.{prefix_color}.bold}}{bar} {{pos}}/{{len}} assets: ({{msg}})",
                prefix_color = if finished { "green" } else { "cyan" },
                bar = if finished { "" } else { " [{bar:40}]" },
            ))
            .unwrap()
            .progress_chars("=> ")
    }

    fn new(mp: MultiProgress, target: SyncTarget) -> Self {
        let spinner = mp.add(ProgressBar::new_spinner());
        spinner.set_style(Progress::get_style(false));
        spinner.set_prefix("Syncing");
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        Self {
            inner: spinner,
            target,
            in_flight: HashSet::new(),
            discovered: 0,
            synced: 0,
            new: 0,
            dupes: 0,
            failed: 0,
        }
    }

    fn get_msg(&self) -> String {
        let mut parts = Vec::new();

        if self.new > 0 {
            let target_msg = match self.target {
                SyncTarget::Cloud { dry_run: true } => "checked",
                SyncTarget::Cloud { dry_run: false } => "uploaded",
                SyncTarget::Studio | SyncTarget::Debug => "written",
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

        let in_flight = self.in_flight.len();
        if in_flight > 0 {
            parts.push(format!("{} processing", in_flight));
        }

        let failed = self.failed;
        if failed > 0 {
            parts.push(format!("{} failed", failed));
        }

        parts.join(", ")
    }

    fn update(&self) {
        self.inner.set_position(self.synced + self.dupes);
        self.inner.set_length(self.discovered);
        self.inner.set_message(self.get_msg());
    }

    fn finish(&self) {
        self.inner.set_prefix("Synced");
        self.inner.set_style(Progress::get_style(true));
        self.inner.finish();
    }
}
