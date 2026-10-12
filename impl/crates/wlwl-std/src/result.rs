//! `wlwl:std.result` 的 **R2 内核** —— v0.11.3 M7 · addendum-11 W-04。
//!
//! 本文件只放 `RESULT_ALL` / `RESULT_ANY` 两个成员;同模块的
//! `RESULT_MAP` / `AND_THEN` / `FLATTEN` / `NULL_OR` 仍是 `wl/std/result.wll`
//! 里的纯 R1(零 kernel)。**形态与 `std.collection` 归层那十个成员相同**:
//! kernel 注入 + 门面改名导出,`EXPORT` 里仍是用户可见的名字。
//!
//! ## 为什么这两个必须归 R2(实测,不是偏好)
//!
//! 两者都要把 `n` 个 `OK` 的载荷**收集成一个新数组**。wlwl 数组不可变,
//! R1 的累加链 `SET(acc, +(acc, [v]))` 每次复制整个数组 —— 这正是 M5
//! 把 `std.collection` 的十个建数组类成员逐个迁 R2 的同一条理由。
//! 实测 R1 累加链的规模曲线(本机、单线程、含进程启动):
//!
//! ```text
//! n=250     27 ms
//! n=500     40 ms
//! n=1000    89 ms
//! n=2000   259 ms
//! n=4000   765 ms      ← 每翻一倍约 ×3,且还在变陡
//! ```
//!
//! 标准库不该带一条这样的曲线。`NULL_OR` 不建数组(只是二选一),故留 R1。
//!
//! ## 为什么内核**能**看见容器里的 `ERR`,而门面式的逐元素闭包不能
//!
//! 语言规范 §8.2 的墙是「**实参**为 `ERR` ⇒ 不求函数体」,判据是被调用者的
//! 名字在不在 §8.3 注册表里。**数组不是 `ERR`**,所以整个数组原样到达内核,
//! 内核逐个读 `Value::Err` 判定即可,自始至终没有任何闭包接到 `ERR`。
//!
//! ⚠ 反例(实测):把容器里的元素喂给任何闭包都会撞墙 ——
//! `ANY(rs, FUN((x), IS_ERR(x)))` 与 `MAP(rs, FUN((x), x))` 都得 `E0102`,
//! 因为逐元素调用本身就是一次带 `ERR` 实参的调用。本文件刻意**不接受**
//! 谓词参数,正是为了不踩这条。
//!
//! 依据:`ADR-0029` D6、stdlib 规范 §22.2 / §22.5。

use wlwl_error::{ErrorCode, WlwlError, WlwlResult};
use wlwl_value::{Outcome, StdHost, Value};

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

/// 取数组实参(只校验元数与类型;元素校验在各自的循环里做,避免重复扫描)。
fn expect_array<'a>(
    host: &mut dyn StdHost,
    fn_name: &str,
    args: &'a [Value],
) -> WlwlResult<&'a [Value]> {
    if args.len() != 1 {
        return Err(arity(host, fn_name, args.len(), 1));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, fn_name, "array", &args[0]));
    };
    Ok(arr)
}

/// 元素必须是 `RESULT`。
///
/// ⚠ **非 `RESULT` 元素报 `E0030`**,与 `RESULT_MAP` 的拒绝面一致
/// (`UNWRAP_OR` 对非 `RESULT` 产生 `E0030`)。规范 §22.1 把这条写成
/// 「实参非 `RESULT` 且非 `ERR` 时报 `E0030`」,`RESULT_ALL` / `RESULT_ANY`
/// 是同一条口径的集合版本。
fn element_must_be_result(host: &mut dyn StdHost, fn_name: &str, v: &Value) -> WlwlResult<()> {
    if matches!(v, Value::Ok(_) | Value::Err(_)) {
        Ok(())
    } else {
        Err(type_err(host, fn_name, "RESULT", v))
    }
}

/// `RESULT_ALL(rs)` → 全部 `OK` 时给 `OK(解包后的数组)`;任一 `ERR` 给**第一个**
/// `ERR`(扫描顺序 = 数组顺序)。
///
/// ⚠ **短路**:遇到第一个 `ERR` 立刻返回,**不再看后面的元素**。
pub fn kernel_result_all(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    let arr = expect_array(host, "RESULT_ALL", &args)?;
    let mut out: Vec<Value> = Vec::with_capacity(arr.len());
    for v in arr {
        element_must_be_result(host, "RESULT_ALL", v)?;
        if let Value::Err(_) = v {
            return Ok(Outcome::normal(v.clone()));
        }
        if let Value::Ok(inner) = v {
            out.push(inner.as_ref().clone());
        }
    }
    Ok(Outcome::normal(Value::Ok(Box::new(Value::Array(out)))))
}

/// `RESULT_ANY(rs)` → 任一 `OK` 时给**第一个** `OK`(原样,不拆壳);
/// 全 `ERR` 时给**第一个** `ERR`;**空数组**给 `OK(NULL)`。
///
/// ⚠ **短路**:扫到第一个 `OK` 立刻返回。全 `ERR` 时扫到末尾才知道
/// 「没有 OK 但有 ERR」,取数组首元素即为第一个 `ERR`。
pub fn kernel_result_any(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    let arr = expect_array(host, "RESULT_ANY", &args)?;
    for v in arr {
        element_must_be_result(host, "RESULT_ANY", v)?;
        if let Value::Ok(_) = v {
            return Ok(Outcome::normal(v.clone()));
        }
    }
    // 没有 OK:空数组给 `OK(NULL)`(「没有候选」不是错误),否则给第一个 ERR。
    Ok(Outcome::normal(match arr.first() {
        Some(first) => first.clone(),
        None => Value::Ok(Box::new(Value::Null)),
    }))
}
