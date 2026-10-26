//! 介面語系。繁體中文是程式裡的原文；其他語言放在 `assets/i18n/messages.tsv`。
//! 找不到譯文時依序退回英文、再退回繁體中文原文。

use std::collections::HashMap;
use std::sync::OnceLock;

use super::commands::C;
use crate::core::settings::UiLanguage;

/// 實際用來查表的語言（已不含「跟隨系統」）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    ZhTw,
    ZhCn,
    Ja,
    Ko,
    De,
    Es,
    Fr,
    It,
    Pt,
}

/// TSV 欄位順序：en, zh-CN, ja, ko, de, es, fr, it, pt。繁體中文不在表內。
fn column(lang: Lang) -> Option<usize> {
    Some(match lang {
        Lang::En => 0,
        Lang::ZhCn => 1,
        Lang::Ja => 2,
        Lang::Ko => 3,
        Lang::De => 4,
        Lang::Es => 5,
        Lang::Fr => 6,
        Lang::It => 7,
        Lang::Pt => 8,
        Lang::ZhTw => return None,
    })
}

fn catalog() -> &'static HashMap<&'static str, [&'static str; 9]> {
    static CELL: OnceLock<HashMap<&'static str, [&'static str; 9]>> = OnceLock::new();
    CELL.get_or_init(|| {
        let text = include_str!("../../assets/i18n/messages.tsv");
        let mut map = HashMap::new();
        for (n, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            assert!(parts.len() == 10, "messages.tsv 第 {} 行應有 10 欄，實際 {}", n + 1, parts.len());
            let mut cols = [""; 9];
            for (i, col) in parts[1..].iter().enumerate() {
                cols[i] = col;
            }
            let key = parts[0];
            assert!(map.insert(key, cols).is_none(), "messages.tsv 重複的鍵：{key}");
        }
        map
    })
}

/// 把作業系統的地區字串（如 `zh_TW.UTF-8`、`pt-BR`）對到支援的語言。
/// 不認識的地區使用英文。
pub fn map_locale(raw: &str) -> Lang {
    let tag = raw.trim().to_ascii_lowercase().replace('_', "-");
    let tag = tag.split('.').next().unwrap_or(tag.as_str());
    if tag.is_empty() || tag == "c" || tag == "posix" {
        return Lang::En;
    }
    let primary = tag.split('-').next().unwrap_or("");
    if primary == "zh" {
        if tag.contains("hant") || tag.contains("-tw") || tag.contains("-hk") || tag.contains("-mo") {
            return Lang::ZhTw;
        }
        return Lang::ZhCn;
    }
    match primary {
        "en" => Lang::En,
        "ja" => Lang::Ja,
        "ko" => Lang::Ko,
        "de" => Lang::De,
        "es" => Lang::Es,
        "fr" => Lang::Fr,
        "it" => Lang::It,
        "pt" => Lang::Pt,
        _ => Lang::En,
    }
}

fn system_lang() -> Lang {
    static CELL: OnceLock<Lang> = OnceLock::new();
    *CELL.get_or_init(|| sys_locale::get_locale().map(|s| map_locale(&s)).unwrap_or(Lang::En))
}

pub fn resolve(choice: UiLanguage) -> Lang {
    match choice {
        UiLanguage::System => system_lang(),
        UiLanguage::En => Lang::En,
        UiLanguage::ZhTw => Lang::ZhTw,
        UiLanguage::ZhCn => Lang::ZhCn,
        UiLanguage::Ja => Lang::Ja,
        UiLanguage::Ko => Lang::Ko,
        UiLanguage::De => Lang::De,
        UiLanguage::Es => Lang::Es,
        UiLanguage::Fr => Lang::Fr,
        UiLanguage::It => Lang::It,
        UiLanguage::Pt => Lang::Pt,
    }
}

/// 選單項目顯示用：去掉 Windows 風格的 `&` 加速鍵，`(&N)` 保留成 `(N)`。
pub fn strip_mnemonic(label: &str) -> String {
    label.replace("(&", "(").replace('&', "")
}

/// 查譯文。`zh_tw` 是繁體中文原文，也是最後的退路。
pub fn translate<'a>(lang: Lang, key: &str, zh_tw: &'a str) -> &'a str {
    if lang == Lang::ZhTw {
        return zh_tw;
    }
    if let Some(text) = lookup(lang, key) {
        return text;
    }
    if lang != Lang::En
        && let Some(text) = lookup(Lang::En, key)
    {
        return text;
    }
    zh_tw
}

