use crate::asset::Asset;
use anyhow::bail;
use image::{DynamicImage, GenericImageView, ImageBuffer, RgbaImage};
use log::{debug, info};
use resvg::usvg::fontdb::Database;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use walkdir::WalkDir;

const MAX_SIZE: u32 = 1024;

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

pub fn pack_spritesheets(
    images: &HashMap<String, DynamicImage>,
) -> anyhow::Result<Vec<Spritesheet>> {
    if images.is_empty() {
        return Ok(Vec::new());
    }

    let mut sorted_paths: Vec<_> = images.keys().collect();
    sorted_paths.sort();

    let mut remaining_paths: HashSet<_> = sorted_paths.iter().cloned().collect();
    let mut spritesheets = Vec::new();

    while !remaining_paths.is_empty() {
        let spritesheet = pack_single_spritesheet(images, &remaining_paths)?;

        for path in spritesheet.sprites.keys() {
            remaining_paths.remove(path);
        }

        spritesheets.push(spritesheet);
    }

    info!(
        "Created {} spritesheet(s) for {} images",
        spritesheets.len(),
        images.len()
    );

    Ok(spritesheets)
}

fn pack_single_spritesheet(
    images: &HashMap<String, DynamicImage>,
    remaining_paths: &HashSet<&String>,
) -> anyhow::Result<Spritesheet> {
    let mut paths_to_pack: Vec<_> = remaining_paths.iter().collect();
    paths_to_pack.sort();

    let mut spritesheet = ImageBuffer::new(MAX_SIZE, MAX_SIZE);
    let mut sprites = HashMap::new();

    let mut current_x = 0;
    let mut current_y = 0;
    let mut row_height = 0;

    for &&path in &paths_to_pack {
        let image = &images[path];
        let width = image.width();
        let height = image.height();

        if width > MAX_SIZE || height > MAX_SIZE {
            if sprites.is_empty() {
                let mut large_sheet = ImageBuffer::new(width, height);
                for y in 0..height {
                    for x in 0..width {
                        large_sheet.put_pixel(x, y, image.get_pixel(x, y));
                    }
                }

                let mut single_sprite = HashMap::new();
                single_sprite.insert(
                    path.clone(),
                    SpriteInfo {
                        x: 0,
                        y: 0,
                        width,
                        height,
                    },
                );

                return Ok(Spritesheet {
                    image: large_sheet,
                    sprites: single_sprite,
                });
            }
            continue;
        }

        if current_x + width > MAX_SIZE {
            current_x = 0;
            current_y += row_height;
            row_height = 0;

            if current_y + height > MAX_SIZE {
                break;
            }
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
        row_height = std::cmp::max(row_height, height);
    }

    if sprites.is_empty() {
        bail!("Could not fit any images into a spritesheet");
    }

    Ok(Spritesheet {
        image: spritesheet,
        sprites,
    })
}

pub async fn collect_images_for_packing(
    asset_dir: &Path,
    spritesheet_matcher: &globset::GlobSet,
    exclude_patterns: &globset::GlobSet,
    fontdb: Arc<Database>,
) -> anyhow::Result<HashMap<String, DynamicImage>> {
    let mut images = HashMap::new();

    for entry in WalkDir::new(asset_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        if exclude_patterns.is_match(path.to_string_lossy().as_ref()) {
            continue;
        }

        let rel_path = path
            .strip_prefix(asset_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        if !spritesheet_matcher.is_match(&rel_path) {
            continue;
        }

        if is_image_file(path) {
            debug!("Processing image for spritesheet: {}", path.display());

            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let data = fs::read(path).await?;

            let asset = Asset::new(file_name, data, &ext, fontdb.clone(), true).await?;
            let image = image::load_from_memory(asset.data())?;

            images.insert(rel_path.to_string(), image);
        }
    }

    Ok(images)
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
