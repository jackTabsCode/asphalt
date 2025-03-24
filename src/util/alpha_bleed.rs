//! Changes pixels in an image that are totally transparent to the color of
//! their nearest non-transparent neighbor. This fixes artifacting when images
//! are resized in some contexts.

use std::collections::VecDeque;

use bit_vec::BitVec;
use image::{DynamicImage, GenericImage, GenericImageView, Rgba};

pub fn alpha_bleed(img: &mut DynamicImage) {
    let (w, h) = img.dimensions();

    // Tells whether a given position has been touched by the bleeding algorithm
    // yet and is safe to sample colors from. In the first pass, we'll set all
    // pixels that aren't totally transparent since this algorithm won't mutate
    // them.
    let mut can_be_sampled = Mask2::new(w, h);

    // The set of images that we've already visited and don't need to queue if
    // traversed again.
    let mut visited = Mask2::new(w, h);

    // A queue of pixels to blend with surrounding pixels with next.
    //
    // Populated initially with all pixels that border opaque pixels. We'll use
    // it to blend outwards from each opaque pixel breadth-first.
    let mut to_visit = VecDeque::new();

    // An iterator of in-bounds positions adjacent to the given one.
    let adjacent_positions = |x, y| {
        DIRECTIONS.iter().filter_map(move |(x_offset, y_offset)| {
            let x_source = (x as i32) + x_offset;
            let y_source = (y as i32) + y_offset;

            if x_source < 0 || y_source < 0 || x_source >= w as i32 || y_source >= h as i32 {
                return None;
            }

            Some((x_source as u32, y_source as u32))
        })
    };

    // Populate the set of initial positions to visit as well as positions that
    // are valid to sample from.
    for y in 0..h {
        for x in 0..w {
            let pixel = img.get_pixel(x, y);

            if pixel[3] != 0 {
                // This pixel is not totally transparent, so we don't need to
                // modify it. We'll add it to the `can_be_sampled` set to
                // indicate it's okay to sample from this pixel.
                can_be_sampled.set(x, y);
                visited.set(x, y);
                continue;
            }

            // Check if any adjacent pixels have non-zero alpha.
            let borders_opaque = adjacent_positions(x, y).any(|(x_source, y_source)| {
                let source = img.get_pixel(x_source, y_source);
                source[3] != 0
            });

            if borders_opaque {
                // This pixel is totally transparent, but borders at least one
                // opaque pixel. We'll add it to the initial set of positions to
                // visit.
                visited.set(x, y);
                to_visit.push_back((x, y));
            }
        }
    }

    loop {
        let queue_length = to_visit.len();
        if queue_length == 0 {
            break;
        }

        let mut mutated_coords: Vec<(u32, u32)> = vec![(0, 0); queue_length];
        for _ in 0..queue_length {
            if let Some((x, y)) = to_visit.pop_front() {
                // Compute the average color from all surrounding pixels that are
                // eligible to be sampled from.
                let mut new_color = (0, 0, 0);
                let mut contributing = 0;

                for (x_source, y_source) in adjacent_positions(x, y) {
                    if can_be_sampled.get(x_source, y_source) {
                        let source = img.get_pixel(x_source, y_source);

                        contributing += 1;
                        new_color.0 += source[0] as u16;
                        new_color.1 += source[1] as u16;
                        new_color.2 += source[2] as u16;
                    } else if !visited.get(x_source, y_source) {
                        visited.set(x_source, y_source);
                        to_visit.push_back((x_source, y_source));
                    }
                }

                let denominator = u16::max(1, contributing);
                let pixel = Rgba([
                    (new_color.0 / denominator) as u8,
                    (new_color.1 / denominator) as u8,
                    (new_color.2 / denominator) as u8,
                    0,
                ]);

                img.put_pixel(x, y, pixel);
                mutated_coords.push((x, y));
            }
        }

        for _ in 0..queue_length {
            if let Some((x, y)) = mutated_coords.pop() {
                // Now that we've bled this pixel, it's eligible to be sampled from for
                // future iterations.
                can_be_sampled.set(x, y);
            }
        }
    }
}

const DIRECTIONS: &[(i32, i32)] = &[
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

struct Mask2 {
    size: (u32, u32),
    data: BitVec,
}

impl Mask2 {
    fn new(w: u32, h: u32) -> Self {
        Self {
            size: (w, h),
            data: BitVec::from_elem((w * h) as usize, false),
        }
    }

    fn get(&self, x: u32, y: u32) -> bool {
        let index = x + y * self.size.0;
        self.data.get(index as usize).unwrap_or(false)
    }

    fn set(&mut self, x: u32, y: u32) {
        let index = x + y * self.size.0;
        self.data.set(index as usize, true);
    }
}
