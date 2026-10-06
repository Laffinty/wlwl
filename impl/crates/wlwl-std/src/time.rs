//! [v0.11.3 M6 / addendum-01 A1] `wlwl:std.time` —— 墙钟与单调钟(R2)。
//!
//! 这是本语言**第一批时间能力**,也是 GAP-4 的第一刀(ADR-0024 §Context:
//! 「标准库目前最大的一块硬缺口」)。
//!
//! ## 只放纯读成员,气泡是下一片
//!
//! 本模块 A1 阶段**只有两个纯读成员** `NOW` / `MONOTONIC`。这与 ADR-0024 草案
//! 第 1 条「时间成员只进气泡语义」不同 —— 那条把两件无关的事耦合了:「读系统
//! 时钟」与「有没有虚拟时钟」毫无关系,气泡只改变*读什么*,不改变*能不能读*。
//! 批准时的重切见 `ADR-0024` §6 与 `addendum-01` §3.7:A1 / A2 / B 三片,
//! 本文件是 **A1**。
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
//! **两个成员单位统一为毫秒是刻意的**:这样 `SLEEP(ms)`(A2)与时钟差值能直接
//! 对齐,不必在每个调用点换算。代价是**不支持亚毫秒测量** —— 记为已知限制,
//! 不是疏漏。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.time",
    functions: &[("NOW", now as StdFn), ("MONOTONIC", monotonic as StdFn)],
};

// ── 助手 ───────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
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
            // A1 的成员不读 ctx;这个 panic 是**故意的** —— 它把「A1 不经宿主」
            // 这件事变成可执行的断言,而不是注释里的一句承诺。
            panic!("std.time A1 must not touch StdCtx (the clock is read directly)")
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
}
