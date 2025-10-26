use crate::util::{alpha_bleed::alpha_bleed, svg::svg_to_png};
use anyhow::{Context, bail};
use blake3::Hasher;
use bytes::Bytes;
use image::DynamicImage;
use relative_path::RelativePathBuf;
use resvg::usvg::fontdb::Database;
use serde::Serialize;
use std::{io::Cursor, sync::Arc};

pub struct Asset {
    /// Relative to Input prefix
    pub path: RelativePathBuf,
    pub data: Bytes,
    pub ty: AssetType,
    processed: bool,
    pub ext: String,
    /// The hash before processing
    pub hash: String,
}

impl Asset {
    pub fn new(path: RelativePathBuf, data: Vec<u8>) -> anyhow::Result<Self> {
        let ext = path
            .extension()
            .context("File has no extension")?
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
            "fbx" => AssetType::Model(ModelType::Fbx),
            "gltf" => AssetType::Model(ModelType::GltfJson),
            "glb" => AssetType::Model(ModelType::GltfBinary),
            "rbxm" | "rbxmx" => {
                let format = if ext == "rbxm" {
                    RobloxModelFormat::Binary
                } else {
                    RobloxModelFormat::Xml
                };

                if is_animation(&data, &format)? {
                    AssetType::Animation
                } else {
                    AssetType::Model(ModelType::Roblox)
                }
            }
            "mp4" => AssetType::Video(VideoType::Mp4),
            "mov" => AssetType::Video(VideoType::Mov),
            _ => bail!("Unknown extension .{ext}"),
        };

        let data = Bytes::from(data);

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

    pub async fn process(&mut self, font_db: Arc<Database>, bleed: bool) -> anyhow::Result<()> {
        if self.processed {
            bail!("Asset has already been processed");
        }

        if self.ext == "svg" {
            self.data = svg_to_png(&self.data, font_db.clone()).await?.into();
            self.ext = "png".to_string();
        }

        if matches!(self.ty, AssetType::Image(ImageType::Png)) && bleed {
            let mut image: DynamicImage = image::load_from_memory(&self.data)?;
            alpha_bleed(&mut image);

            let mut writer = Cursor::new(Vec::new());
            image.write_to(&mut writer, image::ImageFormat::Png)?;
            self.data = Bytes::from(writer.into_inner());
        }

        self.processed = true;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum AssetType {
    Model(ModelType),
    Animation,
    Image(ImageType),
    Audio(AudioType),
    Video(VideoType),
}

impl AssetType {
    // https://create.roblox.com/docs/cloud/guides/usage-assets#supported-asset-types-and-limits

    pub fn asset_type(&self) -> &'static str {
        match self {
            AssetType::Model(_) => "Model",
            AssetType::Animation => "Animation",
            AssetType::Image(_) => "Image",
            AssetType::Audio(_) => "Audio",
            AssetType::Video(_) => "Video",
        }
    }

    pub fn file_type(&self) -> &'static str {
        match self {
            AssetType::Animation => "model/x-rbxm",

            AssetType::Model(ModelType::Fbx) => "model/fbx",
            AssetType::Model(ModelType::GltfJson) => "model/gltf+json",
            AssetType::Model(ModelType::GltfBinary) => "model/gltf-binary",
            AssetType::Model(ModelType::Roblox) => "model/x-rbxm",

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
    Fbx,
    GltfJson,
    GltfBinary,
    Roblox,
}

#[derive(Debug, Clone)]
pub enum VideoType {
    Mp4,
    Mov,
}

pub fn is_animation(data: &[u8], format: &RobloxModelFormat) -> anyhow::Result<bool> {
    let dom = match format {
        RobloxModelFormat::Binary => rbx_binary::from_reader(data)?,
        RobloxModelFormat::Xml => rbx_xml::from_reader(data, Default::default())?,
    };

    let children = dom.root().children();

    let first_ref = *children.first().context("No children found in root")?;
    let first = dom
        .get_by_ref(first_ref)
        .context("Failed to get first child")?;

    Ok(first.class == "KeyframeSequence" || first.class == "CurveAnimation")
}

#[derive(Debug, Clone)]
pub enum RobloxModelFormat {
    Binary,
    Xml,
}
