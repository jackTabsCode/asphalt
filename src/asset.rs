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
    Svg,
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

#[derive(Debug, Clone)]
pub enum ModelFileFormat {
    Binary,
    Xml,
}
