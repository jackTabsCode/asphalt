use resvg::{
    tiny_skia::Pixmap,
    usvg::{Options, Transform, Tree, fontdb::Database},
};
use std::sync::Arc;

pub fn svg_to_png(data: &[u8], fontdb: Arc<Database>) -> anyhow::Result<Vec<u8>> {
    let opt = Options {
        fontdb,
        ..Default::default()
    };

    let rtree = Tree::from_data(data, &opt)?;
    let pixmap_size = rtree.size();

    let mut pixmap = Pixmap::new(pixmap_size.width() as u32, pixmap_size.height() as u32).unwrap();
    resvg::render(&rtree, Transform::identity(), &mut pixmap.as_mut());

    let encoded = pixmap.encode_png()?;

    Ok(encoded)
}
