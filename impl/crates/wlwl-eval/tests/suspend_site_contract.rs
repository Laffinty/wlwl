//! [W-10 / 并发册 §17.0] **宿主挂起点成员的调用点必须在任务体内** —— 契约表。
//!
//! ## 它钉的是什么
//!
//! 并发册 §17.0(规范原文,`docs/wlwl-spec-reference.md` §17.0)写死:
//!
//! > **宿主挂起点成员的调用点必须在任务体内。** 宿主挂起点成员(`std.net` 的
//! > `TCP_*` 族与 `std.time::SLEEP_ASYNC`)在**任务体外**(模块顶层,或任何不在
//! > 任务体里的位置 —— 包括任务体里定义的嵌套闭包)被调用时,实现**必须**产生
//! > 诊断 `E0014`(语言核心册 §11.2)。该诊断**必须**满足两条:① **指名该成员**
//! > (消息里出现成员名);② 位置**指向调用点**(调用点所在行与列),**不得**落到
//! > 模块的第一条语句或任何与调用点无关的位置。⚠ **不得**把它报成「`YIELD`
//! > 出现在任务体外」。
//!
//! 需求清单 §B-1 的实测症状:把 `TCP_RECV` 写进**辅助函数**、再由任务体调用 ⇒
//! 任务**无声死掉**、`AWAIT` 拿到 `NULL`、**stderr 全空** ⇒ 「绝不允许出现
//! 「任务静默结束 + 无诊断」」。
//!
//! ## 判据(实现侧)
//!
//! 「在任务体里」= **调用发生时,当前闭包帧就是任务体帧本身**。
//!
//! | 位置 | 判定 |
//! |---|---|
//! | 模块顶层(无任务) | `E0014`(既有行为,**不变**) |
//! | 任务体**自己的顶层语句** | **合法**(既有行为,**不变**) |
//! | 任务体里**定义的嵌套闭包** | `E0014`(本项新增) |
//! | 任务体调用的**模块顶层辅助函数** | `E0014`(本项新增) |
//!
//! ⚠ **原生帧是边界**:标准库成员回调 WLWL 闭包(`StdHost::call`)时**不**加深
//!   ⇒ `TCP_SERVE` 的 handler 仍归 `std.net` 自己那条诊断(不与本项抢)。
//!
//! ## 位置断言为什么钉到「列」
//!
//! 只断言「行号 > 0」会让「指到模块首行」这种退化也判绿 ⇒ 这里把调用点放在
//! **独立的一行**、缩进固定 ⇒ 行号与列号都能钉死。

use wlwl_eval::Evaluator;

/// 一条诊断的快照(只留本表要断言的字段)。
#[derive(Debug, Clone)]
struct Diag {
    code: String,
    message: String,
    line: u32,
    col: u32,
}

/// 跑一段源码 ⇒ 返回它产出的**全部**诊断(成功 ⇒ 空 vec)。
fn diags(src: &str) -> Vec<Diag> {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_source(src, "t.wll");
    match ev.eval(&ast) {
        Ok(_) => Vec::new(),
        Err(e) => e
            .diagnostics()
            .iter()
            .map(|d| Diag {
                code: d.code.as_str().to_string(),
                message: d.message.clone(),
                line: d.location.line,
                col: d.location.col,
            })
            .collect(),
    }
}

/// 跑一段源码 ⇒ 返回它的结果(失败 ⇒ `!CODE message`)。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

/// 断言「恰好一条 `E0014`,指名 `member`,位置指向 (line, col)」。
fn assert_e0014_at(src: &str, member: &str, line: u32, col: u32) {
    let ds = diags(src);
    assert_eq!(ds.len(), 1, "应恰好一条诊断, got {ds:#?}");
    let d = &ds[0];
    assert_eq!(d.code, "E0014", "诊断码必须是 E0014, got {d:#?}");
    assert!(
        d.message.contains(member),
        "诊断必须指名成员 {member}, got: {}",
        d.message
    );
    assert!(
        !d.message.contains("YIELD"),
        "宿主挂起点成员不得被报成「YIELD 出现在任务体外」, got: {}",
        d.message
    );
    assert_eq!(d.line, line, "诊断必须指向调用点所在行({line}), got {d:#?}");
    assert_eq!(d.col, col, "诊断必须指向调用点所在列({col}), got {d:#?}");
}

// ── 合法形态:任务体自己的顶层语句 ─────────────────────────────────────────

/// **任务体顶层**调用挂起点成员 ⇒ **正常**(既有行为不许变)。
///
/// ⚠ 这是判据的**正向**锚点:判据若把任务体顶层也当成「任务体之外」⇒ 这条会红。
#[test]
fn a_suspend_point_in_the_task_body_top_level_is_fine() {
    let out = actual(
        r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , ( SLEEP_ASYNC(20); 42))))))"#,
    );
    assert_eq!(
        out, "42",
        "任务体顶层的挂起点成员必须照常挂起并继续往下走, got {out:?}"
    );
}

