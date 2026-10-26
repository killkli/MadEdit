//! App 內的 Markdown 預覽視窗：以 `egui_commonmark` 繪製 Markdown，
//! 其中 ```mermaid 區塊交給 `fermaid` 轉成 SVG，再由 `egui_extras` 的 SVG 載入器顯示。
//!
//! 預覽內容會隨文件版本（`Document::version`）自動更新。

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Color32, RichText, emath::Float as _};
use egui::load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};
use egui::mutex::Mutex;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// 文件被切成的片段：一般 Markdown 或 Mermaid 圖表原始碼
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// 交給 egui_commonmark 繪製的 Markdown 文字
    Markdown(String),
    /// Mermaid 圖表原始碼（不含圍欄行）
    Mermaid(String),
}

/// 片段與其在文件中的起始行（從 0 起算；Mermaid 為開頭圍欄所在行）
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub line: usize,
    pub seg: Segment,
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

/// 將 Markdown 文字切成一般 Markdown 與 Mermaid 片段，並記錄各片段的起始行。
/// 其他語言的程式碼區塊（例如內容剛好寫了 mermaid 圍欄的 ```text）不受影響，
/// 因為只有開頭圍欄標示 mermaid 才會被擷取，且開頭圍欄在其他程式碼區塊內不會被辨識。
pub fn split_mermaid(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut md = String::new();
    // 目前片段的起始行、下一個要讀的行號
    let mut md_line = 0usize;
    let mut line_no = 0usize;
    // 目前是否位於「非 mermaid」的程式碼區塊內（內部的 mermaid 字樣要忽略）
    let mut other_fence: Option<(char, usize)> = None;
    let mut lines = text.split_inclusive('\n');
    while let Some(line) = lines.next() {
        let this_line = line_no;
        line_no += 1;
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
                out.push(Piece { line: md_line, seg: Segment::Markdown(std::mem::take(&mut md)) });
            }
            let mut src = String::new();
            // 讀到關閉圍欄為止；沒有關閉圍欄時視為到文件結尾
            for l in lines.by_ref() {
                line_no += 1;
                if is_fence_close(l.trim_end_matches(['\r', '\n']), ch, n) {
                    break;
                }
                src.push_str(l);
            }
            out.push(Piece { line: this_line, seg: Segment::Mermaid(src) });
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
        if md.is_empty() {
            md_line = this_line;
        }
        md.push_str(line);
    }
    if !md.is_empty() {
        out.push(Piece { line: md_line, seg: Segment::Markdown(md) });
    }
    out
}

/// 把一段 Markdown 依頂層區塊（段落、標題、清單、程式碼、表格…）切開，回傳（區塊起始行, 區塊文字）。
/// 分開繪製後才能量到每個區塊在預覽中的位置，用來和編輯區同步捲動。
/// 連結參照定義（`[a]: url`）可能在別的區塊，所以附加到每個區塊後面；
/// 含註腳定義時定義與引用會跨區塊，改為整段不切。
pub fn split_blocks(md: &str) -> Vec<(usize, String)> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    // 與 egui_commonmark 解析時的選項一致，表格等才不會被切壞
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_DEFINITION_LIST;
    let parser = Parser::new_ext(md, opts);
    let mut defs = String::new();
    for (label, d) in parser.reference_definitions().iter() {
        defs.push_str(&format!("[{label}]: <{}>", d.dest));
        if let Some(t) = &d.title {
            defs.push_str(&format!(" \"{}\"", t.replace('"', "\\\"")));
        }
        defs.push('\n');
    }
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    for (ev, r) in parser.into_offset_iter() {
        match ev {
            Event::Start(Tag::FootnoteDefinition(_)) => return vec![(0, md.to_string())],
            Event::Start(_) => {
                if depth == 0 {
                    start = r.start;
                }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    ranges.push(start..r.end);
                }
            }
            _ if depth == 0 => ranges.push(r),
            _ => {}
        }
    }
    // 起始行用累加方式計算，避免每個區塊都從頭數換行
    let (mut line, mut pos) = (0usize, 0usize);
    ranges
        .into_iter()
        .map(|r| {
            line += md[pos..r.start].matches('\n').count();
            pos = r.start;
            let mut text = md[r.clone()].trim_end_matches('\n').to_string();
            if !defs.is_empty() {
                text.push_str("\n\n");
                text.push_str(&defs);
            }
            (line, text)
        })
        .collect()
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

