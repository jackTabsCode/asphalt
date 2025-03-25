use crate::util::{alpha_bleed::alpha_bleed, animation::get_animation, svg::svg_to_png};
use anyhow::{bail, Context};
use blake3::Hasher;
use image::DynamicImage;
use rbxcloud::rbx::v1::assets::AssetType;
use resvg::usvg::fontdb::Database;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct Asset {
    pub path: PathBuf,
    pub data: Vec<u8>,
    pub kind: AssetKind,
    processed: bool,
    ext: String,
}

impl Asset {
    pub fn new(path: PathBuf, data: Vec<u8>) -> anyhow::Result<Self> {
        let ext = path
            .extension()
            .context("File has no extension")?
            .to_str()
            .context("Extension is not valid UTF-8")?
            .to_string();

        let kind = match ext.as_str() {
            "mp3" => AssetKind::Audio(AudioKind::Mp3),
            "ogg" => AssetKind::Audio(AudioKind::Ogg),
            "flac" => AssetKind::Audio(AudioKind::Flac),
            "wav" => AssetKind::Audio(AudioKind::Wav),
            "png" | "svg" => AssetKind::Decal(DecalKind::Png),
            "jpg" => AssetKind::Decal(DecalKind::Jpg),
            "bmp" => AssetKind::Decal(DecalKind::Bmp),
            "tga" => AssetKind::Decal(DecalKind::Tga),
            "fbx" => AssetKind::Model(ModelKind::Model),
            "rbxm" | "rbxmx" => {
                let format = if ext == "rbxm" {
                    ModelFileFormat::Binary
                } else {
                    ModelFileFormat::Xml
                };

                AssetKind::Model(ModelKind::Animation(format))
            }
            _ => bail!("Unknown extension .{ext}"),
        };

        Ok(Self {
            path,
            data,
            kind,
            processed: false,
            ext,
        })
    }

    pub fn hash(&self) -> String {
        let mut hasher = Hasher::new();
        hasher.update(&self.data);
        hasher.finalize().to_string()
    }

    pub fn rel_path(&self, input_path: &Path) -> anyhow::Result<PathBuf> {
        let stripped = self.path.strip_prefix(input_path)?;

        Ok(PathBuf::from(stripped).with_extension(self.ext.clone()))
    }

    pub async fn process(&mut self, font_db: Arc<Database>, bleed: bool) -> anyhow::Result<()> {
        if self.processed {
            bail!("Asset has already been processed");
        }

        let ext = self.path.extension().context("File has no extension")?;
        if ext == "svg" {
            self.data = svg_to_png(&self.data, font_db.clone()).await?;
        }

        match self.kind {
            AssetKind::Model(ModelKind::Animation(ref format)) => {
                self.data = get_animation(&self.data, format)?;
            }
            AssetKind::Decal(_) if bleed => {
                let mut image: DynamicImage = image::load_from_memory(&self.data)?;
                alpha_bleed(&mut image);

                let mut writer = Cursor::new(Vec::new());
                image.write_to(&mut writer, image::ImageFormat::Png)?;
                self.data = writer.into_inner();
            }
            _ => {}
        };

        self.processed = true;

        Ok(())
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
