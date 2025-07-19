use crate::util::{alpha_bleed::alpha_bleed, animation::get_animation, svg::svg_to_png};
use anyhow::{Context, bail};
use blake3::Hasher;
use image::DynamicImage;
use resvg::usvg::fontdb::Database;
use serde::Serialize;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct Asset {
    pub path: PathBuf,
    pub data: Vec<u8>,
    pub ty: AssetType,
    processed: bool,
    ext: String,
    /// The hash before processing
    pub hash: String,
}

impl Asset {
    pub fn new(path: PathBuf, data: Vec<u8>) -> anyhow::Result<Self> {
        let ext = path
            .extension()
            .context("File has no extension")?
            .to_str()
            .context("Extension is not valid UTF-8")?
            .to_string();

        let ty = match ext.as_str() {
            "mp3" => AssetType::Audio(AudioType::Mp3),
            "ogg" => AssetType::Audio(AudioType::Ogg),
            "flac" => AssetType::Audio(AudioType::Flac),
            "wav" => AssetType::Audio(AudioType::Wav),
            "png" | "svg" => AssetType::Image(ImageType::Png),
            "jpg" | "jpeg" => AssetType::Image(ImageType::Jpg),
            "bmp" => AssetType::Image(ImageType::Bmp),
            "tga" => AssetType::Image(ImageType::Tga),
            "fbx" => AssetType::Model(ModelType::Model),
            "rbxm" | "rbxmx" => {
                let format = if ext == "rbxm" {
                    ModelFileFormat::Binary
                } else {
                    ModelFileFormat::Xml
                };

                AssetType::Model(ModelType::Animation(format))
            }
            "mp4" => AssetType::Video(VideoType::Mp4),
            "mov" => AssetType::Video(VideoType::Mov),
            _ => bail!("Unknown extension .{ext}"),
        };

        let mut hasher = Hasher::new();
        hasher.update(&data);
        let hash = hasher.finalize().to_string();

        Ok(Self {
            path,
            data,
            ty,
            processed: false,
            ext,
            hash,
        })
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
            self.ext = "png".to_string();
        }

        match self.ty {
            AssetType::Model(ModelType::Animation(ref format)) => {
                self.data = get_animation(&self.data, format)?;
            }
            AssetType::Image(_) if bleed => {
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
pub enum AssetType {
    Image(ImageType),
    Audio(AudioType),
    Model(ModelType),
    Video(VideoType),
}

impl AssetType {
    // https://create.roblox.com/docs/cloud/guides/usage-assets#supported-asset-types-and-limits

    pub fn asset_type(&self) -> &'static str {
        match self {
            AssetType::Model(ModelType::Model) => "Model",
            AssetType::Model(ModelType::Animation(_)) => "Animation",
            AssetType::Image(_) => "Image",
            AssetType::Audio(_) => "Audio",
            AssetType::Video(_) => "Video",
        }
    }

    pub fn file_type(&self) -> &'static str {
        match self {
            AssetType::Model(ModelType::Model) => "model/fbx",
            AssetType::Model(ModelType::Animation(ModelFileFormat::Binary)) => "model/x-rbxm",
            AssetType::Model(ModelType::Animation(ModelFileFormat::Xml)) => "model/x-rbxmx",
            AssetType::Image(ImageType::Png) => "image/png",
            AssetType::Image(ImageType::Jpg) => "image/jpeg",
            AssetType::Image(ImageType::Bmp) => "image/bmp",
            AssetType::Image(ImageType::Tga) => "image/tga",
            AssetType::Audio(AudioType::Mp3) => "audio/mpeg",
            AssetType::Audio(AudioType::Ogg) => "audio/ogg",
            AssetType::Audio(AudioType::Flac) => "audio/flac",
            AssetType::Audio(AudioType::Wav) => "audio/wav",
            AssetType::Video(VideoType::Mp4) => "video/mp4",
            AssetType::Video(VideoType::Mov) => "video/mov",
        }
    }
}

impl Serialize for AssetType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.asset_type())
    }
}

#[derive(Debug, Clone)]
pub enum AudioType {
    Mp3,
    Ogg,
    Flac,
    Wav,
}

#[derive(Debug, Clone)]
pub enum ImageType {
    Png,
    Jpg,
    Bmp,
    Tga,
}

#[derive(Debug, Clone)]
pub enum ModelType {
    Model,
    Animation(ModelFileFormat), // not uploadable with Open Cloud!
}

#[derive(Debug, Clone)]
pub enum ModelFileFormat {
    Binary,
    Xml,
}

#[derive(Debug, Clone)]
pub enum VideoType {
    Mp4,
    Mov,
}
