use crate::config::{Codegen, CodegenStyle};
use anyhow::bail;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub type CodegenInput = BTreeMap<PathBuf, String>;

pub enum CodegenNode {
    Table(BTreeMap<String, CodegenNode>),
    String(String),
    #[allow(dead_code)]
    Number(u64),
}

pub enum CodegenLanguage {
    TypeScript,
    Luau,
}

pub fn from_codegen_input(input: &CodegenInput, config: &Codegen) -> CodegenNode {
    let mut root = CodegenNode::Table(BTreeMap::new());

    for (path, value) in input {
        match config.style {
            CodegenStyle::Nested => {
                let components = normalize_path_components(path, config.strip_extensions);
                insert_nested(&mut root, &components, value);
            }
            CodegenStyle::Flat => {
                let key = normalize_path_string(path, config.strip_extensions);
                insert_flat(&mut root, &key, value);
            }
        }
    }

    root
}

fn normalize_path_components(path: &Path, strip_extensions: bool) -> Vec<String> {
    let mut components: Vec<String> = Vec::new();
    let total_components = path.iter().count();

    for (i, comp) in path.iter().enumerate() {
        if i == total_components - 1 && strip_extensions {
            let as_path = Path::new(comp);
            if let Some(stem) = as_path.file_stem() {
                components.push(stem.to_string_lossy().to_string());
                continue;
            }
        }
        components.push(comp.to_string_lossy().to_string());
    }
    components
}

fn normalize_path_string(path: &Path, strip_extensions: bool) -> String {
    if strip_extensions {
        if let (Some(file_name), Some(parent)) = (path.file_name(), path.parent()) {
            if let Some(stem) = Path::new(file_name).file_stem() {
                let parent_str = parent.to_string_lossy();
                return if parent_str.is_empty() || parent_str == "." {
                    stem.to_string_lossy().into_owned()
                } else {
                    format!("{}/{}", parent_str, stem.to_string_lossy())
                };
            }
        }
    }
    path.to_string_lossy().into_owned()
}

fn insert_flat(node: &mut CodegenNode, key: &str, content: &str) {
    match node {
        CodegenNode::Table(map) => {
            map.insert(key.into(), CodegenNode::String(content.into()));
        }
        _ => {
            *node = CodegenNode::Table(BTreeMap::new());
            if let CodegenNode::Table(map) = node {
                map.insert(key.into(), CodegenNode::String(content.into()));
            }
        }
    }
}

fn insert_nested(node: &mut CodegenNode, components: &[String], content: &str) {
    if !matches!(node, CodegenNode::Table(_)) {
        *node = CodegenNode::Table(BTreeMap::new());
    }

    if components.is_empty() {
        return;
    }

    if let CodegenNode::Table(map) = node {
        let component = &components[0];

        if components.len() == 1 {
            map.insert(component.clone(), CodegenNode::String(content.into()));
        } else {
            let next_node = map
                .entry(component.clone())
                .or_insert_with(|| CodegenNode::Table(BTreeMap::new()));

            if !matches!(next_node, CodegenNode::Table(_)) {
                *next_node = CodegenNode::Table(BTreeMap::new());
            }

            insert_nested(next_node, &components[1..], content);
        }
    }
}

pub fn generate_code(
    lang: CodegenLanguage,
    name: &str,
    node: &CodegenNode,
) -> anyhow::Result<String> {
    if !matches!(node, CodegenNode::Table(_)) {
        bail!("Root node must be a Table");
    }

    Ok(match lang {
        CodegenLanguage::TypeScript => generate_typescript(name, node),
        CodegenLanguage::Luau => generate_luau(name, node),
    })
}

fn generate_typescript(name: &str, node: &CodegenNode) -> String {
    let body = generate_ts_node(node, 0);
    format!("declare const {}: {}\n\nexport = {}", name, body, name)
}

fn generate_ts_node(node: &CodegenNode, indent: usize) -> String {
    match node {
        CodegenNode::Table(map) => {
            let mut result = String::from("{\n");
            for (k, v) in map {
                result.push_str(&"\t".repeat(indent + 1));
                let k = if is_valid_identifier(k) {
                    k.clone()
                } else {
                    format!("\"{}\"", k)
                };
                result.push_str(&k);
                result.push_str(": ");
                result.push_str(&generate_ts_node(v, indent + 1));
                result.push('\n');
            }
            result.push_str(&"\t".repeat(indent));
            result.push('}');
            result
        }
        CodegenNode::String(_) => "string".to_string(),
        CodegenNode::Number(_) => "number".to_string(),
    }
}

fn generate_luau(name: &str, node: &CodegenNode) -> String {
    let body = generate_luau_node(node, 0);
    format!("local {} = {}\n\nreturn {}", name, body, name)
}

fn generate_luau_node(node: &CodegenNode, indent: usize) -> String {
    match node {
        CodegenNode::Table(map) => {
            let mut result = String::from("{\n");
            for (k, v) in map {
                result.push_str(&"\t".repeat(indent + 1));
                let k = if is_valid_identifier(k) {
                    k.clone()
                } else {
                    format!("[\"{}\"]", k)
                };
                result.push_str(&k);
                result.push_str(" = ");
                result.push_str(&generate_luau_node(v, indent + 1));
                result.push_str(",\n");
            }
            result.push_str(&"\t".repeat(indent));
            result.push('}');
            result
        }
        CodegenNode::String(s) => format!("\"{}\"", s),
        CodegenNode::Number(n) => format!("{}", n),
    }
}

fn is_valid_ident_char_start(value: char) -> bool {
    value.is_ascii_alphabetic() || value == '_'
}

fn is_valid_ident_char(value: char) -> bool {
    value.is_ascii_alphanumeric() || value == '_'
}

fn is_valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();

    match chars.next() {
        Some(first) => {
            if !is_valid_ident_char_start(first) {
                return false;
            }
        }
        None => return false,
    }

    chars.all(is_valid_ident_char)
}
