//! 列印與預覽：產生 HTML 後交給系統瀏覽器（列印時自動開啟瀏覽器的列印對話框）

use std::path::PathBuf;

use crate::core::document::Document;
use crate::core::hexbuf::BYTES_PER_ROW;
use crate::core::model::EditMode;
use crate::core::settings::PrintOptions;
use crate::core::syntax::SyntaxCache;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// 展開頁首頁尾變數：%f 檔名、%p 完整路徑、%n 頁碼、%s 總頁數、%d 日期、%t 時間
///
/// 頁碼由瀏覽器列印時以 CSS counter 產生，因此 %n、%s 以 CSS 計數器代入。
fn expand_vars(fmt: &str, doc: &Document) -> String {
    let now = chrono::Local::now();
    let name = doc.display_name();
    let path = doc.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| name.clone());
    escape(fmt)
        .replace("%f", &escape(&name))
        .replace("%p", &escape(&path))
        .replace("%n", "<span class=\"pn\"></span>")
        .replace("%s", "<span class=\"pt\"></span>")
        .replace("%d", &now.format("%Y-%m-%d").to_string())
        .replace("%t", &now.format("%H:%M:%S").to_string())
}

/// 產生列印用 HTML
pub fn document_html(doc: &Document, opts: &PrintOptions, font_family: &str, font_size: f32, auto_print: bool) -> String {
    let mut body = String::new();
    if doc.mode == EditMode::Hex
        && let Some(hb) = &doc.hex
    {
        let rows = hb.row_count().min(200_000);
        if opts.hex_offset_header > 0 {
            body.push_str("<div class=\"hdr\">Offset    00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F  Text</div>");
        }
        for r in 0..rows {
            let bytes = hb.data.read_slice(r * BYTES_PER_ROW, BYTES_PER_ROW);
            if bytes.is_empty() {
                break;
            }
            let hex: Vec<String> = bytes.iter().enumerate().map(|(i, b)| format!("{}{b:02X}", if i == 8 { " " } else { "" })).collect();
            let text: String = bytes.iter().map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '.' }).collect();
            body.push_str(&format!("<div class=\"l\">{:08X}  {:<49} {}</div>\n", r * BYTES_PER_ROW, hex.join(" "), escape(&text)));
        }
    } else {
        let lines = &doc.editor.lines;
        let mut cache = SyntaxCache::default();
        let theme = crate::core::syntax::default_theme_name(false);
        let digits = lines.len().to_string().len();
        for (i, line) in lines.iter().enumerate() {
            let mut html = String::new();
            if opts.line_numbers {
                html.push_str(&format!("<span class=\"ln\">{:>w$}</span> ", i + 1, w = digits));
            }
            if opts.bookmarks && doc.editor.bookmarks.contains(&i) {
                html.push_str("<span class=\"bm\">●</span>");
            }
            let spans = if opts.syntax_colors {
                cache.highlight(lines, i, 1, &doc.syntax, theme, false).pop().unwrap_or_default()
            } else {
                Vec::new()
            };
            let render = |t: &str| -> String {
                let mut s = escape(t);
                if opts.tab_chars {
                    s = s.replace('\t', "<span class=\"cc\">»</span>\t");
                }
                if opts.space_chars {
                    s = s.replace(' ', "<span class=\"cc\">·</span>");
                }
                s
            };
            if spans.is_empty() {
                html.push_str(&render(line));
            } else {
                for sp in spans {
                    let (r, g, b) = sp.color_rgb;
                    html.push_str(&format!("<span style=\"color:#{r:02x}{g:02x}{b:02x}\">{}</span>", render(&sp.text)));
                }
            }
            if opts.end_of_line {
                html.push_str("<span class=\"cc\">↵</span>");
            }
            body.push_str(&format!("<div class=\"l\">{html}</div>\n"));
        }
    }
    let header = if opts.header {
        format!(
            "<table class=\"hf\"><tr><td>{}</td><td style=\"text-align:center\">{}</td><td style=\"text-align:right\">{}</td></tr></table>",
            expand_vars(&opts.header_left, doc),
            expand_vars(&opts.header_center, doc),
            expand_vars(&opts.header_right, doc)
        )
    } else {
        String::new()
    };
    let footer = if opts.footer {
        format!(
            "<table class=\"hf\"><tr><td>{}</td><td style=\"text-align:center\">{}</td><td style=\"text-align:right\">{}</td></tr></table>",
            expand_vars(&opts.footer_left, doc),
            expand_vars(&opts.footer_center, doc),
            expand_vars(&opts.footer_right, doc)
        )
    } else {
        String::new()
    };
    let font = if font_family.is_empty() { "monospace".to_string() } else { format!("'{}', monospace", font_family.replace('\'', "")) };
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{title}</title><style>
body {{ font-family: {font}; font-size: {size}px; margin: 0; }}
.l {{ white-space: pre-wrap; word-break: break-all; tab-size: {tab}; }}
.ln {{ color: #999; }} .bm {{ color: #3c78e6; }} .cc {{ color: #aac; }}
.hf {{ width: 100%; font-family: sans-serif; font-size: 10px; color: #555; }}
.hdr {{ white-space: pre; color: #36c; }}
thead {{ display: table-header-group; }} tfoot {{ display: table-footer-group; }}
@page {{ margin: 15mm; }}
body {{ counter-reset: page; }}
.pn::after {{ content: counter(page); }} .pt::after {{ content: counter(pages); }}
</style></head><body{onload}>
<table style="width:100%;border-collapse:collapse"><thead><tr><td>{header}</td></tr></thead>
<tfoot><tr><td>{footer}</td></tr></tfoot><tbody><tr><td>{body}</td></tr></tbody></table>
</body></html>"#,
        title = escape(&doc.display_name()),
        font = font,
        size = font_size,
        tab = crate::core::editor::tab_size(),
        onload = if auto_print { " onload=\"window.print()\"" } else { "" },
        header = header,
        footer = footer,
        body = body,
    )
}

/// 將 HTML 寫入暫存檔並以系統瀏覽器開啟
pub fn open_html(name: &str, html: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("MadEdit");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    let path = dir.join(format!("{safe}.html"));
    std::fs::write(&path, html).map_err(|e| e.to_string())?;
    open_with_system(&path)?;
    Ok(path)
}

/// 以系統預設程式開啟檔案
pub fn open_with_system(path: &std::path::Path) -> Result<(), String> {
    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(path).spawn()
    } else if cfg!(windows) {
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(path).spawn()
    };
    result.map(|_| ()).map_err(|e| format!("無法開啟瀏覽器：{e}"))
}

/// Markdown 或 HTML 預覽頁
pub fn preview_html(doc: &Document, markdown: bool) -> String {
    let text = doc.text();
    if markdown {
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:sans-serif;max-width:860px;margin:2em auto;line-height:1.6;padding:0 1em}}pre,code{{background:#f4f4f4}}pre{{padding:.6em;overflow:auto}}table{{border-collapse:collapse}}td,th{{border:1px solid #ccc;padding:4px 8px}}</style></head><body>{}</body></html>",
            escape(&doc.display_name()),
            crate::core::textops::markdown_to_html(&text)
        )
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_html_escapes_and_numbers_lines() {
        let mut doc = Document::default();
        doc.editor.set_text("a<b\nc");
        let opts = PrintOptions { syntax_colors: false, header: false, footer: false, ..Default::default() };
        let html = document_html(&doc, &opts, "", 12.0, true);
        assert!(html.contains("a&lt;b"));
        assert!(html.contains("<span class=\"ln\">1</span>"));
        assert!(html.contains("window.print()"));
    }
}
