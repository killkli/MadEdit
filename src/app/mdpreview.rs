//! App 內的 Markdown 預覽視窗：以 `egui_commonmark` 繪製 Markdown，
//! 其中 ```mermaid 區塊交給 `fermaid` 轉成 SVG，再由 `egui_extras` 的 SVG 載入器顯示。
//!
//! 預覽內容會隨文件版本（`Document::version`）自動更新。

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// 文件被切成的片段：一般 Markdown 或 Mermaid 圖表原始碼
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// 交給 egui_commonmark 繪製的 Markdown 文字
    Markdown(String),
    /// Mermaid 圖表原始碼（不含圍欄行）
    Mermaid(String),
}

/// 判斷一行是否為 mermaid 圍欄開頭，回傳（圍欄字元, 圍欄長度）
fn mermaid_fence_open(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    // 縮排超過 3 個空白就是縮排程式碼區塊，不是圍欄
    if line.len() - t.len() > 3 {
        return None;
    }
    let ch = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|c| *c == ch).count();
    if n < 3 {
        return None;
    }
    // 圍欄後的第一個單字必須是 mermaid
    let info = t[n..].trim();
    (info.split_whitespace().next() == Some("mermaid")).then_some((ch, n))
}

/// 判斷一行是否為關閉圍欄（同字元、長度不少於開頭、之後只能有空白）
fn is_fence_close(line: &str, ch: char, n: usize) -> bool {
    let t = line.trim();
    t.chars().count() >= n && t.chars().all(|c| c == ch)
}

/// 將 Markdown 文字切成一般 Markdown 與 Mermaid 片段。
/// 其他語言的程式碼區塊（例如內容剛好寫了 mermaid 圍欄的 ```text）不受影響，
/// 因為只有開頭圍欄標示 mermaid 才會被擷取，且開頭圍欄在其他程式碼區塊內不會被辨識。
pub fn split_mermaid(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut md = String::new();
    // 目前是否位於「非 mermaid」的程式碼區塊內（內部的 mermaid 字樣要忽略）
    let mut other_fence: Option<(char, usize)> = None;
    let mut lines = text.split_inclusive('\n');
    while let Some(line) = lines.next() {
        let body = line.trim_end_matches(['\r', '\n']);
        if let Some((ch, n)) = other_fence {
            if is_fence_close(body, ch, n) {
                other_fence = None;
            }
            md.push_str(line);
            continue;
        }
        if let Some((ch, n)) = mermaid_fence_open(body) {
            if !md.is_empty() {
                out.push(Segment::Markdown(std::mem::take(&mut md)));
            }
            let mut src = String::new();
            // 讀到關閉圍欄為止；沒有關閉圍欄時視為到文件結尾
            for l in lines.by_ref() {
                if is_fence_close(l.trim_end_matches(['\r', '\n']), ch, n) {
                    break;
                }
                src.push_str(l);
            }
            out.push(Segment::Mermaid(src));
            continue;
        }
        // 一般程式碼區塊開頭：記下來，避免區塊內容被誤判
        let t = body.trim_start();
        if body.len() - t.len() <= 3
            && let Some(ch) = t.chars().next().filter(|c| *c == '`' || *c == '~')
        {
            let n = t.chars().take_while(|c| *c == ch).count();
            if n >= 3 {
                other_fence = Some((ch, n));
            }
        }
        md.push_str(line);
    }
    if !md.is_empty() {
        out.push(Segment::Markdown(md));
    }
    out
}

