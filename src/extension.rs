use rbxcloud::rbx::assets::AssetType;

pub trait FromExtension {
    fn from_extension(extension: &str) -> Option<Self>
    where
        Self: Sized;
}

impl FromExtension for AssetType {
    fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            "png" => Some(AssetType::DecalPng),
            "jpg" | "jpeg" => Some(AssetType::DecalJpeg),
            "bmp" => Some(AssetType::DecalBmp),
            "mp3" => Some(AssetType::AudioMp3),
            "ogg" => Some(AssetType::AudioOgg),
            _ => None,
        }
    }
}
