//! [v0.12.0 / R-3] **诊断位置**契约 —— 核心内建的实参校验错误必须可定位。
//!
//! ## 背景(审计 F-04 / F-05,2026-10-26)
//!
//! 核心内建(`LEN` / `+` / `INDEX_GET` / `INDEX_SET` / `AT_K` / `NEW` /
//! `UPPER` / `LOWER` / `STR` / `TYPE` / `BOOL` / `CONCAT` / `CONTAINS` …)
//! 的**实参校验**诊断,此前一律落在 `<runtime>:0:0` ——
//! 审计抽样 **17/17 命中**。
//!
//! 成因:这些诊断由 `arity_error` / `type_error` / `builtin_error` 构造,
//! 而这三个构造器的签名只有 `(name, msg)`,**拿不到 AST span** ⇒ 一律用
//! `Location::point("<runtime>", 0, 0)` 占位。
//!
//! 语言核心册 §11.1 要求诊断带「位置(文件、行列)与源行摘录」⇒ 占位位置
//! 不合规;而这一族恰是**新手最高频**的一类错误(类型错 / 元数错 / 越界)
//! ⇒ 报错等于没报。
//!
//! ## 修法
//!
//! 分派点(`dispatch_call`)**有**调用点 span ⇒ 在那里把**占位位置**换成
//! 调用点(并补上源行摘录)。已经有真实位置的诊断**原样保留**。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`,同时断言**诊断码**、**行号**、**文件**、
//! **源行摘录**。⚠ 调用点刻意放在**第 2 行**(第 1 行放一条无关语句)——
//! 只断言「行号 > 0」会让「指到模块首行」这种退化也判绿。

use wlwl_eval::Evaluator;

#[derive(Debug, Clone)]
struct Diag {
    code: String,
    message: String,
    file: String,
    line: u32,
    col: u32,
    source_line: Option<String>,
}

/// 跑一段源码,返回它产出的**全部**诊断。
fn run(src: &str) -> Vec<Diag> {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    // `with_source` 让求值器持有源文本 ⇒ 诊断能补上源行摘录(§11.1)。
    let mut ev = Evaluator::new().with_source(src, "t.wll");
    match ev.eval(&ast) {
        Ok(_) => Vec::new(),
        Err(e) => e
            .diagnostics()
            .iter()
            .map(|d| Diag {
                code: d.code.as_str().to_string(),
                message: d.message.clone(),
                file: d.location.file.clone(),
                line: d.location.line,
                col: d.location.col,
                source_line: d.source_line.clone(),
            })
            .collect(),
    }
}

/// 一个用例 = 一条源码 + 期望的诊断码。调用点固定在**第 2 行**。
macro_rules! case {
    ($name:ident, $src:expr, $expect_code:expr) => {
        #[test]
        fn $name() {
            let ds = run($src);
            assert_eq!(
                ds.len(),
                1,
                "case `{}` 应恰好一条诊断, got {:?}",
                stringify!($name),
                ds
            );
            let d = &ds[0];
            assert_eq!(
                d.code,
                $expect_code,
                "case `{}` 的诊断码不对: {}",
                stringify!($name),
                d.message
            );
            // ── 位置必须落在**调用点**(第 2 行),不是模块首行、也不是占位 ──
            assert_eq!(
                d.file,
                "t.wll",
                "case `{}` 的诊断文件不对: {:?}",
                stringify!($name),
                d.file
            );
            assert_eq!(
                d.line,
                2,
                "case `{}` 的诊断行号必须是调用点(第 2 行), got {}",
                stringify!($name),
                d.line
            );
            assert!(
                d.col > 0,
                "case `{}` 的诊断列号必须是真实列, got {}",
                stringify!($name),
                d.col
            );
            // ── §11.1:「源行摘录」也不能缺 ──
            let src_line = d
                .source_line
                .as_deref()
                .unwrap_or_else(|| panic!("case `{}` 的诊断缺源行摘录(§11.1)", stringify!($name)));
            assert!(
                src_line.contains($expect_code) || !src_line.trim().is_empty(),
                "case `{}` 的源行摘录是空的: {:?}",
                stringify!($name),
                src_line
            );
        }
    };
}

