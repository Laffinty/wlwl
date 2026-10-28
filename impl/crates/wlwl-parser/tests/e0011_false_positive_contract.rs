// [v0.12.1 / W-37] 解析器 `E0011` 误报(合法程序被误拒)的**契约用例**。
//!
//! ── 背景 ──────────────────────────────────────────────────────────────
//!
//! 计划文档记录的「最小复现」是下面这串代码**写成多行**的形态:
//
//! ```text
//! LET(f, FUN((conn), (
//!   FOO(conn, CAT(head, body));
//!   "200"
//! )));
//! ```
//!
//! 计划称它报 `E0011: expected ')', got Semicolon`,而同一份写成**单行**
//! 就正常 ⇒ 看上去像「多行嵌套块 ⇒ 解析器误拒合法程序」。
//!
//! ── 实测结论(证伪)───────────────────────────────────────────────────
//!
//! **该症状不是解析器缺陷,而是那份程序本身少了一个右括号。**
//! 少了 `LET` 的那个 `)` 之后,解析器在 `;` 处期待 `)` ⇒ 报
//! `E0011: expected ')', got Semicolon` —— 这是**正确诊断**
//! (缺的是 `)`,不是 `;`)。把那个 `)` 补回去 ⇒ **多行与单行都解析通过**。
//!
//! ⇒ 所以「多行」与「误报」之间没有因果关系;真正的原因是**括号不配对**。
//!
//! ── 本文件钉什么 ──────────────────────────────────────────────────────
//!
//! 1. **正面**:上面那串多行程序**必须**解析成功(以及一批同族合法形态)
//!    ⇒ 「合法程序不得被误拒」从此由门禁守着。
//! 2. **反向守卫**:同一串代码**少一个右括号** ⇒ **必须仍报 `E0011`**
//!    ⇒ 防止将来有人为「消灭 `E0011`」而把判据放宽(例如把
//!    `expect_specific(E0011, "')'")` 改成容错)。
//!
//! ⚠ 本文件**只钉行为、不改解析器**。
//!
//! ⚠ 与 `nested_stmt_semicolon_contract.rs` 是**同一族的两侧**:那边钉
//! 「其它 `E0011` 语义不许被误伤」(`other_e0011_shapes_stay_e0011`),
//! 这边钉「合法程序不许被 `E0011` 误伤」。两侧都绿 ⇒ `E0011` 的判据
//! 既没被放宽(误伤合法程序)、也没被收紧(放过坏程序)。

use wlwl_ast::{Expr, Literal};
use wlwl_error::ErrorCode;
use wlwl_parser::parse;

/// 计划文档记录的「最小复现」原样 —— **多行**形态。
///
/// 三个特征同时在场:① 多行排版 ② 块末是**字符串字面量**
/// ③ 块内有「实参是调用」的嵌套调用(`FOO(conn, CAT(head, body))`)。
const REPRO_MULTILINE: &str =
    "LET(f, FUN((conn), (\n  FOO(conn, CAT(head, body));\n  \"200\"\n)));\n";

/// 同一份程序写成**单行**。
const REPRO_SINGLELINE: &str = "LET(f, FUN((conn), (FOO(conn, CAT(head, body)); \"200\")));\n";

/// 解析失败时的诊断码。
///
/// 契约用例不允许「没报错」蒙混过关:能解析成功就 panic。
fn err_code(src: &str) -> ErrorCode {
    match parse(src, "t.wll") {
        Ok(_) => panic!("expected a parse error, but the source parsed: {src:?}"),
        Err(e) => e.diagnostic().code,
    }
}

/// 计划文档记录的**多行**形态必须解析通过。
///
/// 这条用例就是「`E0011` 误报」指控的正面证人:它绿 ⇒ 解析器没有误拒
/// 这类合法程序;它红 ⇒ 缺陷真实存在。
#[test]
fn reported_multiline_repro_must_parse() {
    let r = parse(REPRO_MULTILINE, "t.wll");
    assert!(
        r.is_ok(),
        "计划记录的多行形态必须解析通过,实际诊断: {:?}",
        r.err().map(|e| e.diagnostic().code)
    );
}

/// 同一份程序写成**单行**也必须通过 ⇒ 排版不影响可解析性。
///
/// 计划里「多行坏、单行好」的现象,若它真的存在,这两条不可能同时绿。
#[test]
fn reported_singleline_repro_must_parse() {
    let r = parse(REPRO_SINGLELINE, "t.wll");
    assert!(
        r.is_ok(),
        "同一份程序的单行形态必须解析通过,实际诊断: {:?}",
        r.err().map(|e| e.diagnostic().code)
    );
}

