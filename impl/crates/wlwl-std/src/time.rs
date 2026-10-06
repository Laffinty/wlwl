//! [v0.11.3 M6 / addendum-01 A1] `wlwl:std.time` —— 墙钟与单调钟(R2)。
//!
//! 这是本语言**第一批时间能力**,也是 GAP-4 的第一刀(ADR-0024 §Context:
//! 「标准库目前最大的一块硬缺口」)。
//!
//! ## A1 放纯读成员,A2 加一个阻塞原语,B(气泡)是下一片
//!
//! **A1** 只落两个纯读成员 `NOW` / `MONOTONIC`。这与 ADR-0024 草案
//! 第 1 条「时间成员只进气泡语义」不同 —— 那条把两件无关的事耦合了:「读系统
//! 时钟」与「有没有虚拟时钟」毫无关系,气泡只改变*读什么*,不改变*能不能读*。
//! 批准时的重切见 `ADR-0024` §6 与 `addendum-01` §3.7:A1 / A2 / B 三片。
//!
//! **A2** 加 `SLEEP(ms)`。它**不**挂起任务,而是直接阻塞调用线程 —— 原因与代价
//! 见 `sleep` 的文档注释与 `addendum-01` §3.10(这一片的实施过程撞上了「wlwl 只有
//! `YIELD()` 能中途续跑」这个运行时能力缺口,是业主裁决的结果,不是实现取巧)。
//!
//! ## 为什么不走宿主(与 `RANDOM_BYTES` 同款)
//!
//! `std.encode` 的 `RANDOM_BYTES` 直接调 `getrandom::fill`、**不经宿主**,
//! 理由写进了规范 §11.6:「`std.encode` 是**源**」。时间同理 —— `std.time` 是
//! 时间的源,成员**直接读系统时钟**。
//!
//! 气泡(B 阶段)要**替换掉本文件读的那个时钟**,那时才需要一个接缝,而接缝
//! **只走宿主**(`StdHost` 加方法,ADR-0024 §6.3-2):因为那个替换**不能是全局可变
//! 状态** —— 否则测试之间互相污染、结果顺序相关,违反 D12-006 同族的「无隐藏
//! 全局状态」纪律。⚠️ 反过来说:**绝不能**给本模块的成员加「可注入时钟」形参,
//! 那等于让 CSPRNG 成员可被降级成可预测源(同一条理由见 `addendum-04` §3.3)。
//!
//! ## 两条口径(ADR-0024 §6.3-1)
//!
//! - **`NOW()` 返回 Unix 毫秒**(`INTEGER`)。不是秒 —— 秒会引入 ±1 s 的量化
//!   误差,直接污染「缓存 5 秒过期」这类真实语义。**不返回 ISO-8601 字符串**:
//!   那是格式化层的事,本模块不扩。
//! - **`MONOTONIC()` 的绝对值无意义,只有差值有效。** 它是 `Instant` 语义
//!   (进程启动起的单调毫秒),**不是** Unix 时间 —— 两者的绝对值不能互相替换,
//!   规范与成员表都写死了这一条。
//!
//! **三个成员单位统一为毫秒是刻意的**:这样 `SLEEP(ms)` 与时钟差值能直接
//! 对齐,不必在每个调用点换算。代价是**不支持亚毫秒测量** —— 记为已知限制,
//! 不是疏漏。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.time",
    functions: &[
        ("NOW", now as StdFn),
        ("MONOTONIC", monotonic as StdFn),
        ("SLEEP", sleep as StdFn),
    ],
};

// ── 助手 ───────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected integer, got {}", crate::value_kind(got)),
    )
}

// ── 时钟接缝(A1 用真实时钟;B 阶段换函数体,不改签名)─────────────────────

/// 墙钟:Unix 纪元起的**毫秒**。
///
/// **`_host` 是刻意留的接缝**:B 阶段的虚拟时钟要通过 `StdHost` 取,那时把这个
/// 形参用上、函数体换掉即可 —— **成员签名与调用点一行都不用改**。这也是为什么
/// A1 现在就按「带 host 形参」的形状写,而不是写一个裸 `now()`。
fn now_ms(_host: &mut dyn StdHost) -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        // 系统时钟早于 1970 是病态情形(改了系统时间 / 容器里 epoch 未初始化)。
        // 不用 `unwrap()` 崩掉一个只读的时钟成员:负毫秒是**可表达**的
        // (减法仍然精确),而崩溃不是。
        Err(e) => -i64::try_from(e.duration().as_millis()).unwrap_or(i64::MAX),
    }
}

/// 单调钟:**进程启动起**的毫秒。
///
/// `Instant` 保证单调不减,且**不受系统时间调整影响**(NTP 校时 / 手动改表 /
/// 夏令时都动不了它)—— 这正是它与 `NOW` 并存的理由:「过了多久」要问它,
/// 「现在几点」要问 `NOW`。⚠️ 它的**绝对值无意义**,跨进程不可比。
fn monotonic_ms(_host: &mut dyn StdHost) -> i64 {
    // 进程启动时的基准,用 `OnceLock` 固定住 —— 否则每次调用都从一个新基准算起,
    // 结果不是单调的,而是「永远等于上一次到现在」。
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let origin = ORIGIN.get_or_init(std::time::Instant::now);
    i64::try_from(origin.elapsed().as_millis()).unwrap_or(i64::MAX)
}

