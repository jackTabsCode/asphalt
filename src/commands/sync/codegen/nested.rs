use self::types::NestedTable;
use super::ast::{AstTarget, Expression};
use super::generate_code;
use anyhow::{bail, Context};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{path::Component as PathComponent, path::Path};

pub(crate) mod types {
    use std::collections::BTreeMap;

    #[derive(Debug)]
    pub enum NestedTable<'a> {
        Folder(BTreeMap<String, NestedTable<'a>>),
        Asset(&'a String),
    }
}

/// Recursively builds a **[`NestedTable`]** (normally a root table) into expressions that can be evaluated in the `ast`.
fn build_table(entry: &NestedTable) -> Expression {
    match entry {
        NestedTable::Folder(entries) => Expression::table(
            entries
                .iter()
                .map(|(component, entry)| (component.into(), build_table(entry)))
                .collect(),
        ),
        NestedTable::Asset(asset_id) => Expression::String(asset_id.to_string()),
    }
}

/**
 * Creates expressions based on a map of assets and builds a table for code generation.
*/
fn generate_expressions(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<Expression> {
    let mut root: BTreeMap<String, NestedTable<'_>> = BTreeMap::new();

    for (file_path, asset_id) in assets.iter() {
        let mut components = vec![];
        let full_path = if strip_extension {
            Path::new(file_path).with_extension("")
        } else {
            PathBuf::from(file_path)
        };
        let path = full_path
            .strip_prefix(strip_dir)
            .context("Failed to strip directory prefix")?;

        for component in path.components() {
            match component {
                PathComponent::RootDir | PathComponent::Prefix(..) | PathComponent::Normal(..) => {
                    components.push(
                        component
                            .as_os_str()
                            .to_str()
                            .context("Failed to resolve path component")?,
                    )
                }
                PathComponent::ParentDir => {
                    if components.pop().is_none() {
                        bail!("Failed to resolve parent directory")
                    }
                }
                _ => {}
            }
        }

        let mut current_directory = &mut root;
        for (index, &component) in components.iter().enumerate() {
            // last component is assumed to be a file.
            if index == components.len() - 1 {
                if current_directory.get_mut(component).is_none() {
                    current_directory.insert(component.to_owned(), NestedTable::Asset(asset_id));
                };
            } else if let NestedTable::Folder(entries) = current_directory
                .entry(component.to_owned())
                .or_insert_with(|| NestedTable::Folder(BTreeMap::new()))
            {
                current_directory = entries;
            } else {
                unreachable!()
            }
        }
    }

    Ok(build_table(&NestedTable::Folder(root)))
}

pub fn generate_luau(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    generate_code(
        generate_expressions(assets, strip_dir, strip_extension)
            .context("Failed to generate nested table")?,
        AstTarget::Luau,
    )
}

pub fn generate_ts(
    assets: &BTreeMap<String, String>,
    strip_dir: &str,
    output_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    generate_code(
        generate_expressions(assets, strip_dir, strip_extension)
            .context("Failed to generate nested table")?,
        AstTarget::Typescript {
            output_dir: output_dir.to_owned(),
        },
    )
}