/// **子任务**的任务体顶层也合法 ⇒ 父任务的任务体帧不许漏进子任务。
///
/// ⚠ 判据的**嵌套方向**守卫:`AWAIT` 内联驱动调度循环 ⇒ 子任务在**父任务的
///   任务体帧之内**执行 ⇒ 若深度不还原,子任务的顶层挂起点会被误判成「嵌套」。
#[test]
fn a_child_task_may_hold_its_own_suspend_point() {
    let out = actual(
        r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (
  AWAIT(SPAWN(FUN(() , ( SLEEP_ASYNC(20); 7 ))))
))))))"#,
    );
    assert_eq!(out, "7", "子任务的任务体顶层挂起点必须合法, got {out:?}");
}

// ── 违规形态:任务体之外 ───────────────────────────────────────────────────

/// **任务体里定义的嵌套闭包**里调用挂起点成员 ⇒ `E0014` + 指名成员 + 指向调用点。
///
/// ⚠ 调用点刻意放在**独立的一行**、缩进 2 ⇒ 行号与列号都能钉死。
#[test]
fn a_suspend_point_in_a_closure_defined_inside_the_task_body_is_e0014() {
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (
  LET(inner, FUN(() , (
    SLEEP_ASYNC(20)
  )));
  inner()
))))))"#;
    assert_e0014_at(src, "SLEEP_ASYNC", 4, 5);
}

/// **模块顶层的辅助函数**里调用挂起点成员、由任务体调用 ⇒ `E0014`(需求清单 §B-1)。
///
/// ⚠ 这就是 §B-1 的验收判据那一条:
///   `LET(helper, FUN((c), (TCP_RECV(c, 4096, 1000))));` ⇒ 明确报 `E0014` 指向此行。
#[test]
fn a_suspend_point_in_a_module_level_helper_is_e0014() {
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
LET(helper, FUN(() , (
  SLEEP_ASYNC(20)
)));
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , helper())))))"#;
    assert_e0014_at(src, "SLEEP_ASYNC", 3, 3);
}

/// **两层嵌套** ⇒ 同样是 `E0014`(深度 > 0 一律拒)。
#[test]
fn a_suspend_point_two_closures_deep_is_e0014() {
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
LET(outer, FUN(() , (
  LET(inner, FUN(() , (
    SLEEP_ASYNC(20)
  )));
  inner()
)));
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , outer())))))"#;
    assert_e0014_at(src, "SLEEP_ASYNC", 4, 5);
}

/// **任务体里的 `SCOPE` 体**(也是一个嵌套闭包)⇒ `E0014`。
///
/// ⚠ `SCOPE(FUN(() , …))` 的体是**闭包** ⇒ 它在任务体帧**之下** ⇒ 深度 1。
#[test]
fn a_suspend_point_in_a_scope_body_inside_the_task_body_is_e0014() {
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (
  SCOPE(FUN(() , (
    SLEEP_ASYNC(20)
  )))
))))))"#;
    assert_e0014_at(src, "SLEEP_ASYNC", 4, 5);
}

/// 违规 ⇒ **必须终止**整个运行(不许「任务静默结束 + 无诊断」)。
///
/// ⚠ 这条是 §B-1 的核心诉求:诊断之外,程序**不许**继续跑完。
#[test]
fn a_suspend_point_outside_the_task_body_aborts_the_run() {
    let out = actual(
        r#"IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
LET(helper, FUN(() , (
  SLEEP_ASYNC(20)
)));
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , helper())))));
PRINT("AFTER")"#,
    );
    assert!(
        out.starts_with("!E0014"),
        "违规必须以 E0014 终止, got {out:?}"
    );
}

// ── 判据是「**被调用时**」,不是「真的挂起了才报」─────────────────────────
//
// 规范 §17.0 的原文是「宿主挂起点成员……**被调用时**,实现**必须**产生诊断
// `E0014`」⇒ 判据是**调用点本身**,不是「这一趟有没有真的等」。
//
// ⚠ **这一条是实现口径的分水岭**:若实现改成「等它真的挂起了才报」⇒
//   「这一趟正好不用等」的嵌套调用就**悄悄放行**了 ⇒ 那还是「静默」。
//
// 形状刻意用**不需要等**的那一趟:`TCP_RECV` 拿着一个**不存在的句柄** ⇒ 成员
// **当场**返回 `ERR` 值(系统册 §21.5:未知句柄 ⇒ `ERR`,不是诊断)⇒ **没有挂起**。