/// 渲染完成、可顯示的 Mermaid 圖
struct Diagram {
    /// SVG 載入器的快取識別（含內容雜湊，內容或主題變更即換新）
    uri: String,
    width: f32,
    height: f32,
}

/// 單一片段的繪製資料
enum Content {
    Markdown(String),
    Diagram(Diagram),
    /// Mermaid 語法錯誤：訊息與原始碼
    Error { message: String, source: String },
}

/// 一個繪製項目與它在原始文件中的起始行（同步捲動用）
struct Item {
    line: usize,
    content: Content,
}

/// 縮放範圍與步進
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const ZOOM_STEP: f32 = 1.1;
/// 符合寬度時，重新排版用的寬度以此為單位量化，避免拖曳視窗邊緣時每個像素都重繪
const WIDTH_BUCKET: f32 = 32.0;
/// Mermaid 圖表的 uri 前綴，只有這類 uri 交給 [`DiagramLoader`]
const DIAGRAM_URI_PREFIX: &str = "bytes://mermaid-";
/// 圖表點陣化倍率上限（避免高倍縮放時產生過大的貼圖）
const RASTER_MAX: f32 = 6.0;

/// 依編輯區第一行，在（起始行, y）錨點之間線性內插出預覽應捲到的 y。
/// 錨點需依行號遞增排序；沒有錨點時回傳 None。
pub fn scroll_target(anchors: &[(usize, f32)], line: usize) -> Option<f32> {
    let i = anchors.partition_point(|a| a.0 <= line).checked_sub(1)?;
    let (l0, y0) = anchors[i];
    let Some(&(l1, y1)) = anchors.get(i + 1) else { return Some(y0) };
    if l1 <= l0 {
        return Some(y0);
    }
    Some(y0 + (y1 - y0) * (line - l0) as f32 / (l1 - l0) as f32)
}

/// Markdown 預覽視窗狀態
pub struct MarkdownPreview {
    /// 預覽中的文件識別碼
    pub doc_id: u64,
    /// 文件顯示名稱（視窗標題用）
    pub title: String,
    /// 已建立內容時的條件；任一變動就重建
    built: Option<BuildKey>,
    items: Vec<Item>,
    cache: CommonMarkCache,
    /// 縮放比例（文字與圖表一起縮放）
    zoom: f32,
    /// 符合寬度：圖表依視窗寬度重新排版，仍太寬才縮小
    fit_width: bool,
    /// 擺放方式：獨立視窗、編輯區右側或下方
    pub placement: Placement,
    /// 剛切換成並排，下一幀把面板設回編輯區的一半大小
    pub(super) reset_size: bool,
    /// 跟隨編輯區捲動（只在並排顯示時有意義，切換擺放方式時自動開關）
    sync_scroll: bool,
    /// 上一幀的內容可用寬度（下一次重建時決定圖表排版寬度）
    avail_w: f32,
    /// 上一幀量到的（起始行, y）錨點
    pub(super) anchors: Vec<(usize, f32)>,
    /// 上次同步時的編輯區第一行；變動才同步，使用者仍能自由捲動預覽
    pub(super) synced_line: Option<usize>,
    /// 並排時上一幀預覽面板所在範圍（主視窗據此避免 Ctrl+滾輪同時縮放編輯區字級）
    pub(super) rect: egui::Rect,
}

/// 重建預覽內容的條件
#[derive(Debug, Clone, PartialEq)]
struct BuildKey {
    version: u64,
    dark: bool,
    base_dir: Option<PathBuf>,
    /// 圖表排版寬度（量化後；0 表示不限）
    width: u32,
}

