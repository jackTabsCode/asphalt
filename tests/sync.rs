use assert_fs::{fixture::ChildPath, prelude::*};
use common::Project;
use predicates::{
    Predicate,
    prelude::{PredicateBooleanExt, predicate},
    str::contains,
};
use std::{fs, path::Path, time::Duration};
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

#[test]
fn dry_run_brace_glob_matches_assets() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "assets/{images,sounds}/**"
        output_path = "output"
    });

    project.add_file_at("assets/images/test1.png", "test1.png");
    project.add_file_at("assets/sounds/test2.jpg", "test2.jpg");

    project
        .run()
        .args(["sync", "cloud", "--dry-run"])
        .assert()
        .failure()
        .stderr(contains("2 new assets"));
}

#[test]
fn brace_glob_sync_does_not_wipe_lockfile() {
    let project = Project::new();
    project.write_config(toml! {
        [creator]
        type = "user"
        id = 12345

        [inputs.assets]
        path = "assets/{images,sounds}/**"
        output_path = "output"
        bleed = false
    });

    let image = project.add_file_at("assets/images/test1.png", "test1.png");
    let sound = project.add_file_at("assets/sounds/test2.jpg", "test2.jpg");

    let expected = {
        let mut table = toml::Table::new();
        table.insert("version".into(), 2.into());

        table.insert("inputs".into(), {
            let mut inputs = toml::Table::new();
            inputs.insert("assets".into(), {
                let mut assets = toml::Table::new();
                assets.insert(hash(&image), {
                    let mut entry = toml::Table::new();
                    entry.insert("asset_id".into(), toml::Value::Integer(1));
                    entry.into()
                });
                assets.insert(hash(&sound), {
                    let mut entry = toml::Table::new();
                    entry.insert("asset_id".into(), toml::Value::Integer(2));
                    entry.into()
                });
                assets.into()
            });
            inputs.into()
        });

        table
    };

    project.write_lockfile(expected.clone());

    project.run().args(["sync"]).assert().success();

    project
        .dir
        .child("asphalt.lock.toml")
        .assert(toml_eq(expected.into()));
}

#[test]
fn studio_sync_emits_single_rbxasset_prefix() {
    // Regression test: AssetRef::Display already prepends "rbxasset://" for
    // Studio refs. The studio backend used to embed the prefix inside the
    // inner string, producing doubled URLs like "rbxasset://rbxasset://...".
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
    project.add_file("test1.png");

    project.run().args(["sync", "studio"]).assert().success();

    let output_file = project.dir.child("output/assets.luau");
    output_file
        .assert(contains("rbxasset://.asphalt-test/"))
        .assert(predicate::str::contains("rbxasset://rbxasset://").not())
        .assert(predicate::str::contains("rbxasset://rbxassetid://").not());
}

#[test]
fn studio_watch_detects_new_file() {
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
    project.add_file("test1.png");

    // Spawn watch process (runs forever until killed)
    let bin = assert_cmd::cargo::cargo_bin!("asphalt");
    let mut child = std::process::Command::new(bin)
        .env("ASPHALT_TEST", "true")
        .env("ASPHALT_API_KEY", "test")
        .current_dir(project.dir.path())
        .args(["sync", "studio", "--watch"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    // Wait for initial sync to complete
    std::thread::sleep(Duration::from_secs(2));

    // Verify initial codegen output contains test1
    let output_file = project.dir.child("output/assets.luau");
    output_file.assert(contains("test1"));

    // Add a second file while watching
    project.add_file("test2.jpg");

    // Wait for watch loop to pick it up (polls every 500ms)
    std::thread::sleep(Duration::from_secs(2));

    // Verify codegen was updated with the new file
    output_file.assert(contains("test2"));

    child.kill().unwrap();
}
