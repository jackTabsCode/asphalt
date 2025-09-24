use serde::{Deserialize, Serialize};

/// A 2D size with width and height
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    #[allow(dead_code)]
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn area(&self) -> u32 {
        self.width * self.height
    }

    #[allow(dead_code)]
    pub fn max_side(&self) -> u32 {
        self.width.max(self.height)
    }

    #[allow(dead_code)]
    pub fn min_side(&self) -> u32 {
        self.width.min(self.height)
    }

    pub fn fits_in(&self, other: Size) -> bool {
        self.width <= other.width && self.height <= other.height
    }
}

/// A 2D rectangle with position and size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_size(size: Size) -> Self {
        Self {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }
    }

    pub fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    pub fn area(&self) -> u32 {
        self.width * self.height
    }

    pub fn right(&self) -> u32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }

    #[allow(dead_code)]
    pub fn contains_point(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Split this rectangle by removing the given rect from it.
    /// Returns up to 4 new rectangles representing the remaining space.
    /// Uses the Guillotine approach to avoid overlapping rectangles.
    pub fn split_by(&self, splitter: &Rect) -> Vec<Rect> {
        let mut result = Vec::new();

        // Only split if the splitter actually intersects
        if !self.intersects(splitter) {
            return vec![*self];
        }

        // If the splitter completely contains this rectangle, return empty
        if splitter.contains_rect(self) {
            return Vec::new();
        }

        // Calculate the intersection bounds
        let left = self.x.max(splitter.x);
        let right = self.right().min(splitter.right());
        let top = self.y.max(splitter.y);
        let bottom = self.bottom().min(splitter.bottom());

        // Left slice (everything to the left of the intersection)
        if self.x < left {
            result.push(Rect::new(self.x, self.y, left - self.x, self.height));
        }

        // Right slice (everything to the right of the intersection)
        if right < self.right() {
            result.push(Rect::new(right, self.y, self.right() - right, self.height));
        }

        // Top slice (everything above the intersection, but only in the middle area)
        if self.y < top {
            result.push(Rect::new(left, self.y, right - left, top - self.y));
        }

        // Bottom slice (everything below the intersection, but only in the middle area)
        if bottom < self.bottom() {
            result.push(Rect::new(
                left,
                bottom,
                right - left,
                self.bottom() - bottom,
            ));
        }

        result
    }

    /// Try to merge this rectangle with another adjacent rectangle.
    /// Returns Some(merged_rect) if they can be merged, None otherwise.
    pub fn try_merge_with(&self, other: &Rect) -> Option<Rect> {
        // Check if rectangles are adjacent and can be merged

        // Horizontal merge (same height and y, adjacent x)
        if self.y == other.y && self.height == other.height {
            if self.right() == other.x {
                return Some(Rect::new(
                    self.x,
                    self.y,
                    self.width + other.width,
                    self.height,
                ));
            }
            if other.right() == self.x {
                return Some(Rect::new(
                    other.x,
                    self.y,
                    self.width + other.width,
                    self.height,
                ));
            }
        }

        // Vertical merge (same width and x, adjacent y)
        if self.x == other.x && self.width == other.width {
            if self.bottom() == other.y {
                return Some(Rect::new(
                    self.x,
                    self.y,
                    self.width,
                    self.height + other.height,
                ));
            }
            if other.bottom() == self.y {
                return Some(Rect::new(
                    self.x,
                    other.y,
                    self.width,
                    self.height + other.height,
                ));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_creation() {
        let size = Size::new(100, 200);
        assert_eq!(size.width, 100);
        assert_eq!(size.height, 200);
        assert_eq!(size.area(), 20000);
        assert_eq!(size.max_side(), 200);
        assert_eq!(size.min_side(), 100);
    }

    #[test]
    fn test_size_fits_in() {
        let small = Size::new(50, 75);
        let large = Size::new(100, 100);
        assert!(small.fits_in(large));
        assert!(!large.fits_in(small));
    }

    #[test]
    fn test_rect_creation() {
        let rect = Rect::new(10, 20, 100, 200);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 200);
        assert_eq!(rect.right(), 110);
        assert_eq!(rect.bottom(), 220);
        assert_eq!(rect.area(), 20000);
    }

    #[test]
    fn test_rect_contains_point() {
        let rect = Rect::new(10, 20, 100, 200);
        assert!(rect.contains_point(50, 100));
        assert!(rect.contains_point(10, 20)); // Top-left corner
        assert!(!rect.contains_point(110, 220)); // Bottom-right corner (exclusive)
        assert!(!rect.contains_point(5, 100)); // Outside left
        assert!(!rect.contains_point(50, 250)); // Outside bottom
    }

    #[test]
    fn test_rect_intersects() {
        let rect1 = Rect::new(0, 0, 100, 100);
        let rect2 = Rect::new(50, 50, 100, 100);
        let rect3 = Rect::new(200, 200, 100, 100);

        assert!(rect1.intersects(&rect2));
        assert!(rect2.intersects(&rect1));
        assert!(!rect1.intersects(&rect3));
        assert!(!rect3.intersects(&rect1));
    }

    #[test]
    fn test_rect_split_by() {
        let rect = Rect::new(0, 0, 100, 100);
        let splitter = Rect::new(25, 25, 50, 50);
        let splits = rect.split_by(&splitter);

        assert_eq!(splits.len(), 4);
        // Should have left, right, top, bottom slices (non-overlapping)
        assert!(splits.contains(&Rect::new(0, 0, 25, 100))); // Left
        assert!(splits.contains(&Rect::new(75, 0, 25, 100))); // Right
        assert!(splits.contains(&Rect::new(25, 0, 50, 25))); // Top (only middle part)
        assert!(splits.contains(&Rect::new(25, 75, 50, 25))); // Bottom (only middle part)
    }

    #[test]
    fn test_rect_split_corner() {
        // Test splitting when the splitter is at the corner
        let rect = Rect::new(0, 0, 100, 100);
        let splitter = Rect::new(0, 0, 50, 50);
        let splits = rect.split_by(&splitter);

        // Should create right and bottom slices
        assert_eq!(splits.len(), 2);
        assert!(splits.contains(&Rect::new(50, 0, 50, 100))); // Right slice
        assert!(splits.contains(&Rect::new(0, 50, 50, 50))); // Bottom slice

        // Calculate total area to ensure it's correct
        let total_split_area: u32 = splits.iter().map(|r| r.area()).sum();
        let expected_remaining = 100 * 100 - 50 * 50; // Total - splitter area
        assert_eq!(total_split_area, expected_remaining);
    }

    #[test]
    fn test_rect_merge_horizontal() {
        let rect1 = Rect::new(0, 0, 50, 100);
        let rect2 = Rect::new(50, 0, 50, 100);
        let merged = rect1.try_merge_with(&rect2);

        assert!(merged.is_some());
        assert_eq!(merged.unwrap(), Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn test_rect_merge_vertical() {
        let rect1 = Rect::new(0, 0, 100, 50);
        let rect2 = Rect::new(0, 50, 100, 50);
        let merged = rect1.try_merge_with(&rect2);

        assert!(merged.is_some());
        assert_eq!(merged.unwrap(), Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn test_rect_no_merge() {
        let rect1 = Rect::new(0, 0, 50, 50);
        let rect2 = Rect::new(100, 100, 50, 50); // Not adjacent
        let merged = rect1.try_merge_with(&rect2);

        assert!(merged.is_none());
    }
}
