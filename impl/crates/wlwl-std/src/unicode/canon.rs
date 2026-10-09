//! UAX #15 规范规范化(NFC / NFD)算法 —— 零第三方依赖手写。
//!
//! 三步,顺序不可换(这是 [v0.11.3 M6 / addendum-03] §3.1 结论一的直接来源):
//!
//! 1. **D68 规范分解**(递归,只取规范分解;`<` 开头的兼容性分解不用)
//! 2. **D115 规范排序**(按组合类稳定重排,starter 是屏障)
//! 3. **D117 规范组合**(带 **blocked** 判断)
//!
//! ⚠️ 只做第 2 步不做第 1 步是最容易犯、也最难发现的错:未分解的输入会
//! **原样返回**,`NFC("1E0A 0323")` 会得到 `1E0A 0323` 而不是 `1E0C 0307` ——
//! 两串渲染完全相同,肉眼、截图、diff 全都看不出来。
//!
//! 输入域是**码点序列**(`&[u32]`),不是 `String`:这让算法能处理代理码点
//! 之类进不了 Rust `char` 的位置,官方测试文件与自检都走这条路。
//! `char` 边界在 `std.text` 的成员层。

use std::borrow::Cow;

use super::norm::{CCC, COMP, DECOMP, NFC_QC_SUSPECT};

// ── Hangul(Unicode Standard §3.12)────────────────────────────────
//
// ⚠️ 这五个常量对应的分解映射**不在** `UnicodeData.txt` 里,所以 [`DECOMP`]
// 表**不覆盖** 11 172 个音节。少了这一段,所有韩文音节规范化后原样返回,
// 而渲染照样「像模像样」。
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
/// 一个初声位组合(L+V)的整除基数。
const N_COUNT: u32 = V_COUNT * T_COUNT; // 588
/// 音节总数。
const S_COUNT: u32 = L_COUNT * N_COUNT; // 11 172

/// 码点的**规范组合类**(combining class)。未登记即 0(starter)。
pub fn ccc(cp: u32) -> u8 {
    match CCC.binary_search_by_key(&cp, |&(c, _)| c) {
        Ok(i) => CCC[i].1,
        Err(_) => 0,
    }
}

/// 该码点是否是 Hangul 音节(`S` 区)。
fn is_hangul_syllable(cp: u32) -> bool {
    cp.wrapping_sub(S_BASE) < S_COUNT
}

/// `NFC_QC` 为 `No` / `Maybe` 的码点 —— 「这个码点**可能**不是 NFC 形态」。
fn is_nfc_suspect(cp: u32) -> bool {
    NFC_QC_SUSPECT.binary_search(&cp).is_ok()
}

/// **D65 规范快速检查**:该串**必定**已是 NFC 形态 ⇒ 该路径不重排、不分配。
///
/// ⚠️ **两个条件都要,少一个就静默放行错误结果**(`norm-check` 全量自检
/// 实测抓到过 1 164 条):
///
/// 1. 串里**没有** `NFC_QC` 为 `No` / `Maybe` 的码点;
/// 2. 串的组合类**非递减**(`ccc(prev) > ccc(cur) && ccc(cur) != 0` 即乱序)。
///
/// 第 2 条最容易被漏掉:`NFC_QC = Yes` 只说明「该码点自身已是 NFC 形态、
/// 且不与前一个 starter 组合」,它对**码点之间的规范排序**只字未提。
/// 反例(U+05B8 QAMATS ccc=18 / U+05B9 HOLAM ccc=19 / U+05B1 HATAF SEGOL
/// ccc=11):三者 `NFC_QC` **全是 Yes**,而串 `05B8 05B9 05B1` 的 ccc 是
/// 18,19,11 —— 已乱序,`NFC` 之后应当是 `05B1 05B8 05B9`。只查第 1 条的
/// 实现会把原串放回去,且**渲染完全相同**,肉眼与截图都看不出来。
///
/// ⚠️ 本判定是**保守**的:反向不成立 —— 串里含 `Maybe` 码点时它返回
/// `false`,而那个串可能本来就是 NFC 形态(官方测试文件 Part1 的单字符行
/// 就是这种:`0301` 单独一串 `NFC` 之后原样不动,可它的 `NFC_QC` 是 `Maybe`)。
/// 契约文面按「**不保证**是不是 NFC」写,不按「不是 NFC」写。
pub fn nfc_quick_ok(input: &[u32]) -> bool {
    let mut last = 0u8;
    input.iter().all(|&cp| quick_step(cp, &mut last))
}

/// **D65 的逐码点判据**。抽成独立函数是为了让 `std.text` 的 `NFC_QC` 成员
/// 能**直接从 `&str` 跑、一次 `Vec<u32>` 都不构造** —— 而 `nfc_quick_ok` 需要
/// 一个连续的 `&[u32]`,从字符串现攒就得先分配。
///
/// 返回 `false` 即整个串「不保证是 NFC 形态」。`last_cc` 由调用方持有并在
/// 码点之间传递(它就是 D65 的 `lastCanonicalClass`)。
pub fn quick_step(cp: u32, last_cc: &mut u8) -> bool {
    if is_nfc_suspect(cp) {
        return false;
    }
    let cc = ccc(cp);
    // `cc != 0` 让 starter(ccc=0)不参与乱序判定 —— 它是屏障,不是普通 0 类记号。
    if *last_cc > cc && cc != 0 {
        return false;
    }
    *last_cc = cc;
    true
}

