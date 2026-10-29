//! `wlwl:std.sync` — 原子共享容器（W-13 / 需求清单 B-4 的候选 (a)）。
//!
//! ## 它解决什么
//!
//! 多个**并发 handler**（`SPAWN` 出来的任务）要共享可变状态时，语言层**没有**
//! 可用的共享单元（规范并发册 §17.0.1 禁止任务之间通过共享可变 cell 传数据）
//! ⇒ 本模块提供**原子容器**：一个原子计数器 + 一个原子共享字典。
//!
//! ⚠ **本轮只做「不需要挂起」的原子容器** —— `MUTEX` / `CONDITION` /
//!   `SEMAPHORE` / `WAIT_GROUP` 需要「挂起后越过阻塞点继续」（`ADR-0017` Step 3，
//!   未落地）⇒ **不在本模块里**（宁可不做，也不要交一个半吊子的锁）。
//!
//! ## 句柄纪律：**不透明 `INTEGER`**
//!
//! 句柄是**不透明整数**（从 `1` 起、单调递增、**永不复用**）⇒ 用户拿到的整数
//! 没有可推断的含义；`0` 永远不是合法句柄。
//!
//! ⚨ **句柄表里绝不存任何 wlwl `Value`** —— `Value` 是 `Rc` 语义（`!Send`），
//!   放进去整张表就 `!Sync` ⇒ 编译器会直接拒绝。表里只有两样东西：
//!   - 原子计数器（`Arc<AtomicI64>`）；
//!   - 共享字典（`Arc<SharedDict>`），其内容是**镜像类型** `SyncValue` ——
//!     一棵 `Send` 安全的纯数据树（见下）。
//!
//! ## 共享字典的值是**镜像**，不是 `Value`
//!
//! `SHARED_DICT_SET` 存进去的是 `SyncValue`（`Send` 安全的纯数据镜像），
//! **不是** wlwl 的 `Value` ⇒ 取回时由镜像**重建**一个 `Value`。
//!
//! - **可共享**：字符串 / 整数 / 浮点 / 布尔 / `NULL` / 数组 / 字典 / `OK` / `ERR`
//!   （嵌套的字典、数组逐层镜像）。
//! - **不可共享**：闭包（捕获的环境是 `Rc`，跨线程不安全）、原生函数、任务句柄、
//!   通道句柄、类与实例 ⇒ `ERR([kind:"SyncError", …])` **值**。
//!
//! ## 失败口径（**不新增诊断码**）
//!
//! | 情形 | 口径 |
//! |---|---|
//! | 元数不对 | **既有** `E0022` |
//! | 实参类型不对（句柄不是整数、键不是字符串/整数） | **既有** `E0030` |
//! | 句柄不存在 / 句柄**种类**不对（计数器句柄喂给字典成员） | `ERR([kind:"SyncError", op, reason])` **值** |
//! | 值不可共享 | `ERR([kind:"SyncError", op, reason])` **值** |
//! | `COUNTER_ADD` 的 `delta` 为负 | `ERR([kind:"SyncError", op, reason])` **值** |
//!
//! ⚠ 分界线：**实参的 wlwl 类型不对 ⇒ 诊断**（`E0022` / `E0030`，写代码时就
//!   该发现）；**句柄在运行期不存在 / 用错了种类 ⇒ 值**（调用方多半要 `IS_ERR`
//!   分支，而不是让整个程序终止）。

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_error::WlwlResult;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.sync",
    functions: &[
        // ── 原子计数器 ────────────────────────────────────────────────
        ("COUNTER", counter as StdFn),
        ("COUNTER_INC", counter_inc as StdFn),
        ("COUNTER_ADD", counter_add as StdFn),
        ("COUNTER_GET", counter_get as StdFn),
        // ── 原子共享字典 ──────────────────────────────────────────────
        ("SHARED_DICT", shared_dict as StdFn),
        ("SHARED_DICT_SET", shared_dict_set as StdFn),
        ("SHARED_DICT_GET", shared_dict_get as StdFn),
        ("SHARED_DICT_SNAPSHOT", shared_dict_snapshot as StdFn),
    ],
};

// ── 句柄表 ────────────────────────────────────────────────────────────────

/// 句柄表里的一条：**只有 `Send` 数据**。
///
/// ⚨ 这里**没有**、也**不许有** wlwl `Value`（`Rc` 语义 ⇒ `!Send` ⇒ 整表
///   `!Sync`）。计数器是 `Arc<AtomicI64>`；字典是 `Arc<SharedDict>`，其内容
///   是 [`SyncValue`] 镜像。
enum Entry {
    /// 一个**原子**计数器。
    Counter(Arc<AtomicI64>),
    /// 一个**原子**共享字典。
    Dict(Arc<SharedDict>),
}

