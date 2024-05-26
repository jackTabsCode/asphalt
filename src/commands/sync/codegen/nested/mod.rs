use self::types::NestedTable;
use crate::LockFile;
use anyhow::{bail, Context};
use ast::{AstTarget, Expression, ReturnStatement};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;
use std::{path::Component as PathComponent, path::Path};

mod ast;

pub(crate) mod types {
    use std::collections::BTreeMap;

    use crate::FileEntry;

    #[derive(Debug)]
    pub enum NestedTable<'a> {
        Folder(BTreeMap<String, NestedTable<'a>>),
        File(&'a FileEntry),
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
        NestedTable::File(file) => Expression::String(format!("rbxassetid://{}", file.asset_id)),
    }
}

/**
 * Creates expressions based on the **[`LockFile`]**, and will strip the prefix
 * and iterate through every file entry and build a table for code generation.
*/
fn generate_expressions(
    lockfile: &LockFile,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<Expression> {
    let mut root: BTreeMap<String, NestedTable<'_>> = BTreeMap::new();

    for (file_path, file_entry) in lockfile.entries.iter() {
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
                    current_directory.insert(component.to_owned(), NestedTable::File(file_entry));
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

pub fn generate_lua(
    lockfile: &LockFile,
    strip_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    generate_code(
        generate_expressions(lockfile, strip_dir, strip_extension)
            .context("Failed to create nested expressions")?,
        AstTarget::Lua,
    )
}

pub fn generate_ts(
    lockfile: &LockFile,
    strip_dir: &str,
    output_dir: &str,
    strip_extension: bool,
) -> anyhow::Result<String> {
    generate_code(
        generate_expressions(lockfile, strip_dir, strip_extension)
            .context("Failed to create nested expressions")?,
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
