use assert_fs::{fixture::ChildPath, prelude::*};
use common::Project;
use predicates::{Predicate, prelude::predicate, str::contains};
use std::{fs, path::Path};
use toml::toml;

mod common;

fn hash(path: &ChildPath) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&fs::read(path).unwrap());
    hasher.finalize().to_string()
}

fn toml_eq(expected: toml::Value) -> impl Predicate<Path> {
    predicate::function(move |path: &Path| {
        let contents = fs::read_to_string(path).unwrap();
        let actual: toml::Value = toml::from_str(&contents).unwrap();
        actual == expected
    })
}

#[test]
fn missing_config_fails() {
    Project::new()
        .run()
        .args(["sync", "--target", "debug"])
        .assert()
        .failure();
}

#[test]
fn debug_creates_output() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 1234

        [inputs.assets]
        path = "input/**/*"
        output_path = "output"
        bleed = false
    });
    let test_file = project.add_file("test1.png");

    project.run().args(["sync", "debug"]).assert().success();

    project
        .dir
        .child(".asphalt-debug/test1.png")
        .assert(predicate::path::eq_file(test_file.path()));
}

#[test]
fn debug_web_assets() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "input/**/*"
        output_path = "output"

        [inputs.assets.web]
        "existing.png" = { id = 1234 }
    });

    project.run().args(["sync", "debug"]).assert().success();

    project
        .dir
        .child("output/assets.luau")
        .assert(contains("existing.png"))
        .assert(contains("1234"));
}

#[test]
fn cloud_output_and_lockfile() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "input/**/*"
        output_path = "output"
    });
    let test_file = project.add_file("test1.png");

    project
        .run()
        .args(["sync", "--api-key", "test"])
        .assert()
        .success();

    project.dir.child("asphalt.lock.toml").assert(toml_eq({
        let mut table = toml::Table::new();
        table.insert("version".into(), 2.into());

        table.insert("inputs".into(), {
            let mut inputs = toml::Table::new();
            inputs.insert("assets".into(), {
                let mut assets = toml::Table::new();
                assets.insert(hash(&test_file), {
                    let mut entry = toml::Table::new();
                    entry.insert("asset_id".into(), 1337.into());
                    entry.into()
                });
                assets.into()
            });
            inputs.into()
        });

        table.into()
    }));
}

#[test]
fn dry_run_none() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "input/**/*"
        output_path = "output"
    });

    project
        .run()
        .args(["sync", "cloud", "--dry-run"])
        .assert()
        .success()
        .stderr(contains("No new assets"));
}

#[test]
fn dry_run_1_new() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "input/**/*"
        output_path = "output"
    });
    project.add_file("test1.png");

    project
        .run()
        .args(["sync", "cloud", "--dry-run"])
        .assert()
        .failure()
        .stderr(contains("1 new assets"));
}

#[test]
fn dry_run_1_new_1_old() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "input/**/*"
        output_path = "output"
    });
    let old_file = project.add_file("test1.png");
    project.add_file("test2.jpg");

    project.write_lockfile({
        let mut table = toml::Table::new();
        table.insert("version".into(), 2.into());

        table.insert("inputs".into(), {
            let mut inputs = toml::Table::new();
            inputs.insert("assets".into(), {
                let mut assets = toml::Table::new();
                assets.insert(hash(&old_file), {
                    let mut entry = toml::Table::new();
                    entry.insert("asset_id".into(), toml::Value::Integer(1));
                    entry.into()
                });
                assets.into()
            });
            inputs.into()
        });

        table
    });

    project
        .run()
        .args(["sync", "cloud", "--dry-run"])
        .assert()
        .failure()
        .stderr(contains("1 new assets"));
}

#[test]
fn dry_run_2_old() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "input/**/*"
        output_path = "output"
    });
    let old_file_1 = project.add_file("test1.png");
    let old_file_2 = project.add_file("test2.jpg");

    project.write_lockfile({
        let mut table = toml::Table::new();
        table.insert("version".into(), 2.into());

        table.insert("inputs".into(), {
            let mut inputs = toml::Table::new();
            inputs.insert("assets".into(), {
                let mut assets = toml::Table::new();
                assets.insert(hash(&old_file_1), {
                    let mut entry = toml::Table::new();
                    entry.insert("asset_id".into(), toml::Value::Integer(1));
                    entry.into()
                });
                assets.insert(hash(&old_file_2), {
                    let mut entry = toml::Table::new();
                    entry.insert("asset_id".into(), toml::Value::Integer(1));
                    entry.into()
                });
                assets.into()
            });
            inputs.into()
        });

        table
    });

    project
        .run()
        .args(["sync", "cloud", "--dry-run"])
        .assert()
        .success()
        .stderr(contains("No new assets"));
}