/// 进程级句柄表。
static HANDLES: LazyLock<Mutex<HashMap<i64, Entry>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// 下一个句柄（从 `1` 起 ⇒ `0` 永远不是合法句柄）。
///
/// ⚠ **单调递增、永不复用** ⇒ 一个过期句柄**永远不会**悄悄指向一个新对象
///   （它只会「不存在」）⇒ 用错句柄是**可诊断**的，不是**静默串号**的。
static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);

/// 登记一个新容器，返回它的句柄。
fn register(entry: Entry) -> i64 {
    let id = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    let mut table = HANDLES.lock().unwrap_or_else(|e| e.into_inner());
    table.insert(id, entry);
    id
}

/// 句柄查找的失败形态。
enum HandleFault {
    /// 句柄**不存在**（没登记过，或来自另一个进程/求值器）。
    Unknown,
    /// 句柄存在，但**种类不对**（拿计数器句柄去问字典成员，等等）。
    WrongKind {
        /// 成员**要**的种类。
        expected: &'static str,
        /// 句柄**实际**的种类。
        got: &'static str,
    },
}

impl HandleFault {
    /// 失败原因（进 `ERR` 载荷的 `reason`）。
    fn reason(&self) -> String {
        match self {
            HandleFault::Unknown => "unknown handle".to_string(),
            HandleFault::WrongKind { expected, got } => {
                format!("the handle is a {got}, but a {expected} is required")
            }
        }
    }
}

fn entry_kind(entry: &Entry) -> &'static str {
    match entry {
        Entry::Counter(_) => "counter",
        Entry::Dict(_) => "shared dict",
    }
}

/// 按句柄取**计数器**（克隆 `Arc` 后立刻放锁 ⇒ 计数**不**在句柄表的锁里做）。
fn counter_by_handle(handle: i64) -> Result<Arc<AtomicI64>, HandleFault> {
    let table = HANDLES.lock().unwrap_or_else(|e| e.into_inner());
    match table.get(&handle) {
        Some(Entry::Counter(c)) => Ok(Arc::clone(c)),
        None => Err(HandleFault::Unknown),
        Some(other) => Err(HandleFault::WrongKind {
            expected: "counter",
            got: entry_kind(other),
        }),
    }
}

/// 按句柄取**共享字典**（克隆 `Arc` 后立刻放锁）。
fn dict_by_handle(handle: i64) -> Result<Arc<SharedDict>, HandleFault> {
    let table = HANDLES.lock().unwrap_or_else(|e| e.into_inner());
    match table.get(&handle) {
        None => Err(HandleFault::Unknown),
        Some(Entry::Dict(d)) => Ok(Arc::clone(d)),
        Some(other) => Err(HandleFault::WrongKind {
            expected: "shared dict",
            got: entry_kind(other),
        }),
    }
}

// ── 共享字典与它的镜像 ─────────────────────────────────────────────────────

/// 共享字典的**键**。
///
/// ⚠ 键域与 wlwl 字典一致：`STRING` / `INTEGER`（规范 §2.1 的封闭键域去掉
///   `INSTANCE` —— 实例是 `Rc` 语义，不可跨线程共享）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SyncKey {
    Str(String),
    Int(i64),
}

/// 共享字典的**值**：**`Send` 安全的纯数据镜像**。
///
/// ⚨ 它**不是** wlwl 的 `Value` —— `Value` 是 `Rc` 语义（`!Send`）⇒ 存进
///   进程级表里会让整表 `!Sync`。镜像只保留「纯数据」，可共享性由
///   [`to_sync`] 把关（闭包 / 原生函数 / 句柄 / 类 ⇒ 不可共享）。
#[derive(Debug, Clone)]
enum SyncValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<SyncValue>),
    /// 字典：保留**插入序**（与 wlwl 字典的插入序一致）。
    Dict(Vec<(SyncKey, SyncValue)>),
    Ok(Box<SyncValue>),
    Err(Box<SyncValue>),
}

