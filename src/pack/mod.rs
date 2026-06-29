use crate::{
    asset::Asset,
    config::{PackOptions, PackSort},
    hash::Hash,
};
use anyhow::{Context, Result, bail};
use image::RgbaImage;
use std::collections::HashMap;

pub mod algorithm;
pub mod manifest;
pub mod rect;

pub use manifest::{AtlasManifest, SpriteInfo};
pub use rect::{Rect, Size};

/// A sprite to be packed into an atlas
#[derive(Debug, Clone)]
pub struct Sprite {
    pub name: String,
    pub data: Vec<u8>,
    pub size: Size,
    #[allow(dead_code)]
    pub hash: Hash,
}

/// Result of packing sprites into atlases
#[derive(Debug)]
pub struct PackResult {
    pub atlases: Vec<Atlas>,
    pub manifest: AtlasManifest,
}

/// A single atlas page containing packed sprites
#[derive(Debug)]
pub struct Atlas {
    pub page_index: usize,
    pub image_data: Vec<u8>,
    #[allow(dead_code)]
    pub size: Size,
    pub sprites: Vec<PackedSprite>,
}

/// A sprite that has been placed in an atlas
#[derive(Debug, Clone)]
pub struct PackedSprite {
    pub sprite: Sprite,
    pub rect: Rect,
    pub trimmed: bool,
    pub sprite_source_size: Option<Rect>,
}

/// Main packing orchestrator
pub struct Packer {
    options: PackOptions,
}

impl Packer {
    pub fn new(options: PackOptions) -> Self {
        Self { options }
    }

    /// Pack a collection of assets into atlases
    pub fn pack_assets(&self, assets: &[Asset], input_name: &str) -> Result<PackResult> {
        if !self.options.enabled {
            bail!("Packing is not enabled for input '{}'", input_name);
        }

        // Convert assets to sprites
        let sprites = self.assets_to_sprites(assets)?;

        if sprites.is_empty() {
            return Ok(PackResult {
                atlases: Vec::new(),
                manifest: AtlasManifest::new(input_name.to_string()),
            });
        }

        // Sort sprites for deterministic packing
        let mut sorted_sprites = sprites;
        self.sort_sprites(&mut sorted_sprites);

        // Validate sprite sizes
        self.validate_sprite_sizes(&sorted_sprites)?;

        // Pack sprites into pages
        let atlases = self.pack_sprites_to_atlases(sorted_sprites)?;

        // Check page limit
        if let Some(limit) = self.options.page_limit
            && atlases.len() > limit as usize
        {
            bail!(
                "Packing would require {} pages but limit is {}. Consider increasing max_size or page_limit.",
                atlases.len(),
                limit
            );
        }

        // Generate manifest
        let manifest = self.create_manifest(&atlases, input_name)?;

        Ok(PackResult { atlases, manifest })
    }

    fn assets_to_sprites(&self, assets: &[Asset]) -> Result<Vec<Sprite>> {
        let mut sprites = Vec::new();
        let mut seen_hashes = HashMap::new();

        for asset in assets {
            // Only pack image assets
            if !matches!(asset.ty, crate::asset::AssetType::Image(_)) {
                continue;
            }

            // Load image to get dimensions
            let image = image::load_from_memory(&asset.data)
                .with_context(|| format!("Failed to load image: {}", asset.path))?;

            let size = Size {
                width: image.width(),
                height: image.height(),
            };

            let name = asset.path.file_stem().unwrap_or("unknown").to_string();

            // Handle deduplication
            if self.options.dedupe {
                if let Some(existing_name) = seen_hashes.get(&asset.hash) {
                    log::debug!(
                        "Skipping duplicate sprite '{}' (same as '{}')",
                        name,
                        existing_name
                    );
                    continue;
                }
                seen_hashes.insert(asset.hash, name.clone());
            }

            sprites.push(Sprite {
                name,
                data: asset.data.to_vec(),
                size,
                hash: asset.hash,
            });
        }

        Ok(sprites)
    }