/// 預覽的擺放方式（類似 Visual Studio 的分割視窗）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// 獨立原生視窗
    Window,
    /// 左右並排：預覽在編輯區右側
    Right,
    /// 上下並排：預覽在編輯區下方
    Bottom,
}

/// 預覽視窗回報給 App 的動作
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MdAction {
    None,
    Close,
}

impl MarkdownPreview {
    pub fn new(doc_id: u64, title: String) -> Self {
        Self {
            doc_id,
            title,
            built: None,
            items: Vec::new(),
            cache: CommonMarkCache::default(),
            zoom: 1.0,
            fit_width: true,
            placement: Placement::Window,
            reset_size: false,
            sync_scroll: false,
            avail_w: 0.0,
            anchors: Vec::new(),
            synced_line: None,
            rect: egui::Rect::NOTHING,
        }
    }

    /// 符合寬度時給 fermaid 的排版寬度（量化後的像素，已扣除縮放）；不限制則為 0
    fn layout_width(&self) -> u32 {
        if !self.fit_width || self.avail_w <= 0.0 {
            return 0;
        }
        (((self.avail_w / self.zoom) / WIDTH_BUCKET).floor().max(1.0) * WIDTH_BUCKET) as u32
    }

    /// 文件版本、主題、資料夾或圖表排版寬度改變時才重建內容（Mermaid 渲染只在此發生，平時每幀不重算）
    pub fn refresh(&mut self, ctx: &egui::Context, text: &str, version: u64, dark: bool, base_dir: Option<PathBuf>) {
        // 另存新檔改變資料夾時，相對路徑圖片也要重新解析，所以資料夾也是重建條件
        let key = BuildKey { version, dark, base_dir, width: self.layout_width() };
        if self.built.as_ref() == Some(&key) {
            return;
        }
// SVG 載入器（resvg）不支援 CSS 變數，請 fermaid 直接依深淺色調色盤輸出實際顏色
        let palette = if dark { fermaid::Palette::dark() } else { fermaid::Palette::light() };
        let opts = fermaid::RenderOptions {
            max_width: (key.width > 0).then_some(key.width as f64),
            palette: Some(palette),
            ..Default::default()
        };
        let mut items = Vec::new();
        for piece in split_mermaid(text) {
            match piece.seg {
                Segment::Markdown(m) => {
                    let m = rewrite_image_paths(&m, key.base_dir.as_deref());
                    for (off, block) in split_blocks(&m) {
                        items.push(Item { line: piece.line + off, content: Content::Markdown(block) });
                    }
                }
                Segment::Mermaid(src) => {
                    let content = match fermaid::render(&src, &opts) {
                        Ok(r) => {
                            let svg = r.svg;
                            let mut h = std::collections::hash_map::DefaultHasher::new();
                            svg.hash(&mut h);
                            let uri = format!("{DIAGRAM_URI_PREFIX}{:016x}.svg", h.finish());
                            // 交給 egui 的位元組快取；同一內容的 uri 不變，已點陣化的圖會直接沿用
                            ctx.include_bytes(uri.clone(), svg.into_bytes());
                            Content::Diagram(Diagram {
                                uri,
                                width: r.width as f32,
                                height: r.height as f32,
                            })
                        }
                        Err(e) => Content::Error { message: format!("第 {} 行第 {} 欄：{}", e.line, e.column, e.message), source: src },
                    };
                    items.push(Item { line: piece.line, content });
                }
            }
        }
        // 只釋放這次不再使用的舊圖（點陣化很花時間，內容沒變的圖要保留快取），避免編輯過程累積
        let keep: std::collections::HashSet<&str> = items.iter().filter_map(|it| match &it.content {
            Content::Diagram(d) => Some(d.uri.as_str()),
            _ => None,
        }).collect();
        for it in &self.items {
            if let Content::Diagram(d) = &it.content
                && !keep.contains(d.uri.as_str())
            {
                ctx.forget_image(&d.uri);
            }
        }
        self.items = items;
        self.built = Some(key);
    }

