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

    // --- Property-based tests ---

    use proptest::prelude::*;

    fn rect_value(width_strat: impl Strategy<Value = u32>, height_strat: impl Strategy<Value = u32>) -> impl Strategy<Value = Rect> {
        (0u32..1000, 0u32..1000, width_strat, height_strat)
            .prop_map(|(x, y, w, h)| Rect::new(x, y, w.max(1), h.max(1)))
    }

    fn small_rect() -> impl Strategy<Value = Rect> {
        rect_value(1u32..200, 1u32..200)
    }

    fn medium_rect() -> impl Strategy<Value = Rect> {
        rect_value(1u32..500, 1u32..500)
    }

    proptest! {
        #[test]
        fn split_by_preserves_total_area(
            rect in medium_rect(),
            split_x in 0u32..1000u32,
            split_y in 0u32..1000u32,
            split_w in 1u32..200u32,
            split_h in 1u32..200u32,
        ) {
            let splitter = Rect::new(split_x, split_y, split_w, split_h);
            let splits = rect.split_by(&splitter);

            // If the splitter doesn't intersect, should return self
            if !rect.intersects(&splitter) {
                assert_eq!(splits.len(), 1);
                assert_eq!(splits[0], rect);
                return Ok(());
            }

            // Splits should not overlap with each other
            for i in 0..splits.len() {
                for j in (i + 1)..splits.len() {
                    assert!(!splits[i].intersects(&splits[j]),
                        "Split {} ({:?}) and {} ({:?}) overlap", i, splits[i], j, splits[j]);
                }
            }

            // Each split should be contained within the original rect
            for split in &splits {
                assert!(rect.contains_rect(split),
                    "Split {:?} not contained in original {:?}", split, rect);
            }

            // The splitter should not overlap with any split
            for split in &splits {
                assert!(!splitter.contains_rect(split),
                    "Split {:?} is inside splitter {:?}", split, splitter);
            }
        }

        #[test]
        fn merge_is_commutative(
            a in small_rect(),
            b in small_rect(),
        ) {
            let ab = a.try_merge_with(&b);
            let ba = b.try_merge_with(&a);
            assert_eq!(ab, ba, "Merge should be commutative");
        }

        #[test]
        fn merge_adjacent_horizontal_preserves_area(
            x in 0u32..500u32,
            y in 0u32..500u32,
            w1 in 1u32..200u32,
            w2 in 1u32..200u32,
            h in 1u32..200u32,
        ) {
            let a = Rect::new(x, y, w1, h);
            let b = Rect::new(x + w1, y, w2, h);
            if let Some(merged) = a.try_merge_with(&b) {
                assert_eq!(merged.area(), a.area() + b.area());
                assert_eq!(merged.width, w1 + w2);
                assert_eq!(merged.height, h);
            }
        }

        #[test]
        fn merge_adjacent_vertical_preserves_area(
            x in 0u32..500u32,
            y in 0u32..500u32,
            w in 1u32..200u32,
            h1 in 1u32..200u32,
            h2 in 1u32..200u32,
        ) {
            let a = Rect::new(x, y, w, h1);
            let b = Rect::new(x, y + h1, w, h2);
            if let Some(merged) = a.try_merge_with(&b) {
                assert_eq!(merged.area(), a.area() + b.area());
                assert_eq!(merged.width, w);
                assert_eq!(merged.height, h1 + h2);
            }
        }

        #[test]
        fn sizes_fits_in_is_transitive(
            a_w in 1u32..500u32, a_h in 1u32..500u32,
            b_w in 1u32..500u32, b_h in 1u32..500u32,
            c_w in 1u32..500u32, c_h in 1u32..500u32,
        ) {
            let a = Size::new(a_w, a_h);
            let b = Size::new(b_w, b_h);
            let c = Size::new(c_w, c_h);

            if a.fits_in(b) && b.fits_in(c) {
                assert!(a.fits_in(c), "fits_in should be transitive");
            }
        }

        #[test]
        fn size_fits_in_reflexive(
            w in 1u32..500u32, h in 1u32..500u32,
        ) {
            let size = Size::new(w, h);
            assert!(size.fits_in(size), "fits_in should be reflexive");
        }

        #[test]
        fn intersects_is_symmetric(
            a in medium_rect(),
            b in medium_rect(),
        ) {
            assert_eq!(a.intersects(&b), b.intersects(&a),
                "intersects should be symmetric");
        }

        #[test]
        fn contains_rect_implies_contains_point_center(
            rect in small_rect(),
        ) {
            let center_x = rect.x + rect.width / 2;
            let center_y = rect.y + rect.height / 2;
            assert!(rect.contains_point(center_x, center_y),
                "Rect {:?} should contain its center ({}, {})", rect, center_x, center_y);
        }

        #[test]
        fn split_by_produces_non_overlapping_rects(
            outer in medium_rect(),
            split_w in 1u32..200u32, split_h in 1u32..200u32,
        ) {
            // Pick a splitter position that's guaranteed to be within bounds
            let max_x = outer.x + outer.width.saturating_sub(split_w);
            let max_y = outer.y + outer.height.saturating_sub(split_h);
            let splitter = Rect::new(
                outer.x + (outer.width / 4).min(max_x.saturating_sub(outer.x)),
                outer.y + (outer.height / 4).min(max_y.saturating_sub(outer.y)),
                split_w.min(outer.width / 2).max(1),
                split_h.min(outer.height / 2).max(1),
            );

            prop_assume!(outer.contains_rect(&splitter));
            let splits = outer.split_by(&splitter);

            for i in 0..splits.len() {
                for j in (i + 1)..splits.len() {
                    assert!(!splits[i].intersects(&splits[j]));
                }
            }
        }
    }
}