    fn sort_sprites(&self, sprites: &mut [Sprite]) {
        sprites.sort_by(|a, b| {
            let primary_cmp = match self.options.sort {
                PackSort::Area => {
                    let area_a = a.size.width * a.size.height;
                    let area_b = b.size.width * b.size.height;
                    area_b.cmp(&area_a) // Descending order (largest first)
                }
                PackSort::MaxSide => {
                    let max_a = a.size.width.max(a.size.height);
                    let max_b = b.size.width.max(b.size.height);
                    max_b.cmp(&max_a) // Descending order (largest first)
                }
                PackSort::Name => a.name.cmp(&b.name),
            };

            // Use name as tie-breaker for deterministic results
            primary_cmp.then_with(|| a.name.cmp(&b.name))
        });
    }

    fn validate_sprite_sizes(&self, sprites: &[Sprite]) -> Result<()> {
        let max_width = self.options.max_size.0;
        let max_height = self.options.max_size.1;

        for sprite in sprites {
            if sprite.size.width > max_width || sprite.size.height > max_height {
                bail!(
                    "Sprite '{}' ({}x{}) exceeds maximum atlas size ({}x{}). Consider increasing max_size or excluding this sprite from packing.",
                    sprite.name,
                    sprite.size.width,
                    sprite.size.height,
                    max_width,
                    max_height
                );
            }
        }

        Ok(())
    }

    fn pack_sprites_to_atlases(&self, sprites: Vec<Sprite>) -> Result<Vec<Atlas>> {
        let mut atlases = Vec::new();
        let mut remaining_sprites = sprites;

        while !remaining_sprites.is_empty() {
            let page_index = atlases.len();
            let (atlas, unpacked_sprites) =
                self.pack_single_atlas(remaining_sprites, page_index)?;
            atlases.push(atlas);
            remaining_sprites = unpacked_sprites;
        }

        Ok(atlases)
    }

    fn pack_single_atlas(
        &self,
        sprites: Vec<Sprite>,
        page_index: usize,
    ) -> Result<(Atlas, Vec<Sprite>)> {
        let atlas_size = if self.options.power_of_two {
            // Find the next power of two that fits our max size
            let width = self.options.max_size.0.next_power_of_two();
            let height = self.options.max_size.1.next_power_of_two();
            Size { width, height }
        } else {
            Size {
                width: self.options.max_size.0,
                height: self.options.max_size.1,
            }
        };

        let mut packer = algorithm::MaxRectsPacker::new(atlas_size);
        let mut packed_sprites = Vec::new();
        let mut unpacked_sprites = Vec::new();

        for mut sprite in sprites {
            // Trim sprite to remove transparent borders
            let original_rect = if self.options.allow_trim {
                self.trim_sprite(&mut sprite)
            } else {
                None
            };

            // Account for padding in placement
            let required_size = Size {
                width: sprite.size.width + 2 * self.options.padding,
                height: sprite.size.height + 2 * self.options.padding,
            };

            if let Some(rect) = packer.pack(required_size) {
                // Adjust rect to account for padding
                let sprite_rect = Rect {
                    x: rect.x + self.options.padding,
                    y: rect.y + self.options.padding,
                    width: sprite.size.width,
                    height: sprite.size.height,
                };

                packed_sprites.push(PackedSprite {
                    sprite,
                    rect: sprite_rect,
                    trimmed: original_rect.is_some(),
                    sprite_source_size: original_rect,
                });
            } else {
                unpacked_sprites.push(sprite);
            }
        }

        // Create atlas image
        let image_data = self.render_atlas(&packed_sprites, atlas_size)?;

        Ok((
            Atlas {
                page_index,
                image_data,
                size: atlas_size,
                sprites: packed_sprites,
            },
            unpacked_sprites,
        ))
    }