// ── 类型类实参校验(E0030)──────────────────────────────────────────────────
case!(len_rejects_an_integer, "LET(pad, 0);\nLEN(1);", "E0030");
case!(
    add_rejects_a_string_operand,
    "LET(pad, 0);\n+(1, \"a\");",
    "E0030"
);
case!(upper_rejects_an_integer, "LET(pad, 0);\nUPPER(1);", "E0030");
case!(lower_rejects_an_integer, "LET(pad, 0);\nLOWER(1);", "E0030");
case!(
    at_k_rejects_a_non_container,
    "LET(pad, 0);\nAT_K([1], \"k\", 0);",
    "E0030"
);
case!(
    at_k_rejects_a_scalar_container,
    "LET(pad, 0);\nAT_K(1, \"k\", 0);",
    "E0030"
);
case!(
    index_set_rejects_a_scalar_target,
    "LET(pad, 0);\nINDEX_SET(\"a\", 0, 1);",
    "E0030"
);
case!(
    new_rejects_two_arguments,
    "LET(pad, 0);\nNEW(1, 2);",
    "E0030"
);
case!(
    concat_rejects_non_string_operands,
    "LET(pad, 0);\nCONCAT(1, 2);",
    "E0030"
);

// ── 元数类实参校验(E0022)──────────────────────────────────────────────────
case!(
    add_rejects_three_arguments,
    "LET(pad, 0);\n+(1, 2, 3);",
    "E0022"
);
case!(len_rejects_zero_arguments, "LET(pad, 0);\nLEN();", "E0022");
case!(sub_rejects_one_argument, "LET(pad, 0);\n-(1);", "E0022");
case!(
    mul_rejects_three_arguments,
    "LET(pad, 0);\n*(1, 2, 3);",
    "E0022"
);
case!(
    str_rejects_two_arguments,
    "LET(pad, 0);\nSTR(1, 2);",
    "E0022"
);
case!(
    type_rejects_two_arguments,
    "LET(pad, 0);\nTYPE(1, 2);",
    "E0022"
);
case!(
    bool_rejects_two_arguments,
    "LET(pad, 0);\nBOOL(1, 2);",
    "E0022"
);
case!(
    contains_rejects_three_arguments,
    "LET(pad, 0);\nCONTAINS([1], 1, 2);",
    "E0022"
);

// ── 越界类(E0036 / E1003)──────────────────────────────────────────────────
case!(
    index_get_rejects_an_out_of_range_index,
    "LET(pad, 0);\nINDEX_GET([1], 5);",
    "E0036"
);
case!(
    index_get_rejects_an_out_of_range_offset,
    "LET(pad, 0);\nINDEX_GET(\"a\", 9);",
    "E0036"
);
case!(
    div_rejects_a_zero_divisor,
    "LET(pad, 0);\n/(1, 0);",
    "E1003"
);
case!(
    mod_rejects_a_zero_divisor,
    "LET(pad, 0);\n%(1, 0);",
    "E1003"
);

// ── 反向守卫:已经有真实位置的诊断**不许**被改写 ──────────────────────────
//
// `CALL(1, 2)` 是**求值器分派期**的检查(不是内建体内部)⇒ 它本来就有位置;
// 修法只换**占位位置** ⇒ 这一条必须原样保留。
#[test]
fn an_already_located_diagnostic_keeps_its_position() {
    let ds = run("LET(pad, 0);\nCALL(1, 2);");
    assert_eq!(ds.len(), 1, "应恰好一条诊断: {ds:?}");
    assert_eq!(ds[0].code, "E0020");
    assert_eq!(
        ds[0].line, 2,
        "分派期诊断的位置必须原样保留, got {:?}",
        ds[0]
    );
    assert!(ds[0].source_line.is_some(), "分派期诊断本就带源行摘录");
}

