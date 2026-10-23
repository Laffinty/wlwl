//! **X-02a** 契约测试 —— `SHIELD` 的屏蔽状态必须是 **per-task**。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 一致性册 §5.4 的 **X-02** 行;并发册
//! §17.3 与附录 **H.4.4**(`SHIELD` 推迟);机制验证与实施前置见
//! `docs/active/x02-shield-cancel-arrival-time.md`(§3.1「屏蔽状态是**共享**的,
//! 不是 per-task」与 §9 的 **X-02a** 批次划分)。
//!
//! ## 这条缺口说的是什么
//!
//! `SHIELD(fn)` 把 `fn` 体内调用的 `TASK_CANCEL_PARENT()` **推迟**到该任务
//! **最外层** `SHIELD` 退出时才生效(并发册 H.4.4:「取消请求只被记录,不被
//! 作用于任务;最外层 SHIELD 退出后,取消请求转为 H.4.1 的待取消标记」)。
//!
//! 实现原先把**屏蔽深度** `shield_depth` 与**待发取消槽**
//! (`shield_pending_cancel` / `shield_pending_reason`)挂在**共享 `Evaluator`**
//! 上 ⇒ 状态在**任务之间**串味:
//!
//! * 任务 A 在 `SHIELD` 块内 `AWAIT`(调度器会把**别的任务** B 拉进来跑)时,
//!   **B 被误遮挡** —— B 的 `TASK_CANCEL_PARENT()` 被**误缓冲**;
//! * 那次待发取消**永不触发** —— B 的屏蔽块退出时共享深度回不到 0。
//!
//! ⇒ 必须把这两类状态改成 **per-task**(落在 `Task` 上:
//! `Task::shield_depth` / `Task::shield_pending_cancel` /
//! `Task::shield_pending_reason`)。
//!
//! ## 本项**不改判据**
//!
//! ⚠ X-02a 只改**状态归属**,**判据本身逐字不变** —— 仍是「取消的**来源**」
//! 判据:`TASK_CANCEL_PARENT()` 在**自己**的 `SHIELD` 块内被推迟;
//! `TASK_CANCEL(handle)` 仍**立即**生效(它**完全不看**屏蔽状态)。
//!
//! ⇒ 「取消的**到达时点**」判据属 **X-02b**(同文 §9),**不在**本文件范围内。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `x01_channel_close_contract.rs` 的形状)。
//!
//! ⚠ **每条用例都断言诊断的条数为 0** —— 本项只改状态归属,**不得**引入
//! 任何诊断(`E****` 一条都不该有)。
//!
//! ⚠ **断言一律钉精确值**:两个读数的用例钉成 `[0, 1]` 这样的**整体字面量**,
//! **不**钉「不等于某个值」。
//!
//! ⚠ **V1 / V2 是判别性的** —— 实测读数(改动前 / 改动后)= `0` / `1`
//! ⇒ 断言写 `1` 就能区分两侧。**V3 是回归守卫**(两侧都是 `1`):它防的是
//! 「改完之后 A 自己的待发取消反而丢了」。其余用例是**守形状**的守卫。

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
                "case `{}` (X-02a / H.4.4) 的值",
                stringify!($name)
            );
            assert_eq!(
                o.diagnostics,
                Vec::<String>::new(),
                "case `{}` (X-02a / H.4.4) 不得产生任何诊断",
                stringify!($name)
            );
        }
    };
}

// ── 缺口本体:别的任务的 `SHIELD` 不得遮挡本任务的取消 ─────────────────

// A 持着 `SHIELD` 并在里面 `AWAIT(B)`;B **没有** `SHIELD`,自己调
// `TASK_CANCEL_PARENT()`。返回值 = B 在调完 `TASK_CANCEL_PARENT()` **紧接着**
// 读到的 `TASK_IS_CANCELLED()`(1 = 是,0 = 否)。
//
// ⚠ **判别性用例**:B 自己的屏蔽深度是 0 ⇒ 取消**立即**生效 ⇒ 读数 `1`。
// 共享深度下 B 被 A 的屏蔽深度**误遮挡** ⇒ 取消被缓冲且永不触发 ⇒ 读数 `0`。
case!(
    x02a_a_other_tasks_shield_does_not_mask_this_tasks_cancel,
    r#"SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            LET(h, bh);
            AWAIT(h)
        )));
        v
    )));
    SET(bh, SPAWN(FUN(() ,
        TASK_CANCEL_PARENT();
        IF(TASK_IS_CANCELLED(), 1, 0)
    )));
    AWAIT(a)
))"#,
    "1"
);

// ── 缺口本体:每个任务缓冲**自己**的待发取消 ──────────────────────────

