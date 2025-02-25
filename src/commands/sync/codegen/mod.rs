use anyhow::Result;
use log::debug;
use std::collections::BTreeMap;
use std::fmt::Write;

mod flat;
mod nested;

#[derive(Debug, Clone)]
pub enum AssetValue {
    Asset(String),
    Sprite {
        id: String,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

pub trait CodeGenerator {
    fn generate_luau(
        &self,
        assets: &BTreeMap<String, AssetValue>,
        strip_extension: bool,
    ) -> Result<String>;

    fn generate_ts(
        &self,
        assets: &BTreeMap<String, AssetValue>,
        output_name: &str,
        strip_extension: bool,
    ) -> Result<String>;
}

pub struct CodeWriter {
    code: String,
    indent_level: usize,
    indent_str: String,
}

impl CodeWriter {
    pub fn new(indent_str: &str) -> Self {
        CodeWriter {
            code: String::new(),
            indent_level: 0,
            indent_str: indent_str.to_string(),
        }
    }

    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    pub fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    pub fn write_indent(&mut self) -> Result<()> {
        for _ in 0..self.indent_level {
            write!(self.code, "{}", self.indent_str)?;
        }
        Ok(())
    }

    pub fn write_line(&mut self, line: &str) -> Result<()> {
        self.write_indent()?;
        writeln!(self.code, "{}", line)?;
        Ok(())
    }

    pub fn into_string(self) -> String {
        self.code
    }
}

pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn get_relative_path(file_path: &str, asset_dir: &str) -> Result<String> {
    let file_path = normalize_path(file_path);
    let asset_dir = normalize_path(asset_dir);

    let asset_dir = if asset_dir.ends_with('/') {
        asset_dir
    } else {
        format!("{}/", asset_dir)
    };

    debug!("File path: {}", file_path);
    debug!("Asset dir: {}", asset_dir);

    if file_path.starts_with(&asset_dir) {
        let rel_path = file_path.strip_prefix(&asset_dir).unwrap_or(&file_path);
        debug!("Relative path: {}", rel_path);
        Ok(rel_path.to_string())
    } else {
        let asset_dir_no_slash = asset_dir.trim_end_matches('/');
        if file_path.starts_with(asset_dir_no_slash) {
            let rel_path = file_path
                .strip_prefix(asset_dir_no_slash)
                .unwrap_or(&file_path)
                .trim_start_matches('/');
            debug!("Relative path (no slash): {}", rel_path);
            Ok(rel_path.to_string())
        } else {
            debug!(
                "Could not strip prefix '{}' from '{}'",
                asset_dir, file_path
            );
            Ok(file_path)
        }
    }
}

pub fn get_generator(
    style: &crate::commands::sync::config::CodegenStyle,
) -> Box<dyn CodeGenerator> {
    match style {
        crate::commands::sync::config::CodegenStyle::Flat => Box::new(flat::FlatCodeGenerator),
        crate::commands::sync::config::CodegenStyle::Nested => {
            Box::new(nested::NestedCodeGenerator)
        }
    }
}

pub fn generate_luau(
    assets: &BTreeMap<String, AssetValue>,
    asset_dir: &str,
    style: &crate::commands::sync::config::CodegenStyle,
    strip_extension: bool,
) -> Result<String> {
    let mut processed_assets = BTreeMap::new();
    for (path, value) in assets {
        let rel_path = get_relative_path(path, asset_dir)?;
        processed_assets.insert(rel_path, value.clone());
    }

    let generator = get_generator(style);
    generator.generate_luau(&processed_assets, strip_extension)
}

pub fn generate_ts(
    assets: &BTreeMap<String, AssetValue>,
    asset_dir: &str,
    output_name: &str,
    style: &crate::commands::sync::config::CodegenStyle,
    strip_extension: bool,
) -> Result<String> {
    let mut processed_assets = BTreeMap::new();
    for (path, value) in assets {
        let rel_path = get_relative_path(path, asset_dir)?;
        processed_assets.insert(rel_path, value.clone());
    }

    let generator = get_generator(style);
    generator.generate_ts(&processed_assets, output_name, strip_extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_relative_path() {
        assert_eq!(
            get_relative_path("input/file.png", "input").unwrap(),
            "file.png"
        );
        assert_eq!(
            get_relative_path("input/dir/file.png", "input").unwrap(),
            "dir/file.png"
        );
        assert_eq!(
            get_relative_path("input\\dir\\file.png", "input").unwrap(),
            "dir/file.png"
        );
    }
}
