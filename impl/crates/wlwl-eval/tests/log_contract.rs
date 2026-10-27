//! `wlwl:std.log` 成员契约 —— W-26（结构化日志 + 运行时指标）。
//!
//! ## 为什么**不走 IMPORT**
//!
//! `std.log` 这个命名空间**还没登记**进 `wlwl-std/src/lib.rs` 的三张表
//! （那是集成者的活）⇒ 走 `IMPORT` 会得到 `E0040 module not found`。
//! ⇒ 本文件**直接拿 `StdFn` 调**（`wlwl_std::log::SPEC` 的成员表）。
//!
//! ⚠ **因此本文件验的是「成员的行为」，不是「命名空间的登记」** ——
//!   登记由集成者统一做，并由 `stdlib_mirror.rs` / 附录 A 镜像那几道闸守。
//!
//! ## 本模块的进程级状态
//!
//! 级别阈值 / 输出目标 / 指标表都是**进程级**的，而 libtest **并行**跑用例
//! ⇒ 用例之间会互相踩（一个用例把级别设成 `ERROR`，另一个用例的
//! 「环境变量缺省级别」断言就被盖住了）。
//! ⇒ 本文件用一把**全局闸**把用例串起来，并在每个用例开头
//! **复位**模块状态（`wlwl_std::log::reset_state()`）。

mod common;

use std::sync::{Mutex, MutexGuard};

use wlwl_error::ErrorCode;
use wlwl_std::{Outcome, StdCtx, StdHost, Value};

/// 用例之间的**全局闸**（进程级状态的护栏）。
static GATE: Mutex<()> = Mutex::new(());

/// 取闸。
fn gate() -> MutexGuard<'static, ()> {
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// 复位**级别阈值**（回到缺省：`WLWL_LOG_LEVEL` → `INFO`）。
///
/// ⚠ 走的是**成员面**（`LOG_SET_LEVEL("")`）⇒ 本文件**不依赖**任何 crate
///   内部可见性（模块是不是 `pub`、有没有 `reset_state` 之类的钩子，都与本
///   文件无关）。
fn reset_level(host: &mut Host) {
    call("LOG_SET_LEVEL", host, vec![s("")]);
}

/// 命名空间的 `SPEC`（**实现真相**）。
///
/// ⚠ 走 `resolve()`（与 `net_contract.rs` 同款）⇒ 本文件不依赖
///   `wlwl_std::log` 这个 Rust 路径的可见性。
fn spec() -> &'static wlwl_std::ModuleSpec {
    match wlwl_std::resolve("wlwl:std.log") {
        Some(wlwl_std::StdBackend::Native(spec)) => spec,
        Some(wlwl_std::StdBackend::Lang(_)) => {
            panic!("wlwl:std.log must be an R2 native module, not an R1 source module")
        }
        None => panic!(
            "wlwl:std.log must resolve — the namespace registration is the integrator's step \
             (wlwl-std/src/lib.rs: `pub mod log;` + the `resolve` arm + `NATIVE_SPECS`)"
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
        .unwrap_or_else(|| panic!("wlwl:std.log has no member {name}"))
}

/// 手写宿主。本模块只读 `ctx.env`（`WLWL_LOG_LEVEL`），不回调宿主。
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
        panic!("std.log has no member that calls back into the host")
    }
    fn diag(&mut self, code: ErrorCode, message: String) -> wlwl_error::WlwlError {
        wlwl_error::WlwlDiagnostic::new(code, message, wlwl_error::Location::point("<test>", 0, 0))
            .into()
    }
}

fn host() -> Host {
    Host(StdCtx::default())
}

fn host_with_env(pairs: &[(&str, &str)]) -> Host {
    let mut ctx = StdCtx::default();
    for (k, v) in pairs {
        ctx.env.insert((*k).to_string(), (*v).to_string());
    }
    Host(ctx)
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

/// 一个**唯一**的临时目录（`$env:TEMP` 下）。
fn scratch() -> std::path::PathBuf {
    common::scratch("wlwl_log")
}

// ── 成员面 ────────────────────────────────────────────────────────────────

/// 成员面**冻结**：7 个成员、名字一字不差。
///
/// ⚠ 少一个 ⇒ 集成者的登记清单与实现对不上；多一个 ⇒ 同上。
///   （这是**唯一**一处把「本项的成员数」钉死的地方。）
#[test]
fn the_member_set_is_exactly_seven() {
    let names: Vec<&str> = spec().functions.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        vec![
            "LOG",
            "LOG_SET_LEVEL",
            "LOG_FILE",
            "LOG_METRICS",
            "METRIC_COUNTER",
            "METRIC_INC",
            "METRIC_TIMER",
        ],
        "wlwl:std.log member set changed"
    );
}

