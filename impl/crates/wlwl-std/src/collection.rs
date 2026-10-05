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
//! L0-A-3 归层第二批。**本文件当前归 R2 的成员共 10 个**:`RANGE` +
//! L0-A-2 的四个 + L0-A-3a 的五个(余 17 成员仍 R1,`SORT` / `SORT_BY` /
//! `DEDUP_BY` / `GROUP_BY` / `KEY_BY` 留 L0-A-3b)。
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

use wlwl_value::{values_equal, Outcome, Signal, StdHost, Value};

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

// ─────────────────────────────────────────────────────────────────────
// [v0.11.3 M5 / addendum-00 L0-A-3a] 归层第二批(纯累加器那一半):
// `ENUMERATE` / `ZIP` / `UNIQ` / `FLAT` / `JOIN`
// ─────────────────────────────────────────────────────────────────────
//
// 与 L0-A-2 同一形态(kernel 注入 + 门面改名导出,`EXPORT` 未动)。这一半全是
// **无跨成员依赖的累加器**,没有排序的稳定性/比较器调用次数问题,也没有
// `DICT` 键控问题 —— 后两者(`SORT` / `SORT_BY` / `DEDUP_BY` / `GROUP_BY` /
// `KEY_BY`)留在 L0-A-3b,因为它们各自带一个需要单独裁决的设计点。
//
// 诊断措辞逐字照抄门面(127 条冻结契约逐字比对):
//   元数 E0022  `NAME: function expects <hi> argument(s), got N`
//              —— `ZIP` 报的是**下界 1**(它是 ≥1 元成员,门面
//                 `IF(<(LEN(args), 1), _DIAG_E0022(… STR(1) …))` 写死了 1)
//   实参 E0030  `NAME: expected array, got <kind>`
//              —— `JOIN` 的分隔符是** bespoke 措辞**:
//                 `JOIN: expected string (separator), got <kind>`,不是 `_NEED` 那款

/// `ENUMERATE(arr)` → `[[0, v0], [1, v1], …]`。无回调、无 `ERR` 分支。
pub fn kernel_enumerate(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "ENUMERATE", args.len(), 1));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "ENUMERATE", "array", &args[0]));
    };
    let mut out: Vec<Value> = Vec::with_capacity(arr.len());
    for (i, v) in arr.iter().enumerate() {
        out.push(Value::Array(vec![Value::Integer(i as i64), v.clone()]));
    }
    Ok(Outcome::normal(Value::Array(out)))
}

/// `ZIP(a, b, …)` → 配对至**较短者**。
///
/// **非数组实参按单列处理,不报 `E0030`**:`ZIP(1, 2)` → `[[1, 2]]`
/// (门面把每个实参 `IF(==(TYPE(a), "ARRAY"), a, [a])` 包成单元素列,
/// 而 `minlen` 就是最短列的长度)。这是 R1 现状,**照搬**。
/// 元数下界 1:报 `E0022 … expects 1 argument(s), got 0`。
pub fn kernel_zip(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.is_empty() {
        return Err(arity(host, "ZIP", 0, 1));
    }
    // 非数组实参按**单列**处理(门面 `IF(==(TYPE(a), "ARRAY"), a, [a])`),
    // 故这里统一物化成一列;代价是每列一次克隆,相对 R1 的「每行重建整个
    // out」仍是 O(n·k) 而不是 O(n²k)。
    let cols: Vec<Vec<Value>> = args
        .iter()
        .map(|a| match a {
            Value::Array(v) => v.clone(),
            other => vec![other.clone()],
        })
        .collect();
    let minlen = cols.iter().map(Vec::len).min().unwrap_or(0);
    let mut out: Vec<Value> = Vec::with_capacity(minlen);
    for i in 0..minlen {
        out.push(Value::Array(cols.iter().map(|c| c[i].clone()).collect()));
    }
    Ok(Outcome::normal(Value::Array(out)))
}

/// `UNIQ(arr)` → 按 `==` 去重,**保留首现**。
///
/// ⚠️ **残余仍是平方级,但平方的来源换了**:R1 版同时有两笔平方成本 ——
/// `PUSH` 重建(归层消掉)与 `INDEX(out, v)` 的**线性成员查找**(消不掉)。
/// 后者不能顺手改成哈希:`values_equal` 的语义要求 `1` 与 `1.0` 判等、
/// `TRUE` 与 `1` **不**判等、`Dict` 判等**无序**、闭包判等是**结构**的
/// (Env 的 PartialEq 快照),而 `Value` 没有 `Hash`;用哈希键就得先把
/// 数值规范化(`Float(NaN)` 在 `values_equal` 下与自身**不**等,哈希按位相等
/// 却会合并 —— 那是可观察的行为差异)。
/// ⇒ 本实现保留 `values_equal` 线性扫描。**把 UNIQ 做成线性是一次独立的
/// 设计裁决**(要给 `Value` 定哈希语义),不是归层顺手能带的事。
pub fn kernel_uniq(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "UNIQ", args.len(), 1));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "UNIQ", "array", &args[0]));
    };
    let mut out: Vec<Value> = Vec::with_capacity(arr.len());
    for v in arr.iter() {
        if !out.iter().any(|seen| values_equal(seen, v)) {
            out.push(v.clone());
        }
    }
    Ok(Outcome::normal(Value::Array(out)))
}

/// `FLAT(arr)` → 展平**一层**。
///
/// 门面每个元素二选一:数组走 `CONCAT`、非数组走 `PUSH`;故 `[[]]` 贡献
/// 零个元素、`[1, [2, [3]]]` 得 `[1, 2, [3]]`(**只一层**,内层数组原样保留)。
pub fn kernel_flat(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "FLAT", args.len(), 1));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "FLAT", "array", &args[0]));
    };
    // 先量一遍容量,免得每遇一个嵌套数组就重新分配。
    let cap: usize = arr
        .iter()
        .map(|v| match v {
            Value::Array(inner) => inner.len(),
            _ => 1,
        })
        .sum();
    let mut out: Vec<Value> = Vec::with_capacity(cap);
    for v in arr.iter() {
        match v {
            Value::Array(inner) => out.extend(inner.iter().cloned()),
            other => out.push(other.clone()),
        }
    }
    Ok(Outcome::normal(Value::Array(out)))
}

/// `JOIN(arr, sep)` → 元素经 `STR` 渲染后以 `sep` 连接。
///
/// 分隔符的类型诊断是 **bespoke 措辞**(`expected string (separator)`),
/// 不是 `_NEED` 那款 `expected <what>` —— 照抄,别「顺手统一」。
pub fn kernel_join(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "JOIN", args.len(), 2));
    }
    let Value::Array(arr) = &args[0] else {
        return Err(type_err(host, "JOIN", "array", &args[0]));
    };
    let Value::String(sep) = &args[1] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "JOIN: expected string (separator), got {}",
                crate::value_kind(&args[1])
            ),
        ));
    };
    let mut s = String::new();
    for (i, v) in arr.iter().enumerate() {
        if i > 0 {
            s.push_str(sep);
        }
        s.push_str(&v.display());
    }
    Ok(Outcome::normal(Value::String(s)))
}