/// 同一份程序,行尾换成 **CRLF** 仍必须通过。
///
/// ⚠ 这条专门防「多行排版」这条线上的回归:行尾字符不参与语法。
#[test]
fn reported_repro_with_crlf_line_endings_must_parse() {
    let crlf = REPRO_MULTILINE.replace('\n', "\r\n");
    let r = parse(&crlf, "t.wll");
    assert!(
        r.is_ok(),
        "CRLF 行尾必须解析通过,实际诊断: {:?}",
        r.err().map(|e| e.diagnostic().code)
    );
}

/// 同族合法形态:**块的位置 / 块的终结符 / 嵌套层数 / 空白**全都必须通过。
///
/// 这批用例是「合法程序不得被误拒」这条规则的**覆盖面**:
///
/// - 位置:`LET` 值 / 调用实参 / `FUN` 体 / `IF` 分支 / `WHILE` 体 / `FOR` 体
/// - 终结符:字符串 / 整数 / 布尔 / 数组 / 字典 / 调用 / 标识符 / 嵌套块
/// - 嵌套层数:2 / 3 / 4 层
/// - 块内语句数:1 / 2 / 3 条
/// - 块末的 `;` 写与不写(§1.10:括号序列内最后一个表达式的 `;` 可省)
#[test]
fn legal_block_shapes_must_parse() {
    let cases: [(&str, &str); 24] = [
        // ── 位置 ──
        ("LET 值", "LET(x, (A(1); \"s\"));\n"),
        ("调用实参", "PRINT((A(1); \"s\"));\n"),
        ("FUN 体", "FUN((x), (A(x); \"s\"));\n"),
        (
            "具名 FUN 体",
            "FUN(handler(conn), (\n  FOO(conn);\n  \"200\"\n));\n",
        ),
        ("IF then 分支", "IF(TRUE, (A(1); \"s\"), 2);\n"),
        ("IF else 分支", "IF(TRUE, 1, (A(1); \"s\"));\n"),
        ("WHILE 体", "WHILE(FALSE, (A(1); \"s\"));\n"),
        ("FOR 体", "FOR(i, [], (A(i); \"s\"));\n"),
        ("块作实参", "FUN((f), (f((A(1); \"s\")); \"out\"));\n"),
        ("FUN 作实参", "PRINT(FUN((x), x));\n"),
        // ── 块的终结符 ──
        ("末为字符串", "LET(x, (A(1); \"s\"));\n"),
        ("末为整数", "LET(x, (A(1); 200));\n"),
        ("末为浮点", "LET(x, (A(1); 2.5));\n"),
        ("末为布尔", "LET(x, (A(1); TRUE));\n"),
        ("末为 NULL", "LET(x, (A(1); NULL));\n"),
        ("末为数组", "LET(x, (A(1); [1, 2]));\n"),
        ("末为字典", "LET(x, (A(1); [\"k\": 1]));\n"),
        ("末为调用", "LET(x, (A(1); B(2)));\n"),
        ("末为标识符", "LET(x, (A(1); head));\n"),
        ("末为二元运算", "LET(x, (A(1); +(1, 2)));\n"),
        // ── 嵌套层数 / 语句数 / `;` 省略 ──
        ("两层嵌套", "LET(x, (A(1); (B(2); \"s\")));\n"),
        ("三层嵌套", "LET(x, (A(1); (B(2); (C(3); \"s\"))));\n"),
        ("块内三条语句", "LET(x, (A(1); B(2); C(3)));\n"),
        ("块末带分号", "LET(x, (A(1); \"s\";));\n"),
    ];
    for (shape, src) in cases {
        assert!(
            parse(src, "t.wll").is_ok(),
            "合法形态「{shape}」不得被误拒,源码: {src:?}"
        );
    }
}

