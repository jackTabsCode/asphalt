use crate::util::svg::svg_to_png;
use anyhow::bail;
use image::{DynamicImage, GenericImageView, ImageBuffer, RgbaImage};
use resvg::usvg::fontdb::Database;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SpriteInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct Spritesheet {
    pub image: RgbaImage,
    pub sprites: HashMap<String, SpriteInfo>,
}

const MAX_SIZE: u32 = 1000;

pub fn pack_spritesheet(images: &HashMap<String, DynamicImage>) -> anyhow::Result<Spritesheet> {
    if images.is_empty() {
        bail!("No images to pack");
    }

    let mut image_entries: Vec<_> = images.iter().collect();

    image_entries.sort_by(|a, b| a.0.cmp(b.0));
    image_entries.sort_by(|a, b| {
        let a_height = a.1.height();
        let b_height = b.1.height();
        b_height.cmp(&a_height)
    });

    let mut spritesheet = ImageBuffer::new(MAX_SIZE, MAX_SIZE);
    let mut sprites = HashMap::new();

    let mut current_x = 0;
    let mut current_y = 0;
    let mut row_height = 0;

    for (path, image) in image_entries {
        let width = image.width();
        let height = image.height();

        if current_x + width > MAX_SIZE {
            current_x = 0;
            current_y += row_height;
            row_height = 0;
        }

        if current_y + height > MAX_SIZE {
            bail!(
                "Cannot fit all images in a {}x{} spritesheet",
                MAX_SIZE,
                MAX_SIZE
            );
        }

        for y in 0..height {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                spritesheet.put_pixel(current_x + x, current_y + y, pixel);
            }
        }

        sprites.insert(
            path.clone(),
            SpriteInfo {
                x: current_x,
                y: current_y,
                width,
                height,
            },
        );

        current_x += width;
        row_height = row_height.max(height);
    }

    Ok(Spritesheet {
        image: spritesheet,
        sprites,
    })
}

pub async fn collect_images_for_packing(
    asset_dir: &Path,
    pack_dirs: &[String],
    exclude_patterns: &globset::GlobSet,
    fontdb: Arc<Database>,
) -> anyhow::Result<HashMap<String, HashMap<String, DynamicImage>>> {
    let mut packs: HashMap<String, HashMap<String, DynamicImage>> = HashMap::new();

    let mut sorted_pack_dirs = pack_dirs.to_vec();
    sorted_pack_dirs.sort();

    for pack_dir in sorted_pack_dirs {
        let full_path = asset_dir.join(&pack_dir);
        if !full_path.exists() || !full_path.is_dir() {
            bail!("Pack directory does not exist: {}", full_path.display());
        }

        let mut images = HashMap::new();

        let mut entries = Vec::new();

        for entry in WalkDir::new(&full_path)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                entries.push(entry);
            }
        }

        entries.sort_by(|a, b| a.path().to_string_lossy().cmp(&b.path().to_string_lossy()));

        for entry in entries {
            let path = entry.path();

            if exclude_patterns.is_match(path.to_string_lossy().as_ref()) {
                continue;
            }

            if is_image_file(path) {
                let image = if is_svg_file(path) {
                    let svg_data = fs::read(path).await?;

                    let png_data = svg_to_png(&svg_data, fontdb.clone()).await?;

                    image::load_from_memory(&png_data)?
                } else {
                    image::open(path)?
                };

                let rel_path = path
                    .strip_prefix(asset_dir)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");

                images.insert(rel_path.to_string(), image);
            }
        }

        if !images.is_empty() {
            packs.insert(pack_dir.clone(), images);
        }
    }

    Ok(packs)
}

fn is_svg_file(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.to_lowercase() == "svg",
        None => false,
    }
}

fn is_image_file(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => {
            let ext = ext.to_lowercase();
            ext == "png"
                || ext == "jpg"
                || ext == "jpeg"
                || ext == "bmp"
                || ext == "tga"
                || ext == "svg"
        }
        None => false,
    }
}
