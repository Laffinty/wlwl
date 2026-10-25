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

/// `TAKE_ERR(x)` —— **一步式「检查并消费」**。
///
/// * `x` 是 `ERR` ⇒ 返回它的**载荷**，并且**消费**掉那份未消费状态
///   （消费由求值器按 §8.6.1 的「取走型」读模式完成 —— 本函数只负责交还载荷）；
/// * `x` **不是** `ERR` ⇒ 返回 `NULL`（**不**消费任何东西）。
///
/// ## 为什么它必须是 **R2 内核**（不是 R1 门面）
///
/// §8.2 的墙是「实参为 `ERR` ⇒ 不求函数体」，判据是**被调用者的名字**在不在
/// §8.3 的消费者注册表里 ⇒ **R1 门面（闭包）拿不到那个 `ERR`**。
/// `EXPECT_ERR` 是同款形状（`wlwl-std/src/test_native.rs` + `wl/std/test.wll` 门面）。
///
/// ## 它补的是哪条缺口
///
/// 语言册 §8.2 的推论已经写死了唯一合规的写法：「检查并消费一个 `ERR` 值**必须**
/// 在拥有它的作用域内进行 —— 先以注册表消费者提取信息或兜底，再把**提取结果**
/// （普通值）传给其他函数」。
///
/// ⇒ 但现成的注册表消费者**没有一个是「一步式」的**：
/// `IS_ERR` 是**观察型**（§8.6.1）⇒ 只看不带走 ⇒ 漏掉 `ERR_PAYLOAD` 就在作用域
/// 结束时撞 `E0103`；而 `UNWRAP_OR(x, d)` 确实消费，却只给**兜底值**、**丢掉载荷**
/// ⇒ 想要「失败原因」就拿不到。
///
/// ⇒ 本成员把两件事合成**一步**：看一眼 + 取走载荷 + 消费掉那份未消费状态。
///
/// ## 判「是不是错」的两种写法
///
/// ```wlwl
/// LET(p, TAKE_ERR(r));            // p = 载荷（已消费）或 NULL
/// IF(==(TYPE(p), "NULL"), (正常分支), (错误分支，用 p))
/// ```
/// ⚠ 语言里**没有 `IS_NULL` 内建**（只有 `IS_OK` / `IS_ERR` / `BOOL` / `TYPE`）
/// ⇒ 判空只能写 `==(TYPE(x), "NULL")`。
/// ⚠ 另一种写法：`IS_ERR(r)` 在 `TAKE_ERR` 之后**仍然为真**（值还是那个 `ERR`，
///   只是未消费状态被结清）⇒ 可以用它当分支条件，且**不会**再撞 `E0103`。
///
/// ⚠ 唯一的歧义：`x` 是 `ERR` 而**载荷本身是 `NULL`**（`ERR(NULL)`）⇒ 与
///   「`x` 不是 `ERR`」不可区分。载荷为 `NULL` 的 `ERR` 是病态用法。
pub fn kernel_take_err(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "TAKE_ERR", args.len(), 1));
    }
    match &args[0] {
        Value::Err(payload) => Ok(Outcome::normal(payload.as_ref().clone())),
        _ => Ok(Outcome::normal(Value::Null)),
    }
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
