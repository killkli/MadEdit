# Issue 1: 核心語法擴充（C# 與深淺色主題）、覆寫模式、控制字元渲染與狀態列計算

## 狀態
closed

## 目標
擴充 `syntax.rs`、`editor.rs`、`model.rs` 與 `buffer.rs`，支援深淺色語法高亮、C# 等新語言、Insert/Overwrite 覆寫模式、可視控制字元轉換（Tab `→`、空白 `·`、行尾 `↵` 座標）、以及原版 MadEdit 多格狀態列所需的字元/位元組位置統計。