// A:`SHIELD { TASK_CANCEL_PARENT(); AWAIT(B) }`;B:`SHIELD { TASK_CANCEL_PARENT() }`
// (B 在 A 的屏蔽窗口内跑)。返回值 = B 在**自己的** `SHIELD` 退出**之后**读到的
// `TASK_IS_CANCELLED()`(1 = 是,0 = 否)。
//
// ⚠ **判别性用例**:B 的待发取消在 **B 自己的**屏蔽块退出时生效 ⇒ 读数 `1`。
// 共享待发取消槽下 B 的屏蔽块退出时共享深度回不到 0 ⇒ 那次取消被吞掉 ⇒ 读数 `0`。
case!(
    x02a_b_each_task_buffers_its_own_pending_cancel,
    r#"SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            LET(h, bh);
            AWAIT(h)
        )));
        v
    )));
    SET(bh, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            0
        )));
        IF(TASK_IS_CANCELLED(), 1, 0)
    )));
    AWAIT(a)
))"#,
    "1"
);

// ── 回归守卫:A 自己的待发取消仍在 A 自己的屏蔽块退出时生效 ───────────

// A:`SHIELD { TASK_CANCEL_PARENT(); AWAIT(B) }`,B 有 `SHIELD` 但不取消。返回值 =
// A 在**自己的** `SHIELD` 退出**之后**读到的 `TASK_IS_CANCELLED()`。
//
// ⚠ 这条**不判别**(改动前后都是 `1`):它防的是「改完之后 A 自己的待发取消
// 反而丢了」—— B 的屏蔽块退出**不得**回放或清空 A 的待发取消。
case!(
    x02a_c_own_pending_cancel_fires_at_own_shield_exit,
    r#"SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            LET(h, bh);
            LET(r, AWAIT(h));
            LET(_, r);
            0
        )));
        IF(TASK_IS_CANCELLED(), 1, 0)
    )));
    SET(bh, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() , 0)));
        v
    )));
    AWAIT(a)
))"#,
    "1"
);

// ── 两个任务**各自**缓冲并**各自**在退出时生效 ────────────────────────

// A 与 B 都在**自己**的 `SHIELD` 里调 `TASK_CANCEL_PARENT()`;B 在 A 的屏蔽
// 窗口内跑。返回值 = `[A 的读数, B 的读数]`,两个读数都取在**各自**的屏蔽块
// **退出之后**。
//
// ⚠ 钉的是「待发取消槽**不共享**」:共享槽下 B 的待发取消会被 A 的屏蔽块退出
// **回放**(A 读数仍为 `1`)或被 B 的屏蔽块退出**清空**(B 读数为 `0`)。
case!(
    x02a_d_both_tasks_fire_their_own_pending_cancel,
    r#"LET MUT(a_read, -1);
LET MUT(b_read, -1);
LET(out, SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            LET(h, bh);
            AWAIT(h);
            0
        )));
        LET(_, v);
        SET(a_read, IF(TASK_IS_CANCELLED(), 1, 0));
        0
    )));
    SET(bh, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            0
        )));
        LET(_, v);
        SET(b_read, IF(TASK_IS_CANCELLED(), 1, 0));
        0
    )));
    LET(r, AWAIT(a));
    LET(_, r);
    0
)));
LET(_, out);
[a_read, b_read]"#,
    "[1, 1]"
);

// ── 金标准(H.4.4):单任务、一个 `SHIELD`、自己取消 ───────────────────

// 返回值 = `[屏蔽块**内**的读数, 屏蔽块**外**的读数]`。
//
// ⚠ H.4.4 的金标准形状是 `FALSE / TRUE / 1` —— 块内**否**、块外**是**。
// 这里用 `1 / 0` 的整数字面量承载同样的两个读数。
case!(
    x02a_e_single_task_gold_standard_inside_and_after,
    r#"LET MUT(inside_read, -1);
LET MUT(after_read, -1);
LET(out, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            TASK_CANCEL_PARENT();
            SET(inside_read, IF(TASK_IS_CANCELLED(), 1, 0));
            0
        )));
        LET(_, v);
        SET(after_read, IF(TASK_IS_CANCELLED(), 1, 0));
        0
    )));
    LET(r, AWAIT(a));
    LET(_, r);
    0
)));
LET(_, out);
[inside_read, after_read]"#,
    "[0, 1]"
);

// ── 嵌套 `SHIELD`:推迟到**最外层**退出 ──────────────────────────────

