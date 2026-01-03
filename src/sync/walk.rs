use crate::{
    asset::{self},
    config::Input,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn walk_input(input: &Input, mp: MultiProgress) -> Vec<PathBuf> {
    let spinner = mp.add(ProgressBar::new_spinner().with_style(
        ProgressStyle::with_template("{prefix:.cyan.bold} `{msg}` {spinner}").unwrap(),
    ));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner.set_prefix("Walking");
    spinner.set_message(input.include.get_prefix().display().to_string());

    WalkDir::new(input.include.get_prefix())
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();
            path == input.include.get_prefix() || input.include.is_match(path)
        })
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.into_path();
            if path.is_file()
                && matches!(path.extension(), Some(ext) if asset::is_supported_extension(ext))
            {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}
