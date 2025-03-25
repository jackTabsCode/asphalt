use anyhow::bail;
use rbxcloud::rbx::v1::assets::AssetType;
use std::path::{Path, PathBuf};

pub struct Asset {
    pub path: PathBuf,
    pub data: Vec<u8>,
    pub kind: AssetKind,
    pub changed: bool,
}

impl Asset {
    pub fn rel_path(&self, input_path: &Path, ext: &str) -> anyhow::Result<PathBuf> {
        let stripped_path_str = self.path.strip_prefix(input_path)?;

        Ok(PathBuf::from(stripped_path_str).with_extension(ext))
    }
}

#[derive(Debug, Clone)]
pub enum AudioKind {
    Mp3,
    Ogg,
    Flac,
    Wav,
}

#[derive(Debug, Clone)]
pub enum DecalKind {
    Png,
    Jpg,
    Bmp,
    Tga,
}

#[derive(Debug, Clone)]
pub enum ModelKind {
    Model,
    Animation(ModelFileFormat), // not uploadable with Open Cloud!
}

#[derive(Debug, Clone)]
pub enum AssetKind {
    Decal(DecalKind),
    Audio(AudioKind),
    Model(ModelKind),
}

impl TryFrom<AssetKind> for AssetType {
    type Error = anyhow::Error;

    fn try_from(value: AssetKind) -> anyhow::Result<Self> {
        match value {
            AssetKind::Audio(AudioKind::Flac) => Ok(AssetType::AudioFlac),
            AssetKind::Audio(AudioKind::Mp3) => Ok(AssetType::AudioMp3),
            AssetKind::Audio(AudioKind::Ogg) => Ok(AssetType::AudioOgg),
            AssetKind::Audio(AudioKind::Wav) => Ok(AssetType::AudioWav),
            AssetKind::Decal(DecalKind::Bmp) => Ok(AssetType::DecalBmp),
            AssetKind::Decal(DecalKind::Jpg) => Ok(AssetType::DecalJpeg),
            AssetKind::Decal(DecalKind::Png) => Ok(AssetType::DecalPng),
            AssetKind::Decal(DecalKind::Tga) => Ok(AssetType::DecalTga),
            AssetKind::Model(ModelKind::Animation(_)) => {
                bail!("Animations cannot be uploaded with Open Cloud")
            }
            AssetKind::Model(ModelKind::Model) => Ok(AssetType::ModelFbx),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModelFileFormat {
    Binary,
    Xml,
}
