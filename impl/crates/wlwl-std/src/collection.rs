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
//! ## [v0.11.3 M5 / addendum-00 L0-A-2] 追加:归层第一批四个
//!
//! L0-A-1 把 `std.collection` 的规模成本按 1k/2k/4k/8k 四档测完
//! ([`../../benches/baseline.txt`](../../benches/baseline.txt) 的 **L0 段**),
//! 五组(`r1_build` + `MAP`/`FILTER`/`CHUNK`/`WINDOW`)**全部平方级坐实**
//! —— 每元素成本随尺寸上涨,`chunk`/`window` 的高档已越过 4。
//! 业主据此把 `MAP` / `FILTER` / `CHUNK` / `WINDOW` 四个归 R2
//! (每元素成本最高的四个,恰好覆盖三种形态:逐元素闭包 / 条件追加 /
// 每轮 `SLICE`)。**形态与 `RANGE` 相同**:kernel 注入 + 门面改名导出,
// `EXPORT` 一字不动 ⇒ 成员面变化 0。
//!
//! 本次同时挖出**第二个**平方级来源(与 `PUSH` 无关):
//! `Value::Dict` 是 `Vec<(Value,Value)>` + `dict_lookup` 线性扫描 +
//! `builtin_index_set` 每次全量克隆 ⇒ `KEY_BY` / `GROUP_BY` / `DEDUP_BY`
//! 受它管。归层**不必**改 `Value::Dict` 的表示(那会毁掉「插入序 = 可观察序」),
//! R2 侧用「内部 `HashMap` + 末次物化」即可 —— 见 [`ADR-0025`] Q1。
//!
//! [`ADR-0025`]: ../../../../docs/adr/0025-array-quadratic-cost-routes.md
//!
//! ## 为什么实测只沉了 `RANGE`(以及它的局限)
//!
//! v0.11 M5 的实测显示**所有建数组的成员**都有同一成本剖面(`MAP` / `FILTER` /
//! `FLAT` / `UNIQ` / `ENUMERATE` / `GROUP_BY` / `JOIN` 都走 `PUSH`)。所以 M5 那一轮
//! 是**治标**:`RANGE` 恢复可用,但 `MAP` 在 1000+ 元素上仍是平方级。这是
//! 已知且已登记的局限(偏差 D11-012),根因修复(解释器侧的写时复制 / 结构共享)
//! 超出批次 A 范围。**L0-A-2 / A-3 就是来消这个治标局限的。**
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

use wlwl_value::{Outcome, Signal, StdHost, Value};

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

