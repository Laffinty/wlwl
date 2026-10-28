//! `wlwl:std.sync` 成员契约 —— W-13（原子共享容器 / 需求清单 B-4 的候选 (a)）。
//!
//! ## 为什么**不走 IMPORT**
//!
//! `std.sync` 这个命名空间的**登记**（`wlwl-std/src/lib.rs` 的
//! `pub mod sync;` + `resolve()` 一行 + `NATIVE_SPECS` 一行）是**集成者**的活
//! ⇒ 走 `IMPORT` 在登记之前会得到 `E0040 module not found`。
//! ⇒ 本文件**直接拿 `StdFn` 调**（`wlwl_std::resolve("wlwl:std.sync")` 拿到的
//! `ModuleSpec` 的成员表），与 `log_contract.rs` **同款**。
//!
//! ⚠ **因此本文件验的是「成员的行为」，不是「命名空间的登记」** ——
//!   登记由集成者统一做，并由 `stdlib_mirror.rs` / 附录 A 镜像那几道闸守。
//!
//! ## 本模块的进程级状态
//!
//! 句柄表是**进程级**的 ⇒ 本文件的用例**各自建自己的容器**（不共享句柄）
//! ⇒ 用例之间互不干扰（不需要 `log_contract.rs` 那把全局闸）。

use std::sync::{Mutex, MutexGuard};

use wlwl_error::ErrorCode;
use wlwl_std::{Outcome, StdCtx, StdHost, Value};

/// 用例之间的**全局闸**（句柄表虽是进程级的，但本文件让每个用例自建容器 ⇒
/// 闸只用来把「并发用例」与其它用例隔开，避免线程调度噪声影响断言）。
static GATE: Mutex<()> = Mutex::new(());

/// 取闸。
fn gate() -> MutexGuard<'static, ()> {
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// 命名空间的 `SPEC`（**实现真相**）。
///
/// ⚠ 走 `resolve()`（与 `net_contract.rs` / `log_contract.rs` 同款）⇒ 本文件
///   **不依赖** `wlwl_std::sync` 这个 Rust 路径的可见性 ⇒ 登记没做时，本文件
///   **编译得过**（不会把整个 `wlwl-eval` 测试集拖垮）。
fn spec() -> &'static wlwl_std::ModuleSpec {
    match wlwl_std::resolve("wlwl:std.sync") {
        Some(wlwl_std::StdBackend::Native(spec)) => spec,
        Some(wlwl_std::StdBackend::Lang(_)) => {
            panic!("wlwl:std.sync must be an R2 native module, not an R1 source module")
        }
        None => panic!(
            "wlwl:std.sync must resolve — the namespace registration is the integrator's step \
             (wlwl-std/src/lib.rs: `pub mod sync;` + the `resolve` arm + `NATIVE_SPECS`)"
        ),
    }
}

/// 按名字取成员（`StdFn`）。
fn member(name: &str) -> wlwl_std::StdFn {
    spec()
        .functions
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .unwrap_or_else(|| panic!("wlwl:std.sync has no member {name}"))
}

/// 手写宿主。本模块不回调宿主。
struct Host(StdCtx);

impl StdHost for Host {
    fn ctx(&mut self) -> &mut StdCtx {
        &mut self.0
    }
    fn call(
        &mut self,
        _f: &Value,
        _a: Vec<Value>,
        _n: &str,
    ) -> Result<Outcome, wlwl_error::WlwlError> {
        panic!("std.sync has no member that calls back into the host")
    }
    fn diag(&mut self, code: ErrorCode, message: String) -> wlwl_error::WlwlError {
        wlwl_error::WlwlDiagnostic::new(code, message, wlwl_error::Location::point("<test>", 0, 0))
            .into()
    }
}

fn host() -> Host {
    Host(StdCtx::default())
}

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

fn i(v: i64) -> Value {
    Value::Integer(v)
}

/// 调一个成员并取它的**值**（失败即 panic —— 契约用例不该静默吞掉失败）。
fn call(name: &str, host: &mut Host, args: Vec<Value>) -> Value {
    let out = member(name)(host, args).unwrap_or_else(|e| {
        panic!("{name} must return a value, got {e:?}");
    });
    out.value
}