/// 一个**原子**共享字典。
///
/// ⚠ 「原子」的粒度是**单键**：一次 `SHARED_DICT_SET` 是一次完整的
///   「读-改-写」（覆盖该键）⇒ 不会被另一个写者**撕裂**（不会写进去半个值）。
///   快照则是**整体**读出（取锁期间的一致副本）⇒ 但快照与并发写之间的
///   先后关系**没有**全局顺序保证（那是并发字典的固有性质，不是本实现的缺陷）。
struct SharedDict {
    /// 键 → 值。
    ///
    /// ⚠ 用 `BTreeMap`（不是 `HashMap`）⇒ 快照的键序**按字典序**、逐次一致
    ///   ⇒ 快照可以当**确定性**的输出比对。
    entries: Mutex<BTreeMap<SyncKey, SyncValue>>,
}

/// wlwl 的键 → 镜像键（键域外 ⇒ `None`）。
fn to_sync_key(v: &Value) -> Option<SyncKey> {
    match v {
        Value::String(s) => Some(SyncKey::Str(s.clone())),
        Value::CellStr(c) => Some(SyncKey::Str(c.as_str())),
        Value::Integer(i) => Some(SyncKey::Int(*i)),
        _ => None,
    }
}

/// 镜像键 → wlwl 的键。
fn from_sync_key(k: &SyncKey) -> Value {
    match k {
        SyncKey::Str(s) => Value::String(s.clone()),
        SyncKey::Int(i) => Value::Integer(*i),
    }
}

/// wlwl 的值 → 镜像（**不可共享** ⇒ `Err(理由)`）。
///
/// ⚨ 「可共享」的判据只有一条：**它能不能被安全地搬到另一个线程**。
///   闭包捕获的环境是 `Rc`（`!Send`）⇒ 不可共享；原生函数 / 任务句柄 /
///   通道句柄 / 类与实例同理。
fn to_sync(v: &Value) -> Result<SyncValue, String> {
    match v {
        Value::Null => Ok(SyncValue::Null),
        Value::Boolean(b) => Ok(SyncValue::Boolean(*b)),
        Value::Integer(i) => Ok(SyncValue::Integer(*i)),
        Value::Float(f) => Ok(SyncValue::Float(*f)),
        Value::String(s) => Ok(SyncValue::String(s.clone())),
        // ⚠ 累加器变体（`Cell` / `CellStr` / `DictCell`）对外**就是**普通的
        //   数组 / 字符串 / 字典 ⇒ 镜像取它们的**内容**，不保留「可写」语义
        //   （共享容器里的值是**快照**，不是活引用）。
        Value::CellStr(c) => Ok(SyncValue::String(c.as_str())),
        Value::Cell(items) => {
            let borrowed = items.borrow();
            let mut out = Vec::with_capacity(borrowed.len());
            for item in borrowed.iter() {
                out.push(to_sync(item)?);
            }
            Ok(SyncValue::Array(out))
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(to_sync(item)?);
            }
            Ok(SyncValue::Array(out))
        }
        Value::Dict(inner) => {
            let mut out = Vec::with_capacity(inner.len());
            for (k, val) in inner.iter() {
                let key = to_sync_key(k).ok_or_else(|| {
                    format!(
                        "a dict key of kind `{}` cannot be shared",
                        crate::value_kind(k)
                    )
                })?;
                out.push((key, to_sync(val)?));
            }
            Ok(SyncValue::Dict(out))
        }
        Value::DictCell(cell) => {
            let inner = cell.inner_rc();
            let mut out = Vec::with_capacity(inner.len());
            for (k, val) in inner.iter() {
                let key = to_sync_key(k).ok_or_else(|| {
                    format!(
                        "a dict key of kind `{}` cannot be shared",
                        crate::value_kind(k)
                    )
                })?;
                out.push((key, to_sync(val)?));
            }
            Ok(SyncValue::Dict(out))
        }
        Value::Ok(inner) => Ok(SyncValue::Ok(Box::new(to_sync(inner)?))),
        Value::Err(inner) => Ok(SyncValue::Err(Box::new(to_sync(inner)?))),
        other => Err(format!(
            "a value of kind `{}` cannot be shared across handlers",
            crate::value_kind(other)
        )),
    }
}

/// 镜像 → wlwl 的值（**总是**成功：镜像里只有纯数据）。
fn from_sync(v: &SyncValue) -> Value {
    match v {
        SyncValue::Null => Value::Null,
        SyncValue::Boolean(b) => Value::Boolean(*b),
        SyncValue::Integer(i) => Value::Integer(*i),
        SyncValue::Float(f) => Value::Float(*f),
        SyncValue::String(s) => Value::String(s.clone()),
        SyncValue::Array(items) => Value::Array(items.iter().map(from_sync).collect()),
        SyncValue::Dict(entries) => Value::dict(
            entries
                .iter()
                .map(|(k, v)| (from_sync_key(k), from_sync(v)))
                .collect(),
        ),
        SyncValue::Ok(inner) => Value::Ok(Box::new(from_sync(inner))),
        SyncValue::Err(inner) => Value::Err(Box::new(from_sync(inner))),
    }
}

