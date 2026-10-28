//! LaTeX math (`$…$` and `$$…$$`) in the Markdown preview, laid out by latex-rust.
//!
//! latex-rust turns a formula into an SVG of glyph outlines from its bundled STIX Two Math
//! font; the SVG goes through the same loader as Mermaid diagrams. The math font has no CJK
//! glyphs and latex-rust has no font fallback, so a top-level `\text{…}` holding characters
//! outside ASCII is cut out of the formula and drawn as ordinary egui text between the
//! math pieces. A formula that still fails is shown as its TeX source.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use egui::{RichText, TextStyle, Ui};
use latex_rust::{Color, Dim, MathFont, MathStyle, SvgOptions};

use super::mdpreview::DIAGRAM_URI_PREFIX;

/// Em size the SVG is laid out at; the preview scales it to its own body text size.
const EM_PT: i64 = 16;

enum Part {
    /// Width, height above the baseline and depth below it are in em.
    Math { uri: String, width: f32, height: f32, depth: f32 },
    Text(String),
}

/// (TeX, display style, text colour)
type MathKey = (String, bool, [u8; 3]);

/// Rendered formulas. `None` means the formula could not be rendered and its source is
/// shown instead.
#[derive(Default)]
pub struct MathCache {
    rendered: RefCell<HashMap<MathKey, Option<Vec<Part>>>>,
}

fn font() -> Option<&'static MathFont> {
    static FONT: OnceLock<Option<MathFont>> = OnceLock::new();
    FONT.get_or_init(|| MathFont::stix_two_math().ok()).as_ref()
}

/// Layout dimensions are already in em.
fn em(d: &Dim) -> f32 {
    f32::from_bits(d.to_ieee32_bits())
}

