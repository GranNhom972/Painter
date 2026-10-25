//! Flood fill (paint bucket) and simple background removal.

use crate::color::Rgba;
use crate::geom::IRect;
use crate::image::{Image, Mask};

/// Pixels 4-connected to the seeds whose colour is within `tolerance` of `target`
/// (largest channel difference, alpha included), restricted to `area`.
/// Returns an image-sized mask and the bounding box of the region.
/// `allowed` (image-sized) restricts the region further (free-form selections).
pub fn region(img: &Image, seeds: &[(i32, i32)], target: Rgba, tolerance: u8, area: IRect, allowed: Option<&Mask>) -> (Mask, Option<IRect>) {
    let area = area.clamp_to(img.width(), img.height());
    let w = img.width() as usize;
    let data = img.data();
    let t = target.to_array();
    let mut seen = vec![0u8; w * img.height() as usize];
    let ok = |i: usize| {
        let p = &data[i * 4..i * 4 + 4];
        p[0].abs_diff(t[0]) <= tolerance
            && p[1].abs_diff(t[1]) <= tolerance
            && p[2].abs_diff(t[2]) <= tolerance
            && p[3].abs_diff(t[3]) <= tolerance
            && allowed.is_none_or(|m| m.data()[i] >= 128)
    };
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let mut stack: Vec<(i32, i32)> = seeds.iter().copied().filter(|&(x, y)| area.contains(x, y)).collect();
    while let Some((x, y)) = stack.pop() {
        let row = y as usize * w;
        if seen[row + x as usize] != 0 || !ok(row + x as usize) {
            continue;
        }
        let mut l = x;
        while l > area.x && seen[row + l as usize - 1] == 0 && ok(row + l as usize - 1) {
            l -= 1;
        }
        let mut r = x;
        while r + 1 < area.right() && seen[row + r as usize + 1] == 0 && ok(row + r as usize + 1) {
            r += 1;
        }
        seen[row + l as usize..=row + r as usize].fill(255);
        x0 = x0.min(l);
        x1 = x1.max(r);
        y0 = y0.min(y);
        y1 = y1.max(y);
        for ny in [y - 1, y + 1] {
            if ny < area.y || ny >= area.bottom() {
                continue;
            }
            let nrow = ny as usize * w;
            let mut in_span = false;
            for i in l..=r {
                let k = nrow + i as usize;
                if seen[k] == 0 && ok(k) {
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
    let bbox = (x0 <= x1).then(|| IRect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1));
    (Mask::from_raw(img.width(), img.height(), seen), bbox)
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
    let (m, bbox) = region(img, &[(x, y)], target, tolerance, img.bounds(), None);
    let Some(bbox) = bbox else { return IRect::default() };
    img.fill_masked_rect(&m, bbox, color);
    bbox
}

/// Makes the background transparent: the region connected to the border of `area`
/// whose colour matches the most frequent border colour.
pub fn remove_background(img: &mut Image, area: IRect, tolerance: u8, allowed: Option<&Mask>) -> IRect {
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
    let w = img.width() as usize;
    border.retain(|&(x, y)| allowed.is_none_or(|m| m.data()[y as usize * w + x as usize] >= 128));
    for &(x, y) in &border {
        *counts.entry(img.get(x, y)).or_default() += 1;
    }
    let Some((&target, _)) = counts.iter().max_by_key(|(c, n)| (**n, c.to_array())) else { return IRect::default() };
    if target.a == 0 {
        return IRect::default();
    }
    let (m, bbox) = region(img, &border, target, tolerance, area, allowed);
    let Some(bbox) = bbox else { return IRect::default() };
    img.fill_masked_rect(&m, bbox, Rgba::TRANSPARENT);
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
        let r = remove_background(&mut img, IRect::new(0, 0, 10, 10), 16, None);
        assert_eq!(r, IRect::new(0, 0, 10, 10));
        assert_eq!(img.get(0, 0).a, 0);
        assert_eq!(img.get(2, 2), Rgba::BLACK);
        // The inside is not connected to the border: kept.
        assert_eq!(img.get(5, 5), Rgba::WHITE);
        // Nothing left to remove on a transparent border.
        assert!(remove_background(&mut img, IRect::new(0, 0, 10, 10), 16, None).is_empty());
    }
}