/// **D68 规范分解**,递归展开到基本码点。
fn decompose_cp(cp: u32, out: &mut Vec<u32>) {
    if let Ok(i) = DECOMP.binary_search_by_key(&cp, |&(c, _)| c) {
        for &c in DECOMP[i].1 {
            decompose_cp(c, out);
        }
        return;
    }
    if is_hangul_syllable(cp) {
        let s = cp - S_BASE;
        out.push(L_BASE + s / N_COUNT);
        out.push(V_BASE + (s % N_COUNT) / T_COUNT);
        let t = s % T_COUNT;
        if t != 0 {
            out.push(T_BASE + t);
        }
        return;
    }
    out.push(cp);
}

/// **D115 规范排序**:相邻交换冒泡,组合类大的往后冒。
///
/// 交换条件 `prev > cc` 且 `prev != 0` —— `prev == 0` 表示撞上 starter,
/// starter 是**屏障**:它的组合类是 0,但它**不是**可以穿过的普通 0 类记号。
/// 少了这个 `prev == 0` 判断,记号会被排到 starter 之后。
fn canonical_order(v: &mut [u32]) {
    for i in 1..v.len() {
        let cc = u16::from(ccc(v[i]));
        if cc == 0 {
            continue;
        }
        let mut j = i;
        while j > 0 {
            let prev = u16::from(ccc(v[j - 1]));
            if prev == 0 || prev <= cc {
                break;
            }
            v.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// 组合 `(starter, 记号)` → 合成码点;不可组合则 `None`。
///
/// 表 [`COMP`] 已在生成阶段扣掉 `Full_Composition_Exclusion`(含脚本专用
/// 排除与 singleton 排除),所以这里查得到就是可组合。
#[allow(
    clippy::manual_is_multiple_of,
    reason = "clippy 建议 `s.is_multiple_of(T_COUNT)`,但该 API 是 **Rust 1.87.0** \
              才稳定的(见 std::primitive::u64 文档的 `1.87.0` 标注),而工作区 \
              `Cargo.toml` 承诺 `rust-version = \"1.85\"`(D13-013 实测下限)。\
              照建议改会把版本承诺变成谎言 —— 四条 CI workflow 全用 `stable`,\
              抓不到这种回归。真要升到 1.87 就改 `rust-version`,两处一起。"
)]
fn compose_pair(a: u32, b: u32) -> Option<u32> {
    // Hangul:L + V → LV,LV + T → LVT。表里没有,只能算。
    let l = a.wrapping_sub(L_BASE);
    if l < L_COUNT {
        let v = b.wrapping_sub(V_BASE);
        if v < V_COUNT {
            return Some(S_BASE + (l * V_COUNT + v) * T_COUNT);
        }
    }
    let s = a.wrapping_sub(S_BASE);
    if s < S_COUNT && s % T_COUNT == 0 {
        let t = b.wrapping_sub(T_BASE);
        if t > 0 && t < T_COUNT {
            return Some(a + t);
        }
    }
    COMP.binary_search_by(|&(x, y, _)| (x, y).cmp(&(a, b)))
        .ok()
        .map(|i| COMP[i].2)
}

/// **D117 规范组合**。入参必须**已分解且已排序**([`nfd`] 的输出)。
///
/// `blocked` 判断就在那个条件里:`last_cc < cc || last_cc == 0`。
/// `last_cc` 是当前 starter 之后最后一个码点的组合类(其后为空时是 0)。
/// ⚠️ 少了 `blocked` 判断,`0041 0323 0307` 会被组合成 `00C8 0307`
/// —— 而正确答案是 `1E0C 0307`(先在阻塞下合 `A` + 下方点,上方点归位)。
fn compose(input: &[u32]) -> Vec<u32> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<u32> = Vec::with_capacity(input.len());
    out.push(input[0]);
    let mut starter_idx = 0usize;
    let mut starter = input[0];
    // 首码点不是 starter(前导记号)时永远不许与它组合 —— 用 `u16::MAX`
    // 让 `last_cc < cc` 与 `last_cc == 0` 同时为假。
    let mut last_cc: u16 = if ccc(input[0]) == 0 { 0 } else { u16::MAX };
    for &ch in &input[1..] {
        let ch_cc = u16::from(ccc(ch));
        if let Some(c) = compose_pair(starter, ch) {
            if last_cc < ch_cc || last_cc == 0 {
                out[starter_idx] = c;
                starter = c;
                continue;
            }
        }
        if ch_cc == 0 {
            starter_idx = out.len();
            starter = ch;
        }
        last_cc = ch_cc;
        out.push(ch);
    }
    out
}

