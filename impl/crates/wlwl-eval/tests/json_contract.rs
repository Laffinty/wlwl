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
/// [W-02] 低层词法层三成员的 IMPORT。**不能**并进 `JSON_IMP` —— W-01 那
/// 十组断言的是旧两成员的行为,让它们顺带依赖新成员会把「冻结基线」和
/// 「新功能」绑在一起:新成员一坏,冻结基线也一起变红,分不清是谁的锅。
const LEX_IMP: &str =
    r#"IMPORT("wlwl:std.json", ["JSON_TOKENIZE", "JSON_LOCATE", "JSON_VALIDATE"]);"#;
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

/// 跑一段**只用新成员**的源(§7 / §8 那几组用)。
fn lex(body: &str) -> String {
    out(&format!("{JSON_IMP} {LEX_IMP} {body}"))
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

// ── §7 W-02:新词法层与 `PARSE` 的接受集**必须逐条一致** ────────────────
//
// ⚠ **这是 W-01 冻结契约存在的全部理由。** `JSON_VALIDATE` 与 `PARSE` 是**同一份
// JSON 语法的两套判定**(一套走 serde_json、一套走本仓手写词法器)。两套一旦分叉,
// 用户就会拿到「`PARSE` 说对、`JSON_VALIDATE` 说错」或反过来 —— 那比没有新成员
// 更糟,因为它让「先验后用」这件事失去意义。
//
// ⇒ 本组把 76 条输入(RFC 8259 合法面 + 拒收清单 + **serde 比 Python 更严的三处**)
//   的两条判定**并排**跑一遍,逐条比对。**任何一条分叉就红。**

/// `JSON_VALIDATE` 与 `PARSE` 对同一输入必须给出**同一判决**。
///
/// ⚠ 判定面只有两个:程序是否以诊断中止。**不比对渲染出的值** —— 第一版把
///   「输出是不是 `OK`」当成判据,于是 `PARSE` 返值、`JSON_VALIDATE` 返 `OK(NULL)`
///   一起被判成「未知」,47 条假分歧。渲染形态不是这一层要管的事。
fn agrees(json_text: &str, want_accept: bool) {
    let lit = wl(json_text);
    let p = run(&format!("PARSE({lit})"));
    let v = lex(&format!("JSON_VALIDATE({lit})"));
    let (pa, va) = (!p.starts_with('!'), !v.starts_with('!'));
    assert_eq!(
        pa, want_accept,
        "PARSE disagreed with the expectation on {json_text:?}: {p}"
    );
    assert_eq!(
        pa, va,
        "PARSE and JSON_VALIDATE diverged on {json_text:?}: PARSE={p} / VALIDATE={v}"
    );
}

#[test]
fn lexer_layer_agrees_with_parse_on_valid_json() {
    for src in [
        "{}",
        r##"{"a": 1}"##,
        r##"{"a": 1, "b": 2}"##,
        r##"  {  "a"  :  1  }  "##,
        "[]",
        "[1]",
        "[1,2,3]",
        r#""""#,
        r#""abc""#,
        r#""\"""#,
        r#""\\""#,
        r#""\/""#,
        r#""\b""#,
        r#""\f""#,
        r#""\n""#,
        r#""\r""#,
        r#""\t""#,
        r#""\u005C""#,
        r#""\u0041""#,
        "0",
        "-0",
        "1",
        "-1",
        "1.0",
        "1E+2",
        "1e-2",
        "1.5e+3",
        "-0.0",
        "true",
        "false",
        "null",
        r##"[{"a":[1,2]},null,true]"##,
        r##"{"a":{"b":{"c":[1,[2,[3]]]}}}"##,
        "1e2",
        "18446744073709551615",
        "100.0",
        "9007199254740993",
        "1e-400",
        // ⚠ 合法代理对、重复键、30 层嵌套、空键键名里的 NUL、tab 当空白
        r#""\uD834\uDD1E""#,
        r#""\uD800\uDC00""#,
        r#""\uDBFF\uDFFF""#,
        r##"{"a":1,"a":2}"##,
        "[\t1\t]",
    ] {
        agrees(src, true);
    }
}

#[test]
fn lexer_layer_agrees_with_parse_on_rejected_json() {
    for src in [
        "NaN",
        "Infinity",
        "-Infinity",
        r##"{"a":1/*c*/}"##,
        "{\"a\":1 // c\n}",
        "[1,]",
        r##"{"a":1,}"##,
        "'a'",
        "01",
        "-01",
        "00",
        "\"a\nb\"",
        "\"a\tb\"",
        "{a:1}",
        "{} x",
        "",
        "abc",
        "+1",
        "0x1F",
        ".5",
        "5.",
        r##"{"a" 1}"##,
        r##"{"a":1"##,
        r#""abc"#,
        // ⚠ 下面五条是 **serde 比 Python 严**的三处边界(ADR-0033 记在案)。两套都
        //   必须拒,否则词法层比 `PARSE` 宽松 ⇒ 分叉。详见 `json/lexer.rs` 的表。
        r#""\uD800""#,       // 孤立高代理
        r#""\uDC00""#,       // 孤立低代理
        r#""\uDD1E\uD834""#, // 代理对换序
        "1e400",             // f64 溢出
    ] {
        agrees(src, false);
    }
    // 嵌套 200 层:serde 的递归上限是 128。单独一条是因为它是个长字符串,
    // 放进上面的表会撑爆可读性。
    let deep: String = "[".repeat(200) + &"]".repeat(200);
    agrees(&deep, false);
    // ⚠ 30 层在上限内 —— 证明那条闸是 128 而不是「一超过 30 层就拒」。
    let ok_deep: String = "[".repeat(30) + &"]".repeat(30);
    agrees(&ok_deep, true);
}

// ── §8 W-02:`JSON_TOKENIZE` / `JSON_LOCATE` 的可观察面 ────────────────
//
// 完整契约在 W-03(「定位契约,逐字节冻结」)。这里只钉**结构与单位**,
// 让「偏移是码点、行列 1 基」这件事有守卫 —— 单位错了以后整批冻结值都要重写。

#[test]
fn json_tokenize_emits_codepoint_offsets_and_is_purely_lexical() {
    // `[1, "a"]` ⇒ 5 个 token;**纯词法无结构**,所以下面这条对 JSON 无效文本
    // 仍然出 token 流。
    let toks = lex(r##"JSON_TOKENIZE("[1, \"a\"]")"##);
    assert!(
        toks.contains("lbracket") && toks.contains("number"),
        "{toks}"
    );
    assert!(
        toks.contains("string") && toks.contains("rbracket"),
        "{toks}"
    );
    // ⚠ **纯词法**:`[}` 没有词法错误(括号与花括号都是合法 token)⇒ 出 token 流。
    //   结构由 `JSON_VALIDATE` 管。这条钉住 §2.1「纯词法,无结构」。
    let structural = lex(r##"JSON_TOKENIZE("[}")"##);
    assert!(structural.contains("lbracket"), "{structural}");
    assert!(!structural.starts_with('!'), "{structural}");
    assert!(lex(r##"JSON_VALIDATE("[}")"##).starts_with('!'));
    // ⚠ **码点偏移**:输入 `["é"]` 的码点是 `[`0 `"`1 `é`2 `"`3 `]`4,而 `é` 在
    //   UTF-8 里占 **2 字节**。字符串 token 是 `[1, 4)`(`"`1 `é`2 `"`3)。
    //   ⇒ **按码点 `end = 4`;按字节会是 `end = 5`**。断 `end` 而不是 `start`,
    //   因为 `start` 在两种单位下都是 1 —— 断它等于什么都没断。
    let off = lex(r##"JSON_TOKENIZE("[\"é\"]")"##);
    assert!(
        off.contains("start: 1, end: 4"),
        "codepoint offsets expected: {off}"
    );
}

#[test]
fn json_locate_returns_one_based_line_and_codepoint_column() {
    // 输入 `{\n  1`:码点 0=`{` 1=`\n` 2=` ` 3=` ` 4=`1`。offset 3 是**第二个
    // 空格**;第 2 行从码点 2 起算,所以它是第 2 列。⚠ 我第一版写的是「column: 3,
    // token_start: 3」—— 两个都错:列该从行首算,`token_start` 也不是 3。
    //
    // `token_start: 0` 也是对的且值得说清:`JSON_TOKENIZE` 是**纯词法**的,这段输入
    // 结构上无效(缺 `}`)但**词法上合法**(就是一个 `lbrace`),所以词法层给出了
    // token 0。若这里换成 `JSON_LOCATE` 依赖结构校验,它会回 `NULL` —— 两条路都要
    // 有人想清楚,否则下一个人会以为它坏了。
    assert_eq!(
        lex("JSON_LOCATE(\"{\\n  1\", 3)"),
        "[line: 2, column: 2, token_start: 0]"
    );
    // ⚠ `column` 按**码点**数:同一个 `é` 占 1 列不是 2。offset 2 指向 `é`,它**在**
    //   字符串 token `[1, 4)` 里 ⇒ `token_start` 是 **1**。
    //
    //   ⚠ 这条同时钉住 `token_start_at` 的方向:第一版写的是
    //   `toks.iter().find(|t| t.start <= offset)` —— 那是「第一个开始早于它的
    //   token」,于是这里会回 **0**(`[` 的起点)。正解是**最后一个**
    //   `start <= offset` 的那个,即包含该偏移的 token。
    assert_eq!(
        lex("JSON_LOCATE(\"[\\\"é\\\"]\", 2)"),
        "[line: 1, column: 3, token_start: 1]"
    );
    // 空输入没有 token ⇒ `token_start` 是 `NULL`,不是 0(0 会被读成「第 0 个
    // token 开始」,那不存在)。
    assert_eq!(
        lex("JSON_LOCATE(\"\", 0)"),
        "[line: 1, column: 1, token_start: NULL]"
    );
    // ⚠ **越界不报错**:给一个仍然有意义的「附近」位置。理由写在实现注释里 ——
    //   报错让你知道 offset 写错了,但用户要的是「这附近是什么」。
    assert_eq!(
        lex("JSON_LOCATE(\"[1]\", 99)"),
        "[line: 1, column: 4, token_start: 2]"
    );
    // 非负整数是硬要求;负数 / 非整数 ⇒ E0030。
    assert!(lex("JSON_LOCATE(\"[1]\", -1)").starts_with("!E0030"));
    assert!(lex("JSON_LOCATE(\"[1]\", 1.5)").starts_with("!E0030"));
}

/// ⚠ `JSON_VALIDATE` 的 ERR 载荷里**必须带 `offset`** —— 不是因为规范要求,
/// 而是因为**这套 API 自己转不过弯**:`JSON_VALIDATE` 报的是 `line`/`column`,
/// 而 `JSON_LOCATE` 的实参是 `offset`。没有 `offset`,用户就得自己把行列反算回
/// 码点下标,而本模块不提供那个反算。⇒ `line`/`column` 给人看,`offset` 给机器
/// 接下一棒,三个一起给。
#[test]
fn json_validate_error_payload_carries_offset_line_and_column() {
    // ⚠⚠ **不能**写 `PRINT(JSON_VALIDATE(bad))`:`PRINT(<ERR>)` 是**静默**的
    //   (实测 `PRINT("x = ", ERR("y"))` 一个字符都不输出),ERR 直接以 **E0102**
    //   逃到顶层 —— 载荷一个字都看不到。必须先用 `ERR_PAYLOAD` 取出来
    //   (它是 §12.7 的注册消费者)。第一版就是写成 `PRINT(...)` 然后拿到 E0102
    //   一脸茫然。
    //
    // 顺带:`{\n  1 2}` 的出错处是码点 4 的 `1`(它该是字符串键),第 2 行从码点 2
    // 起算 ⇒ 第 3 列。我第一版写的是 column 5,是自己算错了。
    // ⚠ **不能**以 `PRINT(...)` 收尾:`out()` 返回的是**最后一个表达式的值**,而
    //   `PRINT(x)` 求值为 `NULL` —— 载荷打进了 stdout,`got` 却是 `NULL`,
    //   断言在 `assert!(got.contains("JsonError"))` 上炸,而屏幕上明明印着载荷。
    //   ⇒ 尾表达式直接用 `ERR_PAYLOAD(e)`(同 `collection_regression.rs` 的写法)。
    let got = lex("LET(e, JSON_VALIDATE(\"{\\n  1 2}\")); ERR_PAYLOAD(e)");
    assert!(got.contains("JsonError"), "{got}");
    assert!(got.contains("op: JSON_VALIDATE"), "{got}");
    assert!(got.contains("offset: 4"), "{got}");
    assert!(got.contains("line: 2"), "{got}");
    assert!(got.contains("column: 3"), "{got}");
    // ⚠ `path` **不出现**:本批这三个成员只做语法,不看结构,没有路径可言。
    //   塞一个恒为 NULL 的 `path` 会让人以为「路径是空的」而不是「此处无路径」。
    assert!(!got.contains("path"), "{got}");
}

/// 元数 / 类型错是**诊断**不是 `ERR` 值 —— 与 §2.3 的「失败的统一口径」一致。
#[test]
fn lexer_layer_arity_and_type_misuse_are_diagnostics() {
    assert!(lex("JSON_TOKENIZE()").starts_with("!E0022"));
    assert!(lex("JSON_TOKENIZE(1)").starts_with("!E0030"));
    assert!(lex("JSON_VALIDATE(1)").starts_with("!E0030"));
    assert!(lex("JSON_LOCATE(\"[1]\")").starts_with("!E0022"));
}
