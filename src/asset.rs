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

    pub fn process(
        &mut self,
        font_db: Arc<fontdb::Database>,
        bleed: bool,
        optimize: bool,
    ) -> anyhow::Result<()> {
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

        if optimize && crate::util::optimize::should_optimize(&self.path.to_path(""), true) {
            self.data = crate::util::optimize::optimize_png(&self.data)?.into();
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_is_supported_extension() {
        assert!(is_supported_extension(OsStr::new("png")));
        assert!(is_supported_extension(OsStr::new("svg")));
        assert!(is_supported_extension(OsStr::new("jpg")));
        assert!(is_supported_extension(OsStr::new("jpeg")));
        assert!(is_supported_extension(OsStr::new("mp3")));
        assert!(is_supported_extension(OsStr::new("fbx")));
        assert!(is_supported_extension(OsStr::new("rbxm")));
        assert!(is_supported_extension(OsStr::new("rbxmx")));
        assert!(is_supported_extension(OsStr::new("mp4")));
        assert!(!is_supported_extension(OsStr::new("txt")));
        assert!(!is_supported_extension(OsStr::new("exe")));
        assert!(!is_supported_extension(OsStr::new("")));
    }

    #[test]
    fn test_asset_new_image_extensions() {
        for (ext, data) in [("png", &b"dummy-png-data"[..]), ("jpg", &b"dummy-jpg-data"[..]), ("bmp", &b"dummy-bmp-data"[..]), ("tga", &b"dummy-tga-data"[..])] {
            let path = RelativePathBuf::from(format!("test.{ext}"));
            let asset = Asset::new(path, Bytes::from_static(data)).unwrap();
            assert_eq!(asset.ext, ext);
            assert!(matches!(asset.ty, AssetType::Image(_)));
        }
    }

    #[test]
    fn test_asset_new_audio_extensions() {
        for (ext, data) in [("mp3", &b"dummy-mp3-data"[..]), ("ogg", &b"dummy-ogg-data"[..]), ("flac", &b"dummy-flac-data"[..]), ("wav", &b"dummy-wav-data"[..])] {
            let path = RelativePathBuf::from(format!("test.{ext}"));
            let asset = Asset::new(path, Bytes::from_static(data)).unwrap();
            assert_eq!(asset.ext, ext);
            assert!(matches!(asset.ty, AssetType::Audio(_)));
        }
    }

    #[test]
    fn test_asset_new_model_extensions() {
        for (ext, data) in [("fbx", &b"dummy-fbx-data"[..]), ("gltf", &b"dummy-gltf-data"[..]), ("glb", &b"dummy-glb-data"[..])] {
            let path = RelativePathBuf::from(format!("test.{ext}"));
            let asset = Asset::new(path, Bytes::from_static(data)).unwrap();
            assert_eq!(asset.ext, ext);
            assert!(matches!(asset.ty, AssetType::Model(_)));
        }
    }

    #[test]
    fn test_asset_new_video_extensions() {
        for (ext, data) in [("mp4", &b"dummy-mp4-data"[..]), ("mov", &b"dummy-mov-data"[..])] {
            let path = RelativePathBuf::from(format!("test.{ext}"));
            let asset = Asset::new(path, Bytes::from_static(data)).unwrap();
            assert_eq!(asset.ext, ext);
            assert!(matches!(asset.ty, AssetType::Video(_)));
        }
    }

    #[test]
    fn test_asset_new_svg_converts_to_png() {
        let path = RelativePathBuf::from("icon.svg");
        let asset = Asset::new(path, Bytes::from_static(b"<svg></svg>")).unwrap();
        assert_eq!(asset.ext, "png", "SVG extension should be renamed to png");
        assert!(matches!(asset.ty, AssetType::Image(_)));
    }

    #[test]
    fn test_asset_new_unknown_extension_fails() {
        let path = RelativePathBuf::from("test.txt");
        let result = Asset::new(path, Bytes::from_static(b"hello"));
        assert!(result.is_err());
    }

    #[test]
    fn test_asset_new_no_extension_fails() {
        let path = RelativePathBuf::from("noext");
        let result = Asset::new(path, Bytes::from_static(b"data"));
        assert!(result.is_err());
    }

    #[test]
    fn test_asset_type_variants() {
        assert_eq!(AssetType::Model(ModelType::Fbx).asset_type(), "Model");
        assert_eq!(AssetType::Animation.asset_type(), "Animation");
        assert_eq!(AssetType::Image(ImageType::Png).asset_type(), "Image");
        assert_eq!(AssetType::Audio(AudioType::Mp3).asset_type(), "Audio");
        assert_eq!(AssetType::Video(VideoType::Mp4).asset_type(), "Video");
    }

    #[test]
    fn test_asset_type_file_types() {
        assert_eq!(AssetType::Animation.file_type(), "model/x-rbxm");
        assert_eq!(AssetType::Model(ModelType::Fbx).file_type(), "model/fbx");
        assert_eq!(AssetType::Model(ModelType::GltfJson).file_type(), "model/gltf+json");
        assert_eq!(AssetType::Model(ModelType::GltfBinary).file_type(), "model/gltf-binary");
        assert_eq!(AssetType::Model(ModelType::Roblox).file_type(), "model/x-rbxm");
        assert_eq!(AssetType::Image(ImageType::Png).file_type(), "image/png");
        assert_eq!(AssetType::Image(ImageType::Jpg).file_type(), "image/jpeg");
        assert_eq!(AssetType::Image(ImageType::Bmp).file_type(), "image/bmp");
        assert_eq!(AssetType::Image(ImageType::Tga).file_type(), "image/tga");
        assert_eq!(AssetType::Audio(AudioType::Mp3).file_type(), "audio/mpeg");
        assert_eq!(AssetType::Audio(AudioType::Ogg).file_type(), "audio/ogg");
        assert_eq!(AssetType::Audio(AudioType::Flac).file_type(), "audio/flac");
        assert_eq!(AssetType::Audio(AudioType::Wav).file_type(), "audio/wav");
        assert_eq!(AssetType::Video(VideoType::Mp4).file_type(), "video/mp4");
        assert_eq!(AssetType::Video(VideoType::Mov).file_type(), "video/mov");
    }

    #[test]
    fn test_asset_ref_cloud_display() {
        let cloud = AssetRef::Cloud(123456789);
        assert_eq!(cloud.to_string(), "rbxassetid://123456789");
    }

    #[test]
    fn test_asset_ref_cloud_zero() {
        let cloud = AssetRef::Cloud(0);
        assert_eq!(cloud.to_string(), "rbxassetid://0");
    }

    #[test]
    fn test_asset_ref_studio_display() {
        let studio = AssetRef::Studio("rbxassetid://123".to_string());
        assert_eq!(studio.to_string(), "rbxasset://rbxassetid://123");
    }

    #[test]
    fn test_asset_ref_from_web_asset() {
        let web = WebAsset { id: 42 };
        let asset_ref: AssetRef = web.into();
        assert_eq!(asset_ref.to_string(), "rbxassetid://42");
    }

    #[test]
    fn test_asset_ref_from_lockfile_entry() {
        let entry = LockfileEntry {
            asset_id: 999,
            sprite_info: None,
        };
        let asset_ref: AssetRef = (&entry).into();
        assert_eq!(asset_ref.to_string(), "rbxassetid://999");
    }

    #[test]
    fn test_asset_type_serialization() {
        let json = serde_json::to_string(&AssetType::Image(ImageType::Png)).unwrap();
        assert_eq!(json, "\"Image\"");
        let json = serde_json::to_string(&AssetType::Animation).unwrap();
        assert_eq!(json, "\"Animation\"");
    }
}