/// 调一个成员并取它的**句柄**（`INTEGER`）。
fn call_handle(name: &str, host: &mut Host, args: Vec<Value>) -> i64 {
    let v = call(name, host, args);
    let Value::Integer(handle) = v else {
        panic!("{name} must return an INTEGER handle, got {v:?}")
    };
    assert!(handle > 0, "the handle must be a positive id, got {handle}");
    handle
}

/// 一个**不可共享**的值（原生函数：闭包 / 句柄 / 类同理）。
fn not_shareable() -> Value {
    Value::NativeFn {
        name: "not_shareable".to_string(),
        invoke: stub_native,
    }
}

/// `not_shareable` 用的桩原生函数（本模块不回调宿主 ⇒ 永不真的被调）。
fn stub_native(
    _host: &mut dyn StdHost,
    _args: Vec<Value>,
) -> Result<Outcome, wlwl_error::WlwlError> {
    Ok(Outcome::normal(Value::Null))
}

/// 从 `ERR` 载荷里取一个**字符串**字段。
fn err_field(payload: &Value, key: &str) -> Option<String> {
    let Value::Err(inner) = payload else {
        return None;
    };
    let Value::Dict(d) = &**inner else {
        return None;
    };
    d.iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == key))
        .and_then(|(_, v)| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
}

/// 取一个 `DICT` 值里的某个键。
fn dict_field<'a>(dict: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Dict(d) = dict else {
        panic!("expected a DICT, got {dict:?}");
    };
    d.iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == key))
        .map(|(_, v)| v)
}

/// 取一个 `DICT` 的键序（按插入序）。
fn dict_keys(dict: &Value) -> Vec<String> {
    let Value::Dict(d) = dict else {
        panic!("expected a DICT, got {dict:?}");
    };
    d.iter()
        .map(|(k, _)| match k {
            Value::String(s) => s.clone(),
            other => panic!("a key must be a string, got {other:?}"),
        })
        .collect()
}

// ── 成员面 ────────────────────────────────────────────────────────────────

/// 成员面**冻结**：8 个成员、名字一字不差。
///
/// ⚠ 少一个 ⇒ 集成者的登记清单与实现对不上；多一个 ⇒ 同上。
///   （这是**唯一**一处把「本项的成员数」钉死的地方。）
#[test]
fn the_member_set_is_exactly_eight() {
    let names: Vec<&str> = spec().functions.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        vec![
            "COUNTER",
            "COUNTER_INC",
            "COUNTER_ADD",
            "COUNTER_GET",
            "SHARED_DICT",
            "SHARED_DICT_SET",
            "SHARED_DICT_GET",
            "SHARED_DICT_SNAPSHOT",
        ],
        "wlwl:std.sync member set changed"
    );
}

// ── 计数器 ────────────────────────────────────────────────────────────────

/// 新计数器从**初始值**开始。
#[test]
fn a_counter_starts_at_its_initial_value() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("COUNTER", &mut h, vec![i(7)]);
    assert_eq!(call("COUNTER_GET", &mut h, vec![i(handle)]), i(7));
}

/// `COUNTER_INC` 返回**新值**（不是旧值）。
#[test]
fn the_counter_increment_returns_the_new_value() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("COUNTER", &mut h, vec![i(0)]);
    assert_eq!(call("COUNTER_INC", &mut h, vec![i(handle), i(3)]), i(3));
    assert_eq!(call("COUNTER_INC", &mut h, vec![i(handle), i(4)]), i(7));
    assert_eq!(call("COUNTER_GET", &mut h, vec![i(handle)]), i(7));
}

/// `COUNTER_INC` 的 `delta` **可以是负数**（那是减法）。
#[test]
fn the_counter_increment_accepts_a_negative_delta() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("COUNTER", &mut h, vec![i(10)]);
    assert_eq!(call("COUNTER_INC", &mut h, vec![i(handle), i(-4)]), i(6));
    assert_eq!(call("COUNTER_GET", &mut h, vec![i(handle)]), i(6));
}

