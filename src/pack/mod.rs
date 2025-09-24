use crate::{
    asset::Asset,
    config::{PackOptions, PackSort},
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
    pub hash: String,
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
        if let Some(limit) = self.options.page_limit {
            if atlases.len() > limit as usize {
                bail!(
                    "Packing would require {} pages but limit is {}. Consider increasing max_size or page_limit.",
                    atlases.len(),
                    limit
                );
            }
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
                .with_context(|| format!("Failed to load image: {}", asset.path.display()))?;

            let size = Size {
                width: image.width(),
                height: image.height(),
            };

            let name = asset
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

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
                seen_hashes.insert(asset.hash.clone(), name.clone());
            }

            sprites.push(Sprite {
                name,
                data: asset.data.clone(),
                size,
                hash: asset.hash.clone(),
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
        use algorithm::MaxRectsPacker;

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

        let mut packer = MaxRectsPacker::new(atlas_size);
        let mut packed_sprites = Vec::new();
        let mut unpacked_sprites = Vec::new();

        for sprite in sprites {
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
                    trimmed: false, // TODO: Implement trimming
                    sprite_source_size: None,
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

    fn render_atlas(&self, packed_sprites: &[PackedSprite], atlas_size: Size) -> Result<Vec<u8>> {
        use image::{ImageBuffer, RgbaImage};
        use std::io::Cursor;

        let mut atlas_image: RgbaImage = ImageBuffer::new(atlas_size.width, atlas_size.height);

        log::debug!("Rendering atlas {}x{} with {} sprites", atlas_size.width, atlas_size.height, packed_sprites.len());

        for (i, packed_sprite) in packed_sprites.iter().enumerate() {
            log::debug!("Rendering sprite {} '{}' at ({}, {}) size {}x{}",
                i, packed_sprite.sprite.name,
                packed_sprite.rect.x, packed_sprite.rect.y,
                packed_sprite.rect.width, packed_sprite.rect.height);

            let sprite_image = image::load_from_memory(&packed_sprite.sprite.data)?;
            let sprite_rgba = sprite_image.to_rgba8();

            log::debug!("Loaded sprite image {}x{}", sprite_rgba.width(), sprite_rgba.height());

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

        // Encode as PNG
        let mut buffer = Cursor::new(Vec::new());
        atlas_image.write_to(&mut buffer, image::ImageFormat::Png)?;
        Ok(buffer.into_inner())
    }

    fn apply_extrude(
        &self,
        _atlas_image: &mut RgbaImage,
        _packed_sprite: &PackedSprite,
    ) -> Result<()> {
        // TODO: Implement edge extrusion for filtering
        // For now, we'll skip this to keep the initial implementation simple
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
