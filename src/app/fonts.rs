//! 系統字型清單與 egui 字型設定（編輯區字型可由使用者任選，CJK 與 emoji 由後備字型補字）

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// 編輯區使用的字型家族名稱（egui 內部名稱）
pub const EDITOR_FAMILY: &str = "editor";

/// 未指定字型時，依序挑選第一個已安裝的等寬字型
const DEFAULT_EDITOR_FONTS: &[&str] = &[
    "Sarasa Term TC",
    "Sarasa Mono TC",
    "Sarasa Term SC",
    "Noto Sans Mono CJK TC",
    "Menlo",
    "SF Mono",
    "Consolas",
    "Cascadia Mono",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Courier New",
];

/// CJK 後備字型（介面與編輯區共用）
const CJK_FALLBACKS: &[&str] = &[
    "PingFang TC",
    "Heiti TC",
    "Hiragino Sans",
    "Microsoft JhengHei",
    "Microsoft YaHei",
    "Noto Sans CJK TC",
    "Noto Sans TC",
    "Source Han Sans TC",
    "WenQuanYi Micro Hei",
    "Droid Sans Fallback",
];

pub struct FontManager {
    collection: fontique::Collection,
    /// 系統字型家族名稱（已排序）
    pub families: Vec<String>,
    /// 目前實際套用的編輯區字型
    pub editor_family: String,
    cjk_family: Option<String>,
}

impl FontManager {
    pub fn new() -> Self {
        let mut collection = fontique::Collection::new(fontique::CollectionOptions {
            shared: false,
            system_fonts: true,
        });
        let mut families: Vec<String> = collection
            .family_names()
            .filter(|n| !n.starts_with('.'))
            .map(str::to_string)
            .collect();
        families.sort_by_key(|n| n.to_lowercase());
        families.dedup();
        let cjk_family = CJK_FALLBACKS
            .iter()
            .find(|n| collection.family_by_name(n).is_some())
            .map(|n| n.to_string());
        Self {
            collection,
            families,
            editor_family: String::new(),
            cjk_family,
        }
    }

    /// 預設的編輯區字型（第一個已安裝的偏好字型）
    pub fn default_editor_family(&mut self) -> String {
        DEFAULT_EDITOR_FONTS
            .iter()
            .find(|n| self.collection.family_by_name(n).is_some())
            .map(|n| n.to_string())
            .unwrap_or_default()
    }

    /// 載入字型家族的一般字重字型檔
    fn load_family(&mut self, name: &str) -> Option<FontData> {
        let family = self.collection.family_by_name(name)?;
        let font = family.default_font()?;
        let blob = font.load(None)?;
        let mut data = FontData::from_owned(blob.data().to_vec());
        data.index = font.index();
        Some(data)
    }

    /// 依設定的編輯區字型重建 egui 字型定義；`requested` 為空字串時自動挑選
    pub fn apply(&mut self, ctx: &egui::Context, requested: &str) {
        let mut defs = FontDefinitions::default();
        let mut editor_chain: Vec<String> = Vec::new();

        let wanted = if requested.is_empty() { self.default_editor_family() } else { requested.to_string() };
        self.editor_family = String::new();
        if !wanted.is_empty()
            && let Some(data) = self.load_family(&wanted)
        {
            defs.font_data.insert("editor-main".into(), Arc::new(data));
            editor_chain.push("editor-main".into());
            self.editor_family = wanted;
        }

        // 編輯區字型與 CJK 後備字型相同時直接共用已載入的資料，介面仍須補上 CJK
        let cjk_key = match self.cjk_family.clone() {
            Some(cjk) if cjk == self.editor_family => Some("editor-main"),
            Some(cjk) => self.load_family(&cjk).map(|data| {
                defs.font_data.insert("cjk".into(), Arc::new(data));
                editor_chain.push("cjk".into());
                "cjk"
            }),
            None => None,
        };
        if let Some(key) = cjk_key {
            // 介面字型：預設字型之後立即以 CJK 補字（emoji 仍排在最後）
            for fam in [FontFamily::Proportional, FontFamily::Monospace] {
                if let Some(list) = defs.families.get_mut(&fam) {
                    list.insert(1.min(list.len()), key.into());
                }
            }
        }

        // 編輯區字型鏈：使用者字型 → CJK → egui 內建等寬字型與 emoji
        if let Some(mono) = defs.families.get(&FontFamily::Monospace) {
            for f in mono {
                if !editor_chain.contains(f) {
                    editor_chain.push(f.clone());
                }
            }
        }
        defs.families.insert(FontFamily::Name(EDITOR_FAMILY.into()), editor_chain);
        ctx.set_fonts(defs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 編輯區選用 CJK 後備字型本身（如 PingFang TC）時，介面字型仍須含 CJK 補字
    #[test]
    fn test_ui_keeps_cjk_when_editor_font_is_cjk_fallback() {
        let mut fm = FontManager::new();
        let Some(cjk) = fm.cjk_family.clone() else { return };
        let ctx = egui::Context::default();
        fm.apply(&ctx, &cjk);
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        let ok = ctx.fonts(|f| f.definitions().families[&FontFamily::Proportional].contains(&"editor-main".to_string()));
        assert!(ok, "介面字型鏈缺少 CJK 字型");
    }
}
