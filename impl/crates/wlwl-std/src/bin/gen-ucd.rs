//! [v0.11.3 M6 / addendum-03] Unicode 规范化表生成器(NFC / NFD / NFC_QC)。
//!
//! 数据源:Unicode 官方 UCD **18.0.0**(`www.unicode.org/Public/18.0.0/ucd/`,
//! `ADR-0026` G1 钉的版本 —— 见 `addendum-03` §3.1)。
//!
//! 用法(仓库根的 `impl/` 目录下):
//!
//! ```text
//! cargo run -p wlwl-std --bin gen-ucd -- <UCD 目录> \
//!     > crates/wlwl-std/src/unicode/norm.rs
//! ```
//!
//! 纪律:**源数据不入库,生成器与产物都入库**(沿 `gen-entities` 的形状)。
//! ⚠️ `addendum-03` §3.2 原先担心「`NFC` 数据量比实体表大一个量级,每次构建
//! 重新生成会拖慢编译」—— **实测证伪**:产物 **92 KB** vs 实体表 **69.5 KB**,
//! **同一量级**。⇒ 产物直接 `include!` 进二进制,构建期**零下载、零生成**。
//!
//! ## 只生成**规范化**需要的四张表(不含 `GRAPHEME_COUNT` / `WIDTH`)
//!
//! 那两个成员的数据源(`emoji-data` / `EastAsianWidth` / `GraphemeBreakProperty`)
//! 与规范化**完全无关**,混进本批会让「正确性只由 `NormalizationTest.txt` 全量
//! 自检证明」那条纪律的适用范围变模糊(见 `addendum-03` §3.1 结论二)。

use std::collections::BTreeMap;
use std::io::Write;

