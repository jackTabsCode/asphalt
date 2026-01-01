use assert_cmd::cargo::cargo_bin_cmd;
use assert_fs::{TempDir, fixture::ChildPath, prelude::*};
use std::{fs, path::Path};

pub struct Project {
    pub dir: TempDir,
}

impl Project {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
        }
    }

    pub fn write_config(&self, contents: toml::Table) {
        self.dir
            .child("asphalt.toml")
            .write_str(&contents.to_string())
            .unwrap();
    }

    pub fn write_lockfile(&self, contents: toml::Table) {
        self.dir
            .child("asphalt.lock.toml")
            .write_str(&contents.to_string())
            .unwrap();
    }

    fn read_test_asset(&self, file_name: &str) -> Vec<u8> {
        let path = Path::new("tests").join("assets").join(file_name);
        fs::read(&path).unwrap()
    }

    pub fn add_file(&self, file_name: &str) -> ChildPath {
        let file = self.dir.child("input").child(file_name);
        file.write_binary(&self.read_test_asset(file_name)).unwrap();
        file
    }

    pub fn run(&self) -> assert_cmd::Command {
        let mut cmd = cargo_bin_cmd!();
        cmd.env("ASPHALT_TEST", "true");
        cmd.env("ASPHALT_API_KEY", "test");
        cmd.current_dir(self.dir.path());
        cmd
    }
}
