use anyhow::Result;
use oxipng::Options;
use std::path::Path;

pub fn optimize_png(data: &[u8]) -> Result<Vec<u8>> {
    let options = Options::default();

    match oxipng::optimize_from_memory(data, &options) {
        Ok(optimized) => Ok(optimized),
        Err(_) => Ok(data.to_vec()),
    }
}

pub fn should_optimize(path: &Path, optimize_flag: bool) -> bool {
    if !optimize_flag {
        return false;
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use std::io::Cursor;

    /// Create a minimal valid 1x1 red PNG inline
    fn create_minimal_png() -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn test_optimize_png_valid_png() {
        let png_data = create_minimal_png();
        let result = optimize_png(&png_data).unwrap();
        // Output should be valid PNG bytes
        assert!(!result.is_empty());
        assert!(result.starts_with(b"\x89PNG"));
    }

    #[test]
    fn test_optimize_png_noop_on_invalid_data() {
        let data = b"this is not a png";
        let result = optimize_png(data).unwrap();
        // Should return the original data unchanged
        assert_eq!(result, data);
    }

    #[test]
    fn test_optimize_png_noop_on_empty() {
        let data = b"";
        let result = optimize_png(data).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_optimize_png_preserves_image() {
        let png_data = create_minimal_png();
        let result = optimize_png(&png_data).unwrap();
        // Both should decode as valid 1x1 images
        let original = image::load_from_memory(&png_data).unwrap();
        let optimized = image::load_from_memory(&result).unwrap();
        assert_eq!(original.dimensions(), optimized.dimensions());
        assert_eq!(original.get_pixel(0, 0), optimized.get_pixel(0, 0));
    }

    #[test]
    fn test_should_optimize_flag_off_returns_false() {
        let path = Path::new("image.png");
        assert!(!should_optimize(path, false));
    }

    #[test]
    fn test_should_optimize_png_extension() {
        let path = Path::new("image.png");
        assert!(should_optimize(path, true));
    }

    #[test]
    fn test_should_optimize_case_insensitive() {
        let path = Path::new("image.PNG");
        assert!(should_optimize(path, true));
        let path = Path::new("image.Png");
        assert!(should_optimize(path, true));
    }

    #[test]
    fn test_should_optimize_non_png_extension() {
        let path = Path::new("image.jpg");
        assert!(!should_optimize(path, true));
        let path = Path::new("image.jpeg");
        assert!(!should_optimize(path, true));
        let path = Path::new("image.svg");
        assert!(!should_optimize(path, true));
    }

    #[test]
    fn test_should_optimize_no_extension() {
        let path = Path::new("image");
        assert!(!should_optimize(path, true));
    }

    #[test]
    fn test_should_optimize_empty_extension() {
        let path = Path::new("image.");
        assert!(!should_optimize(path, true));
    }

    #[test]
    fn test_should_optimize_priority_flag_over_extension() {
        // Even with .png extension, if flag is off, return false
        let path = Path::new("image.png");
        assert!(!should_optimize(path, false));
    }
}