    /// 是否與編輯區並排（停靠在主視窗內）
    pub fn docked(&self) -> bool {
        self.placement != Placement::Window
    }

    /// 切換擺放方式；並排時預設跟隨編輯區捲動並設為編輯區的一半大小，獨立視窗則不跟隨
    pub fn set_placement(&mut self, placement: Placement) {
        self.placement = placement;
        self.sync_scroll = self.docked();
        self.synced_line = None;
        self.reset_size = self.docked();
    }

    /// 工具列：縮放、符合寬度、並排顯示、同步捲動；回傳是否按下關閉
    fn toolbar(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = false;
        ui.horizontal(|ui| {
            if ui.button("−").on_hover_text("縮小（Ctrl+滾輪）").clicked() {
                self.zoom = (self.zoom / ZOOM_STEP).max(ZOOM_MIN);
            }
            if ui.button(format!("{:.0}%", self.zoom * 100.0)).on_hover_text("還原 100%").clicked() {
                self.zoom = 1.0;
            }
            if ui.button("+").on_hover_text("放大（Ctrl+滾輪）").clicked() {
                self.zoom = (self.zoom * ZOOM_STEP).min(ZOOM_MAX);
            }
            ui.separator();
            ui.checkbox(&mut self.fit_width, "符合寬度").on_hover_text("圖表依視窗寬度重新排版");
            ui.separator();
            let choices = [
                (Placement::Window, "獨立視窗", "以獨立視窗顯示預覽"),
                (Placement::Right, "左右並排", "預覽放在編輯區右側，各佔一半，並跟隨編輯區捲動"),
                (Placement::Bottom, "上下並排", "預覽放在編輯區下方，各佔一半，並跟隨編輯區捲動"),
            ];
            for (p, label, tip) in choices {
                if ui.selectable_label(self.placement == p, label).on_hover_text(tip).clicked() && self.placement != p {
                    self.set_placement(p);
                }
            }
            // 同步捲動只在並排時提供
            if self.docked() {
                ui.checkbox(&mut self.sync_scroll, "同步捲動").on_hover_text("預覽跟隨編輯區捲動");
                // 並排時沒有視窗標題列可關，另給關閉按鈕
                close = ui.button("×").on_hover_text("關閉預覽").clicked();
            }
        });
        close
    }

    /// 繪製預覽內容；`top_line` 是編輯區目前第一個可見的行（從 0 起算）
    pub fn ui(&mut self, ui: &mut egui::Ui, top_line: usize) -> MdAction {
        self.rect = ui.max_rect();
        // 獨立視窗才看視窗關閉與 Esc；並排時這些屬於主視窗與編輯區
        if !self.docked() && ui.ctx().input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
            return MdAction::Close;
        }
        let close = egui::Panel::top("md-preview-toolbar").show(ui, |ui| self.toolbar(ui)).inner;
        if close {
            return MdAction::Close;
        }

        // Ctrl+滾輪（或觸控板捏合）縮放；並排時只在滑鼠位於預覽上才縮放，避免與編輯區字級縮放衝突
        let delta = ui.input(|i| i.zoom_delta());
        if delta != 1.0 && (!self.docked() || ui.rect_contains_pointer(ui.max_rect())) {
            self.zoom = (self.zoom * delta).clamp(ZOOM_MIN, ZOOM_MAX);
        }

        // 編輯區第一行改變時，把預覽捲到對應位置
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if self.sync_scroll && self.synced_line != Some(top_line) {
            if let Some(y) = scroll_target(&self.anchors, top_line) {
                area = area.vertical_scroll_offset(y);
                self.synced_line = Some(top_line);
            }
        }