// 同一任务两层 `SHIELD`,内层取消。返回值 = `[内层退出后的读数, 最外层退出后的
// 读数]`。
//
// ⚠ 钉的是「嵌套**不**提前生效」:内层退出后还在外层里 ⇒ 读数 `0`;最外层退出
// 才回放 ⇒ 读数 `1`。
case!(
    x02a_f_nested_shield_defers_until_outermost_exit,
    r#"LET MUT(after_inner, -1);
LET MUT(after_outer, -1);
LET(out, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            LET(w, SHIELD(FUN(() ,
                TASK_CANCEL_PARENT();
                0
            )));
            LET(_, w);
            SET(after_inner, IF(TASK_IS_CANCELLED(), 1, 0));
            0
        )));
        LET(_, v);
        SET(after_outer, IF(TASK_IS_CANCELLED(), 1, 0));
        0
    )));
    LET(r, AWAIT(a));
    LET(_, r);
    0
)));
LET(_, out);
[after_inner, after_outer]"#,
    "[0, 1]"
);

// ── `TASK_CANCEL(handle)` 仍**立即**生效(不看屏蔽状态) ───────────────

// A 在 `SHIELD` 块内 `AWAIT(B)`;B 调 `TASK_CANCEL(a)`。返回值 = A 从 `AWAIT`
// 恢复后读到的 `TASK_IS_CANCELLED()`(1 = 是,0 = 否)。
//
// ⚠ **未改动的语义**:`TASK_CANCEL(handle)` **完全不看**屏蔽状态 ⇒ 立即生效
// ⇒ 读数 `1`。X-02a 只改状态归属,**不得**把它一起推迟。
case!(
    x02a_g_task_cancel_handle_is_not_deferred,
    r#"LET MUT(a_read, -1);
LET(out, SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            LET(h, bh);
            AWAIT(h);
            SET(a_read, IF(TASK_IS_CANCELLED(), 1, 0));
            0
        )));
        LET(_, v);
        0
    )));
    SET(bh, SPAWN(FUN(() ,
        TASK_CANCEL(a);
        0
    )));
    LET(r, AWAIT(a));
    LET(_, r);
    0
)));
LET(_, out);
a_read"#,
    "1"
);

// ── 别的任务的 `SHIELD` 不得**误嵌套**本任务的 `SHIELD` ───────────────

// A 持着 `SHIELD` 并在里面 `AWAIT(B)`;B 在 A 的屏蔽窗口内跑,自己套了**两层**
// `SHIELD`,退出两层之后再调 `TASK_CANCEL_PARENT()`。返回值 = B 读到的
// `TASK_IS_CANCELLED()`(1 = 是,0 = 否)。
//
// ⚠ 钉的是「B 的屏蔽深度**不被 A 抬高**」:B 退出自己两层后深度必须回到 0 ⇒
// 它自己的 `TASK_CANCEL_PARENT()` **立即**生效 ⇒ 读数 `1`。共享深度下 B 的深度
// 退不回去 ⇒ 取消被**误缓冲** ⇒ 读数 `0`。
case!(
    x02a_h_other_tasks_nested_shield_does_not_misnest,
    r#"LET MUT(b_read, -1);
LET(out, SCOPE(FUN(() ,
    LET MUT(bh, 0);
    LET(a, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            LET(h, bh);
            AWAIT(h)
        )));
        LET(_, v);
        0
    )));
    SET(bh, SPAWN(FUN(() ,
        LET(v, SHIELD(FUN(() ,
            LET(w, SHIELD(FUN(() , 0)));
            LET(_, w);
            0
        )));
        LET(_, v);
        TASK_CANCEL_PARENT();
        SET(b_read, IF(TASK_IS_CANCELLED(), 1, 0));
        0
    )));
    LET(r, AWAIT(a));
    LET(_, r);
    0
)));
LET(_, out);
b_read"#,
    "1"
);

// ── 顶层(模块顶层,不在 `SCOPE` 里)的 `SHIELD` 兜底路径 ─────────────────

// 顶层没有「自己的任务」可挂 ⇒ `SHIELD` 走求值器上的兜底计数;顶层
// `TASK_CANCEL_PARENT()` 本来就是 no-op(无作用域可取消)。
// 返回值 = 顶层读到的 `TASK_IS_CANCELLED()`(1 = 是,0 = 否)。
//
// ⚠ 钉的是「顶层兜底**不崩、不误报**」:顶层不得被取消 ⇒ 读数 `0`,且**不得**
// 产生任何诊断。
case!(
    x02a_i_top_level_shield_fallback_is_a_noop,
    r#"LET(v, SHIELD(FUN(() ,
    TASK_CANCEL_PARENT();
    IF(TASK_IS_CANCELLED(), 1, 0)
)));
LET(_, v);
IF(TASK_IS_CANCELLED(), 1, 0)"#,
    "0"
);
