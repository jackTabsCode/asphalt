use anyhow::{bail, Context};
use std::collections::BTreeMap;
use std::{path::Component as PathComponent, path::Path};

use crate::ast::{AstTarget, Expression, ReturnStatement};
use crate::LockFile;
use std::fmt::Write;

use self::types::TarmacTable;

pub(crate) mod types {
    use std::collections::BTreeMap;

    use crate::FileEntry;

    #[derive(Debug)]
    pub enum TarmacTable<'a> {
        Folder(BTreeMap<String, TarmacTable<'a>>),
        File(&'a FileEntry),
    }
}

/// Recursively builds a **[`TarmacTable`]** (normally a root table) into expressions that can be evaluated in the `ast`.
fn build_table(entry: &TarmacTable) -> Expression {
    match entry {
        TarmacTable::Folder(entries) => Expression::table(
            entries
                .iter()
                .map(|(component, entry)| (component.into(), build_table(entry)))
                .collect(),
        ),
        TarmacTable::File(file) => Expression::String(format!("rbxassetid://{}", file.asset_id)),
    }
}

/**
 * Creates expressions based on the **[`LockFile`]**, and will strip the prefix
 * and iterate through every file entry and build a table for code generation.
*/
fn generate_expressions(lockfile: &LockFile, strip_dir: &str) -> anyhow::Result<Expression> {
    let mut root: BTreeMap<String, TarmacTable<'_>> = BTreeMap::new();

    for (file_path, file_entry) in lockfile.entries.iter() {
        let mut components = vec![];
        let path = Path::new(file_path)
            .strip_prefix(strip_dir)
            .context("Failed to strip directory prefix")?;

        for component in path.components() {
            match component {
                PathComponent::RootDir | PathComponent::Prefix(..) | PathComponent::Normal(..) => {
                    components.push(
                        component
                            .as_os_str()
                            .to_str()
                            .expect("Failed to resolve path component"),
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
                    current_directory.insert(component.to_owned(), TarmacTable::File(file_entry));
                };
            } else if let TarmacTable::Folder(entries) = current_directory
                .entry(component.to_owned())
                .or_insert_with(|| TarmacTable::Folder(BTreeMap::new()))
            {
                current_directory = entries;
            } else {
                unreachable!()
            }
        }
    }

    Ok(build_table(&TarmacTable::Folder(root)))
}

pub fn generate_lua(lockfile: &LockFile, strip_dir: &str) -> anyhow::Result<String> {
    generate_code(
        generate_expressions(lockfile, strip_dir).expect("Failed to create tarmac expressions"),
        AstTarget::Lua,
    )
}

pub fn generate_ts(
    lockfile: &LockFile,
    strip_dir: &str,
    output_dir: &str,
) -> anyhow::Result<String> {
    generate_code(
        generate_expressions(lockfile, strip_dir).expect("Failed to create tarmac expressions"),
        AstTarget::Typescript {
            output_dir: output_dir.to_owned(),
        },
    )
}

fn generate_code(expression: Expression, target: AstTarget) -> anyhow::Result<String> {
    let mut buffer = String::new();
    write!(buffer, "{}", ReturnStatement(expression, target))?;
    Ok(buffer)
}