        let zoom = self.zoom;
        let mut anchors = Vec::with_capacity(self.items.len());
        let mut avail_w = self.avail_w;
        // 內容區自己鋪底色（獨立視窗沒有 CentralPanel，否則淺色模式下會露出深色的視窗底），並提高文字對比
        let dark = ui.visuals().dark_mode;
        let fill = egui::Frame::new().fill(if dark { Color32::BLACK } else { Color32::WHITE });
        egui::CentralPanel::default().frame(fill).show(ui, |ui| area.show(ui, |ui| {
            high_contrast(ui.visuals_mut());
            // 文字隨縮放放大（圖表另外依尺寸縮放）
            for ts in ui.style_mut().text_styles.values_mut() {
                ts.size *= zoom;
            }
            avail_w = ui.available_width();
            let origin = ui.cursor().min.y;
            let mut prev_markdown = false;
            for item in &self.items {
                anchors.push((item.line, ui.cursor().min.y - origin));
                match &item.content {
                    Content::Markdown(m) => {
                        // 區塊分開繪製後要自己補上 egui_commonmark 原本在區塊間插入的空行
                        if prev_markdown {
                            ui.add_space(ui.text_style_height(&egui::TextStyle::Body));
                        }
                        CommonMarkViewer::new().show(ui, &mut self.cache, m);
                    }
                    Content::Diagram(d) => {
                        // 依縮放比例顯示；符合寬度時，仍比視窗寬才縮小
                        let mut scale = zoom;
                        if self.fit_width && d.width * scale > avail_w {
                            scale = avail_w / d.width;
                        }
                        let size = egui::vec2(d.width * scale, d.height * scale);
                        // 點陣化倍率取 0.5 的級距（無條件進位），縮放或拉寬視窗時不必每幀重新點陣化，
                        // 只差一點的倍率直接由 GPU 縮放貼圖
                        let raster = ((scale * ui.ctx().pixels_per_point() * 2.0).ceil() / 2.0).clamp(0.5, RASTER_MAX);
                        let hint = egui::load::SizeHint::Scale(raster.ord());
                        match ui.ctx().try_load_texture(&d.uri, Default::default(), hint) {
                            Ok(egui::load::TexturePoll::Ready { texture }) => {
                                ui.add(egui::Image::from_texture(texture).fit_to_exact_size(size));
                            }
                            Ok(egui::load::TexturePoll::Pending { .. }) => {
                                ui.allocate_ui(size, |ui| ui.spinner());
                            }
                            Err(e) => {
                                ui.label(RichText::new(format!("圖表顯示失敗：{e}")).color(Color32::LIGHT_RED));
                            }
                        }
                        ui.add_space(8.0);
                    }
                    Content::Error { message, source } => {
                        ui.label(RichText::new(format!("Mermaid 語法錯誤：{message}")).color(Color32::LIGHT_RED));
                        ui.code(source);
                    }
                }
                prev_markdown = matches!(item.content, Content::Markdown(_));
            }
        }));
        self.anchors = anchors;
        // 寬度或縮放改變會讓圖表排版寬度跨級，下一幀由 refresh 重建
        self.avail_w = avail_w;
        MdAction::None
    }
}

/// 預覽內容的高對比配色：深色為純黑底、淺色為純白底；內文、粗體與標題都接近純黑／純白，
/// 程式碼、表格斑馬紋與引用等次要底色只比底色稍亮／稍暗，仍分得出區塊
fn high_contrast(v: &mut egui::Visuals) {
    let (text, strong, faint, code) = if v.dark_mode {
        (Color32::from_gray(225), Color32::WHITE, Color32::from_gray(22), Color32::from_gray(36))
    } else {
        (Color32::from_gray(28), Color32::BLACK, Color32::from_gray(244), Color32::from_gray(234))
    };
    v.panel_fill = if v.dark_mode { Color32::BLACK } else { Color32::WHITE };
    v.widgets.noninteractive.fg_stroke.color = text;
    v.widgets.active.fg_stroke.color = strong;
    // 引用與次要說明文字（egui 預設把內文透明度減半，在預覽中太淡）
    v.weak_text_color = Some(text.gamma_multiply(0.85));
    v.faint_bg_color = faint;
    v.extreme_bg_color = code;
    v.code_bg_color = code;
}

