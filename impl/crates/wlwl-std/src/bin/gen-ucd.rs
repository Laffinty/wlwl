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
//! ⚠️ **上面那条 `>` 只在 bash / zsh / cmd 下安全。PowerShell 5.1 会把
//! stdout 写成 **UTF-16LE + CRLF**,产物因此翻倍到 186 208 B、编译报
//! 「stream did not contain valid UTF-8」、`git diff` 还把它当二进制。
//! —— 这三条症状**同时**出现就是撞上了这个坑。在 PowerShell 下改用:
//!
//! ```powershell
//! python -c "import subprocess,pathlib,sys; pathlib.Path(r'crates\wlwl-std\src\unicode\norm.rs').write_bytes(subprocess.run([r'target\release\gen-ucd.exe', r'<UCD 目录>'],capture_output=True).stdout)"
//! ```
//!
//! 纪律:**源数据不入库,生成器与产物都入库**(沿 `gen-entities` 的形状)。
//! ⚠️ `addendum-03` §3.2 原先担心「`NFC` 数据量比实体表大一个量级,每次构建
//! 重新生成会拖慢编译」—— **实测证伪**:产物 **93 450 B**(UTF-8 / LF / 十六进制
//! 逐行一条)vs 实体表 **69 550 B**,**1.3 倍**。⇒ 产物直接编译进二进制,构建期
//! **零下载、零生成**。
//!
//! ## 只生成**规范化**需要的四张表(不含 `GRAPHEME_COUNT` / `WIDTH`)
//!
//! 那两个成员的数据源(`emoji-data` / `EastAsianWidth` / `GraphemeBreakProperty`)
//! 与规范化**完全无关**,混进本批会让「正确性只由 `NormalizationTest.txt` 全量
//! 自检证明」那条纪律的适用范围变模糊(见 `addendum-03` §3.1 结论二)。

use std::collections::BTreeMap;
use std::io::Write;

const VERSION: &str = "18.0.0";

// ── W-04 · 字素簇产物(`grapheme` 模式)─────────────────────────────────
//
// 规范化(本文件上半部分)与字素簇**数据源完全无关**,混在一个产物里会让
// `GRAPHEME_COUNT` 的正确性纪律(`addendum-03` §3.4:只由官方
// `GraphemeBreakTest.txt` 全量证明)的适用范围变模糊。故分模式生成。

/// `Grapheme_Cluster_Break` 属性的内部编码。**顺序即产物里的数字**,
/// 按 `GraphemeBreakProperty.txt` 里出现的次序编,好让产物能看出对应关系。
const GCB: &[(&str, u8)] = &[
    ("Other", 0),
    ("CR", 1),
    ("LF", 2),
    ("Control", 3),
    ("Extend", 4),
    ("ZWJ", 5),
    ("Regional_Indicator", 6),
    ("Prepend", 7),
    ("SpacingMark", 8),
    ("L", 9),
    ("V", 10),
    ("T", 11),
    ("LV", 12),
    ("LVT", 13),
];

/// 把 `AAAA` / `AAAA..BBBB` 的一行拆成 `(lo, hi)` 对。
fn span(field: &str) -> (u32, u32) {
    let f = field.trim();
    match f.split_once("..") {
        Some((a, b)) => (
            u32::from_str_radix(a, 16).expect("十六进制"),
            u32::from_str_radix(b, 16).expect("十六进制"),
        ),
        None => {
            let c = u32::from_str_radix(f, 16).expect("十六进制");
            (c, c)
        }
    }
}