/// ⚨ **验收判据**：并发自增**精确**等于请求总数（**无丢失更新**）。
///
/// N 个线程各 `+1` M 次 ⇒ 计数**恰好**是 N×M。
///
/// ⚠ 这是**原子性**的可观测证据：把 `COUNTER_INC` 的 `fetch_add` 换成
///   「先读后写」，并发下必丢更新 ⇒ 这条断言**必红**（反向守卫）。
#[test]
fn the_counter_increment_is_exact_under_concurrency() {
    const THREADS: usize = 8;
    const PER_THREAD: i64 = 500;

    let _g = gate();
    let mut h = host();
    let handle = call_handle("COUNTER", &mut h, vec![i(0)]);

    // 闸门 ⇒ 线程**同时**开始争抢同一个计数器（最大化丢更新的概率）。
    let start = std::sync::Arc::new(std::sync::Barrier::new(THREADS));
    let mut joins = Vec::new();
    for _ in 0..THREADS {
        let handle = handle;
        let start = std::sync::Arc::clone(&start);
        joins.push(std::thread::spawn(move || {
            let mut h = host();
            start.wait();
            for _ in 0..PER_THREAD {
                let out =
                    member("COUNTER_INC")(&mut h, vec![Value::Integer(handle), Value::Integer(1)])
                        .expect("COUNTER_INC returns a value");
                assert!(matches!(out.value, Value::Integer(_)));
            }
        }));
    }
    for j in joins {
        j.join().expect("thread");
    }

    assert_eq!(
        call("COUNTER_GET", &mut h, vec![i(handle)]),
        i((THREADS as i64) * PER_THREAD),
        "a lost update means the increment was not atomic"
    );
}

/// `COUNTER_ADD`（单调那一族）**同样**是原子的。
#[test]
fn the_counter_add_is_exact_under_concurrency() {
    const THREADS: usize = 8;
    const PER_THREAD: i64 = 250;

    let _g = gate();
    let mut h = host();
    let handle = call_handle("COUNTER", &mut h, vec![i(0)]);

    let start = std::sync::Arc::new(std::sync::Barrier::new(THREADS));
    let mut joins = Vec::new();
    for _ in 0..THREADS {
        let handle = handle;
        let start = std::sync::Arc::clone(&start);
        joins.push(std::thread::spawn(move || {
            let mut h = host();
            start.wait();
            for _ in 0..PER_THREAD {
                member("COUNTER_ADD")(&mut h, vec![Value::Integer(handle), Value::Integer(2)])
                    .expect("COUNTER_ADD returns a value");
            }
        }));
    }
    for j in joins {
        j.join().expect("thread");
    }

    assert_eq!(
        call("COUNTER_GET", &mut h, vec![i(handle)]),
        i((THREADS as i64) * PER_THREAD * 2),
        "a lost update means the add was not atomic"
    );
}

/// `COUNTER_ADD` 的 `delta` **必须 ≥ 0** ⇒ 负数是 **`ERR` 值**。
#[test]
fn the_counter_add_rejects_a_negative_delta() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("COUNTER", &mut h, vec![i(5)]);

    let v = call("COUNTER_ADD", &mut h, vec![i(handle), i(-1)]);
    let Value::Err(_) = v else {
        panic!("a negative delta must be an ERR value, got {v:?}")
    };
    // 失败**不改**计数。
    assert_eq!(call("COUNTER_GET", &mut h, vec![i(handle)]), i(5));
}

/// `COUNTER_INC` 与 `COUNTER_ADD` 在**正增量**上行为一致（同一个原子加）。
#[test]
fn the_counter_add_and_increment_agree_on_a_positive_delta() {
    let _g = gate();
    let mut h = host();
    let a = call_handle("COUNTER", &mut h, vec![i(0)]);
    let b = call_handle("COUNTER", &mut h, vec![i(0)]);
    assert_eq!(call("COUNTER_INC", &mut h, vec![i(a), i(5)]), i(5));
    assert_eq!(call("COUNTER_ADD", &mut h, vec![i(b), i(5)]), i(5));
}

// ── 共享字典 ──────────────────────────────────────────────────────────────

/// 新字典是**空的**：读不存在的键 ⇒ `NULL`；快照 ⇒ 空字典。
#[test]
fn a_shared_dict_starts_empty() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    assert_eq!(
        call("SHARED_DICT_GET", &mut h, vec![i(handle), s("k")]),
        Value::Null
    );
    let snap = call("SHARED_DICT_SNAPSHOT", &mut h, vec![i(handle)]);
    let Value::Dict(d) = &snap else {
        panic!("the snapshot must be a DICT, got {snap:?}")
    };
    assert!(d.is_empty(), "a fresh dict must be empty: {snap:?}");
}

