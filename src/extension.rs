use rbxcloud::rbx::assets::AssetType;

pub fn from_extension(extension: &str) -> Option<AssetType> {
    match extension {
        "png" => Some(AssetType::DecalPng),
        "jpg" | "jpeg" => Some(AssetType::DecalJpeg),
        "bmp" => Some(AssetType::DecalBmp),
        "tga" => Some(AssetType::DecalTga),
        "mp3" => Some(AssetType::AudioMp3),
        "ogg" => Some(AssetType::AudioOgg),
        _ => None,
    }
}
