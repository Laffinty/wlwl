//! **X-02b** 契约测试 —— 取消请求的「**到达时点**」判据(按**目标**判定)。
//!
//! 规范基线:v1.0 一致性册 §5.4 的 **X-02** 行;并发册 §17.3(`SHIELD(fn)`:
//! 调用 `fn` 并**屏蔽屏蔽块内收到的取消请求**)与附录 **H.4.4**(`SHIELD` 推迟);
//! 机制验证与裁决见 `docs/active/x02-shield-cancel-arrival-time.md`
//! (§6.6 裁决结果 / §6.7 可实现规格 / §6.8 规范侧要改什么)。
//!
//! ## 这条缺口说的是什么
//!
//! `SHIELD(fn)` 会把 `fn` 体内调用的 `TASK_CANCEL_PARENT()` **推迟**到该任务
//! **最外层** `SHIELD` 退出时才生效(并发册 H.4.4)。
//!
//! **旧口径 =「来源」判据**:只有**屏蔽块内发起**的 `TASK_CANCEL_PARENT()` 被推迟;
//! **别的任务**用 `TASK_CANCEL(handle)` 发起的取消**立即**生效(它**完全不看**
//! 屏蔽状态)。
//!
//! **X-02b 已落地**(业主 2026-10-23 裁决)⇒ 改成「**到达时点**」判据,按**目标**
//! 判定 —— 取消请求**到达目标**时,若该目标正处在 `SHIELD` 块内 ⇒ **推迟**到
//! 它退出自己的屏蔽块;否则**立即**生效。
//!
//! ⚠ **判据主体变了,不是判据被放宽**:判据从「取消由谁发起」改成「取消到达目标
//! 那一刻目标在不在屏蔽块内」⇒ **可见面**随之变化(见下「⚠ 判据切换的可见面
//! 变更」三条)。
//!
//! ## 三条裁决(2026-10-23,业主拍板,三条全取推荐项)
//!
//! | # | 裁决 | 结果 |
//! |---|---|---|
//! | **Q1** | 判据主体 | ✅ **按目标判定**(甲案)—— 每个目标各自按「请求到达我时我是否在屏蔽块内」判断 |
//! | **Q2** | 硬清理豁免 | ✅ **只有作用域退出豁免**(甲案)—— 兄弟失败连带**受**屏蔽保护 |
//! | **Q3** | 屏蔽块边界 | ✅ **帧口径**(甲案)—— 屏蔽块 = `SHIELD` 语句在栈上的那段时间,挂起点即结束 |
//!
//! ⇒ 可实现规格见 §6.7(**S1**–**S7**);规范侧要改什么见 §6.8(⚠ **规范正文不动**
//! —— 三条裁决都对齐并发册 §17.3 的字面;§6.8 的第 2 项是**新增差异登记**:
//! 「作用域退出 = 硬清理,**豁免**于 `SHIELD`」这条**例外**要写进一致性册 §5.4)。
//!
//! ## ⚠ 判据切换的可见面变更(§6.7)
//!
//! 1. **被屏蔽的目标**被跨任务 `TASK_CANCEL(handle)` 命中 ⇒ **推迟**到它的屏蔽块
//!    退出(今天立即生效)⇒ **本文件用例 `a` 钉这条**。
//! 2. 被屏蔽的任务发 `TASK_CANCEL_PARENT()` ⇒ **同域未屏蔽的兄弟立即被取消**
//!    (今天一起推迟)⇒ 探针 E 的读数 `0` → `1` ⇒ **本文件用例 `c` 钉这条**。
//! 3. 兄弟失败连带 ⇒ **被屏蔽的兄弟受保护**(今天立即被杀)⇒ 被屏蔽的兄弟能跑完
//!    临界区。
//!
//! ⚠ **单任务**形态**逐字不变**(H.4.4 金标准 `FALSE` / `TRUE` / `1`)—— 由
//! `x02a_shield_per_task_contract.rs` 的用例 `e` 守卫。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `x02a_shield_per_task_contract.rs` 的形状)。
//!
//! ⚠ **每条用例都断言诊断的条数为 0** —— 本项只改判据,**不得**引入任何诊断
//! (`E****` 一条都不该有)。
//!
//! ⚠ **断言一律钉精确值**:两个读数的用例钉成 `[0, 1]` 这样的**整体字面量**,
//! **不**钉「不等于某个值」。
//!
//! ⚠ **接线要点(本轮踩过的坑)**:被屏蔽的任务体里,`SHIELD(...)` **之后**若还有
//! 别的语句,则该任务返回的是**后面那条语句的值**,不是屏蔽块里的读数 ⇒ 读数必须
//! 用 `SET(<外层 `LET MUT` 单元格>, ...)` 抓出来,或把 `SHIELD(...)` 写成任务的
//! **最后一条**表达式。
//!
//! ⚠ **绑定名不得取成内建名**(例如把变量叫 `is_err` ⇒ `IF(is_err, ...)` 会解析成
//! **内建零参调用** `IS_ERR` ⇒ 返回 `FALSE`)⇒ 本文件一律用 `read` / `flag` /
//! `inside_read` / `after_read` / `flag_read`。

use wlwl_eval::Evaluator;

/// 一次求值的结果。
struct Outcome {
    /// 成功 ⇒ 值的 `display()`;失败 ⇒ 首条诊断的「码 消息」。
    value: String,
    /// 失败 ⇒ 全部诊断的「码 消息」(按报告顺序);成功 ⇒ 空。
    diagnostics: Vec<String>,
}

