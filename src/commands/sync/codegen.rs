use super::config::CodegenStyle;
use std::collections::BTreeMap;
use std::path::Path;

pub struct GeneratorOptions {
    pub output_name: String,
    pub style: CodegenStyle,
    pub strip_extension: bool,
}

pub struct Generator {
    options: GeneratorOptions,
    data: BTreeMap<String, String>,
}

enum DataValue {
    Leaf(String),
    Node(BTreeMap<String, DataValue>),
}

impl DataValue {
    fn as_node_mut(&mut self) -> Option<&mut BTreeMap<String, DataValue>> {
        match self {
            DataValue::Node(ref mut map) => Some(map),
            _ => None,
        }
    }
}

impl Generator {
    pub fn new(data: BTreeMap<String, String>, options: GeneratorOptions) -> Self {
        Generator { data, options }
    }

    pub fn generate_typescript(&self) -> String {
        let data_value = self.build_data_value();
        let mut output = String::new();

        output.push_str(&format!("declare const {}: ", self.options.output_name));
        self.serialize_value_typescript(&data_value, &mut output, 0);
        output.push_str(";\nexport = assets;\n");

        output
    }

    pub fn generate_luau(&self) -> String {
        let data_value = self.build_data_value();
        let mut output = String::new();

        output.push_str("return ");
        self.serialize_value_luau(&data_value, &mut output, 0);

        output
    }

    fn build_data_value(&self) -> DataValue {
        match self.options.style {
            CodegenStyle::Flat => {
                let mut node = BTreeMap::new();

                for (path, value) in &self.data {
                    let key = if self.options.strip_extension {
                        strip_extension(path)
                    } else {
                        path.clone()
                    };

                    node.insert(key, DataValue::Leaf(value.clone()));
                }

                DataValue::Node(node)
            }
            CodegenStyle::Nested => {
                let mut root = BTreeMap::new();

                for (path, value) in &self.data {
                    let key_path = if self.options.strip_extension {
                        strip_extension(path)
                    } else {
                        path.clone()
                    };

                    let parts: Vec<&str> = key_path.split('/').collect();
                    let mut current = &mut root;

                    for (i, part) in parts.iter().enumerate() {
                        if i == parts.len() - 1 {
                            current.insert((*part).to_string(), DataValue::Leaf(value.clone()));
                        } else {
                            current = current
                                .entry((*part).to_string())
                                .or_insert_with(|| DataValue::Node(BTreeMap::new()))
                                .as_node_mut()
                                .unwrap();
                        }
                    }
                }

                DataValue::Node(root)
            }
        }
    }

    #[allow(clippy::only_used_in_recursion)]
    fn serialize_value_typescript(&self, value: &DataValue, output: &mut String, indent: usize) {
        match value {
            DataValue::Leaf(s) => {
                output.push_str(&format!("{:?}", s));
            }
            DataValue::Node(map) => {
                output.push_str("{\n");

                let indent_str = "\t".repeat(indent + 1);

                for (i, (key, val)) in map.iter().enumerate() {
                    output.push_str(&indent_str);

                    if is_valid_ts_identifier(key) {
                        output.push_str(key);
                        output.push_str(": ");
                    } else {
                        output.push_str(&format!("{:?}: ", key));
                    }

                    self.serialize_value_typescript(val, output, indent + 1);
                    if i != map.len() - 1 {
                        output.push(',');
                    }

                    output.push('\n');
                }

                output.push_str(&"\t".repeat(indent));
                output.push('}');
            }
        }
    }

    #[allow(clippy::only_used_in_recursion)]
    fn serialize_value_luau(&self, value: &DataValue, output: &mut String, indent: usize) {
        match value {
            DataValue::Leaf(s) => {
                output.push_str(&format!("{:?}", s));
            }
            DataValue::Node(map) => {
                output.push_str("{\n");

                let indent_str = "\t".repeat(indent + 1);

                for (i, (key, val)) in map.iter().enumerate() {
                    output.push_str(&indent_str);

                    if is_valid_luau_identifier(key) {
                        output.push_str(key);
                        output.push_str(" = ");
                    } else {
                        output.push_str(&format!("[{:?}] = ", key));
                    }

                    self.serialize_value_luau(val, output, indent + 1);

                    if i != map.len() - 1 {
                        output.push(',');
                    }

                    output.push('\n');
                }

                output.push_str(&"\t".repeat(indent));
                output.push('}');
            }
        }
    }
}

fn is_valid_ts_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();

    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => (),
        _ => return false,
    }

    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '$' {
            return false;
        }
    }

    true
}

fn is_valid_luau_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();

    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => (),
        _ => return false,
    }

    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return false;
        }
    }

    true
}

fn strip_extension(path: &str) -> String {
    let path = Path::new(path);
    let mut new_path = String::new();

    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let components: Vec<_> = path.parent().iter().flat_map(|p| p.components()).collect();

    for component in components {
        if let std::path::Component::Normal(os_str) = component {
            if let Some(s) = os_str.to_str() {
                new_path.push_str(s);
                new_path.push('/');
            }
        }
    }

    new_path.push_str(stem);
    new_path
}