/// Mermaid 圖表專用的 SVG 載入器。
/// egui_extras 的 SVG 載入器要顯示文字就得載入全部系統字型（數百毫秒），
/// 這裡的字型資料庫只放一個字型，圖表文字都用它繪製。
pub struct DiagramLoader {
    options: resvg::usvg::Options<'static>,
    /// （uri, 尺寸提示）→ 點陣化結果
    cache: Mutex<HashMap<(String, SizeHint), Result<Arc<egui::ColorImage>, String>>>,
}

impl DiagramLoader {
    pub const ID: &'static str = egui::generate_loader_id!(DiagramLoader);

    /// 安裝到 egui（呼叫端先以 `ID` 確認尚未安裝）；`font` 為（家族名稱, 字型檔內容），沒有字型時圖表不顯示文字
    pub fn install(ctx: &egui::Context, font: Option<(String, fontique::Blob<u8>)>) {
        let mut options = resvg::usvg::Options::default();
        if let Some((name, blob)) = font {
            let db = options.fontdb_mut();
            // 直接共用 fontique 已載入的字型檔，不另外複製
            db.load_font_source(resvg::usvg::fontdb::Source::Binary(Arc::new(blob)));
            // fermaid 的字型清單以 sans-serif／monospace 結尾，兩者都對應到這個字型
            db.set_sans_serif_family(&name);
            db.set_monospace_family(&name);
        }
        ctx.add_image_loader(Arc::new(Self { options, cache: Mutex::new(HashMap::new()) }));
    }
}

