//! Flood fill (paint bucket) and simple background removal.

use crate::color::Rgba;
use crate::geom::IRect;
use crate::image::{Image, Mask};

/// Pixels 4-connected to the seeds whose colour is within `tolerance` of `target`,
/// restricted to `area`.
pub fn region(img: &Image, seeds: &[(i32, i32)], target: Rgba, tolerance: u8, area: IRect) -> Mask {
    let area = area.clamp_to(img.width(), img.height());
    let mut mask = Mask::new(img.width(), img.height());
    let matches = |x: i32, y: i32, mask: &Mask| area.contains(x, y) && mask.get(x, y) == 0 && img.get(x, y).distance(target) <= tolerance;
    let mut stack: Vec<(i32, i32)> = seeds.iter().copied().filter(|&(x, y)| matches(x, y, &mask)).collect();
    while let Some((x, y)) = stack.pop() {
        if !matches(x, y, &mask) {
            continue;
        }
        let mut l = x;
        while matches(l - 1, y, &mask) {
            l -= 1;
        }
        let mut r = x;
        while matches(r + 1, y, &mask) {
            r += 1;
        }
        for i in l..=r {
            mask.set(i, y, 255);
        }
        for ny in [y - 1, y + 1] {
            let mut in_span = false;
            for i in l..=r {
                if matches(i, ny, &mask) {
                    if !in_span {
                        stack.push((i, ny));
                        in_span = true;
                    }
                } else {
                    in_span = false;
                }
            }
        }
    }
    mask
}

/// Paint-bucket fill at (x, y). Returns the modified rectangle (empty if nothing changed).
pub fn flood_fill(img: &mut Image, x: i32, y: i32, color: Rgba, tolerance: u8) -> IRect {
    if !img.in_bounds(x, y) {
        return IRect::default();
    }
    let target = img.get(x, y);
    if target == color && tolerance == 0 {
        return IRect::default();
    }
    let m = region(img, &[(x, y)], target, tolerance, img.bounds());
    let Some(bbox) = m.bbox() else { return IRect::default() };
    img.fill_masked(0, 0, &m, color);
    bbox
}

/// Makes the background transparent: the region connected to the border of `area`
/// whose colour matches the most frequent border colour.
pub fn remove_background(img: &mut Image, area: IRect, tolerance: u8) -> IRect {
    let area = area.clamp_to(img.width(), img.height());
    if area.is_empty() {
        return area;
    }
    let mut border = Vec::new();
    for x in area.x..area.right() {
        border.push((x, area.y));
        border.push((x, area.bottom() - 1));
    }
    for y in area.y..area.bottom() {
        border.push((area.x, y));
        border.push((area.right() - 1, y));
    }
    let mut counts: std::collections::HashMap<Rgba, usize> = Default::default();
    for &(x, y) in &border {
        *counts.entry(img.get(x, y)).or_default() += 1;
    }
    let Some((&target, _)) = counts.iter().max_by_key(|(c, n)| (**n, c.to_array())) else { return IRect::default() };
    if target.a == 0 {
        return IRect::default();
    }
    let m = region(img, &border, target, tolerance, area);
    let Some(bbox) = m.bbox() else { return IRect::default() };
    img.fill_masked(0, 0, &m, Rgba::TRANSPARENT);
    bbox
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed() -> Image {
        // White 10x10 with a black square outline at 2..=7.
        let mut img = Image::filled(10, 10, Rgba::WHITE);
        for i in 2..=7 {
            img.set(i, 2, Rgba::BLACK);
            img.set(i, 7, Rgba::BLACK);
            img.set(2, i, Rgba::BLACK);
            img.set(7, i, Rgba::BLACK);
        }
        img
    }

    #[test]
    fn fill_inside_stays_inside() {
        let mut img = framed();
        let r = flood_fill(&mut img, 4, 4, Rgba::rgb(255, 0, 0), 0);
        assert_eq!(r, IRect::new(3, 3, 4, 4));
        assert_eq!(img.get(5, 5), Rgba::rgb(255, 0, 0));
        assert_eq!(img.get(0, 0), Rgba::WHITE);
        assert_eq!(img.get(2, 2), Rgba::BLACK);
    }

    #[test]
    fn fill_outside_and_noop() {
        let mut img = framed();
        flood_fill(&mut img, 0, 0, Rgba::rgb(0, 0, 255), 0);
        assert_eq!(img.get(9, 9), Rgba::rgb(0, 0, 255));
        assert_eq!(img.get(4, 4), Rgba::WHITE);
        assert!(flood_fill(&mut img, 0, 0, Rgba::rgb(0, 0, 255), 0).is_empty());
        assert!(flood_fill(&mut img, -1, 0, Rgba::BLACK, 0).is_empty());
    }

    #[test]
    fn tolerance() {
        let mut img = Image::filled(4, 1, Rgba::rgb(100, 100, 100));
        img.set(2, 0, Rgba::rgb(110, 100, 100));
        img.set(3, 0, Rgba::rgb(100, 100, 100));
        let mut strict = img.clone();
        flood_fill(&mut strict, 0, 0, Rgba::BLACK, 0);
        assert_eq!(strict.get(3, 0), Rgba::rgb(100, 100, 100));
        flood_fill(&mut img, 0, 0, Rgba::BLACK, 10);
        assert_eq!(img.get(3, 0), Rgba::BLACK);
    }

    #[test]
    fn background_removal() {
        let mut img = framed();
        img.set(4, 4, Rgba::rgb(9, 9, 9));
        let r = remove_background(&mut img, IRect::new(0, 0, 10, 10), 16);
        assert_eq!(r, IRect::new(0, 0, 10, 10));
        assert_eq!(img.get(0, 0).a, 0);
        assert_eq!(img.get(2, 2), Rgba::BLACK);
        // The inside is not connected to the border: kept.
        assert_eq!(img.get(5, 5), Rgba::WHITE);
        // Nothing left to remove on a transparent border.
        assert!(remove_background(&mut img, IRect::new(0, 0, 10, 10), 16).is_empty());
    }
}
