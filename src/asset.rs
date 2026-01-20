use crate::{
    config::WebAsset,
    hash::Hash,
    lockfile::LockfileEntry,
    util::{alpha_bleed::alpha_bleed, svg::svg_to_png},
};
use anyhow::Context;
use bytes::Bytes;
use image::DynamicImage;
use relative_path::RelativePathBuf;
use resvg::usvg::fontdb::{self};
use serde::Serialize;
use std::{ffi::OsStr, fmt, io::Cursor, sync::Arc};

type AssetCtor = fn(&[u8]) -> anyhow::Result<AssetType>;

const SUPPORTED_EXTENSIONS: &[(&str, AssetCtor)] = &[
    ("mp3", |_| Ok(AssetType::Audio(AudioType::Mp3))),
    ("ogg", |_| Ok(AssetType::Audio(AudioType::Ogg))),
    ("flac", |_| Ok(AssetType::Audio(AudioType::Flac))),
    ("wav", |_| Ok(AssetType::Audio(AudioType::Wav))),
    ("png", |_| Ok(AssetType::Image(ImageType::Png))),
    ("svg", |_| Ok(AssetType::Image(ImageType::Png))),
    ("jpg", |_| Ok(AssetType::Image(ImageType::Jpg))),
    ("jpeg", |_| Ok(AssetType::Image(ImageType::Jpg))),
    ("bmp", |_| Ok(AssetType::Image(ImageType::Bmp))),
    ("tga", |_| Ok(AssetType::Image(ImageType::Tga))),
    ("fbx", |_| Ok(AssetType::Model(ModelType::Fbx))),
    ("gltf", |_| Ok(AssetType::Model(ModelType::GltfJson))),
    ("glb", |_| Ok(AssetType::Model(ModelType::GltfBinary))),
    ("rbxm", |data| {
        let format = RobloxModelFormat::Binary;
        if is_animation(data, &format)? {
            Ok(AssetType::Animation)
        } else {
            Ok(AssetType::Model(ModelType::Roblox))
        }
    }),
    ("rbxmx", |data| {
        let format = RobloxModelFormat::Xml;
        if is_animation(data, &format)? {
            Ok(AssetType::Animation)
        } else {
            Ok(AssetType::Model(ModelType::Roblox))
        }
    }),
    ("mp4", |_| Ok(AssetType::Video(VideoType::Mp4))),
    ("mov", |_| Ok(AssetType::Video(VideoType::Mov))),
];

pub fn is_supported_extension(ext: &OsStr) -> bool {
    SUPPORTED_EXTENSIONS.iter().any(|(e, _)| *e == ext)
}

pub struct Asset {
    /// Relative to Input prefix
    pub path: RelativePathBuf,
    pub data: Bytes,
    pub ty: AssetType,
    pub ext: String,
    /// The hash before processing
    pub hash: Hash,
    is_svg: bool,
}

impl Asset {
    pub fn new(path: RelativePathBuf, data: Bytes) -> anyhow::Result<Self> {
        let mut ext = path
            .extension()
            .context("File has no extension")?
            .to_string();

        let ty = SUPPORTED_EXTENSIONS
            .iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, func)| func(&data))
            .context("Unknown file type")??;

        let mut is_svg = false;
        if ext == "svg" {
            ext = "png".to_string();
            is_svg = true;
        }

        let hash = Hash::new_from_bytes(&data);

        Ok(Self {
            path,
            data,
            ty,
            ext,
            hash,
            is_svg,
        })
    }

    pub fn process(&mut self, font_db: Arc<fontdb::Database>, bleed: bool) -> anyhow::Result<()> {
        if self.is_svg {
            self.data = svg_to_png(&self.data, font_db)
                .context("Failed to convert to PNG")?
                .into();
        }

        if bleed && let AssetType::Image(_) = self.ty {
            let mut image: DynamicImage = image::load_from_memory(&self.data)?;
            alpha_bleed(&mut image);

            let mut writer = Cursor::new(Vec::new());
            image.write_to(&mut writer, image::ImageFormat::Png)?;
            self.data = writer.into_inner().into();
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
pub enum AudioType {
    Mp3,
    Ogg,
    Flac,
    Wav,
}

#[derive(Debug, Clone, Copy)]
pub enum ImageType {
    Png,
    Jpg,
    Bmp,
    Tga,
}

#[derive(Debug, Clone, Copy)]
pub enum ModelType {
    Fbx,
    GltfJson,
    GltfBinary,
    Roblox,
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone)]
pub enum AssetRef {
    Cloud(u64),
    Studio(String),
}

impl fmt::Display for AssetRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetRef::Cloud(id) => write!(f, "rbxassetid://{id}"),
            AssetRef::Studio(name) => write!(f, "rbxasset://{name}"),
        }
    }
}

impl From<WebAsset> for AssetRef {
    fn from(value: WebAsset) -> Self {
        AssetRef::Cloud(value.id)
    }
}

impl From<&LockfileEntry> for AssetRef {
    fn from(value: &LockfileEntry) -> Self {
        AssetRef::Cloud(value.asset_id)
    }
}
