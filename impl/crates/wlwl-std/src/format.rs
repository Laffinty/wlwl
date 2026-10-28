//! `wlwl:std.format` — `FORMAT` (v0.4 §10.6 + §15.8, error code E0039)
//! + `FORMAT_NAMED` (W-28).
//!
//! ## 两个成员;既有 `FORMAT` 的语义**一字未改**
//!
//! `FORMAT` 是 v0.4 §10.6 的成员,它的语义被
//! `wlwl-eval/tests/format_contract.rs` 逐条对拍着(连「与全局内建之间
//! 那条**刻意保留**的分歧」都有专门的一条用例钉着)⇒ **一个字都不许改**。
//! W-28 起本模块有**两个**成员:`FORMAT` 与**新增**的 `FORMAT_NAMED` ——
//! 后者是**更宽**的写法,不是替代品:
//!
//! | | `FORMAT`(既有) | `FORMAT_NAMED`(W-28 新增) |
//! |---|---|---|
//! | 具名查找 | 只认**第一个** `DICT` 实参(§10.6) | 认**全部** `DICT` 实参 |
//! | 格式说明 | 无(`{name}` 只有名字) | `{name:spec}` / `{N:spec}` |
//! | 缺键 | 保留占位符原样 | 保留占位符原样(**同款**) |
//!
//! ⇒ 「更宽」只落在**上面两行**;其余(渲染口径 = `STR` 语义、模板畸形 ⇒
//!   `E0039`、未匹配的占位符保留原样)与 `FORMAT` **逐字同款**。
//!
//! This module owns the **template grammar** for `FORMAT` (see
//! [`parse_template`]); both FORMAT entry points share it:
//!
//! - the global builtin in `wlwl-eval` (`resolve_builtin("FORMAT")`,
//!   per spec appendix G `FORMAT` is a global builtin, not a macro),
//!   which renders on the evaluator's `Value` type;
//! - this module's [`std_format`], the `StdFn` bound by
//!   `IMPORT("wlwl:std.format", ["FORMAT"])`, which renders on
//!   `StdValue` (`serde_json::Value`) at the std boundary.
//!
//! Rendering therefore exists twice (once per value world) but the
//! grammar exists once. The std-side render cannot see closures /
//! native fns — the eval→std boundary rejects them with E0030 before
//! dispatch (existing contract for every std module), while the global
//! builtin renders them via `Value::display()` (`<fun(...)>`), matching
//! `STR` semantics (§10.3). One more cosmetic difference: this side
//! renders dict keys in `serde_json::Map` order (BTreeMap unless
//! `preserve_order` is enabled), the eval side in insertion order.

use crate::compat::*;
use crate::{ModuleSpec, StdCtx, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{Outcome, StdHost, Value};

/// One parsed piece of a FORMAT template (v0.4 §10.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatSegment {
    /// Literal text outside `{...}` (a stray `}` is literal too).
    Literal(String),
    /// `{012}` — all-ASCII-digit placeholder; indexes the format args
    /// (the arguments **after** the template).
    Positional(usize),
    /// `{name}` — non-digit, non-empty placeholder; looked up by key in
    /// the first DICT among the format args (in the pure-named pattern
    /// that dict is `args[0]`, exactly as §10.6 specifies).
    Named(String),
}

/// Parse a FORMAT template into segments (v0.4 §10.6).
///
/// Grammar:
/// - `{N}` / `{name}` placeholders: all-ASCII-digits → positional,
///   anything else non-empty → named;
/// - a stray `}` (no open `{`) is a literal character;
/// - **E0039** (`malformed format template`) on a `{` that never
///   closes before end-of-template, and on an empty placeholder `{}`.
///
/// Note: "unmatched" is **not** a parse failure — whether a positional
/// index is in range or a named key exists is a *render-time* concern
/// decided by the args, so an out-of-range `{5}` or missing `{name}`
/// renders as the original text (§10.6 末段: 未匹配的 `{...}` 保留原样).
/// The single parse-time exception is an all-digit index that
/// overflows `usize`: it can never match any arg, so it is folded into
/// a `Literal` segment here rather than erroring.
pub fn parse_template(template: &str) -> Result<Vec<FormatSegment>, StdError> {
    parse_template_as("FORMAT", template)
}