/// 是否為相對路徑圖片位址：沒有協定（`xxx://`、`data:`）、不是絕對路徑、不是 Windows 磁碟機路徑
fn is_relative_image_url(url: &str) -> bool {
    let b = url.as_bytes();
    !url.is_empty()
        && !url.contains("://")
        && !url.starts_with("data:")
        && !url.starts_with(['/', '\\', '#'])
        && !(b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

/// 把 Markdown 圖片的相對路徑改寫成以「文件所在資料夾」為基準的絕對 `file://` 位址。
/// egui_commonmark 預設以程式工作目錄解析相對路徑，不符合編輯 Markdown 的習慣。
/// 路徑以 `<...>` 包起來，因此含空白也能正確解析；百分比編碼（`%20`）交給載入器處理。
/// 文件尚未存檔（沒有 `base_dir`）時不改寫。
pub fn rewrite_image_paths(md: &str, base_dir: Option<&Path>) -> String {
    use pulldown_cmark::{Event, Parser, Tag};
    let Some(base) = base_dir else { return md.to_string() };
    // 收集（位元組範圍, 新位址）；範圍是圖片語法中位址文字的位置
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for (ev, range) in Parser::new(md).into_offset_iter() {
        let Event::Start(Tag::Image { dest_url, .. }) = ev else { continue };
        if !is_relative_image_url(&dest_url) {
            continue;
        }
        // 位址在圖片語法的尾端（標題文字在其後），取最後一次出現的位置；
        // 參照式圖片（`![a][ref]`）的位址不在此範圍內，找不到就略過
        let span = &md[range.clone()];
        let Some(off) = span.rfind(dest_url.as_ref()) else { continue };
        let start = range.start + off;
        let full = base.join(dest_url.as_ref()).to_string_lossy().replace('\\', "/");
        edits.push((start..start + dest_url.len(), format!("<file://{full}>")));
    }
    let mut out = md.to_string();
    // 由後往前替換，前面的位置才不會位移
    for (r, new) in edits.into_iter().rev() {
        // 原位址若已用 `<...>` 包起來，連同角括號一併替換
        let (s, e) = if out[..r.start].ends_with('<') && out[r.end..].starts_with('>') { (r.start - 1, r.end + 1) } else { (r.start, r.end) };
        out.replace_range(s..e, &new);
    }
    out
}

/// fermaid 輸出的 SVG 以 CSS 變數（`var(--fermaid-bg,#fff)`）著色，但 SVG 載入器（resvg）不支援 `var()`，
/// 因此在這裡依主題把變數換成實際顏色：先用主題色或後備值替換，
/// 再收集根元素宣告的中介變數（`--_fg:...`），最多重複數輪解開巢狀引用。
pub fn resolve_svg_vars(svg: &str, theme: &HashMap<&str, &str>) -> String {
    let mut s = svg.to_string();
    let mut local: HashMap<String, String> = HashMap::new();
    for _ in 0..6 {
        let mut out = String::with_capacity(s.len());
        let mut rest = s.as_str();
        while let Some(i) = rest.find("var(--") {
            out.push_str(&rest[..i]);
            let after = &rest[i + 4..];
            // 找對應的右括號（支援巢狀括號）
            let mut depth = 1;
            let mut end = after.len();
            for (k, c) in after.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = k;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let inner = &after[..end];
            let (name, fallback) = match inner.split_once(',') {
                Some((n, f)) => (n.trim(), f.trim()),
                None => (inner.trim(), ""),
            };
            match theme.get(name).map(|v| v.to_string()).or_else(|| local.get(name).cloned()) {
                Some(v) => out.push_str(&v),
                // 中介變數尚未收集到值：原樣保留，下一輪再解析
                None if fallback.is_empty() => {
                    out.push_str("var(");
                    out.push_str(inner);
                    out.push(')');
                }
                None => out.push_str(fallback),
            }
            rest = after.get(end + 1..).unwrap_or("");
        }
        out.push_str(rest);
        // 收集已不含 var() 的 `--_x:值` 宣告
        for part in out.split(['"', ';']) {
            if let Some((k, v)) = part.trim().split_once(':')
                && k.starts_with("--_")
                && !v.contains("var(")
            {
                local.insert(k.to_string(), v.trim().to_string());
            }
        }
        s = out;
        if !s.contains("var(--") {
            break;
        }
    }
    s
}

/// 深色主題下覆蓋 fermaid 預設（淺色）配色的變數
fn dark_theme() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("--fermaid-bg", "#1e1e1e"),
        ("--fermaid-fg", "#dcddde"),
        ("--fermaid-muted", "#aaaaaa"),
        ("--fermaid-line", "#999999"),
        ("--fermaid-surface", "#2a2a2a"),
        ("--fermaid-surface-alt", "#333333"),
        ("--fermaid-border", "#777777"),
    ])
}

