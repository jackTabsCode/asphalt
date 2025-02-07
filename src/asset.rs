use crate::util::{alpha_bleed::alpha_bleed, svg::svg_to_png};
use anyhow::{bail, Context};
use blake3::Hasher;
use image::{DynamicImage, ImageFormat};
use rbx_xml::DecodeOptions;
use rbxcloud::rbx::v1::assets::AssetType as CloudAssetType;
use resvg::usvg::fontdb::Database;
use std::{io::Cursor, sync::Arc};

pub enum AudioKind {
    Mp3,
    Ogg,
    Flac,
    Wav,
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
    ext: String,
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
        font_db: Arc<Database>,
    ) -> anyhow::Result<Self> {
        let kind = match ext {
            "mp3" => AssetKind::Audio(AudioKind::Mp3),
            "ogg" => AssetKind::Audio(AudioKind::Ogg),
            "flac" => AssetKind::Audio(AudioKind::Flac),
            "wav" => AssetKind::Audio(AudioKind::Wav),
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
                AudioKind::Flac => Some(CloudAssetType::AudioFlac),
                AudioKind::Wav => Some(CloudAssetType::AudioWav),
            },
            AssetKind::Model(kind) => match kind {
                ModelKind::Model => Some(CloudAssetType::ModelFbx),
                ModelKind::Animation => None,
            },
        };

        if let AssetKind::Decal(_) = &kind {
            let mut image: DynamicImage = image::load_from_memory(&data)?;
            alpha_bleed(&mut image, 1);

            let format = ImageFormat::from_extension(ext)
                .context("Failed to get image format from extension")?;

            let mut new_data: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            image.write_to(&mut new_data, format)?;

            data = new_data.into_inner();
        }

        Ok(Self {
            name,
            ext: ext.to_string(),
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn extension(&self) -> &str {
        &self.ext
    }

    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }

    pub fn kind(&self) -> &AssetKind {
        &self.kind
    }

    pub fn cloud_type(&self) -> Option<CloudAssetType> {
        self.cloud_type
    }
}
