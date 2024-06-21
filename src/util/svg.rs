use anyhow::Context;
use resvg::{
    tiny_skia::Pixmap,
    usvg::{fontdb::Database, Options, Transform, Tree},
};

pub async fn svg_to_png(data: &[u8], font_db: &Database) -> anyhow::Result<Vec<u8>> {
    let opt = Options::default();

    let rtree = Tree::from_data(data, &opt, font_db).context("Failed to parse SVG file")?;
    let pixmap_size = rtree.size();

    let mut pixmap = Pixmap::new(pixmap_size.width() as u32, pixmap_size.height() as u32)
        .context("Failed to create pixmap")?;
    resvg::render(&rtree, Transform::identity(), &mut pixmap.as_mut());

    let encoded = pixmap.encode_png().context("Failed to encode PNG")?;

    Ok(encoded)
}