// ── 诊断 / 值级错误助手 ───────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, want: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

/// `ERR([kind: "SyncError", op, reason])` —— **值**，不是原生诊断。
///
/// ⚠ 与 `std.log` / `std.fs` / `std.process` **同构**，只是 `kind` 不同：
///   「句柄不存在」「句柄种类不对」「值不可共享」都是**可预期的运行期结果**
///   ⇒ 调用方多半要 `IS_ERR` 分支，而不是让整个程序终止。
fn sync_err(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("SyncError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

fn ok(v: Value) -> WlwlResult<Outcome> {
    Ok(Outcome::normal(v))
}

fn int_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<i64, WlwlError> {
    match v {
        Value::Integer(i) => Ok(*i),
        other => Err(type_err(host, name, "an integer", other)),
    }
}

/// 键参数：**字符串**或**整数**（键域与 wlwl 字典一致，去掉不可共享的实例键）。
fn key_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<SyncKey, WlwlError> {
    match to_sync_key(v) {
        Some(k) => Ok(k),
        None => Err(type_err(host, name, "a string or an integer key", v)),
    }
}

// ── 成员：原子计数器 ───────────────────────────────────────────────────────

/// `COUNTER(initial) -> INTEGER`
///
/// 新建一个**原子**计数器，返回它的**不透明句柄**。
///
/// ⚠ **不幂等**：每次调用都**新建**一个计数器（计数器没有名字 ⇒ 没有可复用
///   的键）⇒ 两次调用给出**两个不同的句柄**、两个**互不相干**的计数器。
pub fn counter(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "COUNTER";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let initial = int_arg(host, NAME, &args[0])?;
    ok(Value::Integer(register(Entry::Counter(Arc::new(
        AtomicI64::new(initial),
    )))))
}

/// `COUNTER_INC(handle, delta) -> INTEGER`
///
/// **原子**自增计数器，返回**新值**。
///
/// ⚨ 原子性是**可观测**的：N 个线程各 `+1` ⇒ 结果**恰好**是 N
///   （先读后写必丢更新 ⇒ 这条由契约测试钉住）。
///
/// ⚠ `delta` **可以是负数**（那是减法）；句柄不存在 / 种类不对 ⇒ **`ERR` 值**。
///
/// ⚠ 溢出按 `i64` **回绕**（与 `std.log` 的 `METRIC_INC` 同款）⇒ 计数溢出到
///   `i64::MAX` 之后会翻成负数（不会中止程序，也不会返回 `ERR`）。
pub fn counter_inc(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "COUNTER_INC";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let delta = int_arg(host, NAME, &args[1])?;
    let counter = match counter_by_handle(handle) {
        Ok(c) => c,
        Err(fault) => return ok(sync_err(NAME, fault.reason())),
    };
    // ⚨ `fetch_add` 返回**旧值** ⇒ 新值 = 旧值 + delta。
    // ⚨ 这一步**必须**是原子的：先读后写在并发下必丢更新。
    let new = counter.fetch_add(delta, Ordering::SeqCst) + delta;
    ok(Value::Integer(new))
}

/// `COUNTER_ADD(handle, delta) -> INTEGER`
///
/// **原子**地把 `delta` 加到计数上，返回**新值**（与 `COUNTER_INC` 同一实现）。
///
/// ⚠ 与 `COUNTER_INC` 的**唯一**差别：`delta` **必须 ≥ 0**（负数 ⇒
///   **`ERR` 值**）⇒ `COUNTER_ADD` 是「**单调**计数器」那一族：它保证计数
///   **只增不减**，适合「累计处理了多少条」这类语义（负增量在那种语义里
///   一定是数据错了）。
///
/// ⚠ 原子性同上：N 个线程各 `+1` ⇒ 恰好 N。
pub fn counter_add(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "COUNTER_ADD";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let delta = int_arg(host, NAME, &args[1])?;
    if delta < 0 {
        return ok(sync_err(
            NAME,
            format!("the delta must not be negative, got {delta}"),
        ));
    }
    let counter = match counter_by_handle(handle) {
        Ok(c) => c,
        Err(fault) => return ok(sync_err(NAME, fault.reason())),
    };
    // ⚨ 同样是 `fetch_add`（**不是**先读后写）⇒ 并发下不丢更新。
    let new = counter.fetch_add(delta, Ordering::SeqCst) + delta;
    ok(Value::Integer(new))
}

/// `COUNTER_GET(handle) -> INTEGER`
///
/// 读计数器的**当前值**（原子读）。
///
/// ⚠ 这是一次**快照**：读到的值可能**立刻**就过时了（别的 handler 正在自增）
///   ⇒ 要「读-改-写」请用 `COUNTER_INC` / `COUNTER_ADD`（那才是原子的）。
pub fn counter_get(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "COUNTER_GET";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let counter = match counter_by_handle(handle) {
        Ok(c) => c,
        Err(fault) => return ok(sync_err(NAME, fault.reason())),
    };
    ok(Value::Integer(counter.load(Ordering::SeqCst)))
}

// ── 成员：原子共享字典 ─────────────────────────────────────────────────────

/// `SHARED_DICT() -> INTEGER`
///
/// 新建一个**原子**共享字典，返回它的**不透明句柄**。
///
/// ⚠ **不幂等**：每次调用都**新建**一个字典 ⇒ 两次调用给出两个不同的句柄。
pub fn shared_dict(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "SHARED_DICT";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), 0));
    }
    ok(Value::Integer(register(Entry::Dict(Arc::new(
        SharedDict {
            entries: Mutex::new(BTreeMap::new()),
        },
    )))))
}