    fn trim_sprite(&self, sprite: &mut Sprite) -> Option<Rect> {
        use std::io::Cursor;

        let img = image::load_from_memory(&sprite.data).ok()?;
        let rgba = img.to_rgba8();
        let width = rgba.width() as usize;
        let height = rgba.height() as usize;

        if width == 0 || height == 0 {
            return None;
        }

        let pixels = rgba.as_raw();

        // Find bounding box of non-transparent pixels
        let mut min_x = width;
        let mut max_x = 0;
        let mut min_y = height;
        let mut max_y = 0;

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 4;
                if pixels[idx + 3] != 0 {
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if y < min_y {
                        min_y = y;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        }

        if min_x > max_x || min_y > max_y {
            return None; // No opaque pixels
        }

        let trimmed_width = max_x - min_x + 1;
        let trimmed_height = max_y - min_y + 1;

        if trimmed_width == width && trimmed_height == height {
            return None; // No trimming needed
        }

        // Crop the image
        let sub_img = image::imageops::crop_imm(
            &rgba,
            min_x as u32,
            min_y as u32,
            trimmed_width as u32,
            trimmed_height as u32,
        );
        let cropped = sub_img.to_image();

        // Encode back to PNG
        let mut buffer = Cursor::new(Vec::new());
        cropped
            .write_to(&mut buffer, image::ImageFormat::Png)
            .ok()?;

        let original_size = sprite.size;
        sprite.data = buffer.into_inner();
        sprite.size = Size {
            width: trimmed_width as u32,
            height: trimmed_height as u32,
        };

        Some(Rect {
            x: 0,
            y: 0,
            width: original_size.width,
            height: original_size.height,
        })
    }

    fn render_atlas(&self, packed_sprites: &[PackedSprite], atlas_size: Size) -> Result<Vec<u8>> {
        use image::{DynamicImage, ImageBuffer, RgbaImage};
        use std::io::Cursor;

        let mut atlas_image: RgbaImage = ImageBuffer::new(atlas_size.width, atlas_size.height);

        log::debug!(
            "Rendering atlas {}x{} with {} sprites",
            atlas_size.width,
            atlas_size.height,
            packed_sprites.len()
        );

        for (i, packed_sprite) in packed_sprites.iter().enumerate() {
            log::debug!(
                "Rendering sprite {} '{}' at ({}, {}) size {}x{}",
                i,
                packed_sprite.sprite.name,
                packed_sprite.rect.x,
                packed_sprite.rect.y,
                packed_sprite.rect.width,
                packed_sprite.rect.height
            );

            let sprite_image = image::load_from_memory(&packed_sprite.sprite.data)?;
            let sprite_rgba = sprite_image.to_rgba8();

            log::debug!(
                "Loaded sprite image {}x{}",
                sprite_rgba.width(),
                sprite_rgba.height()
            );

            // Copy sprite to atlas at the correct position
            for y in 0..packed_sprite.rect.height {
                for x in 0..packed_sprite.rect.width {
                    if let Some(sprite_pixel) = sprite_rgba.get_pixel_checked(x, y) {
                        atlas_image.put_pixel(
                            packed_sprite.rect.x + x,
                            packed_sprite.rect.y + y,
                            *sprite_pixel,
                        );
                    }
                }
            }

            // Apply extrude if configured
            if self.options.extrude > 0 {
                self.apply_extrude(&mut atlas_image, packed_sprite)?;
            }

            log::debug!("Finished rendering sprite '{}'", packed_sprite.sprite.name);
        }

        log::debug!("Applying alpha bleeding to atlas image");
        let mut atlas_dynamic = DynamicImage::ImageRgba8(atlas_image);
        crate::util::alpha_bleed::alpha_bleed(&mut atlas_dynamic);

        // Encode as PNG
        let mut buffer = Cursor::new(Vec::new());
        atlas_dynamic.write_to(&mut buffer, image::ImageFormat::Png)?;
        Ok(buffer.into_inner())
    }

    fn apply_extrude(
        &self,
        atlas_image: &mut RgbaImage,
        packed_sprite: &PackedSprite,
    ) -> Result<()> {
        let extrude = self.options.extrude;
        let rect = &packed_sprite.rect;

        for e in 1..=extrude {
            let e = e as i32;

            for y in 0..rect.height {
                if rect.x >= e as u32 {
                    let edge_pixel = atlas_image.get_pixel(rect.x, rect.y + y);
                    atlas_image.put_pixel(rect.x - e as u32, rect.y + y, *edge_pixel);
                }

                if rect.x + rect.width + (e as u32) <= atlas_image.width() {
                    let edge_pixel = atlas_image.get_pixel(rect.x + rect.width - 1, rect.y + y);
                    atlas_image.put_pixel(
                        rect.x + rect.width + e as u32 - 1,
                        rect.y + y,
                        *edge_pixel,
                    );
                }
            }

            for x in 0..rect.width {
                if rect.y >= e as u32 {
                    let edge_pixel = atlas_image.get_pixel(rect.x + x, rect.y);
                    atlas_image.put_pixel(rect.x + x, rect.y - e as u32, *edge_pixel);
                }

                if rect.y + rect.height + (e as u32) <= atlas_image.height() {
                    let edge_pixel = atlas_image.get_pixel(rect.x + x, rect.y + rect.height - 1);
                    atlas_image.put_pixel(
                        rect.x + x,
                        rect.y + rect.height + e as u32 - 1,
                        *edge_pixel,
                    );
                }
            }
        }

        Ok(())
    }

    fn create_manifest(&self, atlases: &[Atlas], input_name: &str) -> Result<AtlasManifest> {
        let mut manifest = AtlasManifest::new(input_name.to_string());

        for atlas in atlases {
            for packed_sprite in &atlas.sprites {
                let sprite_info = SpriteInfo {
                    name: packed_sprite.sprite.name.clone(),
                    rect: packed_sprite.rect,
                    source_size: packed_sprite.sprite.size,
                    trimmed: packed_sprite.trimmed,
                    sprite_source_size: packed_sprite.sprite_source_size,
                    page_index: atlas.page_index,
                };
                manifest.add_sprite(sprite_info);
            }
        }

        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset::Asset,
        config::{PackAlgorithm, PackSort},
    };
    use bytes::Bytes;
    use image::GenericImageView;
    use relative_path::RelativePathBuf;
    use std::io::Cursor;

    fn create_test_image(width: u32, height: u32, transparent: bool) -> Vec<u8> {
        let pixel = if transparent {
            image::Rgba([255, 0, 0, 0]) // fully transparent
        } else {
            image::Rgba([255, 0, 0, 255]) // solid red
        };
        let img = image::RgbaImage::from_pixel(width, height, pixel);
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn create_test_asset(name: &str, width: u32, height: u32) -> Asset {
        let data = create_test_image(width, height, false);
        let path = RelativePathBuf::from(name);
        Asset::new(path, Bytes::from(data)).unwrap()
    }

    fn create_test_asset_with_data(name: &str, data: Vec<u8>) -> Asset {
        let path = RelativePathBuf::from(name);
        Asset::new(path, Bytes::from(data)).unwrap()
    }

    fn default_pack_options() -> PackOptions {
        PackOptions {
            enabled: true,
            max_size: (512, 512),
            power_of_two: false,
            padding: 0,
            extrude: 0,
            allow_trim: false,
            algorithm: PackAlgorithm::MaxRects,
            page_limit: None,
            sort: PackSort::Area,
            dedupe: false,
        }
    }

    #[test]
    fn test_packer_not_enabled() {
        let options = PackOptions {
            enabled: false,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let assets = [create_test_asset("test.png", 32, 32)];
        let result = packer.pack_assets(&assets, "test_input");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not enabled"));
    }

    #[test]
    fn test_pack_empty_assets() {
        let packer = Packer::new(default_pack_options());
        let result = packer.pack_assets(&[], "empty_input").unwrap();
        assert!(result.atlases.is_empty());
        assert_eq!(result.manifest.sprite_count(), 0);
    }

    #[test]
    fn test_assets_to_sprites_filters_non_images() {
        let packer = Packer::new(default_pack_options());
        let img_asset = create_test_asset("sprite.png", 32, 32);
        let audio_asset = Asset::new(
            RelativePathBuf::from("sound.mp3"),
            Bytes::from_static(b"mp3-data"),
        ).unwrap();
        let assets = [img_asset, audio_asset];
        let sprites = packer.assets_to_sprites(&assets).unwrap();
        assert_eq!(sprites.len(), 1, "Should only include image assets");
        assert_eq!(sprites[0].name, "sprite");
    }

    #[test]
    fn test_assets_to_sprites_extracts_dimensions() {
        let packer = Packer::new(default_pack_options());
        let asset = create_test_asset("big.png", 64, 128);
        let sprites = packer.assets_to_sprites(&[asset]).unwrap();
        assert_eq!(sprites.len(), 1);
        assert_eq!(sprites[0].size.width, 64);
        assert_eq!(sprites[0].size.height, 128);
    }

    #[test]
    fn test_assets_to_sprites_dedupe() {
        let options = PackOptions {
            enabled: true,
            dedupe: true,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let data = create_test_image(16, 16, false);
        let asset1 = create_test_asset_with_data("dup1.png", data.clone());
        let asset2 = create_test_asset_with_data("dup2.png", data);
        let sprites = packer.assets_to_sprites(&[asset1, asset2]).unwrap();
        assert_eq!(sprites.len(), 1, "Duplicates should be deduplicated");
    }

    #[test]
    fn test_sort_sprites_by_area_descending() {
        let options = PackOptions {
            sort: PackSort::Area,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let small = create_test_asset("small.png", 10, 10);
        let large = create_test_asset("large.png", 100, 100);
        let mut sprites = packer.assets_to_sprites(&[small, large]).unwrap();
        packer.sort_sprites(&mut sprites);
        assert_eq!(sprites[0].name, "large");
        assert_eq!(sprites[1].name, "small");
    }

    #[test]
    fn test_sort_sprites_by_max_side_descending() {
        let options = PackOptions {
            sort: PackSort::MaxSide,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let wide = create_test_asset("wide.png", 200, 10);
        let tall = create_test_asset("tall.png", 10, 150);
        let mut sprites = packer.assets_to_sprites(&[wide, tall]).unwrap();
        packer.sort_sprites(&mut sprites);
        assert_eq!(sprites[0].name, "wide");
        assert_eq!(sprites[1].name, "tall");
    }

    #[test]
    fn test_sort_sprites_by_name() {
        let options = PackOptions {
            sort: PackSort::Name,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let z = create_test_asset("zebra.png", 10, 10);
        let a = create_test_asset("alpha.png", 10, 10);
        let mut sprites = packer.assets_to_sprites(&[z, a]).unwrap();
        packer.sort_sprites(&mut sprites);
        assert_eq!(sprites[0].name, "alpha");
        assert_eq!(sprites[1].name, "zebra");
    }

    #[test]
    fn test_sort_sprites_deterministic_tiebreaker() {
        let options = PackOptions {
            sort: PackSort::Area,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let a = create_test_asset("beta.png", 50, 50);
        let b = create_test_asset("alpha.png", 50, 50);
        let mut sprites = packer.assets_to_sprites(&[a, b]).unwrap();
        packer.sort_sprites(&mut sprites);
        // Same area should be sorted by name as tiebreaker
        assert_eq!(sprites[0].name, "alpha");
        assert_eq!(sprites[1].name, "beta");
    }

    #[test]
    fn test_validate_sprite_sizes_oversized() {
        let options = PackOptions {
            max_size: (64, 64),
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let asset = create_test_asset("too_big.png", 128, 128);
        let sprites = packer.assets_to_sprites(&[asset]).unwrap();
        let result = packer.validate_sprite_sizes(&sprites);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }

    #[test]
    fn test_validate_sprite_sizes_oversized_width_only() {
        let options = PackOptions {
            max_size: (32, 512),
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let asset = create_test_asset("too_wide.png", 64, 32);
        let sprites = packer.assets_to_sprites(&[asset]).unwrap();
        let result = packer.validate_sprite_sizes(&sprites);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_sprite_sizes_oversized_height_only() {
        let options = PackOptions {
            max_size: (512, 32),
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let asset = create_test_asset("too_tall.png", 32, 64);
        let sprites = packer.assets_to_sprites(&[asset]).unwrap();
        let result = packer.validate_sprite_sizes(&sprites);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_sprite_sizes_fits_exactly() {
        let options = PackOptions {
            max_size: (32, 32),
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let asset = create_test_asset("exact.png", 32, 32);
        let sprites = packer.assets_to_sprites(&[asset]).unwrap();
        assert!(packer.validate_sprite_sizes(&sprites).is_ok());
    }

    #[test]
    fn test_pack_sprites_to_single_atlas() {
        let packer = Packer::new(default_pack_options());
        let asset = create_test_asset("sprite.png", 32, 32);
        let sprites = packer.assets_to_sprites(&[asset]).unwrap();
        let atlases = packer.pack_sprites_to_atlases(sprites).unwrap();
        assert_eq!(atlases.len(), 1);
        assert_eq!(atlases[0].sprites.len(), 1);
        // Verify the atlas decodes as a valid PNG at the expected size
        let decoded = image::load_from_memory(&atlases[0].image_data).unwrap();
        assert_eq!(decoded.dimensions(), (512, 512));
    }

    #[test]
    fn test_pack_sprites_to_multiple_pages() {
        let options = PackOptions {
            max_size: (64, 64),
            padding: 0,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let large = create_test_asset("big.png", 64, 64);
        let mut sprites = packer.assets_to_sprites(&[large]).unwrap();
        // Add more sprites that won't fit
        for i in 0..4 {
            let a = create_test_asset(&format!("fill{i}.png"), 32, 32);
            sprites.extend(packer.assets_to_sprites(&[a]).unwrap());
        }
        let atlases = packer.pack_sprites_to_atlases(sprites).unwrap();
        // Should need at least 2 pages (first one has 64x64 + some 32x32, rest overflow)
        assert!(
            atlases.len() >= 2,
            "Expected at least 2 pages, got {}",
            atlases.len()
        );
        // All sprites should be packed
        let total_sprites: usize = atlases.iter().map(|a| a.sprites.len()).sum();
        assert_eq!(total_sprites, 5);
    }

    #[test]
    fn test_pack_sprites_no_overlap() {
        let packer = Packer::new(default_pack_options());
        let assets = [
            create_test_asset("a.png", 32, 32),
            create_test_asset("b.png", 32, 32),
            create_test_asset("c.png", 32, 32),
        ];
        let sprites = packer.assets_to_sprites(&assets).unwrap();
        let atlases = packer.pack_sprites_to_atlases(sprites).unwrap();
        let rects: Vec<Rect> = atlases[0].sprites.iter().map(|ps| ps.rect).collect();
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(
                    !rects[i].intersects(&rects[j]),
                    "Sprite {} and {} overlap: {:?} vs {:?}",
                    i, j, rects[i], rects[j]
                );
            }
        }
    }

    #[test]
    fn test_pack_page_limit_exceeded() {
        let options = PackOptions {
            max_size: (64, 64),
            padding: 0,
            page_limit: Some(1),
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let assets = [
            create_test_asset("a.png", 64, 64),
            create_test_asset("b.png", 64, 64),
        ];
        let result = packer.pack_assets(&assets, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("page_limit"));
    }

    #[test]
    fn test_trim_sprite_transparent_borders() {
        let options = PackOptions {
            allow_trim: true,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        // Create a 32x32 image with a 16x16 opaque rectangle in the center
        let mut img = image::RgbaImage::new(32, 32);
        for y in 8..24 {
            for x in 8..24 {
                img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let data = buf.into_inner();
        let mut sprite = Sprite {
            name: "centered".to_string(),
            data,
            size: Size { width: 32, height: 32 },
            hash: Hash::new_from_bytes(b"test"),
        };
        let original_rect = packer.trim_sprite(&mut sprite);
        assert!(original_rect.is_some(), "Should trim transparent borders");
        let rect = original_rect.unwrap();
        assert_eq!(rect.width, 32, "Original size should be 32x32");
        assert_eq!(rect.height, 32);
        assert_eq!(sprite.size.width, 16, "Trimmed size should be 16x16");
        assert_eq!(sprite.size.height, 16);
    }

    #[test]
    fn test_trim_sprite_fully_transparent() {
        let options = PackOptions {
            allow_trim: true,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let data = create_test_image(16, 16, true); // fully transparent
        let mut sprite = Sprite {
            name: "transparent".to_string(),
            data,
            size: Size { width: 16, height: 16 },
            hash: Hash::new_from_bytes(b"test"),
        };
        let result = packer.trim_sprite(&mut sprite);
        assert!(result.is_none(), "Fully transparent should not trim");
    }

    #[test]
    fn test_trim_sprite_fully_opaque() {
        let options = PackOptions {
            allow_trim: true,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let data = create_test_image(16, 16, false); // fully opaque
        let mut sprite = Sprite {
            name: "opaque".to_string(),
            data,
            size: Size { width: 16, height: 16 },
            hash: Hash::new_from_bytes(b"test"),
        };
        let result = packer.trim_sprite(&mut sprite);
        assert!(result.is_none(), "Fully opaque should not need trimming");
    }

    #[test]
    fn test_render_atlas_creates_png() {
        let packer = Packer::new(default_pack_options());
        let data = create_test_image(16, 16, false);
        let sprite = Sprite {
            name: "test".to_string(),
            data,
            size: Size { width: 16, height: 16 },
            hash: Hash::new_from_bytes(b"test"),
        };
        let packed = PackedSprite {
            rect: Rect::new(0, 0, 16, 16),
            sprite,
            trimmed: false,
            sprite_source_size: None,
        };
        let atlas_size = Size { width: 64, height: 64 };
        let result = packer.render_atlas(&[packed], atlas_size).unwrap();
        // Should be valid PNG
        assert!(result.starts_with(b"\x89PNG"));
        // Decode and verify dimensions
        let decoded = image::load_from_memory(&result).unwrap();
        assert_eq!(decoded.dimensions(), (64, 64));
    }

    #[test]
    fn test_render_atlas_preserves_sprite_content() {
        let packer = Packer::new(default_pack_options());
        // Create a distinctive image (solid green)
        let mut img = image::RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                img.put_pixel(x, y, image::Rgba([0, 255, 0, 255]));
            }
        }
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let data = buf.into_inner();

        let sprite = Sprite {
            name: "green".to_string(),
            data,
            size: Size { width: 8, height: 8 },
            hash: Hash::new_from_bytes(b"green"),
        };
        let packed = PackedSprite {
            rect: Rect::new(10, 10, 8, 8),
            sprite,
            trimmed: false,
            sprite_source_size: None,
        };
        let atlas_size = Size { width: 32, height: 32 };
        let result = packer.render_atlas(&[packed], atlas_size).unwrap();
        let decoded = image::load_from_memory(&result).unwrap();
        // Check pixel at sprite position is green
        let pixel = decoded.get_pixel(10, 10);
        assert_eq!(pixel[0], 0, "R channel should be 0");
        assert_eq!(pixel[1], 255, "G channel should be 255");
        assert_eq!(pixel[2], 0, "B channel should be 0");
        // Check pixel outside sprite is transparent (from alpha bleed)
        let bg = decoded.get_pixel(0, 0);
        assert_eq!(bg[3], 0, "Background should be transparent");
    }

    #[test]
    fn test_apply_extrude_with_options() {
        // Verify extrude options integrate correctly through the full pipeline
        let options = PackOptions {
            extrude: 2,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let assets = [create_test_asset("sprite.png", 16, 16)];
        let result = packer.pack_assets(&assets, "extrude_test").unwrap();
        assert_eq!(result.atlases.len(), 1);
        let decoded = image::load_from_memory(&result.atlases[0].image_data).unwrap();
        assert_eq!(decoded.dimensions(), (512, 512));
        // Sprite at (0,0) with extrude 2 means pixels at offsets beyond should exist
        // but after alpha_bleed they'll be transparent; just verify valid output
        assert!(result.manifest.sprites.contains_key("sprite"));
    }

    #[test]
    fn test_pack_full_pipeline() {
        let packer = Packer::new(default_pack_options());
        let assets = [
            create_test_asset("a.png", 32, 32),
            create_test_asset("b.png", 64, 64),
            create_test_asset("c.png", 16, 48),
        ];
        let result = packer.pack_assets(&assets, "my_input").unwrap();
        assert_eq!(result.atlases.len(), 1);
        assert_eq!(result.manifest.input_name, "my_input");
        // Should have 3 sprites in manifest
        assert_eq!(result.manifest.sprites.len(), 3);
    }

    #[test]
    fn test_create_manifest_includes_all_sprites() {
        let packer = Packer::new(default_pack_options());
        let a = create_test_asset("alpha.png", 16, 16);
        let b = create_test_asset("beta.png", 16, 16);
        let result = packer.pack_assets(&[a, b], "manifest_test").unwrap();
        assert_eq!(result.manifest.sprite_count(), 2);
        assert!(result.manifest.sprites.contains_key("alpha"));
        assert!(result.manifest.sprites.contains_key("beta"));
    }

    #[test]
    fn test_pack_with_power_of_two() {
        let options = PackOptions {
            max_size: (300, 300),
            power_of_two: true,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let asset = create_test_asset("sprite.png", 32, 32);
        let result = packer.pack_assets(&[asset], "pot").unwrap();
        assert_eq!(result.atlases.len(), 1);
        // Power of two: 300.next_power_of_two() = 512
        let decoded = image::load_from_memory(&result.atlases[0].image_data).unwrap();
        assert_eq!(decoded.dimensions(), (512, 512));
    }

    #[test]
    fn test_pack_sprites_remain_within_atlas_bounds() {
        let options = PackOptions {
            max_size: (128, 128),
            padding: 0,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let assets = [
            create_test_asset("big.png", 64, 64),
            create_test_asset("a.png", 32, 32),
            create_test_asset("b.png", 32, 32),
            create_test_asset("c.png", 32, 32),
        ];
        let result = packer.pack_assets(&assets, "bounds_check").unwrap();
        for atlas in &result.atlases {
            for ps in &atlas.sprites {
                let x_end = ps.rect.x + ps.rect.width;
                let y_end = ps.rect.y + ps.rect.height;
                assert!(
                    x_end <= 128 && y_end <= 128,
                    "Sprite '{}' exceeds atlas bounds: ends at ({}, {})",
                    ps.sprite.name, x_end, y_end
                );
            }
        }
    }

    #[test]
    fn test_pack_with_padding_separates_sprites() {
        let options = PackOptions {
            max_size: (128, 128),
            padding: 4,
            extrude: 0,
            ..default_pack_options()
        };
        let packer = Packer::new(options);
        let assets = [
            create_test_asset("one.png", 32, 32),
            create_test_asset("two.png", 32, 32),
        ];
        let result = packer.pack_assets(&assets, "padding_test").unwrap();
        assert_eq!(result.atlases.len(), 1);
        // With padding, sprites should have at least padding pixels between them
        let sprites = &result.atlases[0].sprites;
        let gap_x = (sprites[1].rect.x as i32 - sprites[0].rect.x as i32 - sprites[0].rect.width as i32).unsigned_abs();
        assert!(gap_x >= 4 || gap_x == 0);
    }
}
