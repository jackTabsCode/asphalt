use crate::pack::rect::{Rect, Size};
use std::collections::VecDeque;

/// MaxRects algorithm implementation for rectangle packing
///
/// This implements the MaxRects algorithm with Best Area Fit heuristic
/// for deterministic and efficient rectangle packing.
pub struct MaxRectsPacker {
    bin_size: Size,
    free_rects: VecDeque<Rect>,
    used_area: u32,
}

impl MaxRectsPacker {
    /// Create a new MaxRects packer with the given bin size
    pub fn new(bin_size: Size) -> Self {
        let mut free_rects = VecDeque::new();
        free_rects.push_back(Rect::from_size(bin_size));

        Self {
            bin_size,
            free_rects,
            used_area: 0,
        }
    }

    /// Try to pack a rectangle of the given size
    /// Returns Some(Rect) with the position if successful, None if it doesn't fit
    pub fn pack(&mut self, size: Size) -> Option<Rect> {
        if size.width > self.bin_size.width || size.height > self.bin_size.height {
            return None;
        }

        let best_rect = self.find_best_position(size)?;
        self.place_rect(best_rect);
        self.used_area += best_rect.area();
        Some(best_rect)
    }

    /// Find the best position for a rectangle using Best Area Fit heuristic
    fn find_best_position(&self, size: Size) -> Option<Rect> {
        let mut best_rect = None;
        let mut best_area_fit = u32::MAX;
        let mut best_short_side_fit = u32::MAX;

        for free_rect in &self.free_rects {
            if size.fits_in(free_rect.size()) {
                let area_fit = free_rect.area() - size.area();
                let leftover_horizontal = free_rect.width - size.width;
                let leftover_vertical = free_rect.height - size.height;
                let short_side_fit = leftover_horizontal.min(leftover_vertical);

                // Best Area Fit with Short Side Fit as tie-breaker
                if area_fit < best_area_fit
                    || (area_fit == best_area_fit && short_side_fit < best_short_side_fit)
                {
                    best_rect = Some(Rect::new(free_rect.x, free_rect.y, size.width, size.height));
                    best_area_fit = area_fit;
                    best_short_side_fit = short_side_fit;
                }
            }
        }

        best_rect
    }

    /// Place a rectangle and update the free rectangle list
    fn place_rect(&mut self, placed_rect: Rect) {
        let mut new_free_rects = VecDeque::new();

        // Split all intersecting free rectangles
        for free_rect in &self.free_rects {
            if free_rect.intersects(&placed_rect) {
                let splits = free_rect.split_by(&placed_rect);
                for split in splits {
                    // Only add non-degenerate rectangles
                    if split.width > 0 && split.height > 0 {
                        new_free_rects.push_back(split);
                    }
                }
            } else {
                // Keep non-intersecting rectangles as-is
                new_free_rects.push_back(*free_rect);
            }
        }

        self.free_rects = new_free_rects;
        self.remove_redundant_rects();
        self.coalesce_adjacent_rects();
    }