/// `SHARED_DICT_SET(handle, key, value) -> NULL`
///
/// **单键原子写**：把 `value` 写进 `key`（**覆盖**旧值），返回 `NULL`。
///
/// ⚠ 「原子」的粒度是**这一次写**：一个并发写者**看不到**半个值（写进去的
///   要么是旧值、要么是新值）。
///
/// ⚠ `value` 必须是**可共享**的纯数据（闭包 / 原生函数 / 句柄 / 类 ⇒
///   **`ERR` 值**）；嵌套的数组、字典逐层镜像。
///
/// ⚠ 写进去的是**快照**，不是活引用 ⇒ 之后改动原值**不会**反映到字典里。
pub fn shared_dict_set(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "SHARED_DICT_SET";
    if args.len() != 3 {
        return Err(arity(host, NAME, args.len(), 3));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let key = key_arg(host, NAME, &args[1])?;
    let value = match to_sync(&args[2]) {
        Ok(v) => v,
        Err(reason) => return ok(sync_err(NAME, reason)),
    };
    let dict = match dict_by_handle(handle) {
        Ok(d) => d,
        Err(fault) => return ok(sync_err(NAME, fault.reason())),
    };
    let mut entries = dict.entries.lock().unwrap_or_else(|e| e.into_inner());
    entries.insert(key, value);
    ok(Value::Null)
}

/// `SHARED_DICT_GET(handle, key) -> VALUE`
///
/// 读一个键的值；**键不存在 ⇒ `NULL`**。
///
/// ⚠ 与「键存在但值就是 `NULL`」**不可区分**（两者都给 `NULL`）—— 与 wlwl
///   字典读缺省键的行为同款。
pub fn shared_dict_get(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "SHARED_DICT_GET";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let key = key_arg(host, NAME, &args[1])?;
    let dict = match dict_by_handle(handle) {
        Ok(d) => d,
        Err(fault) => return ok(sync_err(NAME, fault.reason())),
    };
    let entries = dict.entries.lock().unwrap_or_else(|e| e.into_inner());
    match entries.get(&key) {
        Some(v) => ok(from_sync(v)),
        None => ok(Value::Null),
    }
}

/// `SHARED_DICT_SNAPSHOT(handle) -> DICT`
///
/// **整体**读出字典（一个普通 wlwl 字典）。
///
/// ⚠ 快照的**键序按字典序**（`BTreeMap` ⇒ 逐次一致）⇒ 快照可以当**确定性**
///   的输出比对。
///
/// ⚠ 快照是**一个时刻**的一致副本：并发写**可能**在快照之前或之后生效
///   ⇒ 快照**不是**全局顺序一致的视图（那需要版本号，本轮不做）。
pub fn shared_dict_snapshot(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "SHARED_DICT_SNAPSHOT";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let dict = match dict_by_handle(handle) {
        Ok(d) => d,
        Err(fault) => return ok(sync_err(NAME, fault.reason())),
    };
    let entries = dict.entries.lock().unwrap_or_else(|e| e.into_inner());
    let snapshot: Vec<(Value, Value)> = entries
        .iter()
        .map(|(k, v)| (from_sync_key(k), from_sync(v)))
        .collect();
    ok(Value::dict(snapshot))
}
