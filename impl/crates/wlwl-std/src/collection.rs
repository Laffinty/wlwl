//! `wlwl:std.collection` 的 **R2 内核** —— 标准库底座 v0.11 M5。
//!
//! ## 为什么这个文件存在:一个基于实测的层归属回退
//!
//! M3-1 起 `std.collection` 全部 17 个成员是纯 wlwl。M5 的基准基线
//! ([`../../benches/baseline.txt`](../../benches/baseline.txt) 的 M5 段)
//! 测出 `RANGE` 在规模上**不可用**,实测(release 档、单线程):
//!
//! ```text
//! RANGE(0, 10 000)    3 623 ms     0.36 ms/元素
//! RANGE(0, 40 000)   86 036 ms     2.15 ms/元素
//! ```
//!
//! 每元素成本随 n 超线性上涨。对照 Phase I1 / B6 的 R2 行:`RANGE(1, 1 000 001, 1)`
//! **加上** 100 万次 `FOR` 整体只要 536–551 ms。根因不是解释器慢,而是语言层
//! **无法线性建数组**:wlwl 数组不可变,`PUSH` 每次复制整个数组。
//!
//! 后果很具体:语言规范 §6.6 的符合性工作负载「100 万次简单循环 < 30 s」
//! 跑不完,而且时间全部消耗在**循环开始之前** —— 光是构造那个区间就远超预算。
//!
//! 据此,业主裁决:**`RANGE` 单独沉回 R2,其余 16 个成员仍留 R1**。
//!
//! ## 为什么实测只沉了 `RANGE`(以及它的局限)
//!
//! 实测显示**所有建数组的成员**都有同一成本剖面(`MAP` / `FILTER` / `FLAT` /
//! `UNIQ` / `ENUMERATE` / `GROUP_BY` / `JOIN` 都走 `PUSH`)。所以本裁决是
//! **治标**:`RANGE` 恢复可用,但 `MAP` 在 1000+ 元素上仍是平方级。这是
//! 已知且已登记的局限(偏差 D11-012),根因修复(解释器侧的写时复制 / 结构共享)
//! 超出批次 A 范围。
//!
//! ## 形态:门面改名导出,不走 `SPEC`
//!
//! `std.collection` 仍是**一个**模块、**一张**导出面,`IMPORT` 的可见面
//! 与签名/语义/诊断**一字未变** —— ADR-0021 §0.2 明写「层归属变更(下沉 /
//! 上浮)不算破坏性变更」。所以 `RANGE` 的 R2 实现经 M3-0 的 kernel 注入
//! 槽交给门面,门面 `LET(RANGE, _RANGE);` 改名导出 —— 与 `std.test` 的
//! `EXPECT_ERR` 同一形态(那里也是「实现归 R2、门面只改名」)。
//!
//! 本文件因此**不是命名空间**:没有 `SPEC`、不进 `resolve()`、不进附录 A。
//! 它与 `kernels.rs`(模块无关的共享内核)分开,理由同 D11-011:内核代码
//! 该跟它服务的模块放在一起。
//!
//! ## 元数与类型检查为什么留在门面
//!
//! R2 这一份保留**元数检查**(1–3 参)与 `step = 0` 的 `E0038`;`arr` 形态
//! 检查不适用(没有 `arr`)。门面的 `_NEED_ARITY("RANGE", ...)` 仍然会先跑,
//! 两者消息措辞相同(照抄 R2 的 `arity()` 口径:只报上限),所以对外表现
//! 与 M3-1 的 R1 门面**逐字一致** —— 75 条冻结契约用例就是为此存在的。

use wlwl_value::{Outcome, StdHost, Value};

use wlwl_error::{ErrorCode, WlwlError, WlwlResult};

/// `RANGE(n)` / `RANGE(start, end)` / `RANGE(start, end, step)` → `[start, end)`。
///
/// `step = 0` 产生 `E0038`(标准库规范 §5 / 语言规范 §11)。
///
/// 整数溢出**饱和**(停,不抛):`checked_add` 返回 `None` 就停,避免
/// `RANGE(0, MAX, 1)` 死循环。这是 R2 版自陈的取舍 —— R1 版按语言规范
/// §2.2 改抛 `E0035`(偏差 D11-003),而 M5 把 `RANGE` 沉回 R2 时把这个
/// 饱和行为一并带了回来(该处置被撤销的事实见台账 D11-003 补记)。
///
/// [D11-019] 原文写「按 §9.5 的饱和约定」—— 语言规范 §9.5 是**程序入口**,
/// 没有任何饱和约定;§2.2 与 §11 的 `W0015` 那行明写「整数溢出抛 E0035,
/// 没有饱和路径」。故这里不引章节号,只陈述本实现的实际取舍。可达性:
/// 需约 9.2×10^18 次迭代,实际不可达。
pub fn kernel_range(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    // 实参里的 ERR 按 §12.6 透明返回。门面是闭包,调用边界已处理过一次;
    // 这里保留是因为本函数是 `wlwl_std` 的公开 API,可能被直接调用。
    if let Some(e) = args.iter().find_map(|v| match v {
        Value::Err(_) => Some(v.clone()),
        _ => None,
    }) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=3).contains(&args.len()) {
        return Err(arity(host, "RANGE", args.len(), 3));
    }
    let to_i64 = |host: &mut dyn StdHost, name: &str, v: &Value| -> WlwlResult<i64> {
        match v {
            Value::Integer(i) => Ok(*i),
            other => Err(type_err(host, name, "integer", other)),
        }
    };
    let (start, end, step) = match args.len() {
        1 => {
            let n = to_i64(host, "RANGE", &args[0])?;
            (0, n, 1i64)
        }
        2 => {
            let s = to_i64(host, "RANGE", &args[0])?;
            let e = to_i64(host, "RANGE", &args[1])?;
            (s, e, 1i64)
        }
        3 => {
            let s = to_i64(host, "RANGE", &args[0])?;
            let e = to_i64(host, "RANGE", &args[1])?;
            let st = to_i64(host, "RANGE", &args[2])?;
            (s, e, st)
        }
        _ => unreachable!("arity-checked 1..=3"),
    };
    if step == 0 {
        return Err(host.diag(
            ErrorCode::E0038,
            "RANGE: step must be non-zero (got 0)".to_string(),
        ));
    }
    let mut out = Vec::new();
    if step > 0 {
        let mut i = start;
        while i < end {
            out.push(Value::Integer(i));
            // Saturate at i64 boundaries to avoid an infinite loop on
            // `RANGE(0, MAX, 1)`. The collection still terminates; we
            // follow §9.5's overflow-saturation convention by silently
            // stopping instead of W0015-ing (the warning machinery is
            // intended for arithmetic, not iteration counts).
            i = match i.checked_add(step) {
                Some(v) => v,
                None => break,
            };
        }
    } else {
        let mut i = start;
        while i > end {
            out.push(Value::Integer(i));
            i = match i.checked_add(step) {
                Some(v) => v,
                None => break,
            };
        }
    }
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, fn_name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{fn_name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, fn_name: &str, expected: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!(
            "{}: expected {}, got {}",
            fn_name,
            expected,
            crate::value_kind(got),
        ),
    )
}