// ─────────────────────────────────────────────────────────────────────
// [v0.11.3 M5 / addendum-00 L0-A-2] 归层第一批:`MAP` / `FILTER` /
// `CHUNK` / `WINDOW` 四个成员沉 R2
// ─────────────────────────────────────────────────────────────────────
//
// ## 形态:kernel 注入 + 门面改名导出(**不走 `SPEC`**)
//
// 沿本文件既有的 `RANGE` 先例(见文件头「形态」段):`std.collection` 是
// **混合模块**,R2 实现经 `StdSource.kernels` 注入,门面 `LET(MAP, _MAP);`
// 改名导出。**`EXPORT` 一字不动** ⇒ 成员面变化为 0(§0 非目标),附录 A /
// `stdlib_appendix_a_sync` / `collection_contract` 的成员集全部不受影响。
//
// > **[L0-A-2 实施更正]** addendum-00 §5 落点 3 写的是「成员从 R1 `EXPORT`
// > 移到 `SPEC.functions`,**两侧都要**」。**本批不采用** —— `SPEC` 形态要求一个
// > 模块同时被 R1 `EXPORT` 与 R2 `SPEC` 两套注册表认领,而该合并路径在仓里
// > **不存在**(全仓只有本文件 / `math` / `test` 三个混合模块,一律走 kernel
// > 注入),凭空造它要动 `resolve()` + 附录 A 生成 + 契约成员集对拍,与
// > 「成员面变化必须为 0」的非目标直接冲突。按本仓既有形态执行,此处登记。
//
// ## 诊断措辞:逐字复刻 R1 门面
//
// 127 条冻结契约(`collection_contract.rs`)逐字比对诊断码 + 消息,故下列
// 措辞**照抄** `wl/std/collection.wll` 的 `_NEED_ARITY` / `_NEED_ARR` /
// `_NEED_FN` / `_NEED_INT` / `_NEED_SIZE` 与 `FILTER` 的谓词检查:
//
//   元数    E0022  `NAME: function expects 2 argument(s), got N`
//   实参形态 E0030 `NAME: expected array, got <kind>`
//   回调    **E0020** `NAME: callback is not callable (got <kind>)`  ← 注意是 E0020
//   尺寸    E0030  `NAME: expected integer, got <kind>` / `NAME: size must be >= 1, got N`
//   谓词    E0030  `FILTER: predicate must return BOOLEAN, got <kind>`
//
// 种类名统一走 `crate::value_kind`(≡ `wlwl_value::type_name`),与 R1 注入的
// `_KIND` 同一个单源。
//
// ## 回调挂起(suspension):透传信号并**丢弃累加器**
//
// 2026-10-05 实测(debug 构建,端到端):`MAP([1,2,3], FUN((x), (YIELD(); …)))`
// 在 `SCOPE` + `SPAWN` 下**于第一个 `YIELD` 处中止**,回调的挂起点之后的代码
// 一次都没跑(副作用探针:`seen` 为 `[]`,对照无 `YIELD` 时为 `[1,2,3]`),
// 整调结果为 `NULL`。⇒ R1 的 `WHILE` 循环**不**在挂起后恢复。
//
// 故本实现与 R1 **行为等价**地:回调返回挂起信号时,立刻把信号原样带回,
// 丢弃已攒的累加器。**不得**消费信号继续跑(那会改变可观察行为),也**不得**
// 把挂起点的值当普通结果返回 —— D11-019 记录过后者造成过一条**假通过**
// (测试体里 `YIELD()` 之后的断言一次没跑却记 `passed = TRUE`)。

/// `TYPE(v) == "FUNCTION"` 的 R2 对应(`wlwl-value` 的 `value_type_name`
/// 把 `Closure` 与 `NativeFn` 都归为 `"FUNCTION"`)。
fn is_callable(v: &Value) -> bool {
    matches!(v, Value::Closure { .. } | Value::NativeFn { .. })
}

/// `_NEED_FN` 的 R2 对应。**码是 `E0020` 不是 `E0030`** —— 照抄门面。
fn not_callable(host: &mut dyn StdHost, fn_name: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0020,
        format!(
            "{fn_name}: callback is not callable (got {})",
            crate::value_kind(got)
        ),
    )
}

/// 回调挂起 ⇒ 原样带回信号、丢弃累加器(见上「回调挂起」段)。
fn propagate_signal(signal: Signal) -> Outcome {
    Outcome {
        value: Value::Null,
        signal,
    }
}

/// `MAP(arr, f)` → `[f(v), …]`。
///
/// **与 `FILTER` 的两处不对称是 R1 现状,照搬不「修正」**:
/// ① 回调返 `ERR` 时 **MAP 不提前退出** —— 它把 `err` 记下后继续跑完剩余
///    元素,**最后一个** `ERR` 覆盖前面的(门面 `IF(IS_ERR(r), SET(err, r), …)`
///    没有 `SET(i, LEN(arr))`,而 `FILTER` 有);
/// ② 回调返 `OK(v)` 时 `OK(v)` 本身被推进结果数组(只挡 `ERR`)。
pub fn kernel_map(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "MAP", args.len(), 2));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "MAP", "array", &args[0]));
    };
    let f = &args[1];
    if !is_callable(f) {
        return Err(not_callable(host, "MAP", f));
    }
    let mut out: Vec<Value> = Vec::with_capacity(arr.len());
    let mut err: Option<Value> = None;
    for v in arr.iter() {
        let r = host.call(f, vec![v.clone()], "MAP")?;
        if r.signal != Signal::None {
            return Ok(propagate_signal(r.signal));
        }
        match r.value {
            Value::Err(_) => err = Some(r.value),
            other => out.push(other),
        }
    }
    Ok(Outcome::normal(match err {
        Some(e) => e,
        None => Value::Array(out),
    }))
}