/// 嵌套闭包里调用挂起点成员、而那一趟**不需要等** ⇒ 仍然 `E0014`。
#[test]
fn a_suspend_point_that_need_not_wait_is_still_e0014_outside_the_task_body() {
    let src = r#"IMPORT("wlwl:std.net", ["TCP_RECV"]);
LET(helper, FUN(() , (
  TCP_RECV(999999, 10, 10)
)));
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , helper())))))"#;
    assert_e0014_at(src, "TCP_RECV", 3, 3);
}

/// 同一形状在**任务体顶层** ⇒ 合法(成员照常返回它的 `ERR` 值)。
///
/// ⚠ 顶层那条**不是**本项新增的判据(它由既有的「挂起信号逃到顶层」通路管)⇒
///   这里断言的是「顶层**没有**被这条新判据误伤」。
#[test]
fn a_suspend_point_that_need_not_wait_is_fine_in_the_task_body_top_level() {
    let out = actual(
        r#"IMPORT("wlwl:std.net", ["TCP_RECV"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , ( TCP_RECV(999999, 10, 10) ))))))"#,
    );
    assert!(
        !out.contains("E0014"),
        "任务体顶层的挂起点成员不许被判成 E0014, got {out:?}"
    );
}

// ── 通道族**不**归 `E0014` 管(并发册 §17.0 的那条注记)─────────────────────
//
// 规范原文:「⚠ **同款规则**适用于阻塞型通道操作(`CHANNEL_SEND` /
// `CHANNEL_RECV`)与 `AWAIT` 在任务体外的调用 —— 但**诊断码不同**:通道族在任务体
// 外调用时**另有**专门诊断(`E0053`,标准库·并发册 §17.2),`E0014` 是**宿主挂起
// 点成员**这一族的码。」
//
// ⇒ 下面两条把「`E0014` 不许抓通道族」钉死。

/// 通道族在**任务体外** ⇒ `E0053`(不是 `E0014`)。
///
/// ⚠ 缓冲区容量取 **0** ⇒ 这次 `CHANNEL_SEND` 一定**会阻塞** ⇒ 才走
///   「要挂起 ⇒ 必须有任务」那条分支(`E0053`);容量 ≥ 1 时它直接进缓冲区、
///   不挂起 ⇒ 什么诊断都没有。
#[test]
fn a_channel_op_at_module_top_level_is_e0053_not_e0014() {
    let out = actual(
        r#"LET(ch, CHANNEL_NEW(0));
CHANNEL_SEND(ch, 1);"#,
    );
    assert!(
        out.starts_with("!E0053"),
        "通道族在任务体外必须是 E0053, got {out:?}"
    );
    assert!(
        !out.contains("E0014"),
        "通道族不许被报成 E0014, got {out:?}"
    );
}

/// 通道族在**嵌套闭包**里挂起 ⇒ **不许**被报成 `E0014`。
///
/// ⚠ 形状刻意让 `CHANNEL_RECV` **真的挂起**(缓冲区空 ⇒ 接收方停泊)⇒
///   若实现把通道族的挂起也抓进 `E0014`,这条会红(接收任务直接失败)。
#[test]
fn a_channel_op_in_a_nested_closure_is_not_e0014() {
    let out = actual(
        r#"LET(ch, CHANNEL_NEW(1));
LET(helper, FUN(() , ( CHANNEL_RECV(ch) )));
SCOPE(FUN(() , (
  LET(r, SPAWN(FUN(() , helper())));
  LET(s, SPAWN(FUN(() , ( CHANNEL_SEND(ch, 7) ))));
  LET(rv, AWAIT(r));
  AWAIT(s);
  rv
)))"#,
    );
    assert_eq!(
        out, "7",
        "通道族在嵌套闭包里必须照常挂起并醒来, 不许被 E0014 抓走, got {out:?}"
    );
}

// ── 既有行为不许变:模块顶层那条 ───────────────────────────────────────────

/// **模块顶层**调用挂起点成员 ⇒ 仍是既有 `E0014`(并发册 §17.0 的老路径)。
#[test]
fn a_suspend_point_at_module_top_level_is_still_e0014() {
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SLEEP_ASYNC(20)"#;
    assert_e0014_at(src, "SLEEP_ASYNC", 2, 1);
}

/// **模块顶层的闭包里**调用挂起点成员 ⇒ 仍是既有 `E0014`。
///
/// ⚠ 顶层那条走的是「挂起信号逃到顶层」那条通路 —— **不是**本项新增的嵌套通路。
#[test]
fn a_suspend_point_in_a_module_level_closure_is_still_e0014() {
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
LET(helper, FUN(() , (
  SLEEP_ASYNC(20)
)));
helper()"#;
    assert_e0014_at(src, "SLEEP_ASYNC", 3, 3);
}
