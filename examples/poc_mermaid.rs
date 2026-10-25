//! PoC：fermaid 產生 SVG → 解析 CSS 變數 → resvg 轉 PNG，驗證中文與配色
use std::collections::HashMap;

/// resvg 不支援 CSS `var()`，這裡把 `var(--名稱,後備值)` 以主題色或後備值替換
fn resolve_vars(svg: &str, theme: &HashMap<&str, &str>) -> String {
    let mut s = svg.to_string();
    // 根元素 style 內的中介變數 `--_x:值`，每輪解析後再收集
    let mut local: HashMap<String, String> = HashMap::new();
    // 先收集 `--_x:值` 的宣告（值可能又是 var(...)）
    for _ in 0..6 {
        let mut out = String::new();
        let mut rest = s.as_str();
        while let Some(i) = rest.find("var(--") {
            out.push_str(&rest[..i]);
            let after = &rest[i + 4..];
            // 找對應的右括號（支援巢狀）
            let mut depth = 1;
            let mut end = 0;
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
            match theme.get(name).copied().map(str::to_string).or_else(|| local.get(name).cloned()) {
                Some(v) => out.push_str(&v),
                // 中介變數尚未收集到值：原樣保留，下一輪再解析
                None if fallback.is_empty() => out.push_str(&format!("var({inner})")),
                None => out.push_str(fallback),
            }
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        // 收集已不含 var() 的 `--_x:值;` 宣告
        for part in out.split(['"', ';']) {
            if let Some((k, v)) = part.trim().split_once(':')
                && k.starts_with("--_") && !v.contains("var(")
            {
                local.insert(k.to_string(), v.trim().to_string());
            }
        }
        s = out;
    }
    s
}

fn main() {
    let out = std::env::args().nth(1).expect("需要輸出目錄");
    let dark = std::env::args().nth(2).as_deref() == Some("dark");
    let mut theme: HashMap<&str, &str> = HashMap::new();
    if dark {
        theme.extend([("--fermaid-bg", "#1e1e1e"), ("--fermaid-fg", "#dcddde"), ("--fermaid-line", "#999999"), ("--fermaid-surface", "#2a2a2a"), ("--fermaid-surface-alt", "#333333"), ("--fermaid-border", "#777777"), ("--fermaid-muted", "#aaaaaa")]);
    }
    let cases = [
        ("flow", "flowchart LR\n  A[開始] --> B{判斷}\n  B -->|是| C[完成]\n  B -->|否| D[重試]"),
        ("seq", "sequenceDiagram\n  使用者->>系統: 登入\n  系統-->>使用者: 回應"),
        ("pie", "pie title 比例\n  \"Rust\": 70\n  \"其他\": 30"),
    ];
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    for (name, src) in cases {
        let t = std::time::Instant::now();
        let r = fermaid::render(src, &fermaid::RenderOptions::default()).expect("render");
        let svg = resolve_vars(&r.svg, &theme);
        println!("{name}: {}x{} render+resolve {}ms 殘留var={}", r.width, r.height, t.elapsed().as_millis(), svg.matches("var(").count());
        std::fs::write(format!("{out}/{name}.resolved.svg"), &svg).unwrap();
        let tree = resvg::usvg::Tree::from_str(&svg, &opt).expect("usvg");
        let size = tree.size().to_int_size().scale_by(2.0).unwrap();
        let mut pm = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
        if dark {
            pm.fill(resvg::tiny_skia::Color::from_rgba8(0x1e, 0x1e, 0x1e, 255));
        } else {
            pm.fill(resvg::tiny_skia::Color::WHITE);
        }
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(2.0, 2.0), &mut pm.as_mut());
        pm.save_png(format!("{out}/{name}.png")).unwrap();
    }
}
