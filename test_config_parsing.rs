use schemars::schema_for;
use serde_json;
use std::fs;

mod config;
mod glob;

use config::Config;

fn main() {
    // Test schema generation
    let schema = schema_for!(Config);
    let schema_json = serde_json::to_string_pretty(&schema).unwrap();

    println!("Generated JSON Schema:");
    println!("{}", schema_json);

    // Test parsing JSON config with $schema property
    let json_config = fs::read_to_string("test-config.json").unwrap();
    let config: Result<Config, _> = serde_json::from_str(&json_config);

    match config {
        Ok(config) => {
            println!("\n✅ Successfully parsed JSON config with $schema property!");
            println!("Creator ID: {}", config.creator.id);
            println!("Creator Type: {:?}", config.creator.ty);
            println!("Codegen Style: {:?}", config.codegen.style);
            println!("TypeScript: {}", config.codegen.typescript);
        },
        Err(e) => {
            println!("\n❌ Failed to parse JSON config: {}", e);
        }
    }
}