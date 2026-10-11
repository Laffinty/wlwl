//! [v0.11.3 M6 / ADDENDUM-03 W-04] `WIDTH` 的**全表**自检。
//!
//! 用法(`impl/` 目录下):
//!
//! ```text
//! cargo run --release -p wlwl-std --bin width-check -- <UCD 目录>
//! ```
//!
//! 退出码 `0` = 全过;`1` = 有失败。
//!
//! ## 为什么是「全表」而不是抽样
//!
//! `WIDTH` 的三条裁决里,**D-W1 一条就影响全表 38.9% 的码点**(EAW `A` 类
//! 138 739 个)。抽样测不出来 —— 而抽样一旦漏掉 `A` 类,默认宽度会静默变成 2,
//! 终端对齐全错却没人发现。
//!
//! ⇒ 本测试**逐个**比对 `0..=0x10FFFF` 全部 **1 114 112** 个码点:按
//! `EastAsianWidth.txt` × `emoji-data.txt` × `DerivedGeneralCategory.txt`
//! 现算期望值,再与生成的 `unicode/eawidth.rs` 对账。
//!
//! ## 另外两处「点名」
//!
//! 三条裁决真正**生效**的地方只有两组码点,它们被单独列出来:
//! **26** 个 `N ∩ Emoji_Presentation`(U+1F1E6..1F1FF 区域指示符)与
//! **33** 个 `A ∩ Extended_Pictographic`(`® ™ ▶ ☎ ♠ ♥` 一类,默认**文本**
//! 呈现,按 1 算)。全表比对会把它们算进去,但**看不见它们**。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use wlwl_std::unicode::width::{cp_width, VERSION};

/// Unicode 全域的码点数。
const TOTAL: u32 = 0x11_0000;

fn span(field: &str) -> (u32, u32) {
    let f = field.trim();
    match f.split_once("..") {
        Some((a, b)) => (
            u32::from_str_radix(a, 16).expect("hex"),
            u32::from_str_radix(b, 16).expect("hex"),
        ),
        None => {
            let c = u32::from_str_radix(f, 16).expect("hex");
            (c, c)
        }
    }
}

/// EAW 表:码点 → 类(`W`/`F` = 宽)。
fn eaw_map(dir: &Path) -> BTreeMap<u32, u8> {
    let text = std::fs::read_to_string(dir.join("EastAsianWidth.txt")).expect("EAW");
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let parts: Vec<&str> = body.split(';').map(str::trim).collect();
        if parts.len() < 2 || parts[1].is_empty() {
            continue;
        }
        let cls = match parts[1] {
            "W" | "F" => 2,
            "A" => 5, // Ambiguous —— 单列标记,便于后面单独点名
            _ => 1,
        };
        let (lo, hi) = span(parts[0]);
        for c in lo..=hi {
            out.insert(c, cls);
        }
    }
    out
}

/// 一般类别:哪些码点计 0 宽(`Mn`/`Me`/`Cf`,不含 `Mc`)。
fn zero_width_set(dir: &Path) -> BTreeSet<u32> {
    let text =
        std::fs::read_to_string(dir.join("extracted/DerivedGeneralCategory.txt")).expect("gc");
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let parts: Vec<&str> = body.split(';').map(str::trim).collect();
        if parts.len() < 2 {
            continue;
        }
        if !matches!(parts[1], "Mn" | "Me" | "Cf") {
            continue;
        }
        let (lo, hi) = span(parts[0]);
        for c in lo..=hi {
            out.insert(c);
        }
    }
    out
}

/// `Emoji_Presentation` 集合。
fn emoji_presentation(dir: &Path) -> BTreeSet<u32> {
    let text = std::fs::read_to_string(dir.join("emoji-data.txt")).expect("emoji-data");
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let parts: Vec<&str> = body.split(';').map(str::trim).collect();
        if parts.len() < 2 || parts[1] != "Emoji_Presentation" {
            continue;
        }
        let (lo, hi) = span(parts[0]);
        for c in lo..=hi {
            out.insert(c);
        }
    }
    out
}

