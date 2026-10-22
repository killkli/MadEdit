Status: ready-for-agent

# Issue 01: 核心搜尋引擎資料結構與比對邏輯

## Description

引入 `regex` crate，實作 `SearchOptions`、`SearchMatch` 與核心比對函式（支援大小寫敏感、全字匹配、正規表達式與 Hex Byte 解析比對）。

## Tasks

1. 在 `Cargo.toml` 引入 `regex = "1.11"`。
2. 建立 `src/core/search.rs`，定義 `SearchOptions` 與 `SearchMatch`。
3. 實作 `find_matches(text: &str, query: &str, options: &SearchOptions) -> Result<Vec<SearchMatch>, String>`。
4. 實作 `parse_hex_query(query: &str) -> Result<Vec<u8>, String>` 與十六進位位元組序列搜尋。
5. 單元測試覆蓋：大小寫匹配、全字邊界匹配、正規表達式語意與十六進位解析比對。
