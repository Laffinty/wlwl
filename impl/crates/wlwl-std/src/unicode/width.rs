//! 显示宽度(`WIDTH` 成员的实现面)—— **零第三方依赖**。
//!
//! 列宽表在 [`super::eawidth`](生成物,`gen-ucd width` 产出),**已经把三条裁决
//! 算进每个码点**(`addendum-03` §3.3):
//!
//! - **D-W1** `EAW=A`(Ambiguous)= 1;`W` / `F` = 2;其余 1。
//! - **D-W2** `Emoji_Presentation` 且基础宽为 1 ⇒ 2(实测只影响 **26** 个码点,
//!   全是 U+1F1E6..1F1FF 区域指示符)。
//! - **D-W3** 一般类别 `Mn` / `Me` / `Cf` ⇒ **0**;`Mc` **不**计 0。
//!
//! ## D-W4 · **簇折叠**(`addendum-03` §3.3)
//!
//! `WIDTH` 是**簇感知**的:先按 UAX #29(见 [`super::grapheme`])切字素簇,
//! **每簇取该簇内码点列宽的最大值**。
//!
//! - `👨‍👩‍👧` = 三个 emoji 各 2 + 两个 ZWJ 各 0 ⇒ 逐码点求和得 **8**,
//!   取簇内最大值得 **2** —— 后者才是终端实际渲染的列数。
//! - `é`(e + U+0301) = 1 + 0 ⇒ 簇内最大 **1**,而不是 1(求和也对)。
//! - `🇨🇳` = 两个区域指示符各 2(D-W2),但它们是**一个**簇 ⇒ 簇内最大 **2**。
//!   ⚠ 这修正了我先前「按 EAW 给 1 会让国旗得 2 而不是 4」的说法:一旦做了
//!   簇折叠,国旗本来就该是 2;D-W2 的 26 个码点仍需要 2,但理由是**单簇最大**
//!   而不是求和。
//!
//! 主流实现(`wcwidth` / `unicode-width`)同样按簇折叠。

use super::eawidth::{UNICODE_VERSION, WIDTH_RUNS};
use super::grapheme::cluster_boundaries;

/// 本表对应的 Unicode 版本 ——「列宽按哪版算」必须可复现。
pub const VERSION: &str = UNICODE_VERSION;

/// 单个码点的列宽(0 / 1 / 2)。未列出的码点列宽 0。
pub fn cp_width(cp: u32) -> u8 {
    match WIDTH_RUNS.binary_search_by_key(&cp, |&(s, _)| s) {
        Ok(i) => WIDTH_RUNS[i].1,
        Err(0) => 0,
        Err(i) => WIDTH_RUNS[i - 1].1,
    }
}

/// 一个码点序列的**显示宽度**(列数)。`WIDTH` 成员就是它。
///
/// **簇折叠**:按 UAX #29 切簇,每簇取簇内码点列宽的最大值,再求和。
/// 空串是 `0`。
pub fn width(cps: &[u32]) -> usize {
    let bounds = cluster_boundaries(cps);
    bounds
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            cps[a..b].iter().map(|&c| cp_width(c)).max().unwrap_or(0) as usize
        })
        .sum()
}

/// 一个 `&str` 的显示宽度(便捷入口)。
pub fn width_str(s: &str) -> usize {
    let cps: Vec<u32> = s.chars().map(u32::from).collect();
    width(&cps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> usize {
        width_str(s)
    }

    #[test]
    fn ascii_and_cjk() {
        assert_eq!(w(""), 0);
        assert_eq!(w("abc"), 3);
        assert_eq!(w("\u{4E2D}\u{6587}"), 4, "两个全角 = 4 列");
    }

    /// D-W1:`A` 类按 1。U+00AE REGISTERED SIGN 的 EAW 是 `A`。
    #[test]
    fn ambiguous_is_one_column() {
        assert_eq!(w("\u{00AE}"), 1);
        assert_eq!(w("\u{2122}"), 1);
    }

    /// D-W3:组合记号与零宽格式字符计 0。
    #[test]
    fn zero_width_marks_and_formats() {
        assert_eq!(w("\u{0301}"), 0, "COMBINING ACUTE ACCENT");
        assert_eq!(w("\u{200B}"), 0, "ZERO WIDTH SPACE");
        assert_eq!(w("\u{200D}"), 0, "ZERO WIDTH JOINER");
        assert_eq!(w("\u{FE0F}"), 0, "VARIATION SELECTOR-16");
        // 但 `Mc`(带间距)不�� 0:天城文 visarga 在真实字体里占宽度。
        assert_eq!(w("\u{0903}"), 1, "DEVANAGARI SIGN VISARGA 是 Mc");
    }

    /// D-W3 + 簇折叠:`e` + U+0301 是一个簇,列宽是**簇内最大**(1),不是和。
    #[test]
    fn combining_mark_folds_into_its_base() {
        assert_eq!(w("e\u{0301}"), 1);
        assert_eq!(w("a\u{0301}\u{0308}"), 1);
    }

    /// D-W4:ZWJ emoji 序列**一个**字形 = 两列。这是折叠存在的全部理由。
    #[test]
    fn zwj_emoji_sequence_is_two_columns() {
        assert_eq!(w("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"), 2);
        assert_eq!(w("\u{1F600}"), 2);
    }

    /// D-W2 + 折叠:两个区域指示符是**一个**簇。
    #[test]
    fn regional_indicator_pair_is_two_columns() {
        assert_eq!(w("\u{1F1E8}\u{1F1F3}"), 2);
        assert_eq!(
            w("\u{1F1E8}\u{1F1F3}\u{1F1E8}\u{1F1F3}"),
            4,
            "两面旗 = 两簇"
        );
    }
}