// ── 级别过滤 ──────────────────────────────────────────────────────────────

/// 低于阈值 ⇒ **不输出**，且**返回 `NULL`**（日志不得改变程序语义）。
#[test]
fn a_level_below_the_threshold_is_filtered_out() {
    let _g = gate();
    let dir = scratch();
    let file = dir.join("run.jsonl");
    let mut h = host();

    reset_level(&mut h);
    call("LOG_SET_LEVEL", &mut h, vec![s("INFO")]);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);

    let v = call("LOG", &mut h, vec![s("DEBUG"), s("suppressed")]);
    assert!(
        matches!(v, Value::Null),
        "a filtered LOG must still return NULL, got {v:?}"
    );

    let v = call("LOG", &mut h, vec![s("INFO"), s("kept")]);
    assert!(
        matches!(v, Value::Null),
        "a written LOG returns NULL too, got {v:?}"
    );

    let body = std::fs::read_to_string(&file).expect("the log file");
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "only the INFO line may be written: {body:?}"
    );
    assert!(
        !body.contains("suppressed"),
        "the filtered line must not reach the target: {body:?}"
    );
    assert!(body.contains("kept"), "body = {body:?}");
}

/// `WLWL_LOG_LEVEL`（宿主环境）决定**缺省**阈值。
#[test]
fn the_env_var_sets_the_default_level() {
    let _g = gate();
    let dir = scratch();
    let file = dir.join("run.jsonl");
    let mut h = host_with_env(&[("WLWL_LOG_LEVEL", "WARN")]);

    reset_level(&mut h);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);
    call("LOG", &mut h, vec![s("INFO"), s("suppressed")]);
    call("LOG", &mut h, vec![s("WARN"), s("kept")]);

    let body = std::fs::read_to_string(&file).expect("the log file");
    assert_eq!(
        body.lines().count(),
        1,
        "INFO is below the WARN threshold ⇒ one line only: {body:?}"
    );
    assert!(body.contains("\"level\":\"WARN\""), "body = {body:?}");
}

/// 显式设定**压过**环境变量。
#[test]
fn an_explicit_level_wins_over_the_env_var() {
    let _g = gate();
    let dir = scratch();
    let file = dir.join("run.jsonl");
    let mut h = host_with_env(&[("WLWL_LOG_LEVEL", "ERROR")]);

    reset_level(&mut h);
    call("LOG_SET_LEVEL", &mut h, vec![s("DEBUG")]);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);
    call("LOG", &mut h, vec![s("DEBUG"), s("kept")]);

    let body = std::fs::read_to_string(&file).expect("the log file");
    assert_eq!(body.lines().count(), 1, "body = {body:?}");
}

// ── JSON 行 ───────────────────────────────────────────────────────────────

