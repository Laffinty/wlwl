// [v0.12.0 / F-08 + F-09] 嵌套块里的 `;` 契约 + `ModuleDecl` 的位置契约。
//!
//! 这两条缺陷的共同形状是**同一个错误在两个位置给出不同的诊断码**
//! (F-08:漏写 `;` ⇒ 顶层 `E0013`、嵌套块 `E0011`)与**同一个构造在两个位置
//! 行为不一致**(F-09:`SEALED` 报错、`IMPORT` 照做、`EXPORT` 静默无效)。
//! 契约用例把它们钉住:改坏任何一侧,本文件必红。
//!
//! ── F-08:嵌套块里省略必需的 `;` ⇒ `E0013` ──────────────────────────
//
// 规范 §1.10:「处于『语句位置』的表达式**必须**以 `;` 结束。在括号序列(§4.4)
// 内部,最后一个表达式的 `;` **可以**省略;该表达式的值即括号序列的值。
// **省略必需的 `;` 产生 `E0013`**。」
//
// ⚠ 这条契约**只**覆盖「表达式后面缺 `;`」这一种情形。其余 `E0011`
// (缺 `)`、多逗号、括号不配对……)必须**原样不动** —— 所以本文件同时
// 锁住两边的期望值(见 `other_e0011_shapes_stay_e0011`)。
//
// ── F-09:`EXPORT` 只出现在模块顶层 ──────────────────────────────────
//
// 规范附录 A.2:「`ModuleDecl`(§9.1 / §9.2)只出现在模块顶层,不作为
// `Expression`。」修之前嵌套的 `EXPORT` **零诊断、退出码 0、什么都不导出**,
// 消费方 `IMPORT` 时才炸 `E0023`(错误信息还指向消费方)。

use wlwl_ast::Expr;
use wlwl_error::ErrorCode;
use wlwl_parser::parse;

/// 解析失败时的诊断码。
///
/// 契约用例不允许「没报错」蒙混过关:能解析成功就 panic。
fn err_code(src: &str) -> ErrorCode {
    match parse(src, "t.wll") {
        Ok(_) => panic!("expected a parse error, but the source parsed: {src:?}"),
        Err(e) => e.diagnostic().code,
    }
}

// ── F-08:嵌套块里省略必需的 `;` ⇒ E0013 ──────────────────────────────

/// 五种嵌套形状(Block / FUN / IF / WHILE / FOR)都必须是 `E0013`。
///
/// 这就是审计批次 R-5 要求的**反向守卫**:把嵌套块的闸门去掉 ⇒ 全部退回
/// `E0011` ⇒ 本用例必红。
#[test]
fn nested_missing_semicolon_is_e0013_in_every_shape() {
    let cases: [(&str, &str); 5] = [
        ("括号序列", "PRINT((PRINT(1) PRINT(2)));"),
        ("FUN 体", "FUN((x), (PRINT(1) PRINT(2)));"),
        ("IF 分支", "IF(TRUE, (PRINT(1) PRINT(2)));"),
        ("WHILE 体", "WHILE(FALSE, (PRINT(1) PRINT(2)));"),
        ("FOR 体", "FOR(x, [], (PRINT(1) PRINT(2)));"),
    ];
    for (shape, src) in cases {
        assert_eq!(
            err_code(src),
            ErrorCode::E0013,
            "{shape} 形状应报 E0013(规范 §1.10),源码: {src}"
        );
    }
}

/// 模块顶层**不许**变:漏写 `;` 一直是 `E0013`,本次只把嵌套块对齐上来。
#[test]
fn top_level_missing_semicolon_stays_e0013() {
    assert_eq!(err_code("PRINT(1)\nPRINT(2);\n"), ErrorCode::E0013);
}

/// 其余 `E0011` 语义**不许**被误伤。
///
/// 这是本次改动最大的风险面:如果判据写成「调用方期待 `)` 失败 ⇒ `E0013`」,
/// 下面每一条都会被改成 `E0013`,等于把另一个语法错误类别搞坏。
#[test]
fn other_e0011_shapes_stay_e0011() {
    // 缺 `)` 且文件到此为止 —— 缺的是 `)`,不是 `;`。
    assert_eq!(err_code("PRINT(1"), ErrorCode::E0011);
    // 调用实参表里缺 `)`(实参表**不是**块,没有分号规则)。
    assert_eq!(err_code("PRINT(1 2);"), ErrorCode::E0011);
    // 括号序列里的多逗号。
    assert_eq!(err_code("PRINT((1, 2));"), ErrorCode::E0011);
    // `FUN` 体内的多逗号(块以 `)` 收尾,`,` 不是块的终结符)。
    assert_eq!(
        err_code("FUN((x), (PRINT(1), PRINT(2)));"),
        ErrorCode::E0011
    );
    // `IF` 的 then 分支以 `,` 收尾 —— `,` 是**合法**的块终结符,不许报错。
    assert!(parse("IF(TRUE, 1, 2);", "t.wll").is_ok());
}

/// 诊断位置必须落在**看到缺 `;` 的那个 token**上,不是块的开头。
///
/// 规范 §11.1 要求诊断带位置;落在块开头会把作者引向错误的行。
#[test]
fn nested_e0013_points_at_the_token_after_the_expression() {
    let e = parse("PRINT((PRINT(1) PRINT(2)));", "t.wll").unwrap_err();
    let d = e.diagnostic();
    assert_eq!(d.code, ErrorCode::E0013);
    assert_eq!(d.location.line, 1);
    // 第 17 列 = 第二个 `PRINT` 的 `P`(`PRINT((PRINT(1) ` 之后)。
    assert_eq!(d.location.col, 17, "诊断应指向第二个 PRINT");
    assert!(
        d.message.contains("expected ';'"),
        "消息应说明缺的是 `;`,实际: {}",
        d.message
    );
}

// ── F-09:`ModuleDecl` 只出现在模块顶层 ───────────────────────────────

/// 嵌套的 `EXPORT` 必须**报错**(修之前:零诊断、退出码 0、什么都不导出)。
#[test]
fn nested_export_is_a_diagnostic_not_a_silent_no_op() {
    let e = parse("LET(b, 111);\nLET(x, (EXPORT([\"b\"])));\n", "t.wll").unwrap_err();
    let d = e.diagnostic();
    // `E0010` = 「期望表达式」:附录 A.2 明写 `ModuleDecl` **不作为**
    // `Expression` ⇒ 嵌套位置上期望的是表达式、拿到的却是一条模块声明。
    assert_eq!(d.code, ErrorCode::E0010);
    assert_eq!(d.location.line, 2, "诊断应指向嵌套 EXPORT 那一行");
    assert!(
        d.message.contains("module top level"),
        "消息应说明它只能写在模块顶层,实际: {}",
        d.message
    );
}

/// 顶层 `EXPORT` 的行为**不许**变(仍然正常导出)。
#[test]
fn top_level_export_still_parses_as_a_declaration() {
    let ast = parse("LET(b, 111);\nEXPORT([\"b\"]);\n", "t.wll").expect("顶层 EXPORT 必须照旧通过");
    let found = match &ast {
        Expr::Block { exprs, .. } => exprs.iter().any(|e| matches!(e, Expr::Export { .. })),
        other => matches!(other, Expr::Export { .. }),
    };
    assert!(found, "顶层 EXPORT 仍应产出一条 `Expr::Export` 声明");
}
