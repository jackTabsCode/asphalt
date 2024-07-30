use anyhow::Context;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use super::{
    ast::{AstTarget, Expression},
    generate_code,
};

fn asset_path(file_path: &str, strip_dir: &str, strip_extension: bool) -> anyhow::Result<String> {
    if strip_extension {
        Path::new(file_path).with_extension("")
    } else {
        PathBuf::from(file_path)
    }
    .to_str()
    .unwrap()
    .strip_prefix(strip_dir)
    .context("Failed to strip directory prefix")
    .map(|s| s.to_string())
}

fn generate_table(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<Expression> {
    let mut expressions: Vec<(Expression, Expression)> = Vec::new();
    for (file_path, asset_id) in assets.iter() {
        let file_stem = asset_path(file_path, strip_dir, strip_extension)?;
        expressions.push((
            Expression::String(file_stem),
            Expression::String(asset_id.clone()),
        ));
    }
    Ok(Expression::table(expressions))
}

pub fn generate_luau(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    let table =
        generate_table(assets, strip_dir, strip_extension).context("Failed to generate table")?;
    generate_code(table, AstTarget::Luau)
}

pub fn generate_ts(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    output_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    let table =
        generate_table(assets, strip_dir, strip_extension).context("Failed to generate table")?;
    generate_code(
        table,
        AstTarget::Typescript {
            output_dir: output_dir.to_owned(),
        },
    )
}