/// 单键写 ⇒ 读得到；**覆盖**旧值。
#[test]
fn a_single_key_write_is_visible_and_overwrites() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    assert_eq!(
        call("SHARED_DICT_SET", &mut h, vec![i(handle), s("k"), i(1)]),
        Value::Null,
        "SHARED_DICT_SET returns NULL"
    );
    assert_eq!(
        call("SHARED_DICT_GET", &mut h, vec![i(handle), s("k")]),
        i(1)
    );

    call("SHARED_DICT_SET", &mut h, vec![i(handle), s("k"), i(2)]);
    assert_eq!(
        call("SHARED_DICT_GET", &mut h, vec![i(handle), s("k")]),
        i(2),
        "a second write must overwrite the first"
    );
}

/// 快照**整体**读出字典，且**键序按字典序**（逐次一致 ⇒ 可确定性比对）。
#[test]
fn the_snapshot_reads_the_whole_dict_with_sorted_keys() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    call("SHARED_DICT_SET", &mut h, vec![i(handle), s("c"), i(3)]);
    call("SHARED_DICT_SET", &mut h, vec![i(handle), s("a"), i(1)]);
    call("SHARED_DICT_SET", &mut h, vec![i(handle), s("b"), i(2)]);

    let snap = call("SHARED_DICT_SNAPSHOT", &mut h, vec![i(handle)]);
    assert_eq!(
        dict_keys(&snap),
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        "the snapshot keys must be sorted: {snap:?}"
    );
    assert_eq!(dict_field(&snap, "a"), Some(&i(1)), "snap = {snap:?}");
    assert_eq!(dict_field(&snap, "b"), Some(&i(2)), "snap = {snap:?}");
    assert_eq!(dict_field(&snap, "c"), Some(&i(3)), "snap = {snap:?}");

    // 再取一次 ⇒ **逐次一致**。
    let again = call("SHARED_DICT_SNAPSHOT", &mut h, vec![i(handle)]);
    assert_eq!(snap, again, "two snapshots of the same content must agree");
}

/// 并发写**不同的键** ⇒ 一个键都不丢（单键写是原子的）。
#[test]
fn concurrent_single_key_writes_keep_every_key() {
    const THREADS: usize = 8;

    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    let start = std::sync::Arc::new(std::sync::Barrier::new(THREADS));
    let mut joins = Vec::new();
    for t in 0..THREADS {
        let handle = handle;
        let start = std::sync::Arc::clone(&start);
        joins.push(std::thread::spawn(move || {
            let mut h = host();
            start.wait();
            for n in 0..100 {
                call_in_thread(
                    &mut h,
                    "SHARED_DICT_SET",
                    vec![
                        Value::Integer(handle),
                        Value::String(format!("t{t}-n{n}")),
                        Value::Integer((t * 100 + n) as i64),
                    ],
                );
            }
        }));
    }
    for j in joins {
        j.join().expect("thread");
    }

    let snap = call("SHARED_DICT_SNAPSHOT", &mut h, vec![i(handle)]);
    let keys = dict_keys(&snap);
    assert_eq!(
        keys.len(),
        THREADS * 100,
        "a concurrent write was lost: {keys:?}"
    );
    // 每个键的值都必须**完整**（不是半个值）。
    for t in 0..THREADS {
        for n in 0..100 {
            let key = format!("t{t}-n{n}");
            assert_eq!(
                dict_field(&snap, &key),
                Some(&i((t * 100 + n) as i64)),
                "the value of {key} is torn"
            );
        }
    }
}

/// 并发写**同一个键** ⇒ 看到的是**某一个**完整写者的值（不是撕裂的）。
#[test]
fn concurrent_writes_to_one_key_are_not_torn() {
    const THREADS: usize = 8;

    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    let start = std::sync::Arc::new(std::sync::Barrier::new(THREADS));
    let mut joins = Vec::new();
    for t in 0..THREADS {
        let handle = handle;
        let start = std::sync::Arc::clone(&start);
        joins.push(std::thread::spawn(move || {
            let mut h = host();
            start.wait();
            for _ in 0..100 {
                call_in_thread(
                    &mut h,
                    "SHARED_DICT_SET",
                    vec![
                        Value::Integer(handle),
                        Value::String("shared".to_string()),
                        Value::Integer(t as i64),
                    ],
                );
            }
        }));
    }
    for j in joins {
        j.join().expect("thread");
    }

    let v = call("SHARED_DICT_GET", &mut h, vec![i(handle), s("shared")]);
    let Value::Integer(who) = v else {
        panic!("the value must be an integer, got {v:?}")
    };
    assert!(
        (0..THREADS as i64).contains(&who),
        "the value must be one whole writer's value, got {who}"
    );
}

