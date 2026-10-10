//! UAX #29 扩展字素簇(extended grapheme cluster)边界 —— **零第三方依赖手写**。
//!
//! 数据来自 [`super::gcb`](生成物,`gen-ucd grapheme` 产出)。正确性**只**由
//! `bin/grapheme-check` 对官方 `GraphemeBreakTest.txt` 的**全量**自检证明
//! (`addendum-03` §3.4);本文件的单测只钉机制。
//!
//! ## 规则表(UAX #29 §3)
//!
//! | 规则 | 内容 |
//! |---|---|
//! | GB1 | sot ÷ Any |
//! | GB2 | Any ÷ eot |
//! | GB3 | CR × LF |
//! | GB4 | (Control \| CR \| LF) ÷ |
//! | GB5 | ÷ (Control \| CR \| LF) |
//! | GB6 | L × (L \| V \| LV \| LVT) |
//! | GB7 | (LV \| V) × (V \| T) |
//! | GB8 | (LVT \| T) × T |
//! | GB9 | × (Extend \| ZWJ) |
//! | GB9a | × SpacingMark |
//! | GB9b | Prepend × |
//! | **GB9c** | 印度语 conjunct:辅音 [\p{InCB=Extend} \p{InCB=Linker}]* Linker […] × 辅音 |
//! | GB11 | \p{Extended_Pictographic} Extend* ZWJ × \p{Extended_Pictographic} |
//! | GB12 / GB13 | RI × RI,**奇数**个 RI 连排时在末位断开 |
//! | GB999 | Any ÷ Any(兜底) |
//!
//! ⚠ **GB9c 是 Unicode 15.1 才加的**(Indic conjunct break)。18.0.0 的
//! `GraphemeBreakTest.txt` 里有它对应的用例(注释里写
//! `Extend_ConjunctExtender`)。只实现到 GB11 的实现会在这里**整段**错,
//! 而且错的是天城文 / 孟加拉文一整类。

use std::borrow::Cow;

use super::gcb::{EXT_PICT, GCB_RUNS, INCB, UNICODE_VERSION};

const INCB_NONE: u8 = 0;
const INCB_LINKER: u8 = 1;
const INCB_CONSONANT: u8 = 2;
const INCB_EXTEND: u8 = 3;

const OTHER: u8 = 0;
const CR: u8 = 1;
const LF: u8 = 2;
const CONTROL: u8 = 3;
const EXTEND: u8 = 4;
const ZWJ: u8 = 5;
const RI: u8 = 6;
const PREPEND: u8 = 7;
const SPACINGMARK: u8 = 8;
const L: u8 = 9;
const V: u8 = 10;
const T: u8 = 11;
const LV: u8 = 12;
const LVT: u8 = 13;

/// 本表对应的 Unicode 版本 ——「字素簇按哪版规则切」必须可复现。
pub const VERSION: &str = UNICODE_VERSION;

/// 码点的 `Grapheme_Cluster_Break` 属性;未列出的码点是 `Other`。
pub fn gcb(cp: u32) -> u8 {
    match GCB_RUNS.binary_search_by_key(&cp, |&(s, _)| s) {
        Ok(i) => GCB_RUNS[i].1,
        Err(0) => OTHER,
        Err(i) => GCB_RUNS[i - 1].1,
    }
}

/// 该码点是否是 `Extended_Pictographic`(GB11 用)。
pub fn is_ext_pict(cp: u32) -> bool {
    EXT_PICT.binary_search(&cp).is_ok()
}

/// 逐簇切片的**边界下标**(升序,首 0 末 `len`)。空输入返回 `[0]`。
pub fn cluster_boundaries(cps: &[u32]) -> Vec<usize> {
    let mut out = vec![0usize];
    for i in 1..cps.len() {
        if is_break(cps, i) {
            out.push(i);
        }
    }
    out.push(cps.len());
    out
}

/// 字素簇数 —— `GRAPHEME_COUNT` 成员就是它。空串是 `0`,不是 `1`。
pub fn count_clusters(cps: &[u32]) -> usize {
    if cps.is_empty() {
        return 0;
    }
    let mut n = 1usize;
    for i in 1..cps.len() {
        if is_break(cps, i) {
            n += 1;
        }
    }
    n
}

/// `i` 处**是否断簇**(即 `i-1` 与 `i` 之间)。
fn is_break(cps: &[u32], i: usize) -> bool {
    let a = gcb(cps[i - 1]);
    let b = gcb(cps[i]);

    // GB3
    if a == CR && b == LF {
        return false;
    }
    // GB4 / GB5
    if a == CR || a == LF || a == CONTROL || b == CR || b == LF || b == CONTROL {
        return true;
    }
    // GB6 / GB7 / GB8 —— 谚文、藏文等
    if a == L && matches!(b, L | V | LV | LVT) {
        return false;
    }
    if matches!(a, LV | V) && matches!(b, V | T) {
        return false;
    }
    if matches!(a, LVT | T) && b == T {
        return false;
    }
    // GB9 / GB9a / GB9b
    if matches!(b, EXTEND | ZWJ | SPACINGMARK) || a == PREPEND {
        return false;
    }
    // GB11:ExtPict Extend* ZWJ x ExtPict
    if a == ZWJ && is_ext_pict(cps[i]) && ext_pict_before(cps, i - 1) {
        return false;
    }
    // GB9.3:印度语 conjunct(见 `indic_conjunct_before`)
    if incb(cps[i]) == INCB_CONSONANT && indic_conjunct_before(cps, i) {
        return false;
    }
    // GB12 / GB13:RI x RI,但**奇数**个连排只在末位断开
    if a == RI && b == RI {
        let mut n = 0usize;
        let mut j = i;
        while j > 0 && gcb(cps[j - 1]) == RI {
            n += 1;
            j -= 1;
        }
        if n % 2 == 1 {
            return false;
        }
        return true;
    }
    // GB999
    true
}