impl ImageLoader for DiagramLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, size_hint: SizeHint) -> ImageLoadResult {
        // 其他圖片（Markdown 內的 png、svg 等）交給 egui_extras 的載入器
        if !uri.starts_with(DIAGRAM_URI_PREFIX) {
            return Err(LoadError::NotSupported);
        }
        let key = (uri.to_owned(), size_hint);
        let result = if let Some(r) = self.cache.lock().get(&key) {
            r.clone()
        } else {
            match ctx.try_load_bytes(uri) {
                Ok(BytesPoll::Ready { bytes, .. }) => {
                    let r = egui_extras::image::load_svg_bytes_with_size(&bytes, size_hint, &self.options).map(Arc::new);
                    self.cache.lock().insert(key, r.clone());
                    r
                }
                Ok(BytesPoll::Pending { size }) => return Ok(ImagePoll::Pending { size }),
                Err(e) => return Err(e),
            }
        };
        result.map(|image| ImagePoll::Ready { image }).map_err(LoadError::Loading)
    }

    fn forget(&self, uri: &str) {
        self.cache.lock().retain(|(u, _), _| u != uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache.lock().values().filter_map(|r| r.as_ref().ok()).map(|img| img.pixels.len() * 4).sum()
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
                Piece { line: 0, seg: Segment::Markdown("# 標題\n\n".into()) },
                Piece { line: 2, seg: Segment::Mermaid("graph LR\n  A-->B\n".into()) },
                Piece { line: 6, seg: Segment::Markdown("\n結尾\n".into()) },
            ]
        );
    }

    #[test]
    fn test_split_ignores_mermaid_inside_other_code_block() {
        let text = "```text\n```mermaid\nx\n```\n";
        // 第二行落在 text 圍欄內，整份都是一般 Markdown
        assert_eq!(split_mermaid(text), vec![Piece { line: 0, seg: Segment::Markdown(text.into()) }]);
    }

    #[test]
    fn test_split_unclosed_fence_and_tilde_and_crlf() {
        assert_eq!(split_mermaid("~~~mermaid\r\npie\r\n  \"a\": 1\r\n"), vec![Piece { line: 0, seg: Segment::Mermaid("pie\r\n  \"a\": 1\r\n".into()) }]);
        assert_eq!(split_mermaid("沒有圖\n"), vec![Piece { line: 0, seg: Segment::Markdown("沒有圖\n".into()) }]);
        assert!(split_mermaid("").is_empty());
    }

    #[test]
    fn test_split_requires_mermaid_info_word() {
        let text = "```mermaidx\nfoo\n```\n";
        assert_eq!(split_mermaid(text), vec![Piece { line: 0, seg: Segment::Markdown(text.into()) }]);
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
    fn test_split_blocks_records_start_lines() {
        let md = "# 標題\n\n段落一\n續行\n\n- a\n- b\n\n```rust\nfn x() {}\n```\n";
        let b = split_blocks(md);
        let lines: Vec<usize> = b.iter().map(|x| x.0).collect();
        assert_eq!(lines, vec![0, 2, 5, 8]);
        assert_eq!(b[1].1, "段落一\n續行");
        assert!(b[3].1.starts_with("```rust"));
    }

    #[test]
    fn test_split_blocks_keeps_table_whole_and_appends_ref_defs() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n\n見 [文件][r]\n\n[r]: https://x.com \"標題\"\n";
        let b = split_blocks(md);
        assert_eq!(b.len(), 2, "{b:?}");
        assert!(b[0].1.starts_with("| a | b |") && b[0].1.contains("| 1 | 2 |"));
        assert!(b[1].1.contains("[r]: <https://x.com> \"標題\""), "{}", b[1].1);
    }

    #[test]
    fn test_split_blocks_footnotes_not_split() {
        let md = "文字[^1]\n\n[^1]: 註腳\n";
        assert_eq!(split_blocks(md), vec![(0, md.to_string())]);
    }

    #[test]
    fn test_scroll_target_interpolates_between_anchors() {
        let a = [(0, 0.0), (10, 100.0), (20, 400.0)];
        assert_eq!(scroll_target(&a, 0), Some(0.0));
        assert_eq!(scroll_target(&a, 5), Some(50.0));
        assert_eq!(scroll_target(&a, 15), Some(250.0));
        // 超過最後一個錨點就停在最後錨點
        assert_eq!(scroll_target(&a, 99), Some(400.0));
        assert_eq!(scroll_target(&[], 3), None);
        // 第一個錨點之前（例如文件開頭是空行）沒有可用位置
        assert_eq!(scroll_target(&[(2, 30.0)], 0), None);
    }

    #[test]
    fn test_render_with_palette_has_no_css_vars() {
        for p in [fermaid::Palette::light(), fermaid::Palette::dark()] {
            let opts = fermaid::RenderOptions { palette: Some(p), ..Default::default() };
            let r = fermaid::render("flowchart LR\n  A[開始] --> B[結束]", &opts).unwrap();
            assert!(!r.svg.contains("var(--"), "SVG 仍含 CSS 變數");
        }
    }

    /// 圖表載入器只放一個字型也能畫出文字：有字型時的點陣圖要比沒有字型時多出文字筆畫
    #[test]
    fn test_diagram_loader_draws_text_with_single_font() {
        let Some(font) = crate::app::fonts::FontManager::new().diagram_font() else {
            return; // 測試環境沒有任何候選字型
        };
        let opts = fermaid::RenderOptions { palette: Some(fermaid::Palette::light()), ..Default::default() };
        let svg = fermaid::render("flowchart LR\n  A[開始 Start] --> B[結束]", &opts).unwrap().svg;
        let raster = |font| {
            let ctx = egui::Context::default();
            DiagramLoader::install(&ctx, font);
            let uri = format!("{DIAGRAM_URI_PREFIX}test.svg");
            ctx.include_bytes(uri.clone(), svg.clone().into_bytes());
            match ctx.try_load_image(&uri, SizeHint::default()).unwrap() {
                ImagePoll::Ready { image } => image,
                ImagePoll::Pending { .. } => panic!("應同步完成點陣化"),
            }
        };
        let with = raster(Some(font));
        let without = raster(None);
        assert_eq!(with.size, without.size);
        let diff = with.pixels.iter().zip(&without.pixels).filter(|(a, b)| a != b).count();
        assert!(diff > 100, "文字未繪製（差異像素 {diff}）");
    }
}