/// 一行 = **一个 JSON 对象**，且**顶层键序固定**。
#[test]
fn the_log_line_is_one_json_object_with_a_stable_key_order() {
    let _g = gate();
    let dir = scratch();
    let file = dir.join("run.jsonl");
    let mut h = host();

    reset_level(&mut h);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);
    call(
        "LOG",
        &mut h,
        vec![
            s("INFO"),
            s("hello \"world\""),
            Value::dict(vec![
                (s("n"), i(1)),
                (s("ok"), Value::Boolean(true)),
                (s("list"), Value::Array(vec![i(1), i(2)])),
            ]),
        ],
    );

    let body = std::fs::read_to_string(&file).expect("the log file");
    let line = body.lines().next().expect("one line");
    assert_eq!(body.lines().count(), 1, "one line per record: {body:?}");

    // ① **合法 JSON**（能被解析器吃下去）。
    let parsed: serde_json::Value =
        serde_json::from_str(line).expect("the line must be valid JSON");
    assert_eq!(parsed["level"], "INFO");
    assert_eq!(parsed["msg"], "hello \"world\"");
    assert_eq!(parsed["fields"]["n"], 1);
    assert_eq!(parsed["fields"]["ok"], true);
    assert_eq!(parsed["fields"]["list"], serde_json::json!([1, 2]));
    assert!(parsed["ts"].is_i64(), "ts must be an integer: {parsed}");

    // ② **顶层键序固定**：`ts` → `level` → `msg` → `fields`。
    let pos = |needle: &str| {
        line.find(needle)
            .unwrap_or_else(|| panic!("{needle} must be in the line: {line}"))
    };
    assert!(pos("\"ts\":") < pos("\"level\":"), "line = {line}");
    assert!(pos("\"level\":") < pos("\"msg\":"), "line = {line}");
    assert!(pos("\"msg\":") < pos("\"fields\":"), "line = {line}");

    // ③ `fields` 的键**按字典序**（不是插入序）⇒ 逐次产出同一行。
    let a = pos("\"fields\":");
    let tail = &line[a..];
    let l = tail.find("\"list\"").expect("the list key");
    let n = tail.find("\"n\"").expect("the n key");
    let o = tail.find("\"ok\"").expect("the ok key");
    assert!(l < n && n < o, "the field keys must be sorted: {line}");
}

/// 同样的一条日志 ⇒ **逐字相同**的一行（除 `ts` 外）。
#[test]
fn the_same_record_produces_the_same_line() {
    let _g = gate();
    let dir = scratch();
    let file = dir.join("run.jsonl");
    let mut h = host();

    reset_level(&mut h);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);
    for _ in 0..2 {
        call(
            "LOG",
            &mut h,
            vec![
                s("INFO"),
                s("same"),
                Value::dict(vec![(s("b"), i(2)), (s("a"), i(1))]),
            ],
        );
    }

    let body = std::fs::read_to_string(&file).expect("the log file");
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 2, "body = {body:?}");
    assert_eq!(
        strip_ts(lines[0]),
        strip_ts(lines[1]),
        "the same record must produce the same line (modulo ts): {body:?}"
    );
}

/// 去掉一行里的 `ts` 部分（只留 `,"level":…` 那一截）。
fn strip_ts(l: &str) -> &str {
    let start = l.find(",\"level\"").expect("the level key");
    &l[start..]
}

// ── 输出目标 ──────────────────────────────────────────────────────────────

/// `LOG_FILE` ⇒ 日志**写进文件**（不是 stderr）。
#[test]
fn switching_the_target_writes_into_the_file() {
    let _g = gate();
    let dir = scratch();
    let file = dir.join("run.jsonl");
    let mut h = host();

    reset_level(&mut h);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);
    call("LOG", &mut h, vec![s("ERROR"), s("into the file")]);

    let body = std::fs::read_to_string(&file).expect("the log file");
    assert!(body.contains("into the file"), "body = {body:?}");
    assert!(body.contains("\"level\":\"ERROR\""), "body = {body:?}");
}

/// `LOG_FILE` 是**追加**写（不截断已有内容）。
#[test]
fn the_file_target_appends() {
    let _g = gate();
    let dir = scratch();
    let file = dir.join("run.jsonl");
    let mut h = host();

    reset_level(&mut h);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);
    call("LOG", &mut h, vec![s("INFO"), s("first")]);
    call("LOG_FILE", &mut h, vec![s(file.to_string_lossy().as_ref())]);
    call("LOG", &mut h, vec![s("INFO"), s("second")]);

    let body = std::fs::read_to_string(&file).expect("the log file");
    assert_eq!(body.lines().count(), 2, "body = {body:?}");
    assert!(
        body.contains("first") && body.contains("second"),
        "body = {body:?}"
    );
}

// ── 指标 ──────────────────────────────────────────────────────────────────

