use std::path::PathBuf;

use anyhow::Context;
use resvg::{
    tiny_skia::Pixmap,
    usvg::{fontdb::Database, Options, Transform, Tree},
};
use tokio::fs::read_to_string;

pub async fn svg_to_png(path: &PathBuf) -> anyhow::Result<Vec<u8>> {
    let str = read_to_string(path)
        .await
        .context("Failed to read SVG file")?;

    let opt = Options::default();

    let mut db = Database::new();
    db.load_system_fonts();

    let rtree = Tree::from_str(&str, &opt, &db).context("Failed to parse SVG file")?;
    let pixmap_size = rtree.size();

    let mut pixmap = Pixmap::new(pixmap_size.width() as u32, pixmap_size.height() as u32)
        .context("Failed to create pixmap")?;
    resvg::render(&rtree, Transform::identity(), &mut pixmap.as_mut());

    let encoded = pixmap.encode_png().context("Failed to encode PNG")?;

    Ok(encoded)
}
