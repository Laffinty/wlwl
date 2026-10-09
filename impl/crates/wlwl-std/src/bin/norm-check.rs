//! [v0.11.3 M6 / addendum-03 W-02] 官方 `NormalizationTest.txt` **全量**自检。
//!
//! 用法(`impl/` 目录下):
//!
//! ```text
//! cargo run --release -p wlwl-std --bin norm-check -- <UCD 目录>
//! ```
//!
//! 退出码 `0` = 全过;`1` = 有失败、**或读不满** [`EXPECTED_ROWS`] 行。
//!
//! ## 为什么是独立二进制而不是 `cargo test`
//!
//! 测试文件 **2.73 MB 且不入库**(源数据入库纪律,与实体表 / UCD 表同款),
//! `cargo test` 也拿不到它 —— CI 上它根本不存在。计划 §6 已预留这条路:
//! 「可把全量表作为 generator 的**自检**,不进契约表」。
//!
//! ## 它测的是**发布路径**
//!
//! 本二进制调的是 [`wlwl_std::unicode::canon`] —— 与 `std.text` 的 `NFC` /
//! `NFD` 成员**同一个函数**。一个自带参考实现、只测自己那份代码的自检,
//! 证明不了任何事(`addendum-03` §3.1 结论一:我的 Python 版只过 33.62%)。
//!
//! ## 「不半张表」怎么被守住
//!
//! 少读一行文件也能「全绿」。所以行数是**冻���常量** [`EXPECTED_ROWS`],
//! 对不上直接红 —— 只跑前 1 000 行、只跑 Part1、只跑 Latin-1 都过不去。
//!
//! ## 范围
//!
//! 只查 **canonical**(NFC / NFD)两列。`NFKC` / `NFKD` 是**另一个语义**
//! 且本批明确不做(需要兼容性分解表),列在报告里但**不判定** —— 免得留下
//! 「跑过了」的错觉。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use wlwl_std::unicode::canon::{nfc, nfc_quick_ok, nfd};

/// Unicode **18.0.0** 官方测试文件的用例行数 —— 与 `norm::UNICODE_VERSION`
/// 同版。换了 Unicode 版本这个数会变,那时**两处一起**改。
const EXPECTED_ROWS: usize = 20_171;

/// 失败样本最多打这么多行(再多也不影响判定,只是刷屏)。
const MAX_REPORTED: usize = 25;