/// `FILTER(arr, f)` → `[v for v in arr if f(v)]`。
///
/// **ERR 处理与 `MAP` 相同**:回调返 `ERR` 时**不提前退出** —— 记下 `err` 后
/// 继续跑完剩余元素,**最后一个** `ERR` 覆盖前面的(门面
/// `IF(IS_ERR(r), SET(err, r), …)` 里没有 `SET(i, LEN(arr))`)。返非
/// `BOOLEAN` ⇒ `E0030` 上抛(门面 `_DIAG_E0030`,该诊断**不**记入 `err`)。
pub fn kernel_filter(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "FILTER", args.len(), 2));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "FILTER", "array", &args[0]));
    };
    let f = &args[1];
    if !is_callable(f) {
        return Err(not_callable(host, "FILTER", f));
    }
    let mut out: Vec<Value> = Vec::new();
    let mut err: Option<Value> = None;
    for v in arr.iter() {
        let r = host.call(f, vec![v.clone()], "FILTER")?;
        if r.signal != Signal::None {
            return Ok(propagate_signal(r.signal));
        }
        match r.value {
            Value::Err(_) => err = Some(r.value),
            Value::Boolean(true) => out.push(v.clone()),
            Value::Boolean(false) => {}
            other => {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!(
                        "FILTER: predicate must return BOOLEAN, got {}",
                        crate::value_kind(&other)
                    ),
                ));
            }
        }
    }
    Ok(Outcome::normal(match err {
        Some(e) => e,
        None => Value::Array(out),
    }))
}

/// `CHUNK(arr, n)` → 定长分块,尾块可短;`n > LEN(arr)` 得一块;空数组得 `[]`。
/// `n < 1` 报 `E0030`(**不**照 `TAKE` 那套「负数给 `[]`」—— 门面 `_NEED_SIZE`
/// 明写理由:两条都悄悄给 `[]` 的话,「我的数组怎么空了」比一条诊断难查)。
///
/// 末块的 `end` 显式钳到 `LEN(arr)`:门面注释明写「钳制行为不该由『SLICE 恰好
/// 也钳得住』来兜底」,故这里也显式钳,不依赖切片越界被拒。
pub fn kernel_chunk(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "CHUNK", args.len(), 2));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "CHUNK", "array", &args[0]));
    };
    let n = match &args[1] {
        Value::Integer(i) => *i,
        other => return Err(type_err(host, "CHUNK", "integer", other)),
    };
    if n < 1 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("CHUNK: size must be >= 1, got {n}"),
        ));
    }
    let n = n as usize;
    let len = arr.len();
    // 每块 1 次分配;块数 = ceil(len / n)。
    let mut out: Vec<Value> = Vec::with_capacity(len.div_ceil(n));
    let mut i = 0usize;
    while i < len {
        let end = i.saturating_add(n).min(len);
        out.push(Value::Array(arr[i..end].to_vec()));
        i = end;
    }
    Ok(Outcome::normal(Value::Array(out)))
}

/// `WINDOW(arr, n)` → 步长 1 滑动窗口,共 `LEN(arr) - n + 1` 个;
/// `n > LEN(arr)` 得 `[]`(**与 `CHUNK` 方向相反**,这是定义决定的,不是笔误)。
///
/// 输出本身就是 Θ(n·k),故**归层后仍不是 O(n)**,只是去掉了 `PUSH` 带来的
/// 额外 n 因子(实测指数由 ~n^2.4 降到 Θ(n·k))。要真正线性窗口得惰性化,
/// 那是 `std.iter` 的事(规范 §14,阻塞于批次 B)。
pub fn kernel_window(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "WINDOW", args.len(), 2));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "WINDOW", "array", &args[0]));
    };
    let n = match &args[1] {
        Value::Integer(i) => *i,
        other => return Err(type_err(host, "WINDOW", "integer", other)),
    };
    if n < 1 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("WINDOW: size must be >= 1, got {n}"),
        ));
    }
    let n = n as usize;
    let len = arr.len();
    let mut out: Vec<Value> = Vec::new();
    if n <= len {
        out.reserve(len - n + 1);
        for i in 0..=(len - n) {
            out.push(Value::Array(arr[i..i + n].to_vec()));
        }
    }
    Ok(Outcome::normal(Value::Array(out)))
}