    /// Remove rectangles that are completely contained within other rectangles
    fn remove_redundant_rects(&mut self) {
        let mut i = 0;
        while i < self.free_rects.len() {
            let rect_i = self.free_rects[i];
            let mut is_redundant = false;

            for j in 0..self.free_rects.len() {
                if i != j {
                    let rect_j = self.free_rects[j];
                    if rect_j.contains_rect(&rect_i) {
                        is_redundant = true;
                        break;
                    }
                }
            }

            if is_redundant {
                self.free_rects.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Coalesce adjacent rectangles to reduce fragmentation
    fn coalesce_adjacent_rects(&mut self) {
        let mut changed = true;
        while changed {
            changed = false;

            for i in 0..self.free_rects.len() {
                for j in (i + 1)..self.free_rects.len() {
                    let rect_i = self.free_rects[i];
                    let rect_j = self.free_rects[j];

                    if let Some(merged) = rect_i.try_merge_with(&rect_j) {
                        // Remove the two rectangles and add the merged one
                        self.free_rects.remove(j);
                        self.free_rects.remove(i);
                        self.free_rects.push_back(merged);
                        changed = true;
                        break;
                    }
                }
                if changed {
                    break;
                }
            }
        }
    }

    /// Get the current number of free rectangles (for testing/debugging)
    #[allow(dead_code)]
    pub fn free_rect_count(&self) -> usize {
        self.free_rects.len()
    }

    /// Calculate the total free area remaining
    #[allow(dead_code)]
    pub fn free_area(&self) -> u32 {
        self.free_rects.iter().map(|r| r.area()).sum()
    }

    /// Calculate the occupancy ratio (0.0 to 1.0)
    #[allow(dead_code)]
    pub fn occupancy(&self) -> f64 {
        let total_area = self.bin_size.area() as f64;
        self.used_area as f64 / total_area
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_packing() {
        let mut packer = MaxRectsPacker::new(Size::new(512, 512));

        // Pack first rectangle
        let rect1 = packer.pack(Size::new(100, 100));
        assert!(rect1.is_some());
        let rect1 = rect1.unwrap();
        assert_eq!(rect1.x, 0);
        assert_eq!(rect1.y, 0);
        assert_eq!(rect1.width, 100);
        assert_eq!(rect1.height, 100);

        // Pack second rectangle
        let rect2 = packer.pack(Size::new(50, 50));
        assert!(rect2.is_some());
        let rect2 = rect2.unwrap();

        // Should not overlap with first rectangle
        assert!(!rect1.intersects(&rect2));
    }

    #[test]
    fn test_oversized_rectangle() {
        let mut packer = MaxRectsPacker::new(Size::new(100, 100));

        // Try to pack a rectangle larger than the bin
        let result = packer.pack(Size::new(200, 50));
        assert!(result.is_none());
    }

    #[test]
    fn test_no_space_remaining() {
        let mut packer = MaxRectsPacker::new(Size::new(100, 100));

        // Fill the entire space
        let result = packer.pack(Size::new(100, 100));
        assert!(result.is_some());

        // Try to pack another rectangle
        let result = packer.pack(Size::new(10, 10));
        assert!(result.is_none());
    }

    #[test]
    fn test_multiple_rectangles() {
        let mut packer = MaxRectsPacker::new(Size::new(512, 512));
        let mut packed_rects = Vec::new();

        // Pack multiple rectangles
        for i in 0..10 {
            let size = Size::new(50 + i * 10, 50 + i * 5);
            if let Some(rect) = packer.pack(size) {
                packed_rects.push(rect);
            }
        }

        // Verify no overlaps
        for i in 0..packed_rects.len() {
            for j in (i + 1)..packed_rects.len() {
                assert!(
                    !packed_rects[i].intersects(&packed_rects[j]),
                    "Rectangles {} and {} overlap",
                    i,
                    j
                );
            }
        }

        // Verify all rectangles fit within the bin
        let bin_rect = Rect::from_size(Size::new(512, 512));
        for rect in &packed_rects {
            assert!(
                bin_rect.contains_rect(rect),
                "Rectangle {:?} doesn't fit in bin",
                rect
            );
        }
    }

    #[test]
    fn test_occupancy_calculation() {
        let mut packer = MaxRectsPacker::new(Size::new(100, 100));

        // Initial occupancy should be 0
        assert_eq!(packer.occupancy(), 0.0);

        // Pack a rectangle that takes up 25% of the space
        packer.pack(Size::new(50, 50));
        assert_eq!(packer.occupancy(), 0.25);

        // Pack another rectangle that takes up another 25%
        packer.pack(Size::new(50, 50));
        assert_eq!(packer.occupancy(), 0.5);
    }

    #[test]
    fn test_deterministic_packing() {
        // Same input should produce same output
        let sizes = vec![
            Size::new(64, 64),
            Size::new(32, 32),
            Size::new(128, 64),
            Size::new(16, 16),
        ];

        let mut results1 = Vec::new();
        let mut packer1 = MaxRectsPacker::new(Size::new(512, 512));
        for size in &sizes {
            results1.push(packer1.pack(*size));
        }

        let mut results2 = Vec::new();
        let mut packer2 = MaxRectsPacker::new(Size::new(512, 512));
        for size in &sizes {
            results2.push(packer2.pack(*size));
        }

        assert_eq!(results1, results2);
    }

    #[test]
    fn test_free_rect_management() {
        let mut packer = MaxRectsPacker::new(Size::new(100, 100));

        // Initially should have one free rectangle (the entire bin)
        assert_eq!(packer.free_rect_count(), 1);
        assert_eq!(packer.free_area(), 10000);

        // Pack a rectangle
        let result = packer.pack(Size::new(50, 50));
        assert!(result.is_some(), "Should be able to pack a 50x50 rectangle");

        // Should have reduced free area
        assert!(
            packer.free_area() < 10000,
            "Free area should be less than 10000, got {}",
            packer.free_area()
        );

        // Free rect count may vary depending on splits and coalescing
        assert!(packer.free_rect_count() > 0);
    }
}