/// [`parse_template`] 的**具名**版本 —— 诊断消息的前缀用 `caller`。
///
/// ⚠ **为什么需要它**:`FORMAT` 与 `FORMAT_NAMED` 共用**同一条**模板文法
///   (一条文法、两处渲染),但它们的诊断消息必须**各自**指名自己 ——
///   `FORMAT: malformed format template: …` 与
///   `FORMAT_NAMED: malformed format template: …`。
/// ⚠ **既有 `FORMAT` 的消息逐字未变**:`parse_template` 传的就是 `"FORMAT"`
///   ⇒ 那条消息与 v0.4 起就一模一样。
fn parse_template_as(caller: &str, template: &str) -> Result<Vec<FormatSegment>, StdError> {
    let chars: Vec<(usize, char)> = template.char_indices().collect();
    let mut segments: Vec<FormatSegment> = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < chars.len() {
        let (open_at, c) = chars[i];
        if c != '{' {
            lit.push(c);
            i += 1;
            continue;
        }
        // Find the closing `}`; E0039 if we hit end-of-template first.
        let mut j = i + 1;
        while j < chars.len() && chars[j].1 != '}' {
            j += 1;
        }
        if j >= chars.len() {
            return Err(StdError {
                code: ErrorCode::E0039,
                message: format!(
                    "{caller}: malformed format template: unclosed '{{' at offset {}",
                    open_at
                ),
            });
        }
        let content: String = chars[i + 1..j].iter().map(|(_, c)| *c).collect();
        if content.is_empty() {
            return Err(StdError {
                code: ErrorCode::E0039,
                message: format!(
                    "{caller}: malformed format template: empty placeholder '{{}}' at offset {}",
                    open_at
                ),
            });
        }
        if !lit.is_empty() {
            segments.push(FormatSegment::Literal(std::mem::take(&mut lit)));
        }
        if content.chars().all(|c| c.is_ascii_digit()) {
            match content.parse::<usize>() {
                Ok(n) => segments.push(FormatSegment::Positional(n)),
                // All digits but too large for usize: can never match an
                // arg — keep the original text (unmatched, not malformed).
                Err(_) => segments.push(FormatSegment::Literal(format!("{{{}}}", content))),
            }
        } else {
            segments.push(FormatSegment::Named(content));
        }
        i = j + 1;
    }
    if !lit.is_empty() {
        segments.push(FormatSegment::Literal(lit));
    }
    Ok(segments)
}