// ── 成员 ───────────────────────────────────────────────────────────────

/// `NOW() -> INTEGER`
///
/// Unix 纪元起的**毫秒**。**墙钟** —— 可能因系统时间调整而**向后跳**,所以
/// **不要**用它测时间差(那是 `MONOTONIC` 的活)。仅在任务顶层 / 同步调用里可用。
pub fn now(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if !args.is_empty() {
        return Err(arity(host, "NOW", args.len(), 0));
    }
    Ok(Outcome::normal(Value::Integer(now_ms(host))))
}

/// `MONOTONIC() -> INTEGER`
///
/// **进程启动起**的毫秒,单调不减且不受系统时间调整影响。
/// ⚠️ **绝对值无意义、跨进程不可比 —— 只有差值有效**(这是 `Instant` 的契约,
/// 也是它不能用来记「绝对时刻」的原因)。亚毫秒测量不支持(单位是毫秒)。
pub fn monotonic(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if !args.is_empty() {
        return Err(arity(host, "MONOTONIC", args.len(), 0));
    }
    Ok(Outcome::normal(Value::Integer(monotonic_ms(host))))
}

/// `SLEEP(ms) -> NULL`
///
/// **阻塞原语**:把调用线程停住 `ms` 毫秒,期间**不调度任何其他任务**,然后返回
/// `NULL`。
///
/// ## 为什么是阻塞,而不是挂起任务(2026-10-06 业主裁决「方案 E」)
///
/// 原本的设计是「任务内挂起、到点唤醒」—— 那样能保住并发。实现时撞上一个**运行时
/// 能力缺口**,查证结论(`addendum-01` §3.10):
///
/// - wlwl 的「中途挂起后**原地续跑**」能力**只有 `YIELD()` 有**。它由
///   `yield_split::split_body_for_yield` 产出分段 + `run_task_segments` 保存
///   `running_env` 两件事共同实现,而**分段器只认 `YIELD()` 这一个名字**。
/// - 任何**非分段**任务体一旦被挂起再唤醒,重入走的是 `invoke_closure`
///   **整段重跑**:绑定从头重来、副作用重放。
/// - `Sleeping` 若走挂起路线,唤醒后会**再次执行同一个 `SLEEP`** ⇒ 无限循环。
///   (实测:8 秒打了 79 次 `tick` 仍不终止。)
///
/// 所以本成员**不挂起**。这条代价**原样写进规范**,不藏:「`SLEEP` 期间不调度
/// 其他任务」是 `addendum-01` §3.8 候选甲**早已接受**的那句代价 —— 选 E 只是把它
/// 从「队列排空后才阻塞」提前到「调用点即刻阻塞」,并没有新增一类代价。
///
/// **附带收益**:B 片(假时钟气泡)因此不再需要调度器那处「队列空 ⇒ 判全阻塞 ⇒
/// 推进虚拟时钟」的分支 —— 气泡内 `SLEEP` 直接推进虚拟时钟并返回即可。
///
/// ## 边界
///
/// - `ms < 0` → `E0030`。负睡眠没有可表达的语义(不是「睡到过去」),不猜。
/// - `ms = 0` 合法:让出一次 CPU 时间片,语义明确,不需要特判。
/// - 单位是毫秒,与 `NOW` / `MONOTONIC` 对齐(亚毫秒不支持,已知限制)。
/// - 与阻塞式 IO 在途时**不可依赖其及时性**(§3.8 已登记的语言级取舍)。
pub fn sleep(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "SLEEP", args.len(), 1));
    }
    let Value::Integer(ms) = &args[0] else {
        return Err(type_err(host, "SLEEP", &args[0]));
    };
    if *ms < 0 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("SLEEP: expected a non-negative duration in ms, got {ms}"),
        ));
    }
    // `ms >= 0` 已由上一行保证,`as u64` 不会回绕。
    std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
    Ok(Outcome::normal(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 契约要的是一个能在**测试里**跑的宿主。`wlwl-std` 的单测没有 eval 的
    /// host,所以这里按 `kernels.rs` 里那个 `Bare` 的同款形状手写一个 ——
    /// 顺带验证了 `StdHost` 的面小到可以手写(只有三个方法),这正是 B 阶段
    /// 加第四个方法的代价依据。
    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut crate::StdCtx {
            // `std.time` 的**任何**成员都不读 ctx;这个 panic 是**故意的** ——
            // 它把「本模块不经宿主取时钟」这件事变成可执行的断言,而不是注释里的
            // 一句承诺。⚠️ 它同时守着一条已付过代价的决定:`SLEEP` 曾经要靠
            // `StdCtx.task_depth` 区分「任务内 / 顶层」,方案 E 改成「一律阻塞」
            // 之后**不再需要那个字段**(详见 `sleep` 的文档注释)。
            panic!("std.time must not touch StdCtx (the clock is read directly)")
        }

        fn call(
            &mut self,
            _f: &Value,
            _args: Vec<Value>,
            _name: &str,
        ) -> wlwl_error::WlwlResult<Outcome> {
            panic!("std.time A1 has no member that calls back into the host")
        }

        fn diag(&mut self, code: ErrorCode, message: String) -> wlwl_error::WlwlError {
            wlwl_error::WlwlDiagnostic::new(
                code,
                message,
                wlwl_error::Location::point("<test>", 0, 0),
            )
            .into()
        }
    }

    #[test]
    fn now_is_plausible_epoch_millis() {
        let out = now(&mut NullHost, vec![]).expect("NOW succeeds");
        let Value::Integer(ms) = out.value else {
            panic!("NOW must return INTEGER")
        };
        // 2020-01-01 = 1_577_836_800_000 ms。只断言「落在 2020 与 2100 之间」——
        // 不断言精确值(那是时钟本身,不是实现)。
        assert!(
            ms > 1_577_836_800_000 && ms < 4_102_444_800_000,
            "NOW() = {ms} is not a plausible epoch-millisecond reading"
        );
    }

    #[test]
    fn monotonic_is_non_decreasing() {
        let mut last = i64::MIN;
        for _ in 0..1000 {
            let Value::Integer(ms) = monotonic(&mut NullHost, vec![]).unwrap().value else {
                panic!("MONOTONIC must return INTEGER")
            };
            assert!(ms >= last, "MONOTONIC went backwards: {last} -> {ms}");
            last = ms;
        }
        assert!(last >= 0, "elapsed time cannot be negative");
    }

    /// 单调钟**不受墙钟调整影响** —— 这是它存在的理由,也是
    /// `MONOTONIC` / `NOW` 必须分开的理由。
    ///
    /// 本测**不能**真的改系统时钟,所以断言的是可测的那一半:两者取自**不同的
    /// 源**(`Instant` vs `SystemTime`),因此不存在「用一个推导另一个」的实现。
    #[test]
    fn the_two_clocks_are_independent_sources() {
        let Value::Integer(wall) = now(&mut NullHost, vec![]).unwrap().value else {
            panic!()
        };
        let Value::Integer(mono) = monotonic(&mut NullHost, vec![]).unwrap().value else {
            panic!()
        };
        // 单调钟从进程启动起算,墙钟从 1970 起算 —— 两者差着四十多年,
        // 除非机器刚启动不到 1000 秒,否则绝不可能落在彼此 1000 秒之内。
        assert!(
            wall.abs_diff(mono) > 1_000_000,
            "the two readings look like the same source ({wall} vs {mono})"
        );
    }

    #[test]
    fn arity_is_checked() {
        let e = now(&mut NullHost, vec![Value::Integer(1)]).unwrap_err();
        assert!(e
            .diagnostic()
            .message
            .contains("expects 0 argument(s), got 1"));
        let e = monotonic(&mut NullHost, vec![Value::Null]).unwrap_err();
        assert!(e
            .diagnostic()
            .message
            .contains("expects 0 argument(s), got 1"));
    }

    // ── SLEEP 的形态契约 ───────────────────────────────────────────────
    //
    // ⚠️ 这里**只测诊断,不测时长**。「睡了多久」是计时断言,写进 `cargo test`
    // 必然抖动 —— 它归契约表(那里断言的是「单调钟至少前进 N 毫秒」这个**下界**)
    // 与基准(吞吐),这里测的是**形状**:元数 / 类型 / 负值 / 返回值。

    #[test]
    fn sleep_rejects_bad_arity_type_and_negative() {
        for bad in [vec![], vec![Value::Integer(1), Value::Integer(2)]] {
            let e = sleep(&mut NullHost, bad.clone()).unwrap_err();
            assert_eq!(
                e.diagnostic().code,
                ErrorCode::E0022,
                "arity {bad:?} must be E0022, got {}",
                e.diagnostic().message
            );
        }
        let e = sleep(&mut NullHost, vec![Value::Float(1.0)]).unwrap_err();
        assert_eq!(
            e.diagnostic().code,
            ErrorCode::E0030,
            "a non-integer duration must be E0030"
        );
        assert!(e.diagnostic().message.contains("expected integer"));
        let e = sleep(&mut NullHost, vec![Value::Integer(-1)]).unwrap_err();
        assert_eq!(
            e.diagnostic().code,
            ErrorCode::E0030,
            "a negative duration must be E0030"
        );
        assert!(e
            .diagnostic()
            .message
            .contains("expected a non-negative duration"));
    }

    /// `ms = 0` 合法并返 `NULL` —— 「让出一次时间片」是可表达的语义,不需要特判,
    /// 也不该被误当成「没写参数」。
    #[test]
    fn sleep_zero_returns_null() {
        let out = sleep(&mut NullHost, vec![Value::Integer(0)]).expect("SLEEP(0) succeeds");
        assert!(
            matches!(out.value, Value::Null),
            "SLEEP must return NULL, got {:?}",
            out.value
        );
    }
}