/// 快照的**形状**：`{"counters": {…}, "histograms": {…}}`。
#[test]
fn the_metrics_snapshot_has_the_documented_shape() {
    let _g = gate();
    let mut h = host();

    let v = call("METRIC_COUNTER", &mut h, vec![s("http.requests")]);
    let Value::Integer(handle) = v else {
        panic!("METRIC_COUNTER must return an INTEGER handle, got {v:?}")
    };
    assert!(handle > 0, "the handle must be a positive id, got {handle}");

    // 同一个名字再注册一次 ⇒ **同一个句柄**（幂等）。
    let again = call("METRIC_COUNTER", &mut h, vec![s("http.requests")]);
    assert_eq!(
        again,
        Value::Integer(handle),
        "the registration is idempotent"
    );

    assert_eq!(call("METRIC_INC", &mut h, vec![i(handle), i(1)]), i(1));
    assert_eq!(call("METRIC_INC", &mut h, vec![i(handle), i(4)]), i(5));

    // 延迟直方图：3 次观测（10 ms / 20 ms / 2000 ms）。
    assert_eq!(call("METRIC_TIMER", &mut h, vec![i(handle), i(10)]), i(1));
    assert_eq!(call("METRIC_TIMER", &mut h, vec![i(handle), i(20)]), i(2));
    assert_eq!(call("METRIC_TIMER", &mut h, vec![i(handle), i(2000)]), i(3));

    let snap = call("LOG_METRICS", &mut h, vec![]);
    let Value::Dict(top) = &snap else {
        panic!("LOG_METRICS must return a DICT, got {snap:?}")
    };
    let field = |k: &str| -> Value {
        top.iter()
            .find(|(key, _)| matches!(key, Value::String(s) if s == k))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("the snapshot must have the key {k}: {snap:?}"))
    };
    let top_keys: Vec<String> = top
        .iter()
        .map(|(k, _)| match k {
            Value::String(s) => s.clone(),
            other => panic!("a top key must be a string, got {other:?}"),
        })
        .collect();
    assert_eq!(
        top_keys,
        vec!["counters".to_string(), "histograms".to_string()],
        "the snapshot shape is {top_keys:?}"
    );

    // counters：名字 → 值。
    let counters = field("counters");
    let Value::Dict(counters) = &counters else {
        panic!("counters must be a DICT, got {counters:?}")
    };
    let counter_value = counters
        .iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == "http.requests"))
        .map(|(_, v)| v.clone());
    assert_eq!(
        counter_value,
        Some(i(5)),
        "the counter must hold the incremented value: {counters:?}"
    );

    // histograms：count / sum / buckets。
    let histograms = field("histograms");
    let Value::Dict(histograms) = &histograms else {
        panic!("histograms must be a DICT, got {histograms:?}")
    };
    let hist = histograms
        .iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == "http.requests"))
        .map(|(_, v)| v.clone())
        .expect("the histogram of the timed metric");
    let Value::Dict(hist) = &hist else {
        panic!("a histogram must be a DICT, got {hist:?}")
    };
    let hfield = |k: &str| -> Value {
        hist.iter()
            .find(|(key, _)| matches!(key, Value::String(s) if s == k))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("the histogram must have the key {k}: {hist:?}"))
    };
    assert_eq!(hfield("count"), i(3), "histogram = {hist:?}");
    assert_eq!(hfield("sum"), i(2030), "histogram = {hist:?}");

    // 桶是**累积**的：`"10"` = 延迟 ≤ 10 ms 的观测数（1 次）。
    let buckets = hfield("buckets");
    let Value::Dict(buckets) = &buckets else {
        panic!("the buckets must be a DICT, got {buckets:?}")
    };
    let bucket = |k: &str| -> Value {
        buckets
            .iter()
            .find(|(key, _)| matches!(key, Value::String(s) if s == k))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("the buckets must have the key {k}: {buckets:?}"))
    };
    assert_eq!(bucket("10"), i(1), "buckets = {buckets:?}");
    assert_eq!(bucket("25"), i(2), "buckets = {buckets:?}");
    assert_eq!(bucket("+Inf"), i(3), "the +Inf bucket is the total");
}

