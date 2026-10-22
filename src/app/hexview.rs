//! 十六進位檢視：位移欄、十六進位欄與文字欄（依文件編碼解碼），支援直接編輯位元組

use eframe::egui::{self, Align2, Color32, CornerRadius, Event, FontFamily, FontId, Key, Pos2, Rect, Sense, Stroke, Vec2};
use encoding_rs::Encoding;

use crate::app::fonts::EDITOR_FAMILY;
use crate::app::view::CharWidths;
use crate::core::document::Document;
use crate::core::encoding;
use crate::core::hexbuf::{HexBuffer, BYTES_PER_ROW};
use crate::core::settings::Settings;

/// 自 `i` 起解碼一個字元，回傳（字元, 位元組數）；無法解碼時回傳 None
fn decode_char_at(bytes: &[u8], i: usize, enc: &'static Encoding) -> Option<(char, usize)> {
    for len in 1..=4.min(bytes.len() - i) {
        let slice = &bytes[i..i + len];
        if let Some(s) = enc.decode_without_bom_handling_and_without_replacement(slice)
            && let Some(c) = s.chars().next()
            && s.chars().count() == 1
        {
            return Some((c, len));
        }
    }
    None
}

/// 一列的文字欄內容：每個位元組對應一個顯示字元（多位元組字元顯示在首位元組，其餘留空）
fn row_text(hb: &HexBuffer, row: usize, enc: &'static Encoding) -> Vec<Option<char>> {
    let row_start = row * BYTES_PER_ROW;
    let row_end = (row_start + BYTES_PER_ROW).min(hb.len());
    let mut cells = vec![None; row_end - row_start];
    let utf16 = enc == encoding_rs::UTF_16LE || enc == encoding_rs::UTF_16BE;
    // 有狀態的編碼（如 ISO-2022-JP）無法逐字解碼，只顯示 ASCII
    let ascii_only = !enc.is_ascii_compatible() && !utf16;
    // 多位元組編碼：自列首前幾個位元組開始解碼，以正確對齊跨列的字元（UTF-16 需維持偶數對齊）
    let back = if ascii_only { 0 } else if utf16 { 2.min(row_start) } else { 3.min(row_start) };
    let start = row_start - back;
    let data = hb.data.read_slice(start, row_end - start + 3);
    let mut i = 0;
    while start + i < row_end {
        let abs = start + i;
        let decoded = if ascii_only { None } else { decode_char_at(&data, i, enc) };
        match decoded {
            Some((c, len)) if !c.is_control() => {
                if abs >= row_start {
                    cells[abs - row_start] = Some(c);
                }
                i += len;
            }
            _ => {
                if abs >= row_start {
                    let b = data[i];
                    cells[abs - row_start] = Some(if (0x20..0x7F).contains(&b) { b as char } else { '.' });
                }
                i += 1;
            }
        }
    }
    cells
}

/// 十六進位檢視的固定 Id
pub fn hex_id(doc_id: u64) -> egui::Id {
    egui::Id::new(("madedit-hex", doc_id))
}

#[derive(Default)]
pub struct HexViewState {
    dragging: bool,
    scroll_drag: Option<f32>,
}