/// 线程里调成员的薄封装（`call` 需要 `&mut Host`，线程里自建一个）。
fn call_in_thread(host: &mut Host, name: &str, args: Vec<Value>) -> Value {
    let out = member(name)(host, args).unwrap_or_else(|e| {
        panic!("{name} must return a value, got {e:?}");
    });
    out.value
}

/// 嵌套的纯数据（数组 / 字典）**逐层**存进去、**原样**取回来。
#[test]
fn nested_plain_data_round_trips() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    let payload = Value::dict(vec![
        (s("n"), i(42)),
        (
            s("list"),
            Value::Array(vec![i(1), s("two"), Value::Boolean(true)]),
        ),
        (
            s("nested"),
            Value::dict(vec![(s("deep"), Value::Array(vec![i(7)]))]),
        ),
    ]);
    call(
        "SHARED_DICT_SET",
        &mut h,
        vec![i(handle), s("p"), payload.clone()],
    );

    let got = call("SHARED_DICT_GET", &mut h, vec![i(handle), s("p")]);
    assert_eq!(got, payload, "nested plain data must round-trip: {got:?}");

    let snap = call("SHARED_DICT_SNAPSHOT", &mut h, vec![i(handle)]);
    assert_eq!(
        dict_field(&snap, "p"),
        Some(&payload),
        "the snapshot must carry the whole value: {snap:?}"
    );
}

/// 整数键也能用（键域与 wlwl 字典一致：`STRING` / `INTEGER`）。
#[test]
fn an_integer_key_is_usable() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    call("SHARED_DICT_SET", &mut h, vec![i(handle), i(7), s("seven")]);
    assert_eq!(
        call("SHARED_DICT_GET", &mut h, vec![i(handle), i(7)]),
        s("seven")
    );
    // 字符串键 `"7"` 与整数键 `7` **不是**同一个键。
    assert_eq!(
        call("SHARED_DICT_GET", &mut h, vec![i(handle), s("7")]),
        Value::Null
    );
}

// ── 失败口径 ──────────────────────────────────────────────────────────────

/// 句柄**不存在** ⇒ **`ERR` 值**（不是诊断）。
#[test]
fn an_unknown_handle_is_an_err_value() {
    let _g = gate();
    let mut h = host();

    let v = call("COUNTER_GET", &mut h, vec![i(9999)]);
    let Value::Err(_) = v else {
        panic!("an unknown handle must be an ERR value, got {v:?}")
    };
    assert_eq!(err_field(&v, "kind").as_deref(), Some("SyncError"), "{v:?}");
    assert_eq!(err_field(&v, "op").as_deref(), Some("COUNTER_GET"), "{v:?}");
    assert!(err_field(&v, "reason").is_some(), "{v:?}");

    let v = call("COUNTER_INC", &mut h, vec![i(9999), i(1)]);
    assert_eq!(err_field(&v, "kind").as_deref(), Some("SyncError"), "{v:?}");

    let v = call("SHARED_DICT_GET", &mut h, vec![i(9999), s("k")]);
    assert_eq!(err_field(&v, "kind").as_deref(), Some("SyncError"), "{v:?}");

    let v = call("SHARED_DICT_SNAPSHOT", &mut h, vec![i(9999)]);
    assert_eq!(err_field(&v, "kind").as_deref(), Some("SyncError"), "{v:?}");
}

/// 句柄**种类**不对（计数器句柄喂给字典成员）⇒ **`ERR` 值**。
#[test]
fn a_handle_of_the_wrong_kind_is_an_err_value() {
    let _g = gate();
    let mut h = host();
    let counter = call_handle("COUNTER", &mut h, vec![i(0)]);
    let dict = call_handle("SHARED_DICT", &mut h, vec![]);

    let v = call("SHARED_DICT_GET", &mut h, vec![i(counter), s("k")]);
    assert_eq!(err_field(&v, "kind").as_deref(), Some("SyncError"), "{v:?}");
    assert_eq!(
        err_field(&v, "op").as_deref(),
        Some("SHARED_DICT_GET"),
        "{v:?}"
    );

    let v = call("COUNTER_GET", &mut h, vec![i(dict)]);
    assert_eq!(err_field(&v, "kind").as_deref(), Some("SyncError"), "{v:?}");
    assert_eq!(err_field(&v, "op").as_deref(), Some("COUNTER_GET"), "{v:?}");
}