fn outcome(src: &str) -> Outcome {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(v) => Outcome {
            value: v.display(),
            diagnostics: Vec::new(),
        },
        Err(e) => {
            let diagnostics: Vec<String> = e
                .diagnostics()
                .iter()
                .map(|d| {
                    if d.message.is_empty() {
                        d.code.as_str().to_string()
                    } else {
                        format!("{} {}", d.code.as_str(), d.message)
                    }
                })
                .collect();
            let value = diagnostics.first().cloned().unwrap_or_default();
            Outcome { value, diagnostics }
        }
    }
}

// 一条用例 = 一个 `#[test]`。
macro_rules! case {
    ($name:ident, $src:expr, $expect_value:expr) => {
        #[test]
        fn $name() {
            let o = outcome($src);
            assert_eq!(
                o.value,
                $expect_value,
                "case `{}` (X-02b / §6.7) 的值",
                stringify!($name)
            );
            assert_eq!(
                o.diagnostics,
                Vec::<String>::new(),
                "case `{}` (X-02b / §6.7) 不得产生任何诊断",
                stringify!($name)
            );
        }
    };
}

// ── 可见面变更 ①:跨任务 `TASK_CANCEL(handle)` 命中**被屏蔽**的目标 ⇒ 推迟 ──

// 目标 `a` 持着 `SHIELD` 并在屏蔽块内 `AWAIT(兄弟)`;兄弟在 `a` 的屏蔽窗口内
// 调 `TASK_CANCEL(a)`。返回值 = `[屏蔽块**内**的读数, 屏蔽块**外**的读数]`。
//
// ⚠ 钉的是 **Q1「按目标判定」**:取消请求**到达 `a`** 时 `a` 正在自己的屏蔽块内
// (`AWAIT` 不展开屏蔽帧 ⇒ 深度仍是 1)⇒ **推迟** ⇒ 屏蔽块内读数 `0`(读不到
// 取消);`a` 退出自己的屏蔽块时待发取消释放 ⇒ 屏蔽块外读数 `1`。
// ⚠ 旧口径(来源判据)下 `TASK_CANCEL(handle)` **完全不看**屏蔽状态 ⇒ 立即生效
// ⇒ 屏蔽块内读数会是 `1` ⇒ **判别性用例**。
case!(
    x02b_a_cross_task_cancel_to_shielded_target_is_deferred,
    r#"LET MUT(inside_read, -1);
LET MUT(after_read, -1);
LET(out, SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            LET(h, bh);
            AWAIT(h);
            SET(inside_read, IF(TASK_IS_CANCELLED(), 1, 0));
            0
        )));
        LET(_, v);
        SET(after_read, IF(TASK_IS_CANCELLED(), 1, 0));
        0
    )));
    SET(bh, SPAWN(FUN(() ,
        TASK_CANCEL(a);
        0
    )));
    LET(ra, AWAIT(a));
    LET(_, ra);
    0
)));
LET(_, out);
[inside_read, after_read]"#,
    "[0, 1]"
);

// ── 契约 ⑥:跨任务 `TASK_CANCEL(handle)` 命中**未屏蔽**的目标 ⇒ 立即生效 ────

// 目标 `a` **没有** `SHIELD`;兄弟调 `TASK_CANCEL(a)`。返回值 = `a` 从 `AWAIT`
// 恢复后读到的 `TASK_IS_CANCELLED()`(1 = 是,0 = 否)。
//
// ⚠ 钉的是「**没**在屏蔽块内的目标**不受**新判据影响」⇒ 取消**立即**生效 ⇒
// 读数 `1`。
// ⚠ 与用例 `a` 构成对照:`a` 唯一多出来的是目标侧的 `SHIELD` ⇒ 读数从 `[0, 1]`
// 变成 `1`(「屏蔽块内读不到」那一半消失)。
case!(
    x02b_b_cross_task_cancel_to_unshielded_target_is_immediate,
    r#"LET MUT(read, -1);
LET(out, SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(h, bh);
        AWAIT(h);
        SET(read, IF(TASK_IS_CANCELLED(), 1, 0));
        0
    )));
    SET(bh, SPAWN(FUN(() ,
        TASK_CANCEL(a);
        0
    )));
    LET(ra, AWAIT(a));
    LET(_, ra);
    0
)));
LET(_, out);
read"#,
    "1"
);

// ── 可见面变更 ②:被屏蔽的发起方 ⇒ 同域**未**屏蔽的兄弟**立即**被取消 ──────

// 被**屏蔽**的任务 `a` 发 `TASK_CANCEL_PARENT()`;`a` 在同一个 `SCOPE` 里还有
// 一个**未屏蔽**的兄弟。返回值 = `a` 在 `AWAIT(兄弟)` 处读到的 `IS_ERR(...)`
// (1 = 兄弟被取消、0 = 没被取消)。
//
// ⚠ 钉的是 **Q1「按目标判定」** 的**直接后果**:取消**逐目标**投递 ⇒ 兄弟**不在**
// 屏蔽块内 ⇒ **立即**被取消 ⇒ 兄弟**来不及跑起来** ⇒ `AWAIT(兄弟)` 得到
// `ERR(kind="Cancelled")` ⇒ 读数 `1`。
// ⚠ 旧口径(来源判据)下整棵子树一起推迟 ⇒ 兄弟**不会**被取消 ⇒ 读数 `0` ⇒
// **判别性用例**。
case!(
    x02b_c_shielded_canceller_kills_unshielded_sibling_immediately,
    r#"LET MUT(flag_read, -1);
LET(out, SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            LET(h, bh);
            LET(r, AWAIT(h));
            SET(flag_read, IF(IS_ERR(r), 1, 0));
            LET(_, r);
            0
        ))
    )));
    SET(bh, SPAWN(FUN(() , 0)));
    LET(ra, AWAIT(a));
    LET(_, ra);
    0
)));
LET(_, out);
flag_read"#,
    "1"
);