/// `end`(不含)之前是否形如 `ExtPict Extend*`(GB11 的左半)。
fn ext_pict_before(cps: &[u32], end: usize) -> bool {
    let mut j = end;
    while j > 0 {
        match gcb(cps[j - 1]) {
            EXTEND => j -= 1,
            ZWJ => break,
            _ => return is_ext_pict(cps[j - 1]),
        }
    }
    j > 0 && is_ext_pict(cps[j - 1])
}

/// 码点的 `Indic_Conjunct_Break`;未列出的码点是 `None`(0)。
pub fn incb(cp: u32) -> u8 {
    match INCB.binary_search_by_key(&cp, |&(s, _)| s) {
        Ok(i) => INCB[i].1,
        Err(0) => INCB_NONE,
        Err(i) => INCB[i - 1].1,
    }
}

/// **GB9.3** 的左半:`[InCB=Extend | InCB=Linker]* Linker [InCB=Extend | InCB=Linker]*`。
///
/// ⚠ 两条**实测**出来的、规则文字里不显眼的点:
///
/// 1. **左侧不需要是 `InCB=Consonant`。** 官方用例 `÷ 0061 × 094D × 0924 ÷`
///    (第 859 行)的左侧是个拉丁 `a`,`÷ 0061 × 094D` 由 GB9(× Extend)兜住,
///    而 `094D × 0924` 仍按 GB9.3 不断 —— 所以「必须落到辅音上」是错的。
/// 2. **前导段在串首同样成立。** 第 179 行 `÷ 094D × 0915 ÷` 左上下文只有一个
///    Linker,没有前置辅音,官方仍判不断。
///
/// 于是判据收敛成:**向左连续吃掉若干 `InCB=Extend` / `InCB=Linker`,其中至少
/// 有一个 Linker**。「至少一个 Linker」不可省 —— 两个辅音之间没有 linker 时
/// 必须断簇(官方用例里有)。
fn indic_conjunct_before(cps: &[u32], i: usize) -> bool {
    let mut j = i;
    let mut saw_linker = false;
    while j > 0 {
        match incb(cps[j - 1]) {
            INCB_LINKER => {
                saw_linker = true;
                j -= 1;
            }
            INCB_EXTEND => j -= 1,
            _ => break,
        }
    }
    saw_linker
}

/// 一个 `&str` 的字素簇数(便捷入口)。
pub fn count_str(s: &str) -> usize {
    let cps: Vec<u32> = s.chars().map(u32::from).collect();
    count_clusters(&cps)
}

/// 逐簇切片。空串返回空 `Vec`。
pub fn clusters(s: &str) -> Vec<String> {
    let cps: Vec<u32> = s.chars().map(u32::from).collect();
    let bounds = cluster_boundaries(&cps);
    bounds
        .windows(2)
        .map(|w| {
            cps[w[0]..w[1]]
                .iter()
                .map(|&c| char::from_u32(c).unwrap_or('\u{FFFD}'))
                .collect()
        })
        .collect()
}

/// 借用版本:只为「数一数」时不复制。保留是因为 `std.text` 的成员边界上
/// 快速路径值得留着(见 `addendum-03` 的 `NFC_QC` 同款理由)。
pub fn count_str_borrowed(s: &str) -> Cow<'_, str> {
    let _ = count_str(s);
    Cow::Borrowed(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> usize {
        count_str(s)
    }

    #[test]
    fn empty_and_single() {
        assert_eq!(n(""), 0, "空串是 0 个簇,不是 1 个");
        assert_eq!(n("a"), 1);
        assert_eq!(n("\u{4E2D}"), 1);
    }

    /// 计划 §2 点名的那个:三个 emoji 用 ZWJ 连起来,用户只看到**一个**字符。
    #[test]
    fn zwj_emoji_sequence_is_one_cluster() {
        assert_eq!(n("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"), 1);
    }

    /// 组合记号不另起一簇(GB9)。⚠ 这条曾经因为**生成器**的 run-length
    /// 把空隙吞掉而红过:表把 `0065 'e'` 记成了 `Control`,于是
    /// `e` + U+0301 断成两簇。判据在生成器,不在这里。
    #[test]
    fn combining_marks_do_not_start_a_cluster() {
        assert_eq!(n("e\u{0301}"), 1);
    }

    #[test]
    fn regional_indicator_splits_by_pairs() {
        assert_eq!(n("\u{1F1E8}\u{1F1F3}"), 1, "两个 RI = 一面旗");
        assert_eq!(n("\u{1F1E8}\u{1F1F3}\u{1F1E8}"), 2, "奇数个 RI 在末位断开");
        assert_eq!(n("\u{1F1E8}\u{1F1F3}\u{1F1E8}\u{1F1F3}"), 2);
    }

    #[test]
    fn crlf_is_one_cluster() {
        assert_eq!(n("\r\n"), 1);
        assert_eq!(n("\r\n\r\n"), 2);
    }

    #[test]
    fn clusters_splits_the_way_boundaries_say() {
        assert_eq!(
            clusters("a\u{0301}b"),
            vec!["a\u{0301}".to_string(), "b".to_string()]
        );
    }
}
