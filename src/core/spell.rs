//! 拼字檢查：載入 Hunspell 字典（.aff / .dic），提供檢查與建議

use std::path::{Path, PathBuf};

/// 內建的英文字典（其他語言自字典資料夾載入）
const EN_US_AFF: &[u8] = include_bytes!("../../assets/dictionaries/en_US.aff");
const EN_US_DIC: &[u8] = include_bytes!("../../assets/dictionaries/en_US.dic");

pub struct SpellChecker {
    dict: spellbook::Dictionary,
}

/// Hunspell 檔案依 .aff 中的 SET 指定編碼（未指定時為 ISO-8859-1），轉為 UTF-8 文字
fn decode_dictionary(aff: &[u8], dic: &[u8]) -> (String, String) {
    let set = aff
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"SET "))
        .map(|v| String::from_utf8_lossy(v).trim().to_string())
        .unwrap_or_else(|| "ISO8859-1".into());
    // encoding_rs 認得 "iso-8859-N"，Hunspell 慣用 "ISO8859-N"
    let label = set.to_ascii_lowercase().replacen("iso8859-", "iso-8859-", 1);
    let enc = encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252);
    let aff = enc.decode(aff).0.into_owned();
    // 內容已轉成 UTF-8，SET 也要一併改寫
    let aff: String = aff
        .lines()
        .map(|l| if l.starts_with("SET ") { "SET UTF-8" } else { l })
        .collect::<Vec<_>>()
        .join("\n");
    (aff, enc.decode(dic).0.into_owned())
}

/// 可能存放字典的資料夾：設定目錄、執行檔旁、macOS App 的 Resources 與開發時的 assets
pub fn dictionary_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = dirs::config_dir() {
        dirs.push(d.join("MadEdit").join("dictionaries"));
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        dirs.push(exe_dir.join("dictionaries"));
        dirs.push(exe_dir.join("../Resources/dictionaries"));
    }
    dirs.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/dictionaries")));
    dirs
}

/// 列出可用的字典語言（如 en_US、fr_FR）
pub fn available_languages() -> Vec<String> {
    let mut langs = vec!["en_US".to_string()];
    for dir in dictionary_dirs() {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "dic") {
                let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
                if p.with_extension("aff").exists() && !langs.contains(&stem) {
                    langs.push(stem);
                }
            }
        }
    }
    langs.sort();
    langs
}

impl SpellChecker {
    /// 載入指定語言的字典
    pub fn load(language: &str, personal: &[String]) -> Result<Self, String> {
        let (aff, dic) = if language == "en_US" {
            (EN_US_AFF.to_vec(), EN_US_DIC.to_vec())
        } else {
            dictionary_dirs()
                .into_iter()
                .find_map(|d| {
                    let aff = std::fs::read(d.join(format!("{language}.aff"))).ok()?;
                    let dic = std::fs::read(d.join(format!("{language}.dic"))).ok()?;
                    Some((aff, dic))
                })
                .ok_or_else(|| format!("找不到 {language} 字典"))?
        };
        let (aff, dic) = decode_dictionary(&aff, &dic);
        let mut dict = spellbook::Dictionary::new(&aff, &dic).map_err(|e| format!("字典格式錯誤：{e}"))?;
        for w in personal {
            let _ = dict.add(w);
        }
        Ok(Self { dict })
    }

    pub fn check(&self, word: &str) -> bool {
        self.dict.check(word)
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.dict.suggest(word, &mut out);
        out.truncate(8);
        out
    }

    /// 加入個人字典（本次執行期間有效，持久化由設定負責）
    pub fn add_word(&mut self, word: &str) {
        let _ = self.dict.add(word);
    }

    /// 找出一行中拼錯的單字（回傳字元索引範圍）；只檢查由字母組成的拉丁字詞
    pub fn misspelled_ranges(&self, line: &str) -> Vec<(usize, usize)> {
        let chars: Vec<char> = line.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_alphabetic() && !is_cjk(chars[i]) {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphabetic() || (chars[i] == '\'' && i + 1 < chars.len() && chars[i + 1].is_alphabetic()))
                    && !is_cjk(chars[i])
                {
                    i += 1;
                }
                // 緊鄰數字或底線的字詞（如變數名稱）不檢查
                let glued = (start > 0 && (chars[start - 1].is_ascii_digit() || chars[start - 1] == '_'))
                    || (i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_'));
                let word: String = chars[start..i].iter().collect();
                if !glued && word.chars().count() > 1 && !self.check(&word) {
                    out.push((start, i));
                }
            } else {
                i += 1;
            }
        }
        out
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_english_dictionary() {
        let sc = SpellChecker::load("en_US", &["MadEdit".to_string()]).unwrap();
        assert!(sc.check("hello"));
        assert!(!sc.check("helo"));
        assert!(sc.suggest("helo").iter().any(|s| s == "hello"));
        assert!(sc.check("MadEdit"));
        assert_eq!(sc.misspelled_ranges("This is wrnog 中文 x1abc"), vec![(8, 13)]);
    }

    #[test]
    fn test_latin1_dictionary_loads() {
        // 法文字典為 ISO-8859 編碼，需轉換後才能載入
        let sc = SpellChecker::load("fr_FR", &[]).unwrap();
        assert!(sc.check("bonjour"));
    }
}