/// **不可共享**的值（闭包 / 原生函数 / 句柄 / 类）⇒ **`ERR` 值**。
#[test]
fn a_value_that_cannot_be_shared_is_an_err_value() {
    let _g = gate();
    let mut h = host();
    let handle = call_handle("SHARED_DICT", &mut h, vec![]);

    let v = call(
        "SHARED_DICT_SET",
        &mut h,
        vec![i(handle), s("f"), not_shareable()],
    );
    let Value::Err(_) = v else {
        panic!("a non-shareable value must be an ERR value, got {v:?}")
    };
    assert_eq!(err_field(&v, "kind").as_deref(), Some("SyncError"), "{v:?}");
    assert_eq!(
        err_field(&v, "op").as_deref(),
        Some("SHARED_DICT_SET"),
        "{v:?}"
    );
    assert!(err_field(&v, "reason").is_some(), "{v:?}");

    // 失败的写**没有**落盘。
    assert_eq!(
        call("SHARED_DICT_GET", &mut h, vec![i(handle), s("f")]),
        Value::Null
    );
}

/// 元数 / 类型错 ⇒ **既有**诊断（`E0022` / `E0030`）。
#[test]
fn arity_and_type_errors_use_the_existing_diagnostics() {
    let _g = gate();

    let e = member("COUNTER")(&mut host(), vec![]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0022,
        "COUNTER with 0 arguments"
    );

    let e = member("COUNTER_INC")(&mut host(), vec![i(1)]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0022,
        "COUNTER_INC with 1 argument"
    );

    let e = member("COUNTER_GET")(&mut host(), vec![]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0022,
        "COUNTER_GET with 0 arguments"
    );

    let e = member("SHARED_DICT")(&mut host(), vec![i(1)]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0022,
        "SHARED_DICT with 1 argument"
    );

    let e = member("SHARED_DICT_SET")(&mut host(), vec![i(1), s("k")]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0022,
        "SHARED_DICT_SET with 2 arguments"
    );

    let e = member("COUNTER")(&mut host(), vec![s("x")]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0030,
        "COUNTER with a non-integer initial value"
    );

    let e = member("COUNTER_INC")(&mut host(), vec![s("h"), i(1)]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0030,
        "COUNTER_INC with a string handle"
    );

    let e =
        member("SHARED_DICT_SET")(&mut host(), vec![i(1), Value::Float(2.0), i(3)]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0030,
        "SHARED_DICT_SET with a float key (the key domain is STRING / INTEGER)"
    );
}

// ── 句柄的重复与幂等 ──────────────────────────────────────────────────────

/// `COUNTER` / `SHARED_DICT` **不幂等**：每次调用都**新建** ⇒ 两次调用给出
/// 两个**不同的**句柄、两个**互不相干**的容器。
#[test]
fn two_containers_of_the_same_kind_get_distinct_handles() {
    let _g = gate();
    let mut h = host();

    let a = call_handle("COUNTER", &mut h, vec![i(0)]);
    let b = call_handle("COUNTER", &mut h, vec![i(0)]);
    assert_ne!(a, b, "two counters must not share a handle");

    // 两个计数器**互不相干**。
    call("COUNTER_INC", &mut h, vec![i(a), i(5)]);
    assert_eq!(call("COUNTER_GET", &mut h, vec![i(a)]), i(5));
    assert_eq!(call("COUNTER_GET", &mut h, vec![i(b)]), i(0));

    let d1 = call_handle("SHARED_DICT", &mut h, vec![]);
    let d2 = call_handle("SHARED_DICT", &mut h, vec![]);
    assert_ne!(d1, d2, "two dicts must not share a handle");

    call("SHARED_DICT_SET", &mut h, vec![i(d1), s("k"), i(1)]);
    assert_eq!(
        call("SHARED_DICT_GET", &mut h, vec![i(d2), s("k")]),
        Value::Null,
        "two dicts must not share their content"
    );
}

/// 句柄**永不复用** ⇒ 同一个整数不可能先后指向两个容器。
#[test]
fn handles_are_never_reused() {
    let _g = gate();
    let mut h = host();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..64 {
        let handle = call_handle("COUNTER", &mut h, vec![i(0)]);
        assert!(
            seen.insert(handle),
            "the handle {handle} was handed out twice"
        );
    }
}
