use super::{AssetValue, CodeGenerator, CodeWriter};
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

pub struct FlatCodeGenerator;

fn process_path(path: &str, strip_extension: bool) -> String {
    let path = if strip_extension {
        Path::new(path)
            .with_extension("")
            .to_string_lossy()
            .to_string()
    } else {
        path.to_string()
    };

    path
}

impl CodeGenerator for FlatCodeGenerator {
    fn generate_luau(
        &self,
        assets: &BTreeMap<String, AssetValue>,
        strip_extension: bool,
    ) -> Result<String> {
        let mut writer = CodeWriter::new("\t");
        writer.write_line("return {")?;

        let mut sorted_assets: Vec<_> = assets.iter().collect();
        sorted_assets.sort_by(|a, b| a.0.cmp(b.0));

        writer.indent();

        for (path, value) in sorted_assets {
            let processed_path = process_path(path, strip_extension);

            match value {
                AssetValue::Asset(asset_id) => {
                    writer.write_line(&format!("[\"{}\"] = \"{}\",", processed_path, asset_id))?;
                }
                AssetValue::Sprite {
                    id,
                    x,
                    y,
                    width,
                    height,
                } => {
                    writer.write_line(&format!("[\"{}\"] = {{", processed_path))?;
                    writer.indent();
                    writer.write_line(&format!("id = \"{}\",", id))?;
                    writer.write_line(&format!("x = {},", x))?;
                    writer.write_line(&format!("y = {},", y))?;
                    writer.write_line(&format!("width = {},", width))?;
                    writer.write_line(&format!("height = {},", height))?;
                    writer.dedent();
                    writer.write_line("},")?;
                }
            }
        }

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
        let mut writer = CodeWriter::new("\t");
        writer.write_line(&format!("declare const {}: {{", output_name))?;

        let mut sorted_assets: Vec<_> = assets.iter().collect();
        sorted_assets.sort_by(|a, b| a.0.cmp(b.0));

        writer.indent();

        for (path, value) in sorted_assets {
            let processed_path = process_path(path, strip_extension);

            match value {
                AssetValue::Asset(_) => {
                    writer.write_line(&format!("\"{}\": string", processed_path))?;
                }
                AssetValue::Sprite { .. } => {
                    writer.write_line(&format!("\"{}\": {{", processed_path))?;
                    writer.indent();
                    writer.write_line("id: string")?;
                    writer.write_line("x: number")?;
                    writer.write_line("y: number")?;
                    writer.write_line("width: number")?;
                    writer.write_line("height: number")?;
                    writer.dedent();
                    writer.write_line("}")?;
                }
            }
        }

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
    fn test_process_path() {
        assert_eq!(process_path("file.png", false), "/file.png");
        assert_eq!(process_path("/file.png", false), "/file.png");
        assert_eq!(process_path("dir/file.png", false), "/dir/file.png");
        assert_eq!(process_path("file.png", true), "/file");
    }

    #[test]
    fn test_generate_luau_flat() {
        let generator = FlatCodeGenerator;

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

        let code = generator.generate_luau(&assets, false).unwrap();
        assert!(code.contains("[\"/dir/sprite.png\"]"));
        assert!(code.contains("[\"/file.png\"]"));
        assert!(code.contains("id = \"rbxassetid://67890\""));
        assert!(code.contains("x = 10"));
        assert!(code.contains("width = 30"));
    }

    #[test]
    fn test_generate_ts_flat() {
        let generator = FlatCodeGenerator;

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
        assert!(code.contains("\"/dir/sprite.png\": {"));
        assert!(code.contains("\"/file.png\": string;"));
        assert!(code.contains("x: number;"));
        assert!(code.contains("width: number;"));
    }
}