const VERSION: &str = "18.0.0";

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: gen-ucd <UCD 目录(应含 UnicodeData.txt 等)>");
    let dir = std::path::PathBuf::from(dir);
    let read = |name: &str| {
        let p = dir.join(name);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} 读不了: {e}", p.display()))
    };

    // ── 1. 组合类(非零)��─ DerivedCombiningClass.txt ────────────────
    // ⚠️ 该文件有**区间行**(`0334..0338`)与 First/Last 区间形式,两者都要展开。
    let mut ccc: BTreeMap<u32, u8> = BTreeMap::new();
    for line in read("extracted/DerivedCombiningClass.txt").lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let mut it = body.split(';').map(str::trim).filter(|s| !s.is_empty());
        let (Some(range), Some(val)) = (it.next(), it.next()) else {
            continue;
        };
        let Ok(v) = val.parse::<u16>() else { continue };
        if v == 0 {
            continue; // ccc=0 的占绝大多数,不入表
        }
        let (lo, hi) = match range.split_once("..") {
            Some((a, b)) => (
                u32::from_str_radix(a, 16).expect("区间起点是十六进制"),
                u32::from_str_radix(b, 16).expect("区间终点是十六进制"),
            ),
            None => {
                let c = u32::from_str_radix(range, 16).expect("单个码点是十六进制");
                (c, c)
            }
        };
        for c in lo..=hi {
            ccc.insert(c, v as u8);
        }
    }

    // ── 2. 规范分解 + 规范组合 ── UnicodeData.txt + CompositionExclusions ──
    // ⚠️ 分解字段以 `<` 开头的是**兼容性**分解(NFKC/NFKD 用),**规范化不用**。
    // ⚠️ `First>` / `Last>` 行(CJK / Hangul 区间)的分解字段是空的,自然跳过。
    let excl: std::collections::HashSet<u32> = read("CompositionExclusions.txt")
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(|l| {
            u32::from_str_radix(l.split(';').next().unwrap_or("").trim(), 16)
                .expect("排除项是十六进制")
        })
        .collect();

    let mut decomp: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for line in read("UnicodeData.txt").lines() {
        let f: Vec<&str> = line.split(';').collect();
        if f.len() < 6 {
            continue;
        }
        let Ok(cp) = u32::from_str_radix(f[0], 16) else {
            continue;
        };
        let d = f[5].trim();
        if d.is_empty() || d.starts_with('<') {
            continue;
        }
        decomp.insert(
            cp,
            d.split_whitespace()
                .map(|x| u32::from_str_radix(x, 16).expect("分解目标是十六进制"))
                .collect(),
        );
    }

    // 规范组合 = 「长度为 2 的规范分解」减去组合排除。
    // ⚠️ `DerivedNormalizationProps.txt` 的 `Full_Composition_Exclusion` 才是**权威**
    // (它随 Unicode 版本演进,`CompositionExclusions.txt` 是其子集快照),两边都取。
    // ⚠️ `DerivedNormalizationProps.txt` 的字段顺序是 **`码点 ; 属性[; 值]`**
    // (码点在前)。而 `Full_Composition_Exclusion` **只有两段**(没有值)。
    // ⚠️ 第一版把第一段当属性名 ⇒ 两个属性**全被跳过**(NFC_QC 解出 0 条、
    // Full_Composition_Exclusion 也一条没读到)。这个 bug 很安静:生成的表
    // 看着正常,只是**少了几百条组合**。
    let mut full_excl: std::collections::HashSet<u32> = excl.clone();
    for line in read("DerivedNormalizationProps.txt").lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let mut it = body.split(';').map(str::trim);
        let (Some(cps), Some(prop)) = (it.next(), it.next()) else {
            continue;
        };
        if prop != "Full_Composition_Exclusion" {
            continue;
        }
        for tok in cps.split_whitespace() {
            let (lo, hi) = match tok.split_once("..") {
                Some((a, b)) => (
                    u32::from_str_radix(a, 16).expect("十六进制"),
                    u32::from_str_radix(b, 16).expect("十六进制"),
                ),
                None => {
                    let c = u32::from_str_radix(tok, 16).expect("十六进制");
                    (c, c)
                }
            };
            for c in lo..=hi {
                full_excl.insert(c);
            }
        }
    }

    let mut comp: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for (cp, seq) in &decomp {
        if seq.len() == 2 && !full_excl.contains(cp) {
            comp.insert((seq[0], seq[1]), *cp);
        }
    }

    // ── 3. NFC_QC = No | Maybe(「需要检查」的码点)── DerivedNormalizationProps ──
    // `No` = 该码点本身不是 NFC 形态;`Maybe` = 取决于前面的 starter。
    // **两个都要**,否则快速检查会在 `Maybe` 码点上给出错误的「已规范化」。
    let mut qc: Vec<u32> = Vec::new();
    for line in read("DerivedNormalizationProps.txt").lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let mut it = body.split(';').map(str::trim);
        let (Some(cps), Some(prop), Some(val)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        if prop != "NFC_QC" || (val != "N" && val != "M") {
            continue;
        }
        for tok in cps.split_whitespace() {
            let (lo, hi) = match tok.split_once("..") {
                Some((a, b)) => (
                    u32::from_str_radix(a, 16).expect("十六进制"),
                    u32::from_str_radix(b, 16).expect("十六进制"),
                ),
                None => {
                    let c = u32::from_str_radix(tok, 16).expect("十六进制");
                    (c, c)
                }
            };
            for c in lo..=hi {
                qc.push(c);
            }
        }
    }
    qc.sort_unstable();
    qc.dedup();

    // ── 输出 ─────────────────────────────────────────────────────────
    let mut o = String::with_capacity(160 * 1024);
    o.push_str(&format!(
        "//! GENERATED by `gen-ucd` from Unicode UCD **{VERSION}**. DO NOT EDIT.\n\
         //!\n\
         //! 重新生成:\n\
         //! ```text\n\
         //! cargo run -p wlwl-std --bin gen-ucd -- <UCD 目录> \\\n\
         //!     > crates/wlwl-std/src/unicode/norm.rs\n\
         //! ```\n\
         //!\n\
         //! 源数据(UCD,约 3.66 MB)**不入库**;本文件与生成器入库 —— 与\n\
         //! `gen-entities` / `sanitize/entities.rs` 同款。构建期零下载。\n\
         //!\n\
         //! 正确性只由官方 `NormalizationTest.txt` **全量**自检证明\n\
         //! (`addendum-03` §3.1 结论一);本表本身**不**冻结任何期望值。\n\n"
    ));

    o.push_str(&format!(
        "/// 本表对应的 Unicode 版本 —— 「NFC 到底按哪版算」必须可复现(`ADR-0026` G1)。\n\
         pub const UNICODE_VERSION: &str = \"{VERSION}\";\n\n"
    ));

    o.push_str(
        "/// 规范组合类(非零)。按码点升序,二分查找。\n\
               pub static CCC: &[(u32, u8)] = &[\n",
    );
    for (c, v) in &ccc {
        o.push_str(&format!("(0x{c:04X},{v}),"));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// 规范分解(**不含**兼容性分解)。按码点升序,二分查找。\n\
               pub static DECOMP: &[(u32, &[u32])] = &[\n",
    );
    for (c, seq) in &decomp {
        let body: Vec<String> = seq.iter().map(|x| format!("0x{x:04X}")).collect();
        o.push_str(&format!("(0x{c:04X},&[{}]),", body.join(",")));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// 规范组合:(starter, 组合记号) → 合成码点。按 (starter, 组合记号) 升序。\n\
               pub static COMP: &[(u32, u32, u32)] = &[\n",
    );
    for ((a, b), c) in &comp {
        o.push_str(&format!("(0x{a:04X},0x{b:04X},0x{c:04X}),"));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// `NFC_QC` 为 `No` 或 `Maybe` 的码点 —— 「这个串**可能**不是 NFC 形态」。\n\
               /// `NFC_QC` 的快速路径:串里一个都没有 ⇒ 已规范化,**不重排**。\n\
               pub static NFC_QC_SUSPECT: &[u32] = &[",
    );
    for c in &qc {
        o.push_str(&format!("0x{c:04X},"));
    }
    o.push_str("];\n");

    print!("{o}");

    // 生成器的**自检**(计数可见,便于计划核对;详见 addendum-03 §3.1)
    let _ = std::io::stdout().flush();
    eprintln!(
        "[gen-ucd] Unicode {VERSION}: ccc(非零)={} 规范分解={} 规范组合={} NFC_QC_suspect={}",
        ccc.len(),
        decomp.len(),
        comp.len(),
        qc.len()
    );
}