fn main() -> ExitCode {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("用法:norm-check <UCD 目录(应含 NormalizationTest.txt)>");
        return ExitCode::from(2);
    };
    let path = PathBuf::from(dir).join("NormalizationTest.txt");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!(
            "读不了 {} —— 自检需要本地 UCD 源数据(它不入库)。",
            path.display()
        );
        return ExitCode::from(2);
    };

    let mut rows = 0usize;
    let mut checks = 0usize;
    let mut conservative = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut per_part: BTreeMap<&str, usize> = BTreeMap::new();
    let mut part = "<preamble>";

    for (lineno, line) in text.lines().enumerate() {
        let s = line.trim();
        if s.is_empty() {
            continue;
        }
        if let Some(tag) = s.strip_prefix('@') {
            part = tag.split_whitespace().next().unwrap_or(part);
            per_part.entry(part).or_default();
            continue;
        }
        if s.starts_with('#') {
            continue;
        }
        let Some((c1, c2, c3)) = parse_row(s) else {
            failures.push(format!("第 {} 行解析不了: {s}", lineno + 1));
            continue;
        };
        rows += 1;
        *per_part.entry(part).or_default() += 1;

        // 官方 CONFORMANCE 段原文:
        //   NFC: c2 == toNFC(c1) == toNFC(c2) == toNFC(c3)
        //   NFD: c3 == toNFD(c1) == toNFD(c2) == toNFD(c3)
        checks += 4;
        if let Some(bad) = diff("NFC(c1)", &c1, &nfc(&c1), &c2) {
            failures.push(bad);
        }
        if let Some(bad) = diff("NFC(c2)", &c2, &nfc(&c2), &c2) {
            failures.push(bad);
        }
        if let Some(bad) = diff("NFC(c3)", &c3, &nfc(&c3), &c2) {
            failures.push(bad);
        }
        if let Some(bad) = diff("NFD(c1)", &c1, &nfd(&c1), &c3) {
            failures.push(bad);
        }
        if let Some(bad) = diff("NFD(c2)", &c2, &nfd(&c2), &c3) {
            failures.push(bad);
        }
        if let Some(bad) = diff("NFD(c3)", &c3, &nfd(&c3), &c3) {
            failures.push(bad);
        }

        // 快检的**唯一**不变量(单方向;反向不成立,见 `nfc_quick_ok` 的文档):
        //   `nfc_quick_ok(x)` 为真 ⇒ `NFC(x) == x`。
        // 它同时堵住两个方向的错:说「已规范化」却没规范化(跳过 = 错结果),
        // 以及没规范化却说「已规范化」。**同一条件**,不拆成两条断言充数。
        checks += 1;
        let quick_ok = nfc_quick_ok(&c1);
        if quick_ok && c1 != c2 {
            failures.push(format!(
                "[快检] NFC({}) 说「已规范化」,但 NFC({}) = {}",
                show(&c1),
                show(&c1),
                show(&c2)
            ));
        }
        // 反向(**允许**发生):快检说「不确定」而串本来就是 NFC 形态。
        // 官方 Part1 的 `Maybe` 单字符行(`0301` 之类)必然落进来。
        // 只记数、不判红 —— 但它是「快检有多保守」的实测读数,不是死代码。
        if !quick_ok && c1 == c2 {
            conservative += 1;
        }
    }

    println!("Unicode {}", wlwl_std::unicode::norm::UNICODE_VERSION);
    println!("用例行数: {rows} / 期望 {EXPECTED_ROWS}");
    for (p, n) in &per_part {
        println!("  {p:<8} {n}");
    }
    println!("断言数: {checks}");
    println!(
        "快检保守侧(说「不确定」而串本已是 NFC 形态): {conservative} 行 —— \
         这些行要多跑一趟规范化,属预期"
    );
    println!("NFKC / NFKD: 本批不做,未判定(见本文件头「范围」)");

    if rows != EXPECTED_ROWS {
        eprintln!(
            "\n✗ 行数不符:读到 {rows} 行,期望 {EXPECTED_ROWS} —— \
             「不半张表」被打破(文件被截断 / 版本不是 18.0.0 / 只读了子集)"
        );
        return ExitCode::from(1);
    }
    if failures.is_empty() {
        println!("\n✓ 全量通过:{rows} 行 × {checks} 条断言,零失败");
        return ExitCode::SUCCESS;
    }
    eprintln!("\n✗ {rows} 行里有 {} 条断言失败:", failures.len());
    for f in failures.iter().take(MAX_REPORTED) {
        eprintln!("  {f}");
    }
    if failures.len() > MAX_REPORTED {
        eprintln!("  …(另 {} 条未列)", failures.len() - MAX_REPORTED);
    }
    ExitCode::from(1)
}

/// 解析一行:`c1;c2;c3;c4;c5` —— 注释从第一个 `#` 起截掉(注释里也有 `;`,
/// 不先截就会把一行切成 11 段)。
fn parse_row(line: &str) -> Option<(Vec<u32>, Vec<u32>, Vec<u32>)> {
    let body = line.split('#').next()?.trim();
    let mut cols = body.split(';');
    let c1 = cp_list(cols.next()?);
    let c2 = cp_list(cols.next()?);
    let c3 = cp_list(cols.next()?);
    Some((c1, c2, c3))
}

fn cp_list(s: &str) -> Vec<u32> {
    s.split_whitespace()
        .map(|t| u32::from_str_radix(t, 16).expect("码点是十六进制"))
        .collect()
}

/// 渲染**加**码点。只渲染会骗人:`NFC` / `NFD` 排错序时两串的渲染完全
/// 相同(`addendum-03` §3.1),不打印码点就看不出失败在哪一位。
fn show(v: &[u32]) -> String {
    let glyphs: String = v
        .iter()
        .map(|c| char::from_u32(*c).map_or_else(|| "?".to_string(), |ch| ch.to_string()))
        .collect();
    let cps: Vec<String> = v.iter().map(|c| format!("{c:04X}")).collect();
    format!("{glyphs}  [{}]", cps.join(" "))
}

/// `label(input)` 与期望不一致时的失败描述;一致则 `None`。
fn diff(label: &str, input: &[u32], got: &[u32], want: &[u32]) -> Option<String> {
    if got == want {
        return None;
    }
    Some(format!(
        "{label}: 输入 {} ⇒ 得到 {},期望 {}",
        show(input),
        show(got),
        show(want)
    ))
}