/// Render a `StdValue` for insertion into a FORMAT result. Mirrors the
/// evaluator's `Value::display()` (which is `STR` semantics, §10.3):
/// STRING as-is, INTEGER `42`, FLOAT `30.0` / `3.14`, BOOLEAN
/// `TRUE`/`FALSE`, NULL `NULL`, ARRAY / DICT structurally.
fn std_display(v: &StdValue) -> String {
    match v {
        StdValue::Null => "NULL".into(),
        StdValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        StdValue::String(s) => s.clone(),
        StdValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 {
                    format!("{:.1}", f)
                } else {
                    f.to_string()
                }
            } else {
                // serde_json guards against NaN/Inf at construction;
                // this arm is unreachable in practice.
                n.to_string()
            }
        }
        StdValue::Array(items) => format!(
            "[{}]",
            items.iter().map(std_display).collect::<Vec<_>>().join(", ")
        ),
        StdValue::Object(m) => format!(
            "[{}]",
            m.iter()
                .map(|(k, v)| format!("{}: {}", k, std_display(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `FORMAT(template, args...) → STRING` — the `wlwl:std.format` module
/// entry (v0.4 §10.6 + §15.8).
///
/// - `args[0]` must be a STRING (the template), else E0030;
/// - `{N}` inserts the N-th format arg (args after the template),
///   rendered via STR semantics; out of range → the placeholder text
///   is kept as-is;
/// - `{name}` looks up the first DICT among the format args; missing
///   key / no DICT arg → kept as-is;
/// - template parse failure → E0039.
pub(super) fn std_format_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    if args.is_empty() {
        return Err(StdError {
            code: ErrorCode::E0022,
            message: "FORMAT: function expects at least 1 argument (template), got 0".into(),
        });
    }
    let template = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => {
            return Err(type_error(
                "FORMAT",
                "string (template) as first arg",
                other,
            ))
        }
    };
    let segments = parse_template(template)?;
    let format_args = &args[1..];
    // Named-lookup source: the first DICT among the format args. In the
    // pure-named pattern this is args[0] exactly as §10.6 specifies;
    // scanning (instead of hardcoding args[0]) is what makes the spec's
    // own mixed example work: FORMAT("hi {0}, age {age}", "alice", ["age": 30]).
    let named = format_args.iter().find_map(|a| match a {
        StdValue::Object(m) => Some(m),
        _ => None,
    });
    let mut out = String::new();
    for seg in &segments {
        match seg {
            FormatSegment::Literal(s) => out.push_str(s),
            FormatSegment::Positional(n) => match format_args.get(*n) {
                Some(v) => out.push_str(&std_display(v)),
                None => {
                    out.push('{');
                    out.push_str(&n.to_string());
                    out.push('}');
                }
            },
            FormatSegment::Named(name) => {
                let hit = named.and_then(|m| m.get(name.as_str()));
                match hit {
                    Some(v) => out.push_str(&std_display(v)),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
            }
        }
    }
    Ok(StdValue::String(out))
}

// ── W-28:FORMAT_NAMED 的格式说明 ───────────────────────────────────────

/// 填充对齐的方向(`{name:spec}` 的 `align` 位)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatAlign {
    /// `<` —— 左对齐。**缺省**。
    Left,
    /// `>` —— 右对齐。
    Right,
    /// `^` —— 居中;多出来的那一格给**右边**(与 Python 的 `str.center` 同款)。
    Center,
}

/// 一条格式说明(`{name:spec}` / `{N:spec}` 的 `spec` 部分)。
///
/// 文法:`[[fill]align][width]`
///
/// - `fill`:**一个**字符(任意字符,含空格);缺省是**空格**;
/// - `align`:`<` / `>` / `^`;**缺省是左对齐**;
/// - `width`:十进制数字;缺省 = **不填充**。
///
/// ⚠ 宽度按**码点**算(与 `LEN` 同一口径),不是字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatSpec {
    /// 填充字符。
    pub fill: char,
    /// 对齐方向。
    pub align: FormatAlign,
    /// 最小宽度(码点数)。
    pub width: usize,
}

/// `align` 位 ⇒ 方向。非法字符 ⇒ `None`。
fn align_of(c: char) -> Option<FormatAlign> {
    match c {
        '<' => Some(FormatAlign::Left),
        '>' => Some(FormatAlign::Right),
        '^' => Some(FormatAlign::Center),
        _ => None,
    }
}

/// 解析一条格式说明。空串 ⇒ `None`(不施加任何格式化)。
///
/// 畸形 ⇒ **E0039**(与 `FORMAT` 的「模板畸形」**同一个**诊断码 ——
/// **不新增诊断码**)。
fn parse_format_spec(spec: &str) -> Result<Option<FormatSpec>, StdError> {
    if spec.is_empty() {
        return Ok(None);
    }
    let mut chars = spec.chars();
    let first = chars.next().unwrap_or(' ');
    let second = chars.next();
    let (fill, align, width_part): (char, FormatAlign, &str) = match second {
        // `[fill]align`:`fill` 是**紧挨着** `align` 的那一个字符。
        Some(c) if align_of(c).is_some() => (
            first,
            align_of(c).unwrap_or(FormatAlign::Left),
            &spec[first.len_utf8() + c.len_utf8()..],
        ),
        // 没有 `fill`:要么 `align` 自己打头,要么整条串就是宽度。
        _ => match align_of(first) {
            Some(a) => (' ', a, &spec[first.len_utf8()..]),
            None => (' ', FormatAlign::Left, spec),
        },
    };
    let width = if width_part.is_empty() {
        0
    } else if width_part.bytes().all(|b| b.is_ascii_digit()) {
        // 宽度溢出 `usize` ⇒ 报「畸形」,而不是截断成一个荒谬的宽度。
        width_part.parse::<usize>().map_err(|_| StdError {
            code: ErrorCode::E0039,
            message: format!(
                "FORMAT_NAMED: malformed format spec: width out of range: {width_part}"
            ),
        })?
    } else {
        return Err(StdError {
            code: ErrorCode::E0039,
            message: format!("FORMAT_NAMED: malformed format spec: {spec}"),
        });
    };
    Ok(Some(FormatSpec { fill, align, width }))
}

/// 按格式说明把 `text` 填到最小宽度。
///
/// ⚠ **内容比宽度长 ⇒ 原样返回**(不截断)—— 与 Python 的 `format` 同款。
fn apply_format_spec(text: &str, spec: &FormatSpec) -> String {
    let len = text.chars().count();
    if len >= spec.width {
        return text.to_string();
    }
    let pad = spec.width - len;
    let mut out = String::with_capacity(spec.width);
    match spec.align {
        FormatAlign::Left => {
            out.push_str(text);
            for _ in 0..pad {
                out.push(spec.fill);
            }
        }
        FormatAlign::Right => {
            for _ in 0..pad {
                out.push(spec.fill);
            }
            out.push_str(text);
        }
        FormatAlign::Center => {
            let left = pad / 2;
            for _ in 0..left {
                out.push(spec.fill);
            }
            out.push_str(text);
            for _ in 0..(pad - left) {
                out.push(spec.fill);
            }
        }
    }
    out
}

/// `FORMAT_NAMED(template, args...) → STRING` —— `wlwl:std.format` 的
/// **第二个**成员(W-28)。
///
/// ## 与既有 [`std_format`] 的差别只有两处
///
/// 1. **具名查找看全部 `DICT` 实参**(按出现序,第一个命中者胜出);
///    既有 `FORMAT` 只认**第一个** `DICT`(§10.6 的明文口径,一字未改)。
/// 2. 占位符可以带一条**格式说明**:`{name:spec}` / `{N:spec}`。
///
/// 其余**逐字同款**:渲染口径 = `STR` 语义(§10.3);模板畸形 ⇒ `E0039`;
/// 未匹配的占位符 ⇒ **保留原样**(缺键**不**报错)。
///
/// ## 占位符文法
///
/// - `{name}` ⇒ 具名;`{N}` ⇒ 位置(第 N 个**模板之后**的实参);
/// - `{name:spec}` / `{N:spec}` ⇒ 加一条格式说明(见 [`FormatSpec`]);
/// - `{}`(空占位符)、`{:spec}`(空名字)、未闭合的 `{` ⇒ **E0039**;
/// - **缺失的键** / 越界的下标 ⇒ **保留占位符原样**(连格式说明一起保留)。
///
/// ## 格式说明文法:`[[fill]align][width]`
///
/// - `fill`:一个字符(任意,含空格);缺省空格;
/// - `align`:`<` 左 / `>` 右 / `^` 居中;**缺省左对齐**;
/// - `width`:十进制数字;缺省不填充。宽度按**码点**算。
///
/// ⚠ **`:` 一律是「键与格式说明」的分隔符** ⇒ 本成员的**键里不能含 `:`**
///   (既有 `FORMAT` 的键可以 —— 那是另一个成员的文法)。
/// ⚠ **键里不能含 `:`** 这条是本成员**新增**的约束,不是对 `FORMAT` 的改动。
pub(super) fn std_format_named_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    if args.is_empty() {
        return Err(StdError {
            code: ErrorCode::E0022,
            message: "FORMAT_NAMED: function expects at least 1 argument (template), got 0".into(),
        });
    }
    let template = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => {
            return Err(type_error(
                "FORMAT_NAMED",
                "string (template) as first arg",
                other,
            ))
        }
    };
    let segments = parse_template_as("FORMAT_NAMED", template)?;
    let format_args = &args[1..];
    // 具名查找源:**全部** `DICT` 实参(按出现序)。
    //
    // ⚠ 这正是本成员与既有 `FORMAT` 的**唯一**语义差别之一:后者只认
    //   **第一个** `DICT`(§10.6 明文,一字未改);本成员是**新**成员,口径更宽。
    let dicts: Vec<&serde_json::Map<String, StdValue>> = format_args
        .iter()
        .filter_map(|a| match a {
            StdValue::Object(m) => Some(m),
            _ => None,
        })
        .collect();
    let mut out = String::new();
    for seg in &segments {
        match seg {
            FormatSegment::Literal(s) => out.push_str(s),
            FormatSegment::Positional(n) => match format_args.get(*n) {
                Some(v) => out.push_str(&std_display(v)),
                None => {
                    out.push('{');
                    out.push_str(&n.to_string());
                    out.push('}');
                }
            },
            FormatSegment::Named(content) => {
                // `{head:spec}` —— `head` 是键(或下标),`spec` 是格式说明。
                let (head, spec_text) = match content.find(':') {
                    Some(i) => (&content[..i], &content[i + 1..]),
                    None => (content.as_str(), ""),
                };
                if head.is_empty() {
                    // `{:spec}` —— 名字为空(与 `{}` 同款:空占位符是畸形)。
                    return Err(StdError {
                        code: ErrorCode::E0039,
                        message: format!(
                            "FORMAT_NAMED: malformed format template: empty placeholder name in '{{{content}}}'"
                        ),
                    });
                }
                // ⚠ 格式说明**先**解析:模板畸形就是畸形,与数据在不在场无关。
                let spec = parse_format_spec(spec_text)?;
                // `head` 全是 ASCII 数字 ⇒ 位置下标(与 `FORMAT` 的 `{0}` 同一条
                // 规则);否则是具名键。
                let hit = if head.bytes().all(|b| b.is_ascii_digit()) {
                    match head.parse::<usize>() {
                        Ok(idx) => format_args.get(idx),
                        // 下标溢出 `usize` ⇒ 永远命中不了 ⇒ 保留原样。
                        Err(_) => None,
                    }
                } else {
                    dicts.iter().find_map(|m| m.get(head))
                };
                match hit {
                    Some(v) => {
                        let text = std_display(v);
                        match &spec {
                            Some(spec) => out.push_str(&apply_format_spec(&text, spec)),
                            None => out.push_str(&text),
                        }
                    }
                    // 未命中 ⇒ **保留占位符原样**(含格式说明)。
                    None => {
                        out.push('{');
                        out.push_str(content);
                        out.push('}');
                    }
                }
            }
        }
    }
    Ok(StdValue::String(out))
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.format",
    functions: &[
        ("FORMAT", std_format as StdFn),
        // [W-28] `FORMAT_NAMED` —— **新成员追加在末尾**(仓内惯例:老成员在前)。
        ("FORMAT_NAMED", std_format_named as StdFn),
    ],
};

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_format(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, std_format_inner, args)
}

