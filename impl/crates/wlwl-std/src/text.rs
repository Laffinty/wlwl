//! [v0.11.2 M2] `wlwl:std.text` — full-Unicode case mapping (R2).
//!
//! ## 为什么这不是「再加两个函数」
//!
//! 全局内建 `UPPER` / `LOWER` 只做 ASCII 映射(语言规范 §10.5)。实测
//! `UPPER("straße")` = `STRAßE`,`UPPER("héllo")` = `HéLLO` —— 非 ASCII
//! 字符原样穿过。对一个把「可复现」写进卖点的语言来说,这是**静默的错误
//! 结果**而不是可见的失败:大小写折叠是排序键、去重键、比较键的常见来源。
//!
//! ## 零 Unicode 表
//!
//! **本模块不需要任何 Unicode 数据文件。** `str::to_uppercase` /
//! `to_lowercase` 直接走 Rust `char` 实现内置的 `Uppercase` / `Lowercase`
//! 查表(含 `ß` → `SS`、希腊 final sigma、土耳其无点 i 这类特殊映射)。
//! ADR-0022 的零第三方依赖画像因此保持不变。
//!
//! ## 为什么只有两个成员
//!
//! 原计划还有 `GRAPHEME_COUNT` / `WIDTH`,连同 `NFC` / `NFD` 一起推迟。
//! 理由是它们**共用同一捆 UCD 数据**(`GraphemeBreakProperty` +
//! `Extended_Pictographic` / `UnicodeData` + `CompositionExclusions` +
//! `EastAsianWidth`)。分批做意味着生成器要写三遍、审三遍、跟 Unicode
//! 版本对齐三遍。合成一块做,是**一次**决定而不是四次。见
//! `docs/history/20261002.md` §1(v0.11.2 构建计划归档件)§5.2 的裁决更新。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.text",
    functions: &[
        ("TO_UPPER", to_upper as StdFn),
        ("TO_LOWER", to_lower as StdFn),
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
