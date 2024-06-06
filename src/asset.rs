use anyhow::{bail, Context};
use blake3::Hasher;
use image::{DynamicImage, ImageFormat};
use rbx_xml::DecodeOptions;
use rbxcloud::rbx::v1::assets::{AssetCreator, AssetType as CloudAssetType};
use std::io::Cursor;

use crate::{upload::upload_cloud_asset, util::alpha_bleed::alpha_bleed};

enum AudioKind {
    Mp3,
    Ogg,
}

enum DecalKind {
    Png,
    Jpg,
    Bmp,
    Tga,
}

enum ModelKind {
    Model,
    Animation, // not uploadable with Open Cloud!
}

enum AssetKind {
    Decal(DecalKind),
    Audio(AudioKind),
    Model(ModelKind),
}

pub struct Asset {
    name: String,
    data: Vec<u8>,

    kind: AssetKind,
    cloud_type: Option<CloudAssetType>,
}

enum ModelFileFormat {
    Binary,
    Xml,
}

fn verify_animation(data: Vec<u8>, format: ModelFileFormat) -> anyhow::Result<()> {
    let slice = data.as_slice();
    let dom = match format {
        ModelFileFormat::Binary => rbx_binary::from_reader(slice)?,
        ModelFileFormat::Xml => rbx_xml::from_reader(slice, DecodeOptions::new())?,
    };

    let children = dom.root().children();

    let first_ref = *children.first().context("No children found in root")?;
    let first = dom
        .get_by_ref(first_ref)
        .context("Failed to get first child")?;

    if first.class != "KeyframeSequence" {
        bail!("Root class name is not KeyframeSequence");
    }

    Ok(())
}

impl Asset {
    pub fn new(name: String, mut data: Vec<u8>, ext: &str) -> anyhow::Result<Self> {
        let kind = match ext {
            "mp3" => AssetKind::Audio(AudioKind::Mp3),
            "ogg" => AssetKind::Audio(AudioKind::Ogg),
            "png" => AssetKind::Decal(DecalKind::Png),
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

                verify_animation(data.clone(), format)?;

                AssetKind::Model(ModelKind::Animation)
            }
            _ => bail!("Unknown extension .{ext}"),
        };

        let cloud_type = match &kind {
            AssetKind::Decal(kind) => match kind {
                DecalKind::Png => Some(CloudAssetType::DecalPng),
                DecalKind::Jpg => Some(CloudAssetType::DecalJpeg),
                DecalKind::Bmp => Some(CloudAssetType::DecalBmp),
                DecalKind::Tga => Some(CloudAssetType::DecalTga),
            },
            AssetKind::Audio(kind) => match kind {
                AudioKind::Mp3 => Some(CloudAssetType::AudioMp3),
                AudioKind::Ogg => Some(CloudAssetType::AudioOgg),
            },
            AssetKind::Model(kind) => match kind {
                ModelKind::Model => Some(CloudAssetType::ModelFbx),
                ModelKind::Animation => None,
            },
        };

        if let AssetKind::Decal(_) = &kind {
            let mut image: DynamicImage = image::load_from_memory(&data)?;
            alpha_bleed(&mut image);

            let format = ImageFormat::from_extension(ext)
                .context("Failed to get image format from extension")?;

            let mut new_bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            image.write_to(&mut new_bytes, format)?;

            data = new_bytes.into_inner();
        }

        Ok(Self {
            name,
            data,
            kind,
            cloud_type,
        })
    }

    pub fn hash(&self) -> String {
        let mut hasher = Hasher::new();
        hasher.update(&self.data);
        hasher.finalize().to_string()
    }

    async fn upload_cloud(
        self,
        creator: AssetCreator,
        api_key: String,
        cloud_type: CloudAssetType,
    ) -> anyhow::Result<u64> {
        upload_cloud_asset(self.data, self.name, cloud_type, api_key, creator).await
    }

    async fn upload_animation(self, creator: AssetCreator, api_key: String) -> anyhow::Result<u64> {
        Ok(0)
    }

    pub async fn upload(self, creator: AssetCreator, api_key: String) -> anyhow::Result<u64> {
        match &self.kind {
            AssetKind::Decal(_) | AssetKind::Audio(_) | AssetKind::Model(ModelKind::Model) => {
                let cloud_type = self
                    .cloud_type
                    .ok_or_else(|| anyhow::anyhow!("Invalid cloud type"))?;
                self.upload_cloud(creator, api_key, cloud_type).await
            }
            AssetKind::Model(ModelKind::Animation) => self.upload_animation(creator, api_key).await,
        }
    }
}