/// 正面钉住**语义**:多行形态解析出来的块,其值必须是末尾那个字符串字面量。
///
/// 只断言「能解析」太弱 —— 一份被误改成别的形状的程序也能解析通过。
/// 这里把 AST 形状一并钉住,确保「多行形态 = 单行形态」这件事在**语义**
/// 上也成立,而不只是不报错。
#[test]
fn reported_multiline_repro_keeps_its_block_value() {
    let ast = parse(REPRO_MULTILINE, "t.wll").expect("多行形态必须解析通过");

    // 顶层是 `LET(f, …)`。
    let fun = match &ast {
        Expr::Let { value, .. } => value.as_ref(),
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Let { value, .. } => value.as_ref(),
            other => panic!("expected Let, got {:?}", other),
        },
        other => panic!("expected Let, got {:?}", other),
    };
    // `LET` 的值是 `FUN`。
    let body = match fun {
        Expr::Fun { body, .. } => body.as_ref(),
        other => panic!("expected Fun, got {:?}", other),
    };
    // `FUN` 的体是块:一次调用 + 一个字符串字面量。
    let stmts = match body {
        Expr::Block { exprs, .. } => exprs,
        other => panic!("expected Block body, got {:?}", other),
    };
    assert_eq!(
        stmts.len(),
        2,
        "块里应是两条语句:一次调用 + 一个字符串字面量"
    );
    match &stmts[0] {
        Expr::Call { name, args, .. } => {
            assert_eq!(name, "FOO");
            assert_eq!(args.len(), 2, "FOO 的实参应是 conn 与嵌套调用");
            // 第二个实参是嵌套调用 `CAT(head, body)`。
            match &args[1] {
                Expr::Call { name, args, .. } => {
                    assert_eq!(name, "CAT");
                    assert_eq!(args.len(), 2);
                }
                other => panic!("expected nested Call CAT, got {:?}", other),
            }
        }
        other => panic!("expected Call FOO, got {:?}", other),
    }
    // 块的值 = 最后一个表达式 = 字符串字面量 `"200"`。
    match &stmts[1] {
        Expr::Literal(Literal::String(s), _) => assert_eq!(s, "200"),
        other => panic!("expected String literal \"200\", got {:?}", other),
    }
}

/// 反向守卫①:同一串代码**少一个右括号** ⇒ 必须仍报 `E0011`。
///
/// 这正是计划记录的那份症状(`E0011: expected ')', got Semicolon`)。
/// 它**必须**继续报 `E0011`:
///
/// - 不许变成「解析通过」⇒ 那是把坏程序放过;
/// - 不许变成 `E0013`(`expected ';'`)⇒ 那是把「缺 `)`」误判成
///   「缺 `;`」,等于改坏了另一个语法错误类别
///   (见 `nested_stmt_semicolon_contract.rs` 的
///   `other_e0011_shapes_stay_e0011`)。
#[test]
fn missing_one_closing_paren_still_reports_e0011() {
    // 计划记录的那份程序:少了 `LET` 的那个 `)`。
    let broken = "LET(f, FUN((conn), (\n  FOO(conn, CAT(head, body));\n  \"200\"\n));\n";
    assert_eq!(
        err_code(broken),
        ErrorCode::E0011,
        "少一个右括号必须仍报 E0011(缺的是 `)`,不是 `;`)"
    );

    // 诊断消息也一并钉住 —— 计划的原始记录就是这句话。
    let e = parse(broken, "t.wll").unwrap_err();
    let d = e.diagnostic();
    assert_eq!(d.code, ErrorCode::E0011);
    assert!(
        d.message.contains("expected ')'"),
        "消息应说明缺的是 `)`,实际: {}",
        d.message
    );
    assert_eq!(d.location.line, 4, "诊断应指向程序末尾那个 `;` 所在行");
}

/// 反向守卫②:少掉**任意一个**右括号 ⇒ 解析**必须失败**。
///
/// 比守卫①更强:它把「少一个 `)`」的所有落点(参数表 / 实参表 / 块 /
/// `FUN` / `LET`)全扫一遍,任何一个落点被「悄悄放过」都必红。
///
/// ⚠ 诊断码只在守卫① 里断言 `E0011`:少了**参数表**的 `)` 属于另一类
/// 错误(解析器在参数位看到 `(` ⇒ `E0010 expected parameter name`),
/// 那是**正确**的分类,本文件不碰它。
#[test]
fn missing_any_single_closing_paren_is_never_silently_accepted() {
    let bytes = REPRO_MULTILINE.as_bytes();
    let positions: Vec<usize> = bytes
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b')')
        .map(|(i, _)| i)
        .collect();
    assert!(positions.len() >= 5, "复现串里应有多个 `)` 可供删除");

    for p in positions {
        let mut broken = REPRO_MULTILINE.to_string();
        broken.remove(p);
        assert!(
            parse(&broken, "t.wll").is_err(),
            "少掉字节 {p} 处的 `)` 后,程序不合法 ⇒ 必须解析失败,实际: {broken:?}"
        );
    }
}
