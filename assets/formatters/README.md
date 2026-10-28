# 隨附格式化工具（「帶 DPRINT」發行版）

這個目錄存放**版本與雜湊的唯一來源**，以及要放進發行包的設定檔：

- `manifest.json` — dprint 執行檔與每支 Wasm 插件的固定版本、下載網址與 SHA-256。
  `scripts/fetch_formatter.sh`（macOS/Linux）與 `scripts/fetch_formatter.ps1`（Windows）
  只從這裡取得版本，下載後逐檔驗雜湊，輸出到 `target/formatters/`。
- `dprint.json` — 隨附的 dprint 設定；插件一律用 `./plugins/*.wasm` 相對路徑，
  不會在執行期上網下載。`src/core/dprint.rs::LANGUAGES` 必須與這份清單一致
  （`test_bundled_config_lists_exactly_the_supported_plugins` 會檢查）。
- `dprint.entitlements` — macOS helper 簽章用的最小權限
  （`com.apple.security.cs.allow-unsigned-executable-memory`，wasmtime 需要）。

收錄的語言（經典＋常用、離線可用，clang-format 為必要項）：

| 插件 | 副檔名／檔名 |
| --- | --- |
| clang-format | c cc cpp cxx h hh hpp hxx m mm |
| typescript | ts tsx js jsx mjs cjs mts cts |
| json | json jsonc |
| malva | css scss sass less |
| markup | html vue svelte astro xml svg jinja j2 twig njk vto mustache hbs handlebars wsdl xsd xslt xsl |
| yaml | yaml yml |
| markdown | md markdown |
| toml | toml |
| graphql | graphql gql |
| sql | sql |
| dockerfile | dockerfile（`Dockerfile` 檔名） |
| shfmt | sh bash zsh mksh bats（`.bashrc`、`.zshrc` 等檔名） |
| ruff | py |
| gofumpt | go |
| mago | php |
| stylua | lua luau |
| csharpier | cs csx csproj props targets slnx config xaml axaml（`.xml` 由 markup 優先） |

17 支插件解壓後約 56 MB；全部為離線 Wasm，首次使用時由 dprint 編譯到使用者快取。

Rust 沒有收錄：dprint 對 Rust 仍是轉呼外部 `rustfmt`，未隨附。Java/Ruby 沒有
現成的 dprint 插件。

已知限制：`clangFormat` 區段必須留空——clang-format 0.2.1 的 Wasm 插件不接受
區段內的選項（任何鍵都會回報 configuration error）；縮排與行寬來自全域的
`indentWidth` / `lineWidth`（對應 `IndentWidth` / `ColumnLimit`）。升級插件後可在
`dprint.json` 的 `clangFormat` 內加入 clang-format 原生選項（如 `BasedOnStyle`）。

## 升級方式

1. 改 `manifest.json` 的版本、網址與 SHA-256（官方 release 頁與
   plugins.dprint.dev 會公布或可自行計算）。
2. `rm -rf target/formatters && make formatters` 重新下載。
3. `cargo test dprint -- --ignored` 跑端到端煙霧測試。
4. 各平台以 `MADEDIT_WITH_FORMATTER=1` 重新打包，並實際開 App 跑一次
   格式化（macOS 特別要驗證 helper 簽章）。

## 發行變體

每個平台同時產出兩種包：乾淨版（名稱不變，不含任何 dprint 檔案）與
`-dprint` 版（額外帶 `dprint` 執行檔、`dprint.json` 與 `plugins/`）。
乾淨版使用此功能時會退回 PATH 上的 `dprint`，設定檔同樣使用本目錄的
`dprint.json`。
