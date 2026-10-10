//! [v0.11.3 M6 / ADDENDUM-03 W-04] 官方 `GraphemeBreakTest.txt` **全量**自检。
//!
//! 用法(`impl/` 目录下):
//!
//! ```text
//! cargo run --release -p wlwl-std --bin grapheme-check -- <UCD 目录>
//! ```
//!
//! 退出码 `0` = 全过;`1` = 有失败、**或读不满** [`EXPECTED_ROWS`] 行。
//!
//! ## 为什么是独立二进制而不是 `cargo test`
//!
//! 测试文件 **137 KB 且不入库**(源数据入库纪律),`cargo test` 在 CI 上
//! 拿不到它。同 `bin/norm-check`。
//!
//! ## 它测的是**发布路径**
//!
//! 直接调 `wlwl_std::unicode::grapheme` —— 与 `std.text` 的 `GRAPHEME_COUNT`
//! **同一个函数**。一个自带参考实现的自检证明不了任何事。
//!
//! ## 行数是冻的常量
//!
//! 少读一行也能「全绿」,所以 [`EXPECTED_ROWS`] 钉死 —— 只跑前 100 条、
//! 只跑 ASCII、只跑某一类边界都过不去。

use std::path::PathBuf;

use wlwl_std::unicode::grapheme::cluster_boundaries;

const EXPECTED_ROWS: usize = 853;

/// 一行解析成 `(码点序列, 每个码点之前是否断簇)`。
///
/// 官方格式:`÷ 000D ÷ 0308 ×` —— `÷` 是边界,`×` 是不断。**行首那个 `÷`**
/// 不算内部断点(它是 GB1 的 sot),所以 `breaks_before[0]` 恒为 `true` 而
/// 不计入簇数。
fn parse_row(line: &str) -> Option<(Vec<u32>, Vec<bool>)> {
    let body = line.split('#').next()?.trim();
    if body.is_empty() {
        return None;
    }
    let mut cps = Vec::new();
    let mut breaks = Vec::new();
    for tok in body.split_whitespace() {
        match tok {
            "\u{00F7}" => breaks.push(true),  // ÷
            "\u{00D7}" => breaks.push(false), // ×
            hex => cps.push(u32::from_str_radix(hex, 16).expect("十六进制码点")),
        }
    }
    if cps.is_empty() {
        return None;
    }
    // breaks 比 cps 多一个:末位的边界标记。
    breaks.truncate(cps.len());
    Some((cps, breaks))
}

fn main() -> std::process::ExitCode {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("用法:grapheme-check <UCD 目录(应含 GraphemeBreakTest.txt)>");
        return std::process::ExitCode::from(2);
    };
    let path = PathBuf::from(dir).join("GraphemeBreakTest.txt");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!(
            "读不了 {} —— 自检需要本地 UCD 源数据(它不入库)。",
            path.display()
        );
        return std::process::ExitCode::from(2);
    };

    let mut rows = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut with_breaks = 0usize; // 含至少一个 `×` 的行 —— 真正考边界规则的

    for (lineno, line) in text.lines().enumerate() {
        let Some((cps, breaks)) = parse_row(line) else {
            continue;
        };
        rows += 1;
        if breaks.iter().any(|b| !*b) {
            with_breaks += 1;
        }

        let bounds = cluster_boundaries(&cps);
        let want: Vec<usize> = {
            let mut v = vec![0usize];
            for (i, b) in breaks.iter().enumerate().skip(1) {
                if *b {
                    v.push(i);
                }
            }
            v.push(cps.len());
            v
        };
        if bounds != want {
            failures.push(format!(
                "  第 {} 行: {} \n    期望边界 {:?}\n    实际边界 {:?}",
                lineno + 1,
                show(&cps),
                want,
                bounds
            ));
        }
    }

    println!("Unicode {}", wlwl_std::unicode::grapheme::VERSION);
    println!("用例行数: {rows} / 期望 {EXPECTED_ROWS}");
    println!("其中含不断簇标记(`×`)的行: {with_breaks}");

    if rows != EXPECTED_ROWS {
        eprintln!(
            "\n✗ 行数不符:读到 {rows} 行,期望 {EXPECTED_ROWS} —— \
             Unicode 版本变了(看文件头的 `-18.0.0.txt`),或只读了子集"
        );
        return std::process::ExitCode::from(1);
    }
    if failures.is_empty() {
        println!("\n✓ 全量通过:{rows} 行,零失败");
        return std::process::ExitCode::SUCCESS;
    }
    eprintln!("\n✗ {rows} 行里有 {} 行边界不对:", failures.len());
    for f in failures.iter().take(25) {
        eprintln!("{f}");
    }
    if failures.len() > 25 {
        eprintln!("  …(另 {} 条未列)", failures.len() - 25);
    }
    std::process::ExitCode::from(1)
}

fn show(cps: &[u32]) -> String {
    let glyphs: String = cps
        .iter()
        .map(|&c| char::from_u32(c).unwrap_or('\u{FFFD}'))
        .collect();
    let hex: Vec<String> = cps.iter().map(|c| format!("{c:04X}")).collect();
    format!("{glyphs:?}  [{}]", hex.join(" "))
}