/// [W-28] `FORMAT_NAMED` 的直通边界包装:Value→内部表示→Value。
pub fn std_format_named(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, std_format_named_inner, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segs(t: &str) -> Vec<FormatSegment> {
        parse_template(t).expect("template parses")
    }

    fn fmt(args: Vec<StdValue>) -> Result<String, StdError> {
        let mut ctx = StdCtx::default();
        std_format_inner(&mut ctx, args).map(|v| match v {
            StdValue::String(s) => s,
            other => panic!("FORMAT must return a string, got {:?}", other),
        })
    }

    fn s(v: &str) -> StdValue {
        StdValue::String(v.into())
    }

    // ── parse_template ─────────────────────────────────────────────

    #[test]
    fn parse_empty_template_is_ok_empty() {
        assert_eq!(segs(""), vec![]);
    }

    #[test]
    fn parse_plain_text_is_single_literal() {
        assert_eq!(
            segs("hello world"),
            vec![FormatSegment::Literal("hello world".into())]
        );
    }

    #[test]
    fn parse_positional_placeholders() {
        assert_eq!(
            segs("hi {0}, you are {1}"),
            vec![
                FormatSegment::Literal("hi ".into()),
                FormatSegment::Positional(0),
                FormatSegment::Literal(", you are ".into()),
                FormatSegment::Positional(1),
            ]
        );
    }

    #[test]
    fn parse_named_placeholder() {
        assert_eq!(
            segs("{name}!"),
            vec![
                FormatSegment::Named("name".into()),
                FormatSegment::Literal("!".into())
            ]
        );
    }

    #[test]
    fn parse_mixed_placeholders() {
        assert_eq!(
            segs("{0}-{name}"),
            vec![
                FormatSegment::Positional(0),
                FormatSegment::Literal("-".into()),
                FormatSegment::Named("name".into())
            ]
        );
    }

    #[test]
    fn parse_leading_zero_index_is_positional() {
        assert_eq!(segs("{00}"), vec![FormatSegment::Positional(0)]);
        assert_eq!(segs("{01}"), vec![FormatSegment::Positional(1)]);
    }

    #[test]
    fn parse_non_ascii_digit_content_is_named() {
        // Only ASCII digits are positional; a unicode digit is a name.
        assert_eq!(segs("{٣}"), vec![FormatSegment::Named("٣".into())]);
    }

    #[test]
    fn parse_stray_close_brace_is_literal() {
        assert_eq!(segs("a } b"), vec![FormatSegment::Literal("a } b".into())]);
    }

    #[test]
    fn parse_unclosed_brace_is_e0039() {
        let err = parse_template("hi {0").unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
        assert!(err.message.contains("unclosed"), "{}", err.message);
    }

    #[test]
    fn parse_lone_open_brace_is_e0039() {
        // The spec §10.6 example of a parse failure: "`{` 单独出现".
        let err = parse_template("{").unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
    }

    #[test]
    fn parse_empty_placeholder_is_e0039() {
        let err = parse_template("a {} b").unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
        assert!(err.message.contains("empty"), "{}", err.message);
    }

    #[test]
    fn parse_overflow_index_folds_to_literal_not_error() {
        assert_eq!(
            segs("{99999999999999999999999}"),
            vec![FormatSegment::Literal("{99999999999999999999999}".into())]
        );
    }

    // ── std_format rendering ───────────────────────────────────────

    #[test]
    fn format_positional_spec_example() {
        // Spec §10.6 example 1.
        let out = fmt(vec![
            s("hi {0}, you are {1} years old"),
            s("alice"),
            StdValue::from(30),
        ])
        .unwrap();
        assert_eq!(out, "hi alice, you are 30 years old");
    }

    #[test]
    fn format_named_spec_example() {
        // Spec §10.6 example 2 (pure-named pattern: the dict IS args[0]
        // of the format args).
        let out = fmt(vec![
            s("hi {name}, age {age}"),
            serde_json::json!({"name": "alice", "age": 30}),
        ])
        .unwrap();
        assert_eq!(out, "hi alice, age 30");
    }

    #[test]
    fn format_mixed_spec_example() {
        // Spec §10.6 example 3 — the mixed pattern that motivates the
        // "first DICT among args" lookup rule.
        let out = fmt(vec![
            s("hi {0}, age {age}"),
            s("alice"),
            serde_json::json!({"age": 30}),
        ])
        .unwrap();
        assert_eq!(out, "hi alice, age 30");
    }

    #[test]
    fn format_repeated_placeholder() {
        let out = fmt(vec![s("{0} and {0} and {0}"), s("x")]).unwrap();
        assert_eq!(out, "x and x and x");
    }

    #[test]
    fn format_conversions_via_str_semantics() {
        assert_eq!(fmt(vec![s("{0}"), StdValue::from(42)]).unwrap(), "42");
        // Float literals avoid approx-PI constants (`3.14`) to keep
        // `clippy::approx_constant` (deny) happy.
        assert_eq!(fmt(vec![s("{0}"), StdValue::from(30.0)]).unwrap(), "30.0");
        assert_eq!(fmt(vec![s("{0}"), StdValue::from(1.25)]).unwrap(), "1.25");
        assert_eq!(fmt(vec![s("{0}"), StdValue::Bool(true)]).unwrap(), "TRUE");
        assert_eq!(fmt(vec![s("{0}"), StdValue::Bool(false)]).unwrap(), "FALSE");
        assert_eq!(fmt(vec![s("{0}"), StdValue::Null]).unwrap(), "NULL");
        assert_eq!(
            fmt(vec![s("{0}"), serde_json::json!([1, 2, 3])]).unwrap(),
            "[1, 2, 3]"
        );
        assert_eq!(
            fmt(vec![s("{0}"), serde_json::json!({"a": 1})]).unwrap(),
            "[a: 1]"
        );
    }

    #[test]
    fn format_unmatched_positional_kept_literal() {
        let out = fmt(vec![s("{0} + {5}"), s("a")]).unwrap();
        assert_eq!(out, "a + {5}");
    }

    #[test]
    fn format_unmatched_named_key_kept_literal() {
        let out = fmt(vec![s("{name}"), serde_json::json!({"other": 1})]).unwrap();
        assert_eq!(out, "{name}");
    }

    #[test]
    fn format_named_without_dict_kept_literal() {
        let out = fmt(vec![s("hi {name}"), s("alice")]).unwrap();
        assert_eq!(out, "hi {name}");
    }

    #[test]
    fn format_named_skips_non_dict_args() {
        // First DICT among the format args wins; scalars are skipped.
        let out = fmt(vec![
            s("{name}"),
            StdValue::from(1),
            serde_json::json!({"name": "n"}),
        ])
        .unwrap();
        assert_eq!(out, "n");
    }

    #[test]
    fn format_positional_refers_to_dict_arg_renders_it() {
        // {0} pointing at the dict itself renders it structurally.
        let out = fmt(vec![s("{0}"), serde_json::json!({"a": 1})]).unwrap();
        assert_eq!(out, "[a: 1]");
    }

    #[test]
    fn format_template_not_string_is_e0030() {
        let err = fmt(vec![StdValue::from(42)]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn format_zero_args_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_format_inner(&mut ctx, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
        assert!(err.message.contains("at least 1"), "{}", err.message);
    }

    #[test]
    fn format_malformed_template_is_e0039() {
        let err = fmt(vec![s("bad { template")]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0039);
    }

    #[test]
    fn format_empty_template_and_no_args() {
        assert_eq!(fmt(vec![s("")]).unwrap(), "");
    }

    // [W-28] 成员面:两个成员,**老成员在前、新成员追加**。
    //
    // ⚠ 少一个 ⇒ 集成者的登记清单与实现对不上;多一个 ⇒ 同上。
    //   （`FORMAT` 的**名字**与**位置**都不能变 —— 既有语义一字未改。）
    #[test]
    fn spec_lists_format_then_format_named() {
        assert_eq!(SPEC.path, "wlwl:std.format");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["FORMAT", "FORMAT_NAMED"]);
    }
}