fn lookup(lang: Lang, key: &str) -> Option<&'static str> {
    let col = column(lang)?;
    let text = catalog().get(key).map(|cols| cols[col])?;
    if text.is_empty() { None } else { Some(text) }
}

pub fn command_text(lang: Lang, c: C) -> String {
    strip_mnemonic(translate(lang, c.id(), c.label()))
}

/// 把譯文裡的 `{n}`、`{q}` 換成數字或文字。
pub fn fill<'a>(template: &'a str, n: Option<usize>, q: Option<&str>) -> String {
    let mut s = template.to_string();
    if let Some(n) = n {
        s = s.replace("{n}", &n.to_string());
    }
    if let Some(q) = q {
        s = s.replace("{q}", q);
    }
    s
}

/// 語言選單上「跟隨系統」要翻成目前介面語言，其他語言維持自己的名稱。
pub fn language_choice_label(ui_lang: Lang, choice: UiLanguage) -> &'static str {
    if choice == UiLanguage::System {
        translate(ui_lang, "lang.system", "跟隨系統")
    } else {
        choice.native_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::commands::C;

    #[test]
    fn locale_tags_map_to_supported_languages() {
        assert_eq!(map_locale("zh_TW.UTF-8"), Lang::ZhTw);
        assert_eq!(map_locale("zh-Hant-HK"), Lang::ZhTw);
        assert_eq!(map_locale("zh-MO"), Lang::ZhTw);
        assert_eq!(map_locale("zh-Hans-CN"), Lang::ZhCn);
        assert_eq!(map_locale("zh_CN"), Lang::ZhCn);
        assert_eq!(map_locale("zh"), Lang::ZhCn);
        assert_eq!(map_locale("ja_JP"), Lang::Ja);
        assert_eq!(map_locale("ko-KR"), Lang::Ko);
        assert_eq!(map_locale("de_DE.UTF-8"), Lang::De);
        assert_eq!(map_locale("es-MX"), Lang::Es);
        assert_eq!(map_locale("fr_FR"), Lang::Fr);
        assert_eq!(map_locale("it-IT"), Lang::It);
        assert_eq!(map_locale("pt_BR"), Lang::Pt);
        assert_eq!(map_locale("pt-PT"), Lang::Pt);
        assert_eq!(map_locale("en-US"), Lang::En);
        assert_eq!(map_locale("C"), Lang::En);
        assert_eq!(map_locale("POSIX"), Lang::En);
        assert_eq!(map_locale(""), Lang::En);
        assert_eq!(map_locale("th_TH"), Lang::En);
    }

    #[test]
    fn traditional_chinese_uses_source_text() {
        assert_eq!(translate(Lang::ZhTw, "menuNew", "新增檔案(&N)"), "新增檔案(&N)");
    }

    #[test]
    fn missing_key_falls_back_to_source() {
        assert_eq!(translate(Lang::De, "no-such-key", "原文"), "原文");
    }

    #[test]
    fn every_command_is_translated() {
        const ALL: [Lang; 10] = [
            Lang::En,
            Lang::ZhTw,
            Lang::ZhCn,
            Lang::Ja,
            Lang::Ko,
            Lang::De,
            Lang::Es,
            Lang::Fr,
            Lang::It,
            Lang::Pt,
        ];
        for c in crate::app::commands::ALL {
            for lang in ALL {
                if lang == Lang::ZhTw {
                    continue;
                }
                let text = translate(lang, c.id(), "");
                assert!(!text.is_empty(), "{} 缺少 {lang:?}", c.id());
            }
        }
    }

    #[test]
    fn every_markdown_preview_text_is_translated() {
        let keys: Vec<_> = catalog().iter().filter(|(k, _)| k.starts_with("md.")).collect();
        assert!(!keys.is_empty());
        for (key, cols) in keys {
            assert!(cols.iter().all(|c| !c.is_empty()), "{key} 缺少譯文");
        }
    }

    #[test]
    fn english_new_file_keeps_mnemonic() {
        assert_eq!(command_text(Lang::En, C::New), "New (N)");
        assert_eq!(command_text(Lang::ZhTw, C::Find), "尋找(F)...");
    }
}