/// 渲染完成、可顯示的 Mermaid 圖
struct Diagram {
    /// SVG 載入器的快取識別（含內容雜湊，內容或主題變更即換新）
    uri: String,
    bytes: Vec<u8>,
    width: f32,
    height: f32,
}

/// 單一片段的繪製資料
enum Item {
    Markdown(String),
    Diagram(Diagram),
    /// Mermaid 語法錯誤：訊息與原始碼
    Error { message: String, source: String },
}

/// Markdown 預覽視窗狀態
pub struct MarkdownPreview {
    /// 預覽中的文件識別碼
    pub doc_id: u64,
    /// 文件顯示名稱（視窗標題用）
    pub title: String,
    /// 已建立內容時的文件版本與主題；任一變動就重建
    built: Option<(u64, bool, Option<PathBuf>)>,
    items: Vec<Item>,
    cache: CommonMarkCache,
}

/// 預覽視窗回報給 App 的動作
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MdAction {
    None,
    Close,
}

impl MarkdownPreview {
    pub fn new(doc_id: u64, title: String) -> Self {
        Self { doc_id, title, built: None, items: Vec::new(), cache: CommonMarkCache::default() }
    }

    /// 文件版本或主題改變時才重建內容（Mermaid 渲染只在此發生，平時每幀不重算）
    pub fn refresh(&mut self, ctx: &egui::Context, text: &str, version: u64, dark: bool, base_dir: Option<PathBuf>) {
        // 另存新檔改變資料夾時，相對路徑圖片也要重新解析，所以資料夾也是重建條件
        let key = (version, dark, base_dir);
        if self.built.as_ref() == Some(&key) {
            return;
        }
        // 釋放舊圖的 SVG 快取，避免編輯過程累積
        for it in &self.items {
            if let Item::Diagram(d) = it {
                ctx.forget_image(&d.uri);
            }
        }
        let theme = if dark { dark_theme() } else { HashMap::new() };
        let opts = fermaid::RenderOptions::default();
        self.items = split_mermaid(text)
            .into_iter()
            .map(|seg| match seg {
                Segment::Markdown(m) => Item::Markdown(rewrite_image_paths(&m, key.2.as_deref())),
                Segment::Mermaid(src) => match fermaid::render(&src, &opts) {
                    Ok(r) => {
                        let svg = resolve_svg_vars(&r.svg, &theme);
                        let mut h = std::collections::hash_map::DefaultHasher::new();
                        svg.hash(&mut h);
                        Item::Diagram(Diagram {
                            uri: format!("bytes://mermaid-{:016x}.svg", h.finish()),
                            bytes: svg.into_bytes(),
                            width: r.width as f32,
                            height: r.height as f32,
                        })
                    }
                    Err(e) => Item::Error { message: format!("第 {} 行第 {} 欄：{}", e.line, e.column, e.message), source: src },
                },
            })
            .collect();
        self.built = Some(key);
    }

