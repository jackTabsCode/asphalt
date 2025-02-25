use super::{AssetValue, CodeGenerator, CodeWriter};
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub struct NestedCodeGenerator;

#[derive(Debug)]
enum NestedValue<'a> {
    Folder(BTreeMap<String, NestedValue<'a>>),
    Asset(&'a AssetValue),
}

fn get_path_components(path: &str, strip_extension: bool) -> Vec<String> {
    let path_buf = if strip_extension {
        Path::new(path).with_extension("")
    } else {
        PathBuf::from(path)
    };

    path_buf
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

fn build_nested_tree(
    assets: &BTreeMap<String, AssetValue>,
    strip_extension: bool,
) -> Result<BTreeMap<String, NestedValue<'_>>> {
    let mut root = BTreeMap::new();

    for (path, value) in assets {
        let components = get_path_components(path, strip_extension);
        if components.is_empty() {
            continue;
        }

        let mut current = &mut root;

        for (i, component) in components.iter().enumerate() {
            if i == components.len() - 1 {
                if let Some(existing) = current.get(component) {
                    match existing {
                        NestedValue::Folder(_) => {
                            bail!("Path conflict: {} is both a folder and a file", component);
                        }
                        NestedValue::Asset(_) => {
                            bail!("Duplicate asset path: {}", path);
                        }
                    }
                }
                current.insert(component.clone(), NestedValue::Asset(value));
            } else {
                let entry = current
                    .entry(component.clone())
                    .or_insert_with(|| NestedValue::Folder(BTreeMap::new()));

                current = match entry {
                    NestedValue::Folder(children) => children,
                    NestedValue::Asset(_) => {
                        bail!("Path conflict: {} is both a file and a folder", component);
                    }
                };
            }
        }
    }

    Ok(root)
}

fn write_nested_luau(
    writer: &mut CodeWriter,
    tree: &BTreeMap<String, NestedValue<'_>>,
) -> Result<()> {
    let mut sorted_keys: Vec<_> = tree.keys().collect();
    sorted_keys.sort();

    for key in sorted_keys {
        match &tree[key] {
            NestedValue::Folder(children) => {
                writer.write_line(&format!("{} = {{", key))?;

                writer.indent();
                write_nested_luau(writer, children)?;
                writer.dedent();

                writer.write_line("},")?;
            }
            NestedValue::Asset(value) => match value {
                AssetValue::Asset(asset_id) => {
                    writer.write_line(&format!("{} = \"{}\",", key, asset_id))?;
                }
                AssetValue::Sprite {
                    id,
                    x,
                    y,
                    width,
                    height,
                } => {
                    writer.write_line(&format!("{} = {{", key))?;

                    writer.indent();
                    writer.write_line(&format!("id = \"{}\",", id))?;
                    writer.write_line(&format!("x = {},", x))?;
                    writer.write_line(&format!("y = {},", y))?;
                    writer.write_line(&format!("width = {},", width))?;
                    writer.write_line(&format!("height = {},", height))?;
                    writer.dedent();

                    writer.write_line("},")?;
                }
            },
        }
    }

    Ok(())
}

fn write_nested_ts(
    writer: &mut CodeWriter,
    tree: &BTreeMap<String, NestedValue<'_>>,
) -> Result<()> {
    let mut sorted_keys: Vec<_> = tree.keys().collect();
    sorted_keys.sort();

    for key in sorted_keys {
        match &tree[key] {
            NestedValue::Folder(children) => {
                writer.write_line(&format!("{}: {{", key))?;

                writer.indent();
                write_nested_ts(writer, children)?;
                writer.dedent();

                writer.write_line("}")?;
            }
            NestedValue::Asset(value) => match value {
                AssetValue::Asset(_) => {
                    writer.write_line(&format!("{}: string", key))?;
                }
                AssetValue::Sprite { .. } => {
                    writer.write_line(&format!("{}: {{", key))?;

                    writer.indent();
                    writer.write_line("id: string")?;
                    writer.write_line("x: number")?;
                    writer.write_line("y: number")?;
                    writer.write_line("width: number")?;
                    writer.write_line("height: number")?;
                    writer.dedent();

                    writer.write_line("}")?;
                }
            },
        }
    }

    Ok(())
}

impl CodeGenerator for NestedCodeGenerator {
    fn generate_luau(
        &self,
        assets: &BTreeMap<String, AssetValue>,
        strip_extension: bool,
    ) -> Result<String> {
        let tree = build_nested_tree(assets, strip_extension)?;

        let mut writer = CodeWriter::new("\t");
        writer.write_line("return {")?;

        writer.indent();
        write_nested_luau(&mut writer, &tree)?;
        writer.dedent();

        writer.write_line("}")?;

        Ok(writer.into_string())
    }

    fn generate_ts(
        &self,
        assets: &BTreeMap<String, AssetValue>,
        output_name: &str,
        strip_extension: bool,
    ) -> Result<String> {
        let tree = build_nested_tree(assets, strip_extension)?;

        let mut writer = CodeWriter::new("\t");
        writer.write_line(&format!("declare const {}: {{", output_name))?;

        writer.indent();
        write_nested_ts(&mut writer, &tree)?;
        writer.dedent();

        writer.write_line("};")?;
        writer.write_line(&format!("export = {};", output_name))?;

        Ok(writer.into_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_path_components() {
        let components = get_path_components("dir/file.png", false);
        assert_eq!(components, vec!["dir", "file.png"]);

        let components = get_path_components("dir/file.png", true);
        assert_eq!(components, vec!["dir", "file"]);

        let components = get_path_components("file.png", false);
        assert_eq!(components, vec!["file.png"]);
    }

    #[test]
    fn test_generate_luau_nested() {
        let generator = NestedCodeGenerator;

        let mut assets = BTreeMap::new();
        assets.insert(
            "file.png".to_string(),
            AssetValue::Asset("rbxassetid://12345".to_string()),
        );
        assets.insert(
            "dir/sprite.png".to_string(),
            AssetValue::Sprite {
                id: "rbxassetid://67890".to_string(),
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            },
        );
        assets.insert(
            "dir/subdir/other.png".to_string(),
            AssetValue::Asset("rbxassetid://54321".to_string()),
        );

        let code = generator.generate_luau(&assets, false).unwrap();
        assert!(code.contains("dir = {"));
        assert!(code.contains("sprite.png = {"));
        assert!(code.contains("id = \"rbxassetid://67890\""));
        assert!(code.contains("x = 10"));
        assert!(code.contains("subdir = {"));
        assert!(code.contains("other.png = \"rbxassetid://54321\""));
        assert!(code.contains("file.png = \"rbxassetid://12345\""));
    }

    #[test]
    fn test_generate_ts_nested() {
        let generator = NestedCodeGenerator;

        let mut assets = BTreeMap::new();
        assets.insert(
            "file.png".to_string(),
            AssetValue::Asset("rbxassetid://12345".to_string()),
        );
        assets.insert(
            "dir/sprite.png".to_string(),
            AssetValue::Sprite {
                id: "rbxassetid://67890".to_string(),
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            },
        );

        let code = generator.generate_ts(&assets, "assets", false).unwrap();
        assert!(code.contains("declare const assets:"));
        assert!(code.contains("dir: {"));
        assert!(code.contains("sprite.png: {"));
        assert!(code.contains("id: string;"));
        assert!(code.contains("width: number;"));
        assert!(code.contains("file.png: string;"));
    }
}