/// Index just past the `}` that closes the group whose body starts at `start`.
fn group_end(tex: &str, start: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut chars = tex[start..].char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split a formula into math pieces and the text of top-level `\text{…}` groups that hold
/// non-ASCII characters. `true` marks text. Math next to a cut keeps an empty group `{}`
/// on that side, so a binary operator such as `-` keeps its spacing.
fn split_text_groups(tex: &str) -> Vec<(bool, String)> {
    const TEXT: &str = "\\text{";
    let mut parts: Vec<(bool, String)> = Vec::new();
    let (mut depth, mut from, mut i) = (0usize, 0usize, 0usize);
    let bytes = tex.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if depth == 0 && tex[i..].starts_with(TEXT) => {
                let body = i + TEXT.len();
                let Some(end) = group_end(tex, body) else { break };
                let text = &tex[body..end - 1];
                if !text.is_ascii() {
                    parts.push((false, tex[from..i].to_string()));
                    parts.push((true, text.to_string()));
                    from = end;
                }
                i = end;
                continue;
            }
            b'\\' => i += 1,
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    parts.push((false, tex[from..].to_string()));
    let n = parts.len();
    for (k, (text, s)) in parts.iter_mut().enumerate() {
        if !*text {
            if k > 0 {
                s.insert_str(0, "{}");
            }
            if k + 1 < n {
                s.push_str("{}");
            }
        }
    }
    parts.retain(|(text, s)| *text || !s.trim().trim_matches(['{', '}']).trim().is_empty());
    parts
}

fn render_math(ctx: &egui::Context, tex: &str, display: bool, color: [u8; 3]) -> Option<Part> {
    let font = font()?;
    let ast = latex_rust::parse(tex).ok()?;
    let style = if display { MathStyle::Display } else { MathStyle::Text };
    let tree = latex_rust::layout(&ast, font, style).ok()?;
    let mut options = SvgOptions::new();
    options.font_size_pt = Dim::from_i64(EM_PT);
    options.color = Color::rgb(color[0], color[1], color[2]);
    options.display = display;
    let svg = latex_rust::render_svg(&tree, font, &options).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    svg.hash(&mut h);
    let uri = format!("{DIAGRAM_URI_PREFIX}math-{:016x}.svg", h.finish());
    ctx.include_bytes(uri.clone(), svg.into_bytes());
    Some(Part::Math { uri, width: em(&tree.width), height: em(&tree.height), depth: em(&tree.depth) })
}

fn build(ctx: &egui::Context, tex: &str, display: bool, color: [u8; 3]) -> Option<Vec<Part>> {
    split_text_groups(tex)
        .into_iter()
        .map(|(text, s)| if text { Some(Part::Text(s)) } else { render_math(ctx, &s, display, color) })
        .collect()
}

impl MathCache {
    /// Drop every rendered formula and its SVG, e.g. after the theme changed the text colour.
    pub fn clear(&self, ctx: &egui::Context) {
        for parts in self.rendered.borrow_mut().drain().filter_map(|(_, parts)| parts) {
            for part in parts {
                if let Part::Math { uri, .. } = part {
                    ctx.forget_image(&uri);
                }
            }
        }
    }

    /// The `render_math_fn` callback of egui_commonmark.
    pub fn show(&self, ui: &mut Ui, tex: &str, inline: bool) {
        let c = ui.visuals().text_color();
        let color = [c.r(), c.g(), c.b()];
        let mut rendered = self.rendered.borrow_mut();
        let parts = rendered
            .entry((tex.to_owned(), !inline, color))
            .or_insert_with(|| build(ui.ctx(), tex, !inline, color));
        let Some(parts) = parts else {
            let source = if inline { format!("${tex}$") } else { format!("$${tex}$$") };
            ui.label(RichText::new(source).code());
            return;
        };
        // Labels in a wrapped row hang from the row top; measure where their baseline falls
        // so the formula can share it.
        let font = TextStyle::Body.resolve(ui.style());
        let size = font.size;
        let label = ui.painter().layout_no_wrap("x".into(), font, c);
        let label_height = label.size().y;
        let baseline = label.rows[0].pos.y + label.rows[0].row.glyphs[0].pos.y;
        if !inline {
            // Display math gets a row of its own, centred like in other Markdown viewers.
            if ui.cursor().min.x > ui.max_rect().min.x {
                ui.end_row();
            }
            let total: f32 = parts.iter().map(|p| match p {
                Part::Math { width, .. } => width * size,
                Part::Text(t) => ui.painter().layout_no_wrap(t.clone(), TextStyle::Body.resolve(ui.style()), c).size().x,
            }).sum();
            ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
        }
        for part in parts.iter() {
            match part {
                Part::Text(t) => {
                    ui.label(t.as_str());
                }
                Part::Math { uri, width, height, depth } => {
                    paint_math(ui, uri, *width * size, *height * size, *depth * size, baseline, label_height);
                }
            }
        }
        if !inline {
            ui.end_row();
        }
    }
}

/// Draw the formula with its baseline `baseline` below the row top, where a text label puts
/// its own. The reserved box only claims the width and, for deep formulas, a taller row;
/// a formula taller than the text reaches into the gap above the row.
fn paint_math(ui: &mut Ui, uri: &str, width: f32, height: f32, depth: f32, baseline: f32, label_height: f32) {
    // Wrap first, so the row top is read from the row the formula lands on.
    if width > ui.available_size_before_wrap().x && ui.cursor().min.x > ui.max_rect().min.x {
        ui.end_row();
    }
    let row_top = ui.cursor().min.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, (baseline + depth).max(label_height)), egui::Sense::hover());
    let image_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), row_top + baseline - height), egui::vec2(width, height + depth));
    let pixels = ((height + depth) * ui.ctx().pixels_per_point()).ceil().max(1.0) as u32;
    if let Ok(egui::load::TexturePoll::Ready { texture }) = ui.ctx().try_load_texture(uri, Default::default(), egui::load::SizeHint::Height(pixels)) {
        egui::Image::from_texture(texture).paint_at(ui, image_rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_cuts_only_top_level_non_ascii_text() {
        assert_eq!(
            split_text_groups(r"= 480 - \text{認列上班工時}"),
            vec![(false, "= 480 - {}".to_string()), (true, "認列上班工時".to_string())]
        );
        assert_eq!(split_text_groups(r"a + \text{if } b"), vec![(false, r"a + \text{if } b".to_string())]);
        assert_eq!(split_text_groups(r"\frac{\text{中}}{2}"), vec![(false, r"\frac{\text{中}}{2}".to_string())]);
        assert_eq!(
            split_text_groups(r"x \text{當} y"),
            vec![(false, "x {}".to_string()), (true, "當".to_string()), (false, "{} y".to_string())]
        );
    }

    #[test]
    fn test_render_formulas_to_svg() {
        let ctx = egui::Context::default();
        for tex in [r"\frac{a+b}{c}", r"\sum_{i=1}^{n} i", r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}"] {
            assert!(matches!(render_math(&ctx, tex, false, [0, 0, 0]), Some(Part::Math { width, .. }) if width > 0.0), "{tex}");
        }
        assert!(build(&ctx, r"= 480 - \text{認列上班工時}", false, [0, 0, 0]).is_some());
        assert!(build(&ctx, r"\frac{\text{中}}{2}", false, [0, 0, 0]).is_none());
        assert!(build(&ctx, r"\frac{1}{", false, [0, 0, 0]).is_none());
    }

    #[test]
    fn test_clear_drops_rendered_formulas() {
        let ctx = egui::Context::default();
        let cache = MathCache::default();
        let key = (r"x^2".to_string(), false, [0, 0, 0]);
        cache.rendered.borrow_mut().insert(key.clone(), build(&ctx, &key.0, false, [0, 0, 0]));
        cache.clear(&ctx);
        assert!(cache.rendered.borrow().is_empty());
    }
}
