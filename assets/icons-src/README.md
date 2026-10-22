# 工具列圖示來源說明

本目錄存放**原版 MadEdit 的 XPM 圖示來源檔**，是 `assets/icons/*.png` 的唯一真實來源。
所有 PNG 均由 `../make_toolbar_icons.py` 自動轉換而得，**請勿手動編輯 PNG**。

## 來源專案

| 項目 | 說明 |
| --- | --- |
| 上游專案 | [MadEdit](https://github.com/madedit/madedit)（C++ / wxWidgets 跨平台文字與十六進位編輯器） |
| 圖示來源路徑 | MadEdit / MadEdit-Mod 原始碼中的 `images/*.xpm` |
| 原始格式 | XPM（X PixMap），16×16 像素 |
| 授權 | GNU GPL（上游 MadEdit 為 GPL v2+；MadEdit-Mod 分支為 GPL v3） |

MadEdit 與 MadEdit-Mod 皆為自由軟體，其圖示同樣受 GNU GPL 授權。
本專案（MadEdit Rust 重製版）為該專案的衍生重製，**整體散布時須遵循相同之 GNU GPL 條款**，
並在散布物中保留本說明文件與原版授權聲明。

## 圖示清單（共 38 個）

### 檔案

| 圖示 | 用途 | 圖示 | 用途 |
| --- | --- | --- | --- |
| `new.xpm` | 新增檔案 | `fileopen.xpm` | 開啟檔案 |
| `filesave.xpm` | 儲存檔案 | `saveall.xpm` | 全部儲存 |
| `fileclose.xpm` | 關閉檔案 | `closeall.xpm` | 全部關閉 |
| `reload.xpm` | 重新載入 | `print.xpm` | 列印（未實作） |
| `preview.xpm` | 預覽列印（未實作） | `quit.xpm` | 離開程式 |

### 編輯

| 圖示 | 用途 | 圖示 | 用途 |
| --- | --- | --- | --- |
| `undo.xpm` | 復原（未實作） | `redo.xpm` | 重做（未實作） |
| `cut.xpm` | 剪下 | `copy.xpm` | 複製 |
| `paste.xpm` | 貼上 | `find.xpm` | 搜尋 |
| `findnext.xpm` | 下一個符合項 | `findprev.xpm` | 上一個符合項 |
| `replace.xpm` | 取代 | `comment.xpm` | 註解 |
| `uncomment.xpm` | 取消註解 | `indent.xpm` | 增加縮排 |
| `unindent.xpm` | 減少縮排 | `down.xpm` | 向下移動 |

### 檢視與模式

| 圖示 | 用途 | 圖示 | 用途 |
| --- | --- | --- | --- |
| `textmode.xpm` | 文字模式 | `columnmode.xpm` | 列塊模式 |
| `hexmode.xpm` | 十六進位模式 | `nowrap.xpm` | 不自動換行 |
| `wrapbycol.xpm` | 依欄位換行 | `wrapbywin.xpm` | 依視窗換行 |
| `encoding.xpm` | 字元編碼 | `syntax.xpm` | 語法突顯 |

### 導覽與輔助

| 圖示 | 用途 | 圖示 | 用途 |
| --- | --- | --- | --- |
| `goposition.xpm` | 指定位置跳轉 | `bookmark_toggle.xpm` | 切換書籤 |
| `bookmark_next.xpm` | 下一個書籤 | `bookmark_prev.xpm` | 上一個書籤 |
| `bookmark_clear.xpm` | 清除書籤 | `help.xpm` | 說明 |

## 重新產生 PNG

```bash
cd /home/tenyi/Projects/MadEdit
python3 assets/make_toolbar_icons.py            # 產生 assets/icons/*.png（2 倍放大，32×30）
python3 assets/make_toolbar_icons.py --scale 1  # 產生 1 倍尺寸（16×15）
```

轉換腳本僅使用 Python 標準函式庫（`zlib` + `struct`），不引入額外相依套件，
並以最近鄰插值放大以保留像素圖的銳利邊緣。
