//! Toolbar icons: the original MadEdit XPM art (converted to PNG) plus a dark-theme variant.
//!
//! The art was drawn for a light gray Windows toolbar, so many icons are outlined or filled
//! in black and vanish on a dark panel. The dark variant lightens the black areas that form
//! the icon's silhouette: the outline and solid black bodies (the binoculars, the Replace
//! arrow). Dark strokes drawn on a light fill inside the icon, such as the text lines on a
//! page or the rim around the binocular lenses, keep their color because the light fill
//! already makes them visible.

use std::collections::VecDeque;

use image::RgbaImage;

/// Names of the icons in `assets/icons/`, embedded at compile time.
macro_rules! icon_table {
    ($($name:literal),* $(,)?) => {
        pub const ICONS: &[(&str, &[u8])] = &[
            $(($name, include_bytes!(concat!("../../assets/icons/", $name, ".png")))),*
        ];
    };
}

icon_table!(
    "new", "fileopen", "filesave", "saveall", "fileclose", "closeall", "print", "undo", "redo", "cut", "copy",
    "paste", "indent", "unindent", "comment", "uncomment", "find", "findnext", "findprev", "replace",
    "goposition", "nowrap", "wrapbywin", "wrapbycol", "textmode", "columnmode", "hexmode", "bookmark_toggle",
    "bookmark_next", "bookmark_prev", "bookmark_clear", "preview", "encoding", "syntax", "reload", "quit",
    "help", "down",
);

/// Pixels darker than this relative luminance have less than about 3:1 contrast against the
/// dark panel (gray 27), so they are lightened when they form the icon's silhouette.
const DARK_LUMINANCE: f32 = 0.12;

/// Opaque pixels at least this light count as a light fill that a dark stroke is drawn on.
const LIGHT_FILL_LUMINANCE: f32 = 0.35;