/// 顯示十六進位檢視；回傳是否取得焦點
pub fn show(ui: &mut egui::Ui, doc: &mut Document, state: &mut HexViewState, settings: &Settings, widths: &mut CharWidths, editor_family: &str, is_dark: bool) -> bool {
    let ctx = ui.ctx().clone();
    let font = FontId::new(settings.font_size, FontFamily::Name(EDITOR_FAMILY.into()));
    widths.prepare(&ctx, &font, editor_family, "0123456789ABCDEF.".chars());
    let cw = widths.cell_w;
    let row_h = (widths.text_h * settings.line_spacing.clamp(100, 250) as f32 / 100.0).round();
    let enc = doc.encoding;
    let read_only = doc.read_only;
    let Some(hb) = doc.hex.as_mut() else { return false };

    let full = ui.available_rect_before_wrap();
    let sb = 14.0;
    let header_h = row_h;
    let area = Rect::from_min_max(Pos2::new(full.left(), full.top() + header_h), Pos2::new(full.right() - sb, full.bottom()));
    let offset_digits = if hb.len() > 0xFFFF_FFFF { 12 } else { 8 };
    let off_x = area.left() + cw;
    let hex_x = off_x + (offset_digits + 2) as f32 * cw;
    let hex_col_x = |i: usize| hex_x + (i * 3 + if i >= 8 { 1 } else { 0 }) as f32 * cw;
    let text_x = hex_col_x(BYTES_PER_ROW) + cw;
    let vis_rows = ((area.height() / row_h).floor() as usize).max(1);
    let total_rows = hb.row_count();
    let max_top = total_rows.saturating_sub(1);

    let id = hex_id(doc.id);
    let resp = ui.interact(area, id, Sense::click_and_drag());
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    let focused = resp.has_focus();
    if focused {
        ctx.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
    }

    // 滑鼠座標 → (位元組位移, 是否在文字欄)
    let hit = |hb: &HexBuffer, p: Pos2| -> (usize, bool) {
        let row = hb.top_row + ((p.y - area.top()) / row_h).floor().max(0.0) as usize;
        let in_text = p.x >= text_x - cw / 2.0;
        let col = if in_text {
            ((p.x - text_x) / cw).floor().max(0.0) as usize
        } else {
            let rel = ((p.x - hex_x) / cw).max(0.0);
            let rel = if rel > 24.0 { rel - 1.0 } else { rel };
            (rel / 3.0).floor() as usize
        };
        ((row * BYTES_PER_ROW + col.min(BYTES_PER_ROW - 1)).min(hb.len()), in_text)
    };

    let modifiers = ctx.input(|i| i.modifiers);
    if let Some(p) = resp.interact_pointer_pos() {
        if resp.drag_started() || resp.clicked() {
            let (off, in_text) = hit(hb, p);
            hb.in_text_pane = in_text;
            hb.move_to(off, modifiers.shift);
            state.dragging = true;
        } else if resp.dragged() && state.dragging {
            if p.y < area.top() {
                hb.top_row = hb.top_row.saturating_sub(1);
            } else if p.y > area.bottom() {
                hb.top_row = (hb.top_row + 1).min(max_top);
            }
            let (off, _) = hit(hb, p);
            hb.move_to(off, true);
        }
    }
    if resp.drag_stopped() {
        state.dragging = false;
    }

    // 鍵盤
    let mut caret_moved = false;
    if focused {
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            match ev {
                Event::Key { key, pressed: true, modifiers: m, .. } => {
                    let ext = m.shift;
                    let c = hb.caret;
                    let page = vis_rows.saturating_sub(1).max(1) * BYTES_PER_ROW;
                    match key {
                        Key::ArrowLeft => hb.move_to(c.saturating_sub(1), ext),
                        Key::ArrowRight => hb.move_to(c + 1, ext),
                        Key::ArrowUp => hb.move_to(c.saturating_sub(BYTES_PER_ROW), ext),
                        Key::ArrowDown => hb.move_to((c + BYTES_PER_ROW).min(hb.len()), ext),
                        Key::PageUp => {
                            hb.top_row = hb.top_row.saturating_sub(vis_rows.saturating_sub(1));
                            hb.move_to(c.saturating_sub(page), ext)
                        }
                        Key::PageDown => {
                            hb.top_row = (hb.top_row + vis_rows.saturating_sub(1)).min(max_top);
                            hb.move_to((c + page).min(hb.len()), ext)
                        }
                        Key::Home if m.command => hb.move_to(0, ext),
                        Key::End if m.command => hb.move_to(hb.len(), ext),
                        Key::Home => hb.move_to(c - c % BYTES_PER_ROW, ext),
                        Key::End => hb.move_to((c - c % BYTES_PER_ROW + BYTES_PER_ROW - 1).min(hb.len()), ext),
                        Key::Tab => hb.in_text_pane = !hb.in_text_pane,
                        Key::Insert => hb.overwrite = !hb.overwrite,
                        Key::Backspace if !read_only => hb.delete(true),
                        Key::Delete if !read_only && !m.shift => hb.delete(false),
                        Key::Escape => hb.anchor = None,
                        _ => continue,
                    }
                    caret_moved = true;
                }
                Event::Text(t) | Event::Ime(egui::ImeEvent::Commit(t)) if !read_only => {
                    if hb.in_text_pane {
                        if let Ok(bytes) = encoding::encode_text(&t, enc, false) {
                            hb.type_bytes(&bytes);
                        }
                    } else {
                        for ch in t.chars() {
                            if let Some(d) = ch.to_digit(16) {
                                hb.type_hex_digit(d as u8);
                            }
                        }
                    }
                    caret_moved = true;
                }
                _ => {}
            }
        }
        ctx.output_mut(|o| {
            o.ime = Some(egui::output::IMEOutput { rect: area, cursor_rect: Rect::from_min_size(area.min, Vec2::new(2.0, row_h)), should_interrupt_composition: false })
        });
    }

    // 滾輪
    if ui.rect_contains_pointer(full) {
        let dy = ctx.input(|i| i.smooth_scroll_delta()).y;
        if dy != 0.0 {
            let rows = (-dy / row_h).round() as i64;
            hb.top_row = (hb.top_row as i64 + rows).clamp(0, max_top as i64) as usize;
        }
    }

    // 捲軸
    let vbar = Rect::from_min_max(Pos2::new(full.right() - sb, full.top()), full.max);
    let vresp = ui.interact(vbar, id.with("vbar"), Sense::click_and_drag());
    let track = vbar.shrink(2.0);
    let thumb_h = (track.height() * vis_rows as f32 / (total_rows + vis_rows) as f32).clamp(20.0, track.height());
    if let Some(p) = vresp.interact_pointer_pos() {
        if vresp.drag_started() {
            state.scroll_drag = Some(thumb_h / 2.0);
        }
        if let Some(grab) = state.scroll_drag {
            let f = ((p.y - grab - track.top()) / (track.height() - thumb_h).max(1.0)).clamp(0.0, 1.0) as f64;
            hb.top_row = (f * max_top as f64).round() as usize;
        }
    }
    if vresp.drag_stopped() {
        state.scroll_drag = None;
    }

    // 游標可見
    if caret_moved {
        let row = hb.caret / BYTES_PER_ROW;
        if row < hb.top_row {
            hb.top_row = row;
        } else if row >= hb.top_row + vis_rows {
            hb.top_row = row + 1 - vis_rows;
        }
    }
    hb.top_row = hb.top_row.min(max_top);

    // 繪製
    let visuals = ui.visuals().clone();
    let bg = visuals.extreme_bg_color;
    let fg = visuals.text_color();
    let faint = fg.gamma_multiply(0.5);
    let sel_color = visuals.selection.bg_fill.gamma_multiply(0.6);
    let painter = ui.painter_at(full);
    painter.rect_filled(full, CornerRadius::ZERO, bg);
    let header_bg = if is_dark { Color32::from_gray(40) } else { Color32::from_gray(235) };
    painter.rect_filled(Rect::from_min_max(full.min, Pos2::new(full.right() - sb, full.top() + header_h)), CornerRadius::ZERO, header_bg);
    painter.text(Pos2::new(off_x, full.top()), Align2::LEFT_TOP, "Offset", font.clone(), faint);
    for i in 0..BYTES_PER_ROW {
        painter.text(Pos2::new(hex_col_x(i), full.top()), Align2::LEFT_TOP, format!("{i:02X}"), font.clone(), faint);
    }
    painter.text(Pos2::new(text_x, full.top()), Align2::LEFT_TOP, "0123456789ABCDEF", font.clone(), faint);

    let sel = hb.selection();
    let text_color = if is_dark { Color32::from_rgb(200, 210, 220) } else { Color32::from_rgb(30, 30, 30) };
    let offset_color = if is_dark { Color32::from_rgb(120, 170, 220) } else { Color32::from_rgb(0, 90, 160) };
    for vr in 0..=vis_rows {
        let row = hb.top_row + vr;
        if row >= total_rows {
            break;
        }
        let y = area.top() + vr as f32 * row_h;
        let row_start = row * BYTES_PER_ROW;
        let bytes = hb.data.read_slice(row_start, BYTES_PER_ROW);
        painter.text(Pos2::new(off_x, y), Align2::LEFT_TOP, format!("{:0w$X}", row_start, w = offset_digits), font.clone(), offset_color);
        let cells = row_text(hb, row, enc);
        for (i, b) in bytes.iter().enumerate() {
            let off = row_start + i;
            let in_sel = sel.is_some_and(|(s, e)| off >= s && off < e);
            if in_sel {
                painter.rect_filled(Rect::from_min_size(Pos2::new(hex_col_x(i), y), Vec2::new(cw * 2.0, row_h)), CornerRadius::ZERO, sel_color);
                painter.rect_filled(Rect::from_min_size(Pos2::new(text_x + i as f32 * cw, y), Vec2::new(cw, row_h)), CornerRadius::ZERO, sel_color);
            }
            painter.text(Pos2::new(hex_col_x(i), y), Align2::LEFT_TOP, format!("{b:02X}"), font.clone(), text_color);
            if let Some(Some(c)) = cells.get(i) {
                painter.text(Pos2::new(text_x + i as f32 * cw, y), Align2::LEFT_TOP, c.to_string(), font.clone(), text_color);
            }
        }
        // 游標
        if hb.caret / BYTES_PER_ROW == row {
            let i = hb.caret % BYTES_PER_ROW;
            let (hx, tx) = (hex_col_x(i) + if hb.low_nibble { cw } else { 0.0 }, text_x + i as f32 * cw);
            let strong = Stroke::new(2.0, fg);
            let weak = Stroke::new(1.0, faint);
            let (hs, ts) = if hb.in_text_pane { (weak, strong) } else { (strong, weak) };
            if hb.overwrite {
                painter.rect_stroke(Rect::from_min_size(Pos2::new(hx, y), Vec2::new(cw, row_h)), CornerRadius::ZERO, hs, egui::StrokeKind::Inside);
                painter.rect_stroke(Rect::from_min_size(Pos2::new(tx, y), Vec2::new(cw, row_h)), CornerRadius::ZERO, ts, egui::StrokeKind::Inside);
            } else {
                painter.vline(hx, y..=y + row_h, hs);
                painter.vline(tx, y..=y + row_h, ts);
            }
        }
    }
    painter.vline(hex_x - cw, area.y_range(), Stroke::new(1.0, faint.gamma_multiply(0.5)));
    painter.vline(text_x - cw / 2.0, area.y_range(), Stroke::new(1.0, faint.gamma_multiply(0.5)));

    let frac = if max_top == 0 { 0.0 } else { hb.top_row as f64 / max_top as f64 } as f32;
    painter.rect_filled(vbar, CornerRadius::ZERO, visuals.faint_bg_color);
    painter.rect_filled(
        Rect::from_min_size(Pos2::new(track.left(), track.top() + (track.height() - thumb_h) * frac), Vec2::new(track.width(), thumb_h)),
        CornerRadius::same(3),
        visuals.widgets.inactive.bg_fill,
    );
    focused
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::piece_table::PieceTable;

    #[test]
    fn test_row_text_decodes_multibyte() {
        let mut bytes = b"ABCDEFGHIJKLMN".to_vec();
        bytes.extend_from_slice("中文".as_bytes()); // 中 跨越第 0、1 列
        let hb = HexBuffer::new(PieceTable::from_bytes(bytes));
        let r0 = row_text(&hb, 0, encoding_rs::UTF_8);
        assert_eq!(r0[0], Some('A'));
        assert_eq!(r0[14], Some('中'));
        assert_eq!(r0[15], None);
        let r1 = row_text(&hb, 1, encoding_rs::UTF_8);
        assert_eq!(r1[0], None);
        assert_eq!(r1[1], Some('文'));
    }
}
