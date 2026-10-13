//! `wlwl:std.json` 成员契约 —— [v0.11.3 M7 / addendum-09 **W-01**]
//!
//! ## 这个文件现在只钉**旧两个成员**(`PARSE` / `STRINGIFY`),一个新成员都没有
//!
//! `addendum-09` §3.6 写着「`PARSE` / `STRINGIFY` **一条都不许动**」(照 Go
//! `encoding/json/v2` 保留旧行为的体例)。⚠ **但在本文件出现之前,那句话没有任何
//! 守卫**:全仓对 `std.json` 的测试只有
//! - `src/json.rs` 的 8 条单元测试 —— 它们走**内部函数** `std_parse_inner`,
//!   **不经 `wrap()` 边界、不经解释器**,而且期望值取自 `serde_json` 自己
//!   (即实现的依赖,不是独立裁判);
//! - `stdlib_appendix_a_sync.rs` —— 只对账**成员清单**,不碰行为。
//!
//! ⇒ **经 wlwl 程序能观察到的行为,一条都没被钉住**。W-02 起要新增
//! `JSON_TOKENIZE` / `JSON_LOCATE` / `JSON_VALIDATE`,一旦新词法层与旧 `PARSE`
//! 对同一输入给出不同结论,没有任何东西会发现。所以本文件是 W-02 的**前置**。
//!
//! ## 「应然」侧的来源(两套,不混)
//!
//! 1. **值** ← Python `json`(RFC-strict:挂 `parse_constant` 让它拒收
//!    `NaN` / `Infinity`)。⚠ **Python 默认接受这三个** —— 那是它的非标准扩展,
//!    与 RFC 8259 相反。拿朴素 Python 当裁判会「验证」出错误结论。
//!    ⚠ Python 只有 `int` / `float`,wlwl 有 `INTEGER` 与 `FLOAT` 两个种类,
//!    所以**类型映射不是 Python 能裁决的**,它属于「现状冻结」侧。
//! 2. **诊断码与消息** ← **现状冻结**。本文件的价值就是「逐字不变」,所以这些
//!    是被冻结的对象,不是被验证的对象。
//!
//! 两套必须分开的原因:值可以拿去和独立实现对账,而错误码**没有**独立实现可对
//! —— `E0070` 是本仓的码。混成一套就会让「与 Python 一致」悄悄变成「与现状一致」。
//!
//! ## 为什么所有 JSON 文本都过 `wl()`
//!
//! 测试里出现的 JSON 文本要**先变成 wlwl 源里的字符串字面量**,而 wlwl 只认
//! `\" \\ \/ \b \f \n \r \t` 这几个转义(不认 `\uXXXX`)。手写这一层转义时踩过
//! Rust 原始字符串的坑(`r#""\"""#` 会在 `"\` 处提前终止,而它在编译期报的
//! 是「unicode escape sequence」这种与真实原因无关的错)。⇒ 转义收进 `wl()`
//! 一处,测试里只写**原始 JSON 文本**。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

const JSON_IMP: &str = r#"IMPORT("wlwl:std.json", ["PARSE", "STRINGIFY"]);"#;
/// `LN` 是 `std.math` 的成员、不是全局内建(实测单独用会 `E0020`),造
/// `NaN` 得显式 IMPORT。
const NAN_SRC: &str = r#"-(LN(0), LN(0))"#;

/// 把**原始 JSON 文本**包成 wlwl 源码里的字符串字面量。
fn wl(json_text: &str) -> String {
    let mut out = String::from("\"");
    for ch in json_text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '/' => out.push('/'),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_json_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// 跑一段源,回显示值 / `!CODE message`。
fn out(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    }
}

fn run(body: &str) -> String {
    out(&format!("{JSON_IMP} {body}"))
}

/// `PARSE(<原始 JSON 文本>)` 的显示值。
fn parse(json_text: &str) -> String {
    run(&format!("PARSE({})", wl(json_text)))
}

fn parse_type(json_text: &str) -> String {
    run(&format!("TYPE(PARSE({}))", wl(json_text)))
}

/// 只取错误码。诊断消息里含 serde_json 的措辞与列号,**故意不冻结** ——
/// 那是依赖升级就会动的东西,冻它等于把 CI 焊在 serde_json 的版本上。
fn parse_code(json_text: &str) -> String {
    let got = parse(json_text);
    match got.split_once(' ') {
        Some((c, _)) if c.starts_with('!') => c.to_string(),
        _ => panic!("expected a diagnostic, got a value: {got}"),
    }
}

