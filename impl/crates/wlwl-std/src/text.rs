//! [v0.11.2 M2] `wlwl:std.text` — 完整 Unicode 简单大小写映射 + 规范规范化(R2)。
//!
//! ## 为什么这不是「再加几个函数」
//!
//! 全局内建 `UPPER` / `LOWER` 只做 ASCII 映射(语言规范 §10.5)。实测
//! `UPPER("straße")` = `STRAßE`,`UPPER("héllo")` = `HéLLO` —— 非 ASCII
//! 字符原样穿过。对一个把「可复现」写进卖点的语言来说,这是**静默的错误
//! 结果**而不是可见的失败:大小写折叠是排序键、去重键、比较键的常见来源。
//!
//! 同理,`==(a, b)` 在组合字符上会给出一个**可见但错误**的答案:`"é"` 一个
//! 是 U+00E1,另一个是 `U+0065 U+0301`,渲染一模一样,比较结果 FALSE。
//! 索引、去重、缓存键的标准前置是 `==(NFC(a), NFC(b))`。
//!
//! ## 大小写部分零 Unicode 表
//!
//! **大小写映射不需要任何 Unicode 数据文件。** `str::to_uppercase` /
//! `to_lowercase` 直接走 Rust `char` 实现内置的 `Uppercase` / `Lowercase`
//! 查表(含 `ß` → `SS`、希腊 final sigma、土耳其无点 i 这类特殊映射)。
//!
//! ## 规范化部分**要**表,而且正确性只有一处证据
//!
//! `NFC` / `NFD` / `NFC_QC` 走 [`crate::unicode::canon`],数据来自 UCD
//! **18.0.0**(生成器 `bin/gen-ucd` 产出入库表,3.66 MB 源数据不入库)。
//!
//! ⚠ **本文件的单测与契约表都不是正确性证据。** 规范化排错一个码点时,
//! 渲染结果**完全相同** —— 人眼、截图、字节 diff 全都看不出来。唯一
//! 证据是 `bin/norm-check` 对官方 `NormalizationTest.txt` 的**全量**自检
//! (20 171 行 × 100 855 条断言,`addendum-03` §3.2)。
//!
//! ## 为什么只有两个成员
//!
//! 原计划还有 `GRAPHEME_COUNT` / `WIDTH`,连同 `NFC` / `NFD` 一起推迟。
//! 理由是它们**共用同一捆 UCD 数据**(`GraphemeBreakProperty` +
//! `Extended_Pictographic` / `UnicodeData` + `CompositionExclusions` +
//! `EastAsianWidth`)。分批做意味着生成器要写三遍、审三遍、跟 Unicode
//! 版本对齐三遍。合成一块做,是**一次**决定而不是四次。见
//! `docs/history/20261002.md` §1(v0.11.2 构建计划归档件)§5.2 的裁决更新。

use crate::unicode::canon;
use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.text",
    functions: &[
        ("TO_UPPER", to_upper as StdFn),
        ("TO_LOWER", to_lower as StdFn),
        ("NFC", nfc as StdFn),
        ("NFD", nfd as StdFn),
        ("NFC_QC", nfc_qc as StdFn),
    ],
};

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected string, got {}", crate::value_kind(got)),
    )
}