/// WCAG relative luminance of an sRGB color, 0 (black) to 1 (white).
pub fn luminance(r: u8, g: u8, b: u8) -> f32 {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn opaque(img: &RgbaImage, x: u32, y: u32) -> bool {
    img.get_pixel(x, y)[3] >= 128
}

fn is_dark(img: &RgbaImage, x: u32, y: u32) -> bool {
    let p = img.get_pixel(x, y);
    p[3] >= 128 && luminance(p[0], p[1], p[2]) < DARK_LUMINANCE
}

/// Dark pixels to lighten. Starting from dark pixels on the transparent outside, it walks
/// through connected dark pixels (8-neighbourhood, so thin diagonal strokes count) and marks
/// every one that touches the outside or has no light fill next to it. Dark pixels next to
/// a light fill (and not the outside) are strokes on that fill and stay dark; the walk still
/// passes through them so a black body behind them is reached.
fn silhouette_dark_pixels(img: &RgbaImage) -> Vec<bool> {
    let (w, h) = img.dimensions();
    let idx = |x: u32, y: u32| (y * w + x) as usize;
    let mut mark = vec![false; (w * h) as usize];
    let mut seen = vec![false; (w * h) as usize];
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
    let neighbours = |x: u32, y: u32| {
        let mut out = Vec::with_capacity(8);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if (dx, dy) != (0, 0) && nx >= 0 && ny >= 0 && (nx as u32) < w && (ny as u32) < h {
                    out.push((nx as u32, ny as u32));
                }
            }
        }
        out
    };
    let touches_outside = |x: u32, y: u32| {
        x == 0 || y == 0 || x == w - 1 || y == h - 1 || neighbours(x, y).into_iter().any(|(nx, ny)| !opaque(img, nx, ny))
    };
    let on_light_fill = |x: u32, y: u32| {
        neighbours(x, y).into_iter().any(|(nx, ny)| {
            let p = img.get_pixel(nx, ny);
            p[3] >= 128 && luminance(p[0], p[1], p[2]) >= LIGHT_FILL_LUMINANCE
        })
    };
    for y in 0..h {
        for x in 0..w {
            if is_dark(img, x, y) && touches_outside(x, y) {
                seen[idx(x, y)] = true;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        mark[idx(x, y)] = touches_outside(x, y) || !on_light_fill(x, y);
        for (nx, ny) in neighbours(x, y) {
            if !seen[idx(nx, ny)] && is_dark(img, nx, ny) {
                seen[idx(nx, ny)] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    mark
}

/// Lightens a dark color while keeping its hue: black becomes mid-light gray, dark blue
/// becomes light blue. It stays well below white so white highlights inside the icon (the
/// binocular lenses) still stand out from the lightened outline.
fn lighten(r: u8, g: u8, b: u8) -> [u8; 3] {
    let (h, s, l) = rgb_to_hsl(r, g, b);
    hsl_to_rgb(h, s, 0.74 - l * 0.4)
}

/// Dark-theme variant of an icon (see the module docs).
pub fn dark_variant(img: &RgbaImage) -> RgbaImage {
    let mark = silhouette_dark_pixels(img);
    let mut out = img.clone();
    for (i, p) in out.pixels_mut().enumerate() {
        if mark[i] {
            let [r, g, b] = lighten(p[0], p[1], p[2]);
            p[0] = r;
            p[1] = g;
            p[2] = b;
        }
    }
    out
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if s == 0.0 {
        return [to_u8(l); 3];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [to_u8(hue(h + 1.0 / 3.0)), to_u8(hue(h)), to_u8(hue(h - 1.0 / 3.0))]
}

/// Decodes an embedded icon.
pub fn decode(bytes: &[u8]) -> Option<RgbaImage> {
    image::load_from_memory(bytes).ok().map(|i| i.to_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Panel colors of egui's light and dark themes, which the toolbar sits on.
    const LIGHT_BG: [u8; 3] = [248, 248, 248];
    const DARK_BG: [u8; 3] = [27, 27, 27];

    fn contrast(a: f32, b: f32) -> f32 {
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Share of the icon's outline (opaque pixels next to transparency) that has a pixel
    /// standing out from the background by at least 2:1 within `radius` source pixels
    /// (2 per point). Below about half, the icon reads as a few loose dots.
    fn visible_outline(img: &RgbaImage, bg: [u8; 3], radius: u32) -> f32 {
        let (w, h) = img.dimensions();
        let bg_l = luminance(bg[0], bg[1], bg[2]);
        let stands_out = |x: u32, y: u32| {
            let p = img.get_pixel(x, y);
            p[3] >= 128 && contrast(luminance(p[0], p[1], p[2]), bg_l) >= 2.0
        };
        let (mut edge, mut visible) = (0, 0);
        for y in 0..h {
            for x in 0..w {
                if !opaque(img, x, y) {
                    continue;
                }
                let touches_outside = x == 0 || y == 0 || x == w - 1 || y == h - 1 || {
                    let n = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)];
                    n.iter().any(|&(nx, ny)| !opaque(img, nx, ny))
                };
                if touches_outside {
                    edge += 1;
                    let near = (x.saturating_sub(radius)..=(x + radius).min(w - 1))
                        .any(|nx| (y.saturating_sub(radius)..=(y + radius).min(h - 1)).any(|ny| stands_out(nx, ny)));
                    if near {
                        visible += 1;
                    }
                }
            }
        }
        if edge == 0 { 1.0 } else { visible as f32 / edge as f32 }
    }

    /// Share of the icon's opaque pixels that sit in a solid patch blending into the panel:
    /// the pixel and its whole 3x3 neighbourhood are opaque and below 1.5:1 contrast with
    /// `bg`. Such patches read as holes in the icon (black binocular bodies on dark), even
    /// when the outline is visible.
    fn hidden_mass(img: &RgbaImage, bg: [u8; 3]) -> f32 {
        let (w, h) = img.dimensions();
        let bg_l = luminance(bg[0], bg[1], bg[2]);
        let blends = |x: u32, y: u32| {
            let p = img.get_pixel(x, y);
            p[3] >= 128 && contrast(luminance(p[0], p[1], p[2]), bg_l) < 1.5
        };
        let (mut total, mut hidden) = (0, 0);
        for y in 0..h {
            for x in 0..w {
                if !opaque(img, x, y) {
                    continue;
                }
                total += 1;
                let inner = x > 0 && y > 0 && x < w - 1 && y < h - 1;
                if inner && (x - 1..=x + 1).all(|nx| (y - 1..=y + 1).all(|ny| blends(nx, ny))) {
                    hidden += 1;
                }
            }
        }
        if total == 0 { 0.0 } else { hidden as f32 / total as f32 }
    }

    /// The light theme shows the original art, whose bevelled icons (Indent, Comment) have a
    /// white rim around a dark frame, so a contrasting pixel within one point counts. The
    /// dark variant is held to the strict one-pixel check and must not keep solid black areas.
    #[test]
    fn test_every_icon_is_visible_on_both_themes() {
        let mut failures = Vec::new();
        for (name, bytes) in ICONS {
            let img = decode(bytes).expect("icon decodes");
            let dark_img = dark_variant(&img);
            let light = visible_outline(&img, LIGHT_BG, 2);
            let dark = visible_outline(&dark_img, DARK_BG, 1);
            let mass = hidden_mass(&dark_img, DARK_BG);
            if light < 0.6 || dark < 0.9 || mass > 0.02 {
                failures.push(format!("{name}: light {light:.2}, dark {dark:.2}, hidden on dark {mass:.2}"));
            }
        }
        assert!(failures.is_empty(), "icons that are hard to see:\n{}", failures.join("\n"));
    }

    /// The checks must catch the original problem: the black binoculars vanish on dark, both
    /// their outline and their solid bodies.
    #[test]
    fn test_original_find_icon_is_flagged_on_dark() {
        let (_, bytes) = ICONS.iter().find(|(n, _)| *n == "find").unwrap();
        let img = decode(bytes).unwrap();
        assert!(visible_outline(&img, DARK_BG, 1) < 0.5);
        assert!(hidden_mass(&img, DARK_BG) > 0.1, "{}", hidden_mass(&img, DARK_BG));
    }

    #[test]
    fn test_dark_variant_keeps_enclosed_details() {
        // A white 5x5 page with a black outline and one black "text" pixel in the middle.
        let mut img = RgbaImage::from_pixel(7, 7, image::Rgba([0, 0, 0, 0]));
        for y in 1..6 {
            for x in 1..6 {
                let edge = x == 1 || y == 1 || x == 5 || y == 5;
                img.put_pixel(x, y, if edge { image::Rgba([0, 0, 0, 255]) } else { image::Rgba([255, 255, 255, 255]) });
            }
        }
        img.put_pixel(3, 3, image::Rgba([0, 0, 0, 255]));
        let dark = dark_variant(&img);
        assert!(luminance(dark.get_pixel(1, 1)[0], dark.get_pixel(1, 1)[1], dark.get_pixel(1, 1)[2]) > 0.5, "outline is lightened");
        assert_eq!(dark.get_pixel(3, 3).0, [0, 0, 0, 255], "enclosed text pixel keeps its color");
    }

    /// Writes a 2x montage of every icon on the light panel, on the dark panel as drawn before,
    /// and on the dark panel with the dark variant: `MADEDIT_ICON_MONTAGE=out.png cargo test montage -- --ignored`.
    #[test]
    #[ignore]
    fn montage() {
        let out = std::env::var("MADEDIT_ICON_MONTAGE").unwrap_or_else(|_| "icons.png".into());
        let (scale, cell, per_row) = (2u32, 36u32, 13usize);
        let bands = ICONS.len().div_ceil(per_row) as u32;
        let mut sheet = RgbaImage::new(per_row as u32 * cell * scale, bands * 3 * cell * scale);
        let rows: [([u8; 3], bool); 3] = [(LIGHT_BG, false), (DARK_BG, false), (DARK_BG, true)];
        for (r, (bg, adapt)) in rows.iter().enumerate() {
            for (i, (_, bytes)) in ICONS.iter().enumerate() {
                let (col, row) = (i % per_row, (i / per_row) * 3 + r);
                let img = decode(bytes).unwrap();
                let img = if *adapt { dark_variant(&img) } else { img };
                for py in 0..cell * scale {
                    for px in 0..cell * scale {
                        let (ix, iy) = (px / scale, py / scale);
                        let mut c = image::Rgba([bg[0], bg[1], bg[2], 255]);
                        if (2..2 + img.width()).contains(&ix) && (2..2 + img.height()).contains(&iy) {
                            let p = img.get_pixel(ix - 2, iy - 2);
                            if p[3] >= 128 {
                                c = image::Rgba([p[0], p[1], p[2], 255]);
                            }
                        }
                        sheet.put_pixel(col as u32 * cell * scale + px, row as u32 * cell * scale + py, c);
                    }
                }
            }
        }
        sheet.save(out).unwrap();
    }
}
