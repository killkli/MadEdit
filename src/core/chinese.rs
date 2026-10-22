//! 簡繁轉換與日文漢字轉換（對照表取自原版 MadEdit）

use std::collections::HashMap;
use std::sync::OnceLock;

use super::chinese_tables::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChineseConversion {
    SimpToTrad,
    TradToSimp,
    KanjiToTrad,
    KanjiToSimp,
    ChineseToKanji,
}

/// 依轉換方向與安全模式建立對照表（安全模式不轉換易誤轉的字，如「于」→「於」）
fn table(conv: ChineseConversion, safe: bool) -> &'static HashMap<char, char> {
    static CACHE: OnceLock<HashMap<(ChineseConversion, bool), HashMap<char, char>>> = OnceLock::new();
    let all = CACHE.get_or_init(|| {
        let build = |base: &[(u32, u32)], unsafe_extra: Option<&[(u32, u32)]>| {
            let mut m: HashMap<char, char> = base
                .iter()
                .filter_map(|&(a, b)| Some((char::from_u32(a)?, char::from_u32(b)?)))
                .collect();
            if let Some(extra) = unsafe_extra {
                for &(a, b) in extra {
                    if let (Some(a), Some(b)) = (char::from_u32(a), char::from_u32(b)) {
                        m.insert(a, b);
                    }
                }
            }
            m
        };
        let mut all = HashMap::new();
        for safe in [true, false] {
            let pick = |t: &'static [(u32, u32)]| (!safe).then_some(t);
            all.insert((ChineseConversion::SimpToTrad, safe), build(SIMP2TRAD, pick(SIMP2TRAD_UNSAFE)));
            all.insert((ChineseConversion::TradToSimp, safe), build(TRAD2SIMP, pick(TRAD2SIMP_UNSAFE)));
            all.insert((ChineseConversion::KanjiToTrad, safe), build(KANJI2TRAD, None));
            all.insert((ChineseConversion::KanjiToSimp, safe), build(KANJI2SIMP, None));
            all.insert((ChineseConversion::ChineseToKanji, safe), build(CHINESE2KANJI, None));
        }
        all
    });
    &all[&(conv, safe)]
}

/// 轉換文字，回傳（結果, 轉換字數）
pub fn convert(text: &str, conv: ChineseConversion, safe: bool) -> (String, usize) {
    let t = table(conv, safe);
    let mut count = 0;
    let out = text
        .chars()
        .map(|c| match t.get(&c) {
            Some(&r) => {
                count += 1;
                r
            }
            None => c,
        })
        .collect();
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simp_trad_roundtrip() {
        assert_eq!(convert("简体中文", ChineseConversion::SimpToTrad, true).0, "簡體中文");
        assert_eq!(convert("繁體中文", ChineseConversion::TradToSimp, true).0, "繁体中文");
        // 安全模式不轉換「于」
        assert_eq!(convert("于", ChineseConversion::SimpToTrad, true).0, "于");
        assert_eq!(convert("于", ChineseConversion::SimpToTrad, false).0, "於");
    }
}
