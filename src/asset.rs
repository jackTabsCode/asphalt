use crate::{
    upload::{upload_animation, upload_cloud_asset},
    util::{alpha_bleed::alpha_bleed, svg::svg_to_png},
};
use anyhow::{bail, Context};
use blake3::Hasher;
use image::{DynamicImage, ImageFormat};
use rbx_xml::DecodeOptions;
use rbxcloud::rbx::v1::assets::{AssetCreator, AssetType as CloudAssetType};
use resvg::usvg::fontdb::Database;
use std::{io::Cursor, path::Path};
use tokio::fs::write;

pub enum AudioKind {
    Mp3,
    Ogg,
}

pub enum DecalKind {
    Png,
    Jpg,
    Bmp,
    Tga,
}

pub enum ModelKind {
    Model,
    Animation, // not uploadable with Open Cloud!
}

pub enum AssetKind {
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

fn verify_animation(data: Vec<u8>, format: ModelFileFormat) -> anyhow::Result<Vec<u8>> {
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

    let mut writer = Cursor::new(Vec::new());
    rbx_binary::to_writer(&mut writer, &dom, &[first_ref])?;

    Ok(writer.into_inner())
}

pub struct UploadResult {
    pub asset_id: u64,
    pub csrf: Option<String>,
}

impl Asset {
    pub async fn new(
        name: String,
        mut data: Vec<u8>,
        mut ext: &str,
        font_db: &Database,
    ) -> anyhow::Result<Self> {
        let kind = match ext {
            "mp3" => AssetKind::Audio(AudioKind::Mp3),
            "ogg" => AssetKind::Audio(AudioKind::Ogg),
            "png" => AssetKind::Decal(DecalKind::Png),
            "jpg" => AssetKind::Decal(DecalKind::Jpg),
            "bmp" => AssetKind::Decal(DecalKind::Bmp),
            "tga" => AssetKind::Decal(DecalKind::Tga),
            "svg" => {
                data = svg_to_png(&data, font_db).await?;
                ext = "png";
                AssetKind::Decal(DecalKind::Png)
            }
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

            let mut new_data: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            image.write_to(&mut new_data, format)?;

            data = new_data.into_inner();
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

    pub fn kind(&self) -> &AssetKind {
        &self.kind
    }

    async fn upload_cloud(
        self,
        creator: AssetCreator,
        api_key: String,
        cloud_type: CloudAssetType,
    ) -> anyhow::Result<UploadResult> {
        let asset_id =
            upload_cloud_asset(self.data, self.name, cloud_type, api_key, creator).await?;

        Ok(UploadResult {
            asset_id,
            csrf: None,
        })
    }

    async fn upload_animation(
        self,
        creator: AssetCreator,
        cookie: String,
        csrf: Option<String>,
    ) -> anyhow::Result<UploadResult> {
        let result = upload_animation(self.data, self.name, cookie, csrf, creator).await?;

        Ok(UploadResult {
            asset_id: result.asset_id,
            csrf: Some(result.csrf),
        })
    }

    pub async fn upload(
        self,
        creator: AssetCreator,
        api_key: String,
        cookie: Option<String>,
        csrf: Option<String>,
    ) -> anyhow::Result<UploadResult> {
        match &self.kind {
            AssetKind::Decal(_) | AssetKind::Audio(_) | AssetKind::Model(ModelKind::Model) => {
                let cloud_type = self
                    .cloud_type
                    .ok_or_else(|| anyhow::anyhow!("Invalid cloud type"))?;

                self.upload_cloud(creator, api_key, cloud_type).await
            }
            AssetKind::Model(ModelKind::Animation) => {
                if let Some(cookie) = cookie {
                    self.upload_animation(creator, cookie, csrf).await
                } else {
                    bail!("Cookie required for uploading animations")
                }
            }
        }
    }

    pub async fn write(self, path: &Path) -> anyhow::Result<()> {
        write(path, self.data)
            .await
            .with_context(|| format!("Failed to write asset to {}", path.display()))
    }
}