/// 计数器的自增**精确**：N 个线程各 `+1` ⇒ 恰好 N。
///
/// ⚠ 这是**原子性**的可观测证据：把 `METRIC_INC` 改成「先读后写」，
///   并发下必丢更新 ⇒ 这条断言**必红**（反向守卫）。
#[test]
fn the_counter_increment_is_exact_under_concurrency() {
    const THREADS: usize = 8;
    const PER_THREAD: i64 = 500;

    let _g = gate();
    let mut h = host();
    let v = call("METRIC_COUNTER", &mut h, vec![s("http.concurrent")]);
    let Value::Integer(handle) = v else {
        panic!("METRIC_COUNTER must return an INTEGER handle, got {v:?}")
    };

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
                    member("METRIC_INC")(&mut h, vec![Value::Integer(handle), Value::Integer(1)])
                        .expect("METRIC_INC returns a value");
                assert!(matches!(out.value, Value::Integer(_)));
            }
        }));
    }
    for j in joins {
        j.join().expect("thread");
    }

    let snap = call("LOG_METRICS", &mut h, vec![]);
    let Value::Dict(top) = &snap else {
        panic!("LOG_METRICS must return a DICT, got {snap:?}")
    };
    let counters = top
        .iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == "counters"))
        .map(|(_, v)| v.clone())
        .expect("the counters section");
    let Value::Dict(counters) = &counters else {
        panic!("counters must be a DICT, got {counters:?}")
    };
    let value = counters
        .iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == "http.concurrent"))
        .map(|(_, v)| v.clone());
    assert_eq!(
        value,
        Some(i((THREADS as i64) * PER_THREAD)),
        "a lost update means the increment was not atomic: {counters:?}"
    );
}

// ── 失败口径 ──────────────────────────────────────────────────────────────

/// 元数 / 类型错 ⇒ **既有**诊断（`E0022` / `E0030`）。
#[test]
fn arity_and_type_errors_use_the_existing_diagnostics() {
    let _g = gate();

    let e = member("LOG")(&mut host(), vec![s("INFO")]).unwrap_err();
    assert_eq!(e.diagnostic().code, ErrorCode::E0022, "LOG with 1 argument");

    let e = member("LOG")(&mut host(), vec![s("INFO"), s("m"), s("not a dict")]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0030,
        "LOG with a non-dict field"
    );

    let e = member("LOG_SET_LEVEL")(&mut host(), vec![]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0022,
        "LOG_SET_LEVEL with 0 arguments"
    );

    let e = member("METRIC_INC")(&mut host(), vec![i(1)]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0022,
        "METRIC_INC with 1 argument"
    );

    let e = member("METRIC_INC")(&mut host(), vec![s("h"), i(1)]).unwrap_err();
    assert_eq!(
        e.diagnostic().code,
        ErrorCode::E0030,
        "METRIC_INC with a string handle"
    );
}

/// 运行期可预期的失败 ⇒ **`ERR([kind:"LogError", …])` 值**（不是诊断）。
#[test]
fn predictable_failures_are_err_values() {
    let _g = gate();
    let mut h = host();

    // 认不得的级别名。
    let v = call("LOG_SET_LEVEL", &mut h, vec![s("VERBOSE")]);
    let Value::Err(payload) = v else {
        panic!("an unknown level must be an ERR value, got {v:?}")
    };
    let Value::Dict(d) = *payload else {
        panic!("the ERR payload must be a DICT")
    };
    let field = |k: &str| -> Option<String> {
        d.iter()
            .find(|(key, _)| matches!(key, Value::String(s) if s == k))
            .and_then(|(_, v)| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
    };
    assert_eq!(
        field("kind").as_deref(),
        Some("LogError"),
        "the payload = {d:?}"
    );
    assert_eq!(
        field("op").as_deref(),
        Some("LOG_SET_LEVEL"),
        "the payload = {d:?}"
    );
    assert!(
        field("reason").is_some(),
        "the payload must carry a reason: {d:?}"
    );

    // 认不得的句柄。
    let v = call("METRIC_INC", &mut h, vec![i(9999), i(1)]);
    let Value::Err(_) = v else {
        panic!("an unknown handle must be an ERR value, got {v:?}")
    };

    // 打不开的日志文件。
    let v = call("LOG_FILE", &mut h, vec![s("no_such_dir_zzz/run.jsonl")]);
    let Value::Err(_) = v else {
        panic!("an unopenable file must be an ERR value, got {v:?}")
    };
}