fn case_map(
    host: &mut dyn StdHost,
    name: &str,
    args: Vec<Value>,
    upper: bool,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() != 1 {
        return Err(arity(host, name, args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(type_err(host, name, &args[0]));
    };
    let mapped = if upper {
        s.to_uppercase()
    } else {
        s.to_lowercase()
    };
    Ok(wlwl_value::Outcome::normal(Value::String(mapped)))
}

pub fn to_upper(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    case_map(host, "TO_UPPER", args, true)
}

pub fn to_lower(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    case_map(host, "TO_LOWER", args, false)
}

// ── 规范化(v0.11.3 M6 / addendum-03 W-03)──────────────────────────
//
// 算法与四张 UCD 表都在 [`crate::unicode`];这里只做 `String` ↔ 码点序列
// 的转接与诊断。**正确性不在本文件证明** —— `NFC` / `NFD` 的正确性只由
// `bin/norm-check` 对官方 `NormalizationTest.txt` 的**全量**自检说了算
// (`addendum-03` §3.2)。下面这层薄包装不该有自己的「看起来对」的断言。

/// 取唯一实参;不是 `STRING` 就发 `E0030`,元数不对发 `E0022`。
fn str_arg<'a>(
    host: &mut dyn StdHost,
    name: &str,
    args: &'a [Value],
) -> wlwl_error::WlwlResult<&'a str> {
    if args.len() != 1 {
        return Err(arity(host, name, args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(type_err(host, name, &args[0]));
    };
    Ok(s)
}

/// 码点序列 → `String`。
///
/// `None` 分支**不可达**:入参来自一个合法 `String`(没有代理码点),而规范
/// 分解 / 组合 / 排序都不会凭空造出新码点,UCD 表里也不存在代理区。这里
/// 选择**跳过**而不是 panic —— 一份坏表不该把整个运行打挂 —— 同时用
/// `debug_assert` 让它在测试与开发构建里叫出来。
fn to_string(cps: &[u32]) -> String {
    let mut s = String::with_capacity(cps.len());
    for &cp in cps {
        match char::from_u32(cp) {
            Some(c) => s.push(c),
            None => debug_assert!(false, "规范化产出了非标量值 U+{cp:04X}"),
        }
    }
    s
}

/// `NFC(s)` —— 规范组合。`"e"` + U+0301 → `"é"`(**长度变短**)。
pub fn nfc(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "NFC", &args)?;
    // 快检命中时直接借原串:不进 `Vec<u32>`、不重排、不查表(纯扫描)。
    // ⚠ 成员边界仍要产出 owned `STRING`,所以那次 `to_owned()` 拷贝省不掉 ——
    // 「快检不分配」是**算法层**(`canon::nfc` 返回 `Cow::Borrowed`)的性质,
    // 不是成员层的。
    let out = if quick_ok(s) {
        s.to_owned()
    } else {
        to_string(&canon::nfc(&s.chars().map(u32::from).collect::<Vec<_>>()))
    };
    Ok(wlwl_value::Outcome::normal(Value::String(out)))
}

/// `NFD(s)` —— 规范分解。`"é"` → `"e"` + U+0301(**长度变长**)。
pub fn nfd(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "NFD", &args)?;
    let cps: Vec<u32> = s.chars().map(u32::from).collect();
    Ok(wlwl_value::Outcome::normal(Value::String(to_string(
        &canon::nfd(&cps),
    ))))
}

/// `NFC_QC(s)` —— 规范快速检查(**性能闸门,不是语义糖**)。
///
/// 语义是**单方向**的:
/// - `TRUE` ⇒ 该串**必定**已是 NFC 形态(可安全跳过规范化);
/// - `FALSE` ⇒ **不保证** —— 它可能已经是,也可能不是。要判定得走 `NFC`。
///
/// 文面必须这么写。反方向不成立:官方测试文件里 202 行的串本来就是 NFC
/// 形态(`Maybe` 类单码点行,如单独一个 U+0301),快检照样返回 `FALSE`。
/// 写成「返回 `FALSE` 即表示不是 NFC 形态」会教调用方一个假事实。
pub fn nfc_qc(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "NFC_QC", &args)?;
    Ok(wlwl_value::Outcome::normal(Value::Boolean(quick_ok(s))))
}

/// 从 `&str` 跑 D65 判据,一次 `Vec` 都不构造。
fn quick_ok(s: &str) -> bool {
    let mut last = 0u8;
    s.chars()
        .all(|ch| canon::quick_step(u32::from(ch), &mut last))
}

#[cfg(test)]
mod tests {
    fn run(upper: bool, s: &str) -> String {
        if upper {
            s.to_uppercase()
        } else {
            s.to_lowercase()
        }
    }

    /// 全局内建 `UPPER` / `LOWER` 只做 ASCII —— 这几条是本模块存在的理由,
    /// 期望值取自 Unicode 的简单大小写映射规则,不是从实现输出抄的。
    #[test]
    fn non_ascii_actually_moves() {
        // Latin-1 补充
        assert_eq!(run(true, "héllo"), "HÉLLO");
        assert_eq!(run(false, "ÉÀÜ"), "éàü");
        // Latin 扩展 A(带变音符的拉丁字母)
        assert_eq!(run(true, "āáǎà"), "ĀÁǍÀ");
        // 西里尔
        assert_eq!(run(true, "привет"), "ПРИВЕТ");
        assert_eq!(run(false, "ПРИВЕТ"), "привет");
        // 希腊
        assert_eq!(run(true, "αβγ"), "ΑΒΓ");
    }

    /// 简单映射的**长度会变**的情形。这是最容易被「逐字符 to_upper」的
    /// 实现漏掉的一类。
    #[test]
    fn sharp_s_expands() {
        // § SpecialCasing.txt:ß 的大写是 "SS"(两个码点)
        assert_eq!(run(true, "straße"), "STRASSE");
        // 土耳其无点 i / 点上 i
        assert_eq!(run(false, "İ"), "i̇");
    }

    #[test]
    fn greek_final_sigma_is_contextual() {
        // Σ 在词尾小写为 ς,词中为 σ
        assert_eq!(run(false, "ΑΣ"), "ας");
        assert_eq!(run(false, "ΣΑ"), "σα");
    }

    #[test]
    fn ascii_is_unchanged_in_shape() {
        assert_eq!(run(true, "Hello, World 123"), "HELLO, WORLD 123");
        assert_eq!(run(false, "Hello"), "hello");
        assert_eq!(run(true, ""), "");
        assert_eq!(run(false, ""), "");
    }
}
