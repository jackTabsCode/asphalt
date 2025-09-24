//! Wrapper around globset's Glob type that has better serialization
//! characteristics by coupling Glob and GlobMatcher into a single type.
//! https://github.com/Roblox/tarmac/blob/master/src/glob.rs

use std::{
    fmt,
    path::{Path, PathBuf},
};

use globset::{Glob as InnerGlob, GlobMatcher};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

pub use globset::Error;

#[derive(Debug, Clone)]
pub struct Glob {
    inner: InnerGlob,
    matcher: GlobMatcher,
}

impl Glob {
    pub fn new(glob: &str) -> Result<Self, Error> {
        let inner = InnerGlob::new(glob)?;
        let matcher = inner.compile_matcher();

        Ok(Glob { inner, matcher })
    }

    pub fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        self.matcher.is_match(path)
    }

    pub fn get_prefix(&self) -> PathBuf {
        get_non_pattern_prefix(Path::new(self.inner.glob()))
    }
}

impl PartialEq for Glob {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Glob {}

impl Serialize for Glob {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.inner.glob())
    }
}

impl<'de> Deserialize<'de> for Glob {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let glob = <String as Deserialize>::deserialize(deserializer)?;

        Glob::new(glob.as_str()).map_err(D::Error::custom)
    }
}

impl fmt::Display for Glob {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

// A basic set of characters that might indicate the use of glob pattern syntax.
// This is to distinguish portions of a glob that are fixed paths (e.g.
// "foo.png") from ones that are leveraging patterns (e.g. "*.png").
//
// This approach has false positives, as it will treat escape sequences like
// `[*]` as pattern syntax, but those should be rare enough to be acceptable
//
// Glob syntax described here: https://docs.rs/globset/0.4.4/globset/#syntax
const GLOB_PATTERN_CHARACTERS: &str = "*?{}[]";

fn get_non_pattern_prefix(glob_path: &Path) -> PathBuf {
    let mut prefix = PathBuf::new();

    for component in glob_path.iter() {
        let component_str = component.to_str().unwrap();

        if GLOB_PATTERN_CHARACTERS
            .chars()
            .any(|special_char| component_str.contains(special_char))
        {
            break;
        }

        prefix.push(component);
    }

    prefix
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simple_prefix() {
        assert_eq!(
            get_non_pattern_prefix(Path::new("a/b/**/*.png")),
            PathBuf::from("a/b")
        );
    }

    #[test]
    fn prefix_only() {
        assert_eq!(
            get_non_pattern_prefix(Path::new("a/**/b/*.png")),
            PathBuf::from("a")
        );
    }

    #[test]
    fn no_prefix() {
        assert_eq!(
            get_non_pattern_prefix(Path::new("**/b/*.png")),
            PathBuf::from("")
        );
    }

    #[test]
    fn whole_path() {
        assert_eq!(
            get_non_pattern_prefix(Path::new("a/b/foo.png")),
            PathBuf::from("a/b/foo.png")
        )
    }
}