// ── §1 RFC 8259 §3 / Appendix A:合法语法面逐条冻结 ─────────────────────
//
// 期望值经 Python `json`(RFC-strict)逐条核对通过。⚠ 显示格式是 **wlwl 自己的**
// (`[a: 1]` 是字典、`TRUE`/`FALSE`/`NULL` 是大写种类名、字符串**不带引号**),
// 与 JSON 的写法不同 —— 冻的是「用户看到什么」,不是「JSON 长什么样」。

#[test]
fn rfc8259_valid_grammar_surface_is_frozen() {
    // 对象
    assert_eq!(parse(r##"{"a": 1}"##), "[a: 1]");
    assert_eq!(parse(r##"{"a": 1, "b": 2}"##), "[a: 1, b: 2]");
    assert_eq!(parse(r##"  {  "a"  :  1  }  "##), "[a: 1]");
    // 数组
    assert_eq!(parse("[1]"), "[1]");
    assert_eq!(parse("[1,2,3]"), "[1, 2, 3]");
    // 字符串与转义(渲染不带引号,故逐条注明)
    assert_eq!(parse(r#""""#), ""); // 空串
    assert_eq!(parse(r#""abc""#), "abc");
    assert_eq!(parse(r#""\"""#), "\""); // \" ⇒ "
    assert_eq!(parse(r#""\\""#), r"\"); // \\ ⇒ 一个反斜杠
    assert_eq!(parse(r#""\/""#), "/"); // \/ ⇒ /
    assert_eq!(parse(r#""\u005C""#), r"\"); // \u005C ⇒ 反斜杠
    assert_eq!(parse(r#""\u0041""#), "A"); // \u0041 ⇒ A
                                           // ⚠ \u2028 / \u2029 语法**合法**(RFC 8259 §2 只把它们当危险输出、不当语法错),
                                           //   但显示含不可见字符 ⇒ 不在这里逐字节比对,见 round-trip 那组。
                                           // 字面量
    assert_eq!(parse("true"), "TRUE");
    assert_eq!(parse("false"), "FALSE");
    assert_eq!(parse("null"), "NULL");
    // 嵌套
    assert_eq!(
        parse(r##"[{"a":[1,2]},null,true]"##),
        "[[a: [1, 2]], NULL, TRUE]"
    );
    assert_eq!(
        parse(r##"{"a":{"b":{"c":[1,[2,[3]]]}}}"##),
        "[a: [b: [c: [1, [2, [3]]]]]]"
    );
    // ⚠ `{}` 与 `[]` 的 display **都是空串**(wlwl 的空数组与空字典渲染同形),
    //   所以类型归 §3 那组判,这里不重复。
}

#[test]
fn rfc8259_number_grammar_surface_is_frozen() {
    // 与 Python 逐条核对。⚠ `-0`:Python 给 `int 0`,wlwl 给 **FLOAT `-0.0`** ——
    // 类型差异不是值差异,见下一组。
    assert_eq!(parse("0"), "0");
    assert_eq!(parse("1"), "1");
    assert_eq!(parse("-1"), "-1");
    assert_eq!(parse("1.0"), "1.0");
    assert_eq!(parse("1E+2"), "100.0");
    assert_eq!(parse("1e-2"), "0.01");
    assert_eq!(parse("1.5e+3"), "1500.0");
    assert_eq!(parse("-0.0"), "-0.0");
}

// ── §2 RFC 8259「不应接受」清单:24 条全部 E0070 ───────────────────────
//
// 实测 Python(RFC-strict)与本实现对 24/24 给出同一判决。

#[test]
fn rfc8259_must_not_accept_list_all_give_e0070() {
    for src in [
        "NaN",               // ⚠ Python 默认**接受** —— 裁判必须挂 parse_constant
        "Infinity",          //    同上
        "-Infinity",         //    同上
        r##"{"a":1/*c*/}"##, // 注释
        "{\"a\":1 // c\n}",  // 行注释
        "[1,]",              // 数组尾逗号
        r##"{"a":1,}"##,     // 对象尾逗号
        "'a'",               // 单引号
        "01",                // 前导零
        "-01",
        "00",
        "\"a\nb\"", // 字符串里的裸控制字符
        "\"a\tb\"",
        "{a:1}",        // 键没引号
        "{} x",         // 尾部垃圾
        "",             // 空输入
        "abc",          // 裸词
        "+1",           // 前导 +
        "0x1F",         // 十六进制
        ".5",           // 缺整数位
        "5.",           // 缺小数位
        r##"{"a" 1}"##, // 缺冒号
        r##"{"a":1"##,  // 对象未闭合
        "\"abc",        // 字符串未闭合
    ] {
        assert_eq!(parse_code(src), "!E0070", "json text = {src:?}");
    }
}

// ── §3 数值种类映射:INTEGER 还是 FLOAT(最容易被改坏的一条)────────────
//
// `json_to_value` 的规则是 `n.as_i64()` 命中 ⇒ `INTEGER`,否则 `FLOAT`。
// **Python 裁决不了这一条**,所以它是现状冻结,不是对账结果。

#[test]
fn number_kind_mapping_is_frozen() {
    // `TYPE` 返回**大写**种类名(`value_type_name`)。
    assert_eq!(parse_type("1"), "INTEGER");
    assert_eq!(parse_type("1.0"), "FLOAT");
    assert_eq!(parse_type("1e2"), "FLOAT");
    // ⚠ `-0` 是 FLOAT 而不是 INTEGER —— serde_json 把 `-0` 收成 f64。
    assert_eq!(parse_type("-0"), "FLOAT");
    assert_eq!(parse_type("-0.0"), "FLOAT");
    // i64 边界之内仍是 INTEGER(2^53+1 精确 ⇒ 没在中途掉进 f64)
    assert_eq!(parse("9007199254740993"), "9007199254740993");
    assert_eq!(parse_type("9223372036854775807"), "INTEGER");
    assert_eq!(parse_type("-9223372036854775808"), "INTEGER");
    // 空数组 / 空字典的显示同形,靠 TYPE 区分
    assert_eq!(run("TYPE(PARSE(\"[]\"))"), "ARRAY");
    assert_eq!(run("TYPE(PARSE(\"{}\"))"), "DICT");
}

/// ⚠ **已知缺陷 D13-016**:`PARSE` 对 **i64 之外的整数静默丢精度**。
///
/// `18446744073709551615`(u64 上界)在 JSON 里合法,Python 给精确值;本实现
/// `as_i64()` 返回 `None` ⇒ 走 `as_f64()` ⇒ 变成 `18446744073709551616.0` ——
/// **值被改了**,不只是类型变了。`-9223372036854775809` 同理变成
/// `-9223372036854775808`。
///
/// 本组**冻结的是这个缺陷本身**:§3.6 说旧成员一条都不许动,所以本批**不修**,
/// 只把它钉住并登记为 D13-016 —— 使它不会在无人在意时变得更糟,也不会被
/// 「顺手改一下」而无记录地消失。
#[test]
fn known_defect_d13_016_integer_beyond_i64_loses_precision() {
    assert_eq!(parse_type("9223372036854775808"), "FLOAT");
    assert_eq!(parse("18446744073709551615"), "18446744073709551616.0");
    assert_eq!(parse("-9223372036854775809"), "-9223372036854775808.0");
}

// ── §4 `STRINGIFY` 的可观察面 ─────────────────────────────────────────

#[test]
fn stringify_surface_is_frozen() {
    // 紧凑形(无空格)+ **键按字典序输出**(规范 §3 明写「字典键按字典序输出」)
    assert_eq!(run(r##"STRINGIFY(["a": 1])"##), r#"{"a":1}"#);
    assert_eq!(run(r##"STRINGIFY(["b": 1, "a": 2])"##), r#"{"a":2,"b":1}"#);
    // 嵌套与类型自然映射
    assert_eq!(run("STRINGIFY([NULL, TRUE, 1, 1.5])"), "[null,true,1,1.5]");
    assert_eq!(
        run(r##"STRINGIFY(["a": [1, ["b": NULL]]])"##),
        r#"{"a":[1,{"b":null}]}"#
    );
    // 转义:JSON 自己的 `\"` `\\` `\n` 等;**`/` 不转义**
    assert_eq!(run(r#"STRINGIFY("a\"b\\c\nd")"#), r#""a\"b\\c\nd""#);
    assert_eq!(run(r#"STRINGIFY("a/b")"#), r#""a/b""#);
}

/// ⚠ `STRINGIFY` **不转义** `<` `>` `&`(规范 §11 的实测条款:JSON 自身的转义
/// 不含这五个,加固是**调用方**的责任)。冻它是因为它反直觉,且新成员很容易
/// 「顺手补上」而无人察觉。
#[test]
fn stringify_does_not_escape_html_danger_chars() {
    assert_eq!(run(r##"STRINGIFY(["<a>": "x&y"])"##), r#"{"<a>":"x&y"}"#);
}

// ── §5 边界误用(D11-019 一族,与 `src/json.rs` 的单测同族但走公开面)────

#[test]
fn arity_and_type_misuse_are_frozen() {
    assert_eq!(
        run("PARSE()"),
        "!E0022 PARSE: function expects 1 argument(s), got 0"
    );
    assert_eq!(
        run("PARSE(1, 2)"),
        "!E0022 PARSE: function expects 1 argument(s), got 2"
    );
    assert_eq!(run("PARSE(1)"), "!E0030 PARSE: expected string, got number");
    assert_eq!(
        run("PARSE(NULL)"),
        "!E0030 PARSE: expected string, got null"
    );
    assert_eq!(
        run("STRINGIFY(1, 2)"),
        "!E0022 STRINGIFY: function expects 1 argument(s), got 2"
    );
    // D11-019:整数字典键**不放宽**
    assert_eq!(
        run(r##"STRINGIFY([2: "v"])"##),
        "!E0030 std argument: expected string dict key, got integer"
    );
    // ADR-0022 §4:闭包跨界一律 E0030
    assert_eq!(
        run("STRINGIFY(FUN((), 1))"),
        "!E0030 std argument: expected json-representable value, got function"
    );
    // `OK(v)` 跨界**拆到内层**(D11-019)
    assert_eq!(run("STRINGIFY(OK(1))"), "1");
    // ⚠ §12.6:`ERR` 在调用边界就短路,成员体根本不执行 ⇒ 这里是 **E0102** 而不是
    //   E0030。这条很容易在重构 `wrap()` 时被改掉。
    assert_eq!(
        run(r#"STRINGIFY(ERR("e"))"#),
        "!E0102 unhandled ERR escaped to top level: e"
    );
}

#[test]
fn non_finite_floats_cannot_be_stringified() {
    let run2 = |body: &str| {
        out(&format!(
            "{JSON_IMP} IMPORT(\"wlwl:std.math\", [\"LN\"]); {body}"
        ))
    };
    let want = "!E0030 std argument: expected finite number, got NaN/Inf float";
    assert_eq!(run2("STRINGIFY(LN(0))"), want);
    assert_eq!(run2(&format!("STRINGIFY({NAN_SRC})")), want);
}

// ── §6 round-trip 恒等(独立一组,**显式标注它耦合两个成员**)──────────
//
// ⚠ 这是「两个成员串起来」的性质,所以 `STRINGIFY` 一旦单独变化这里也会红,而
//   **红的原因不一定是 `PARSE` 的错**。单独分组就是为了让「哪一侧坏了」一眼可辨。
//   期望值来自 Python `json` 的紧凑 canonical 形式,逐条核对通过。

#[test]
fn parse_stringify_round_trip_matches_python_canonical() {
    let rt = |json_text: &str| run(&format!("STRINGIFY(PARSE({}))", wl(json_text)));
    assert_eq!(
        rt(r##"{"a":[1,2.5,null,true]}"##),
        r#"{"a":[1,2.5,null,true]}"#
    );
    assert_eq!(rt(r##" { "k" : [ 1 , 2 ] } "##), r#"{"k":[1,2]}"#);
    assert_eq!(rt(r#""aAb""#), r#""aAb""#);
    assert_eq!(rt("9223372036854775807"), "9223372036854775807");
    // ⚠ 这两条**不是**恒等:它们把 §3 那条缺陷暴露在 round-trip 面上
    //   (合法的 u64 上界进去,出来是一个不同的浮点数)。
    assert_eq!(rt("-0"), "-0.0");
    assert_eq!(rt("18446744073709551615"), "1.8446744073709552e+19");
}