fn emit_grapheme(read: &dyn Fn(&str) -> String) {
    use std::collections::BTreeMap;

    // ── GCB 属性(区间形式;未列出的码点 = Other)──
    let mut gcb: BTreeMap<u32, u8> = BTreeMap::new();
    for line in read("GraphemeBreakProperty.txt").lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let mut it = body.split(';').map(str::trim).filter(|s| !s.is_empty());
        let (Some(rng), Some(prop)) = (it.next(), it.next()) else {
            continue;
        };
        let Some(&(_, code)) = GCB.iter().find(|(n, _)| *n == prop) else {
            eprintln!("[gen-ucd] 未知 GCB 属性 {prop:?},跳过");
            continue;
        };
        let (lo, hi) = span(rng);
        for c in lo..=hi {
            gcb.insert(c, code);
        }
    }

    // ── `InCB`(UAX #44)—— GB9.3 的三个值。──
    // ⚠ 原本这里想用「Indic 脚本范围 + GCB」近似,自测 853 条里有 23 条
    // 因这个近似失败。而真数据只有 **3 190** 个码点(Linker 23 / Consonant 913 /
    // Extend 2 254),小得可以入库 —— **真数据比近似严格**。
    const INCB_NONE: u8 = 0;
    const INCB_LINKER: u8 = 1;
    const INCB_CONSONANT: u8 = 2;
    const INCB_EXTEND: u8 = 3;
    let mut incb: BTreeMap<u32, u8> = BTreeMap::new();
    for line in read("DerivedCoreProperties.txt").lines() {
        if !line.contains("InCB") {
            continue;
        }
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        // 行形式是 `094D..0950 ; InCB; Linker` —— 值在**第三**段,不是第二段。
        let parts: Vec<&str> = body.split(';').map(str::trim).collect();
        if parts.len() < 3 || parts[1] != "InCB" {
            continue;
        }
        let code = match parts[2] {
            "Linker" => INCB_LINKER,
            "Consonant" => INCB_CONSONANT,
            "Extend" => INCB_EXTEND,
            other => {
                eprintln!("[gen-ucd] 未知 InCB 值 {other:?},跳过");
                continue;
            }
        };
        let (lo, hi) = span(parts[0]);
        for c in lo..=hi {
            incb.insert(c, code);
        }
    }

    // ── Extended_Pictographic(UAX #29 GB11 / GB9c 要用)──
    // ⚠ 它**不是**业务上的「emoji」:键帽、区域指示符一类不在其中,而
    // `©`、`▶` 一类在其中却默认文本呈现(`addendum-03` §3.3 的实测)。
    let mut ep: BTreeMap<u32, u8> = BTreeMap::new();
    for line in read("emoji-data.txt").lines() {
        let body = line.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let mut it = body.split(';').map(str::trim).filter(|s| !s.is_empty());
        let (Some(rng), Some(prop)) = (it.next(), it.next()) else {
            continue;
        };
        if prop != "Extended_Pictographic" {
            continue;
        }
        let (lo, hi) = span(rng);
        for c in lo..=hi {
            ep.insert(c, 1);
        }
    }

    // ── 输出 ──
    let mut o = String::with_capacity(220 * 1024);
    o.push_str(&format!(
        "//! GENERATED by `gen-ucd grapheme` from Unicode UCD **{VERSION}**. DO NOT EDIT.\n\
         //!\n\
         //! 重新生成:\n\
         //! ```text\n\
         //! cargo run -p wlwl-std --bin gen-ucd -- <UCD 目录> grapheme \\\n\
         //!     > crates/wlwl-std/src/unicode/gcb.rs\n\
         //! ```\n\
         //!\n\
         //! 正确性只由官方 **`GraphemeBreakTest.txt` 全量**自检证明\n\
         //! (`bin/grapheme-check`;见 `addendum-03` §3.4)。本表本身**不**冻结任何期望值。\n\
         //!\n\
         //! ⚠ 产物里的 `rustfmt::skip` 是刻意的:两张表每行一条,展开会让 `cargo fmt`\n\
         //! 把 30 万行的表重排 —— 那会让「与生成器逐字节一致」这件事变成另一个要维护的东西。\n\
         \n\
         /// 本表对应的 Unicode 版本 ——「字素簇按哪版规则切」必须可复现。\n\
         pub const UNICODE_VERSION: &str = \"{VERSION}\";\n\
         \n\
         /// `Grapheme_Cluster_Break` 属性(区间起点 + 编码)。按码点升序,二分查找。\n\
         /// **未列出的码点一律是 `Other`(0)** —— 那正是 UAX #29 的默认值。\n\
         #[rustfmt::skip]\n\
         pub static GCB_RUNS: &[(u32, u8)] = &[\n"
    ));
    // GCB:区间起点 + 属性。
    // ⚠ 缺失的码点 = `Other`(0),所以**空隙必须断段** —— 只在
    // `属性变了` 时才发起一段的写法会把空隙吞进前一段:
    // `0000..0009 ; Control` 之后一直向后看到 `0300 ; Extend`,
    // 于是整个 `0000..02FF` 都被记成 `Control`(`e 'e'` 的 gcb 算出 3 就是这个错。)
    let mut runs: Vec<(u32, u8)> = Vec::new();
    for cp in *gcb.keys().next().unwrap_or(&0)..=*gcb.keys().next_back().unwrap_or(&0) {
        let v = gcb.get(&cp).copied().unwrap_or(0);
        if runs.last().is_none_or(|&(_, pv)| pv != v) {
            runs.push((cp, v));
        }
    }
    for (s, v) in &runs {
        o.push_str(&format!("(0x{s:04X},{v}),"));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// `Extended_Pictographic` 码点(UAX #29 GB11 的 `ExtPict`)。GB9c 的\n\
         /// `InCB=Linker`(ZWJ)与 `InCB=Extend` 需要它,故**必须**整表在库。\n\
         #[rustfmt::skip]\n\
         pub static EXT_PICT: &[u32] = &[",
    );
    for c in ep.keys() {
        o.push_str(&format!("0x{c:04X},"));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// `Indic_Conjunct_Break`(UAX #44)—— GB9.3 的三个值:1 = Linker / 2 = Consonant\n\
         /// / 3 = Extend。**未列出的码点一律是 0(None)**。\n\
         /// ⚠ 这张表一开始是一个**近似**(「Indic 脚本范围 + GCB」),它在官方 853 条\n\
         /// 用例里失败了 23 条 —— 而真数据只有 3 190 个码点,真数据比近似严格。\n\
         #[rustfmt::skip]\n\
         pub static INCB: &[(u32, u8)] = &[\n",
    );
    let mut runs: Vec<(u32, u8)> = Vec::new();
    for cp in *incb.keys().next().unwrap_or(&0)..=*incb.keys().next_back().unwrap_or(&0) {
        let v = incb.get(&cp).copied().unwrap_or(INCB_NONE);
        if runs.last().is_none_or(|&(_, pv)| pv != v) {
            runs.push((cp, v));
        }
    }
    for (s, v) in &runs {
        o.push_str(&format!("(0x{s:04X},{v}),"));
    }
    o.push_str("];\n");

    print!("{o}");
    eprintln!(
        "[gen-ucd grapheme] Unicode {VERSION}: GCB 码点 {} / 区间 {} / ExtPict {} / InCB 码点 {}",
        gcb.len(),
        runs.len(),
        ep.len(),
        incb.len()
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .expect("usage: gen-ucd <UCD 目录(应含 UnicodeData.txt 等)> [norm|grapheme]");
    // 第二个参数选产物。缺省 `norm`,好让上一批那行不带参数的重建命令继续可用
    // —— 那条命令的产物是**逐字节冻结**过的,改它的调用方式就得跟着改对照。
    let mode = args.next().unwrap_or_else(|| "norm".to_string());
    let dir = std::path::PathBuf::from(dir);
    let read = |name: &str| {
        let p = dir.join(name);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} 读不了: {e}", p.display()))
    };

    if mode == "grapheme" {
        emit_grapheme(&read);
        return;
    }
    assert_eq!(mode, "norm", "未知的产物模式 {mode:?}");

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
               #[rustfmt::skip]
pub static CCC: &[(u32, u8)] = &[\n",
    );
    for (c, v) in &ccc {
        o.push_str(&format!("(0x{c:04X},{v}),"));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// 规范分解(**不含**兼容性分解)。按码点升序,二分查找。\n\
               #[rustfmt::skip]
pub static DECOMP: &[(u32, &[u32])] = &[\n",
    );
    for (c, seq) in &decomp {
        let body: Vec<String> = seq.iter().map(|x| format!("0x{x:04X}")).collect();
        o.push_str(&format!("(0x{c:04X},&[{}]),", body.join(",")));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// 规范组合:(starter, 组合记号) → 合成码点。按 (starter, 组合记号) 升序。\n\
               #[rustfmt::skip]
pub static COMP: &[(u32, u32, u32)] = &[\n",
    );
    for ((a, b), c) in &comp {
        o.push_str(&format!("(0x{a:04X},0x{b:04X},0x{c:04X}),"));
    }
    o.push_str("];\n\n");

    o.push_str(
        "/// `NFC_QC` 为 `No` 或 `Maybe` 的码点 —— 「这个串**可能**不是 NFC 形态」。\n\
               /// `NFC_QC` 的快速路径:串里一个都没有 ⇒ 已规范化,**不重排**。\n\
               #[rustfmt::skip]
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