// ── 反向守卫:占位位置**不是**万能钥匙 ────────────────────────────────────
//
// `wlwl:std.fs` 的成员实参校验**本来就**带位置(审计实测 `p05b2_bytes_err.wll:5:7`)
// ⇒ 内建侧的改法不得波及标准库成员层。
#[test]
fn a_stdlib_member_diagnostic_keeps_its_position() {
    let src = "LET(pad, 0);\nIMPORT(\"wlwl:std.encode\", [\"UTF8_DECODE\"]);\nUTF8_DECODE([300]);";
    let ast = wlwl_parser::parse(src, "t.wll").expect("parse");
    let mut ev = Evaluator::new().with_source(src, "t.wll");
    let err = ev.eval(&ast).expect_err("越界字节必须报错");
    let d = err.diagnostic();
    assert_eq!(d.code.as_str(), "E0030");
    assert_eq!(
        d.location.line, 3,
        "标准库成员的诊断位置必须原样保留, got {:?}",
        d.location
    );
}

// ── R-3·F-03:宿主挂起点成员在**模块顶层**被调用时的诊断 ──────────────────
//
// 审计 F-03 / F-05:旧文案把这一族说成「`YIELD` used outside a step context」
// (用户写的是 `TCP_ACCEPT`),位置还落到**模块第一条语句**。
// ⇒ 现在:诊断**指名成员**、位置指向**调用点**。
#[test]
fn a_top_level_host_suspend_names_the_member_and_points_at_the_call() {
    let src = concat!(
        "IMPORT(\"wlwl:std.net\", [\"TCP_LISTEN\", \"TCP_ACCEPT\"]);\n",
        "LET(listener, TCP_LISTEN(\"127.0.0.1\", 0));\n",
        "LET(nope, TCP_ACCEPT(listener, 50));"
    );
    let ds = run(src);
    assert_eq!(ds.len(), 1, "应恰好一条诊断: {ds:?}");
    let d = &ds[0];
    assert_eq!(d.code, "E0014");
    assert!(
        d.message.contains("TCP_ACCEPT"),
        "诊断必须指名成员 TCP_ACCEPT, got: {}",
        d.message
    );
    assert!(
        !d.message.contains("YIELD"),
        "诊断不得把宿主挂起点成员说成 YIELD, got: {}",
        d.message
    );
    assert!(
        !d.message.contains("Scheduler"),
        "诊断不得泄露 Rust 内部实现细节, got: {}",
        d.message
    );
    assert_eq!(d.line, 3, "诊断必须指向调用点(第 3 行), got {}", d.line);
}

#[test]
fn a_top_level_sleep_async_names_the_member() {
    let src = concat!(
        "IMPORT(\"wlwl:std.time\", [\"SLEEP_ASYNC\"]);\n",
        "SLEEP_ASYNC(50);"
    );
    let ds = run(src);
    assert_eq!(ds.len(), 1, "应恰好一条诊断: {ds:?}");
    let d = &ds[0];
    assert_eq!(d.code, "E0014");
    assert!(
        d.message.contains("SLEEP_ASYNC"),
        "诊断必须指名成员 SLEEP_ASYNC, got: {}",
        d.message
    );
    assert_eq!(d.line, 2, "诊断必须指向调用点(第 2 行), got {}", d.line);
}
// ── 反向守卫:显式 `YIELD()` 的既有行为**不许**变 ──────────────────────────
//
// `YIELD()` 在顶层仍然是 `E0014` + 既有文案 + **YIELD 调用点**的位置
// (D12-002 / v0.11.2 W-04 的既有契约)。
#[test]
fn an_explicit_yield_keeps_its_message_and_call_site() {
    let src = "PRINT(\"before\");\nYIELD();";
    let ds = run(src);
    assert_eq!(ds.len(), 1, "应恰好一条诊断: {ds:?}");
    let d = &ds[0];
    assert_eq!(d.code, "E0014");
    assert!(
        d.message.contains("YIELD used outside a step context"),
        "显式 YIELD 的文案不许变, got: {}",
        d.message
    );
    assert_eq!(
        d.line, 2,
        "显式 YIELD 的位置必须仍是 YIELD 调用点, got {}",
        d.line
    );
}