/// **规范分解(NFD)**:分解 + 排序。
pub fn nfd(input: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(input.len() + input.len() / 2);
    for &cp in input {
        decompose_cp(cp, &mut out);
    }
    canonical_order(&mut out);
    out
}

/// **规范组合(NFC)**:分解 + 排序 + 组合。
///
/// 快检命中时**原样借回**入参(零分配)。这就是计划里「快检命中时应为
/// O(n) 且不分配」那条的实现面。
pub fn nfc(input: &[u32]) -> Cow<'_, [u32]> {
    if nfc_quick_ok(input) {
        return Cow::Borrowed(input);
    }
    Cow::Owned(compose(&nfd(input)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cps(s: &str) -> Vec<u32> {
        s.chars().map(u32::from).collect()
    }

    fn show(v: &[u32]) -> String {
        v.iter()
            .map(|c| char::from_u32(*c).unwrap_or('?'))
            .collect()
    }

    /// 机制钉子,不是正确性证据 —— 期望值取自 Unicode 的**规则**(UAX #15
    /// 的 D68/D115/D117 与 Unicode Standard §3.12),不是从本实现输出抄的。
    /// 全量证据在 `norm-check`。
    #[test]
    fn composition_and_decomposition_round_trip() {
        // U+0041 U+0301 → U+00C1(规范组合)
        assert_eq!(
            nfc(&cps("D\u{301}\u{307}")).to_vec(),
            cps("D\u{301}\u{307}")
        );
        // 没有阻塞者时组合照常发生:D + 下方点 → U+1E0C,上方点跟在后面。
        assert_eq!(
            nfc(&cps("D\u{323}\u{307}")).to_vec(),
            cps("\u{1e0c}\u{307}")
        );
        // U+00C1 → U+0041 U+0301(规范分解)
        assert_eq!(show(&nfd(&cps("\u{c1}"))), "A\u{301}");
        // 已经是 NFC 的串原样穿过
        assert_eq!(show(&nfc(&cps("\u{c1}"))), "\u{c1}");
    }

    /// **D115 的边界**:两个记号顺序颠倒时要重排,而重排**只影响码点序**、
    /// 不影响渲染 —— 这类错单测抽查能抓到,但渲染对比永远抓不到。
    #[test]
    fn canonical_order_reorders_marks_across_a_starter() {
        // U+0301(ccc 230) 与 U+0327(ccc 202) 逆序,应排成 0327 在前。
        let got = nfd(&cps("\u{301}\u{327}"));
        assert_eq!(got, cps("\u{327}\u{301}"));
        // starter 是屏障:记号不会被排过 starter。
        let got = nfd(&cps("\u{301}A\u{327}"));
        assert_eq!(got, cps("\u{301}A\u{327}"));
    }

    /// **D117 的 blocked 判断**。这一条同时钉住「先在阻塞下组合、再让上方
    /// 点归位」的次序:去 blocked 判断的实现会得到 `U+00C8 U+0307`。
    #[test]
    fn blocked_combination_order() {
        // U+0041 U+0323 U+0307 → U+1E0C U+0307
        assert_eq!(
            nfc(&cps("D\u{301}\u{307}")).to_vec(),
            cps("D\u{301}\u{307}")
        );
        // 没有阻塞者时组合照常发生:D + 下方点 → U+1E0C,上方点跟在后面。
        assert_eq!(
            nfc(&cps("D\u{323}\u{307}")).to_vec(),
            cps("\u{1e0c}\u{307}")
        );
    }

    /// Hangul 走**算法**分解 / 组合,表里查不到 —— 这是「表 + 算法两半
    /// 都得在」的证据。
    #[test]
    fn hangul_is_algorithmic_not_tabulated() {
        // U+AC01 = L+V+T,分解**带尾声** U+11A8;U+AC00 = L+V 不带。两者不可混用。
        assert_eq!(nfd(&cps("\u{ac01}")), cps("\u{1100}\u{1161}\u{11a8}"));
        assert_eq!(nfd(&cps("\u{ac00}")), cps("\u{1100}\u{1161}"));
        // U+AC01 = L+V+T,分解**带尾声** U+11A8;U+AC00 = L+V 不带。两者不可混用。
        assert_eq!(
            nfc(&cps("\u{1100}\u{1161}\u{11a8}")).to_vec(),
            cps("\u{ac01}")
        );
        assert_eq!(nfc(&cps("\u{1100}\u{1161}")).to_vec(), cps("\u{ac00}"));
    }

    /// 快检的**保守**方向:说「是 NFC」时必须真的是 NFC。
    #[test]
    fn quick_check_never_lies_in_the_safe_direction() {
        assert!(nfc_quick_ok(&cps("hello")));
        assert!(!nfc_quick_ok(&cps("A\u{301}")));
        // 反向不成立:`0301` 单独一串规范化后不动,但快检仍说「不确定」。
        assert_eq!(nfc(&cps("\u{301}")).to_vec(), cps("\u{301}"));
        assert!(!nfc_quick_ok(&cps("\u{301}")));
    }
}
