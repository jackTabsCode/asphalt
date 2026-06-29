use fs_err::tokio as fs;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt::Display, path::Path, str::FromStr};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Hash(blake3::Hash);

impl Hash {
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        let hash = blake3::hash(bytes);
        Hash(hash)
    }

    pub fn as_u64(&self) -> u64 {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&self.0.as_bytes()[..8]);
        bytes[0] &= 0x7f;
        u64::from_be_bytes(bytes)
    }

    pub fn from_hex(value: &str) -> Result<Self, blake3::HexError> {
        Ok(Self(blake3::Hash::from_str(value)?))
    }

    pub async fn new_from_file(path: &Path) -> Result<Self, std::io::Error> {
        let bytes = fs::read(path).await?;
        Ok(Self::new_from_bytes(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_new_from_bytes_deterministic() {
        let hash1 = Hash::new_from_bytes(b"hello");
        let hash2 = Hash::new_from_bytes(b"hello");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_new_from_bytes_different_inputs() {
        let hash1 = Hash::new_from_bytes(b"hello");
        let hash2 = Hash::new_from_bytes(b"world");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_as_u64_stable() {
        let hash = Hash::new_from_bytes(b"hello");
        let first = hash.as_u64();
        let second = hash.as_u64();
        assert_eq!(first, second);
    }

    #[test]
    fn test_as_u64_msb_cleared() {
        // Verify the most significant bit is cleared (positive i64)
        let hash = Hash::new_from_bytes(b"\xff\xff\xff\xff\xff\xff\xff\xff");
        let val = hash.as_u64();
        // MSB of the first byte (byte 0 in big-endian) should be 0
        assert!(val < (1u64 << 63), "MSB should be cleared");
    }

    #[test]
    fn test_as_u64_unique_for_different_inputs() {
        let hash_a = Hash::new_from_bytes(b"aaaa");
        let hash_b = Hash::new_from_bytes(b"bbbb");
        assert_ne!(hash_a.as_u64(), hash_b.as_u64());
    }

    #[test]
    fn test_from_hex_round_trip() {
        let original = Hash::new_from_bytes(b"round-trip");
        let hex = original.to_string();
        let parsed = Hash::from_hex(&hex).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_from_hex_invalid() {
        assert!(Hash::from_hex("not-a-valid-hex-string!!!!").is_err());
    }

    #[test]
    fn test_display_is_hex() {
        let hash = Hash::new_from_bytes(b"display");
        let display = hash.to_string();
        // blake3 hex should be 64 lowercase hex chars
        assert_eq!(display.len(), 64);
        assert!(display.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_ord_consistent() {
        let hash_a = Hash::new_from_bytes(b"a");
        let hash_b = Hash::new_from_bytes(b"b");
        // Ord should be total and consistent
        assert!(hash_a < hash_b || hash_a > hash_b || hash_a == hash_b);
        assert_eq!(hash_a.cmp(&hash_b), hash_a.partial_cmp(&hash_b).unwrap());
    }

    #[test]
    fn test_ord_equal_for_same() {
        let hash = Hash::new_from_bytes(b"same");
        assert_eq!(hash.cmp(&hash), Ordering::Equal);
        assert!(hash <= hash);
        assert!(hash >= hash);
    }

    #[test]
    fn test_serialization_round_trip_json() {
        let hash = Hash::new_from_bytes(b"serde-test");
        let json = serde_json::to_string(&hash).unwrap();
        let deserialized: Hash = serde_json::from_str(&json).unwrap();
        assert_eq!(hash, deserialized);
    }

    #[test]
    fn test_serialization_round_trip_toml_value() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Wrapper {
            hash: Hash,
        }
        let hash = Hash::new_from_bytes(b"toml-test");
        let wrapper = Wrapper { hash };
        let toml_str = toml::to_string(&wrapper).unwrap();
        let deserialized: Wrapper = toml::from_str(&toml_str).unwrap();
        assert_eq!(hash, deserialized.hash);
    }

    #[test]
    fn test_hash_equality() {
        let hash1 = Hash::new_from_bytes(b"equality");
        let hash2 = Hash::new_from_bytes(b"equality");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_as_u64_length() {
        // as_u64 should always return exactly 8 bytes worth
        let hash = Hash::new_from_bytes(b"any-data");
        let val = hash.as_u64();
        // Should fit in u64
        let _bytes = val.to_be_bytes();
        assert_eq!(_bytes.len(), 8);
    }
}

impl Ord for Hash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl PartialOrd for Hash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let hash = blake3::Hash::from_str(&str).map_err(serde::de::Error::custom)?;
        Ok(Hash(hash))
    }
}