    /// 繪製預覽內容
    pub fn ui(&mut self, ui: &mut egui::Ui) -> MdAction {
        if ui.ctx().input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
            return MdAction::Close;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(900.0));
            for item in &self.items {
                match item {
                    Item::Markdown(m) => {
                        CommonMarkViewer::new().show(ui, &mut self.cache, m);
                    }
                    Item::Diagram(d) => {
                        // 只縮小不放大，保持圖表原始尺寸的清晰度
                        let scale = (ui.available_width() / d.width).min(1.0);
                        let img = egui::Image::from_bytes(d.uri.clone(), d.bytes.clone())
                            .fit_to_exact_size(egui::vec2(d.width * scale, d.height * scale));
                        ui.add(img);
                        ui.add_space(8.0);
                    }
                    Item::Error { message, source } => {
                        ui.label(RichText::new(format!("Mermaid 語法錯誤：{message}")).color(Color32::LIGHT_RED));
                        ui.code(source);
                    }
                }
            }
        });
        MdAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_extracts_mermaid_block() {
        let segs = split_mermaid("# 標題\n\n```mermaid\ngraph LR\n  A-->B\n```\n\n結尾\n");
        assert_eq!(
            segs,
            vec![
                Segment::Markdown("# 標題\n\n".into()),
                Segment::Mermaid("graph LR\n  A-->B\n".into()),
                Segment::Markdown("\n結尾\n".into()),
            ]
        );
    }

    #[test]
    fn test_split_ignores_mermaid_inside_other_code_block() {
        let text = "```text\n```mermaid\nx\n```\n";
        // 第二行落在 text 圍欄內，整份都是一般 Markdown
        assert_eq!(split_mermaid(text), vec![Segment::Markdown(text.into())]);
    }

    #[test]
    fn test_split_unclosed_fence_and_tilde_and_crlf() {
        assert_eq!(split_mermaid("~~~mermaid\r\npie\r\n  \"a\": 1\r\n"), vec![Segment::Mermaid("pie\r\n  \"a\": 1\r\n".into())]);
        assert_eq!(split_mermaid("沒有圖\n"), vec![Segment::Markdown("沒有圖\n".into())]);
        assert!(split_mermaid("").is_empty());
    }

    #[test]
    fn test_split_requires_mermaid_info_word() {
        let text = "```mermaidx\nfoo\n```\n";
        assert_eq!(split_mermaid(text), vec![Segment::Markdown(text.into())]);
    }

    #[test]
    fn test_rewrite_relative_image_uses_document_dir() {
        let base = Path::new("/home/u/docs");
        let out = rewrite_image_paths("看圖 ![圖](img/a.png \"標題\") 結束", Some(base));
        assert_eq!(out, "看圖 ![圖](<file:///home/u/docs/img/a.png> \"標題\") 結束");
        // 已用角括號包起來的位址不會變成兩層
        let out = rewrite_image_paths("![x](<pic 1.png>)", Some(base));
        assert_eq!(out, "![x](<file:///home/u/docs/pic 1.png>)");
    }

    #[test]
    fn test_rewrite_keeps_absolute_remote_and_unsaved() {
        let base = Path::new("/d");
        for md in ["![a](/abs/a.png)", "![a](https://x.com/a.png)", "![a](file:///x.png)", "![a](data:image/png;base64,AAAA)", "![a](C:\\x.png)"] {
            assert_eq!(rewrite_image_paths(md, Some(base)), md);
        }
        // 尚未存檔：不改寫
        assert_eq!(rewrite_image_paths("![a](a.png)", None), "![a](a.png)");
        // 連結（非圖片）不受影響
        assert_eq!(rewrite_image_paths("[a](a.md)", Some(base)), "[a](a.md)");
    }

    #[test]
    fn test_resolve_vars_uses_theme_and_fallback() {
        let svg = r#"<svg style="--_bg:var(--fermaid-bg,#fff);--_fg:var(--fermaid-fg,#111)" fill="var(--_fg)"><rect fill="var(--_bg)"/></svg>"#;
        let light = resolve_svg_vars(svg, &HashMap::new());
        assert!(!light.contains("var("), "{light}");
        assert!(light.contains(r##"fill="#111""##) && light.contains(r##"fill="#fff""##));
        let dark = resolve_svg_vars(svg, &dark_theme());
        assert!(dark.contains(r##"fill="#dcddde""##) && dark.contains(r##"fill="#1e1e1e""##));
    }

    #[test]
    fn test_render_flowchart_has_no_unresolved_vars() {
        let r = fermaid::render("flowchart LR\n  A[開始] --> B[結束]", &fermaid::RenderOptions::default()).unwrap();
        assert!(!resolve_svg_vars(&r.svg, &HashMap::new()).contains("var("));
        assert!(!resolve_svg_vars(&r.svg, &dark_theme()).contains("var("));
    }
}