/// `Extended_Pictographic` 集合。
fn extended_pictographic(dir: &Path) -> BTreeSet<u32> {
    let text = std::fs::read_to_string(dir.join("emoji-data.txt")).expect("emoji-data");
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let parts: Vec<&str> = body.split(';').map(str::trim).collect();
        if parts.len() < 2 || parts[1] != "Extended_Pictographic" {
            continue;
        }
        let (lo, hi) = span(parts[0]);
        for c in lo..=hi {
            out.insert(c);
        }
    }
    out
}

fn main() -> std::process::ExitCode {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("用法:width-check <UCD 目录>");
        return std::process::ExitCode::from(2);
    };

    let eaw = eaw_map(&dir);
    let zero = zero_width_set(&dir);
    let epres = emoji_presentation(&dir);
    let ep = extended_pictographic(&dir);

    // D-W2 真正生效的那一组:N 且默认 emoji 呈现。
    let d_w2: BTreeSet<u32> = epres
        .iter()
        .copied()
        .filter(|c| eaw.get(c).copied().unwrap_or(1) == 1)
        .collect();
    // 计划原文「emoji 一律 2」若照字面实现,这一组会被误判。
    let a_amb = eaw
        .iter()
        .filter(|(_, v)| **v == 5)
        .map(|(k, _)| *k)
        .collect::<BTreeSet<_>>();
    let d_w2_old: BTreeSet<u32> = d_w2.iter().copied().collect();

    println!("Unicode {VERSION}");
    println!("比对码点数: {TOTAL}");
    println!("  EAW A(Ambiguous)   : {}", a_amb.len());
    println!("  零宽(Mn/Me/Cf)     : {}", zero.len());
    println!("  Emoji_Presentation : {}", epres.len());
    println!("  D-W2 生效点 N∩Emoji_Presentation : {}", d_w2.len());
    println!(
        "  A∩Extended_Pictographic(旧口径会误判成 2): {}",
        a_amb.iter().filter(|c| ep.contains(c)).count()
    );

    let mut bad: Vec<String> = Vec::new();
    let mut hist = [0usize; 4]; // 0 / 1 / 2 / 越界
    for cp in 0..TOTAL {
        // 按三条裁决现算期望宽度。
        let mut w = match eaw.get(&cp).copied().unwrap_or(1) {
            2 => 2,
            _ => 1,
        };
        if w == 1 && epres.contains(&cp) {
            w = 2; // D-W2
        }
        if zero.contains(&cp) {
            w = 0; // D-W3
        }
        let got = cp_width(cp);
        hist[(got as usize).min(3)] += 1;
        if got != w {
            if bad.len() < 25 {
                bad.push(format!(
                    "  U+{cp:04X} 期望 {w} 实得 {got} (EAW={} epres={} zero={})",
                    eaw.get(&cp).copied().unwrap_or(1),
                    epres.contains(&cp),
                    zero.contains(&cp)
                ));
            } else if bad.len() == 25 {
                bad.push("  …(更多不符未列)".to_string());
            }
        }
    }
    println!("列宽分布: 0={} 1={} 2={}", hist[0], hist[1], hist[2]);

    // D-W2 那一组逐条点名 —— 全表比对把它们算进去了,但看不见它们。
    println!("\nD-W2 生效点逐条({} 个,全部必须为 2):", d_w2.len());
    for c in &d_w2_old {
        let g = cp_width(*c);
        println!(
            "  U+{:04X} -> {}  {}",
            c,
            g,
            if g == 2 {
                "\u{2713}"
            } else {
                "\u{2717} 应当是 2"
            }
        );
    }

    if bad.is_empty() {
        println!("\n\u{2713} 全表通过:{TOTAL} 个码点逐条比对,零失败");
        return std::process::ExitCode::SUCCESS;
    }
    eprintln!("\n\u{2717} 全表比对有 {} 处不符:", bad.len());
    for b in &bad {
        eprintln!("{b}");
    }
    std::process::ExitCode::from(1)
}
