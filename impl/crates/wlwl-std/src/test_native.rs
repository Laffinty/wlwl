//! `wlwl:std.test` 的 **R2 原生内核** —— 标准库底座 v0.11 M3-3(ADR-0021)。
//!
//! 混合形态:对外契约层是 R1 门面 `wl/std/test.wll`(6 个成员都在那里导出,
//! 见标准库规范 §8)。本文件只留三件纯 wlwl 表达不了的事:
//!
//! 1. **注册表** —— `TEST` 推入 [`StdCtx::tests`];注册表是每调用级的
//!    宿主状态,R1 侧没有对应的存储面。
//! 2. **计时与调用** —— `RUN_TESTS` 排空注册表、逐条计时、调用测试体、
//!    组装结果字典。计时要 `Instant`,测试体调用要走宿主的 `call`(闭包
//!    只能由求值器调用)。
//! 3. **`EXPECT_ERR`** —— §8 要求「`x` 为 `ERR` → `OK(载荷)`」,而语言
//!    自身的 ERR 透明调用语义(§8.2)在**调用边界**就把 ERR 实参短路掉,
//!    成员体根本不执行(实测:任何 `FUN` 收到 ERR 实参,调用结果都是那个
//!    ERR,与函数体无关;变长形参也照样短路)。所以它不能写成带函数体的
//!    门面,只能由门面**改名导出**本文件这一份。详见 `wl/std/test.wll`
//!    的文件头。
//!
//! `ASSERT` / `ASSERT_EQ` / `ASSERT_NEQ` 的**载荷构造**已随 M3-3 搬进
//! 门面(纯 wlwl 表达得了:`OK(TRUE)` / `ERR([...])` / `INDEX_SET` 追加可选
//! 键),本文件不再有这三个函数。
//!
//! ## 成员名不再由本文件决定
//!
//! M3 之前这里是 `NAMES` + `SPEC.functions` 两份名单,靠 eval 侧的
//! `names_match_catalog` 对拍。M3 起成员面由门面的 `EXPORT` 声明决定,
//! 附录 A 镜像与 `wlwl-eval/tests/test_contract.rs`(对标准库规范 §8 表格
//! 的外部对照)负责锁。**本文件只提供内核实现,不提供名册。**
//!
//! ## ERR 约定
//!
//! 断言失败是**值**不是诊断:`ASSERT_*` 返回 `ERR(载荷)`,由 `RUN_TESTS`
//! 捕获并记进结果字典的 `error` 字段(标准库规范 §8)。测试体求值期间
//! 逃逸的 `ERR` 同样被捕获,该项 `passed = FALSE`。

use wlwl_value::{Outcome, StdHost, TestEntry, Value};

use std::time::Instant;

use wlwl_error::{ErrorCode, WlwlError, WlwlResult};

// ─────────────────────────────────────────────────────────────────────
// R1 门面注入的三个内核
// ─────────────────────────────────────────────────────────────────────

/// `TEST(name, body)` —— 类型检查在门面做完,这里只注册并返回 `NULL`。
///
/// 实参里的 `ERR` 按 §12.6 透明返回(门面是闭包,调用边界已经处理过一次;
/// 这里保留检查是因为本函数是 `wlwl_std` 的公开 API,可能被直接调用)。
pub fn kernel_test(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "TEST", args.len(), 2));
    }
    let name = match &args[0] {
        Value::String(s) => s.clone(),
        other => return Err(type_err(host, "TEST", "string", other)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(type_err(host, "TEST", "function", &args[1]));
    }
    host.ctx().tests.push(TestEntry {
        name,
        body: args[1].clone(),
    });
    Ok(Outcome::normal(Value::Null))
}

/// `EXPECT_ERR(x)` —— `x` 为 `ERR` → `OK(载荷)`;否则 `ERR(E0049)`。
///
/// 见文件头:这一份**不能**被门面包一层 `FUN`,否则 §8 的「返回 `OK(载荷)`」
/// 会被语言的 ERR 透明调用语义吃掉。
pub fn kernel_expect_err(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        // Short-circuit: input was ERR — that IS the expected case.
        return Ok(Outcome::normal(Value::Ok(Box::new(e))));
    }
    if args.len() != 1 {
        return Err(arity(host, "EXPECT_ERR", args.len(), 1));
    }
    // Input is not ERR — that's the failure mode for EXPECT_ERR.
    let payload = build_payload("E0049", &args[0], None, "test_expect_err_failed");
    Ok(Outcome::normal(make_err(
        host,
        ErrorCode::E0049,
        format!(
            "EXPECT_ERR failed: input was not ERR (got {})",
            args[0].display()
        ),
        payload,
    )))
}

/// `RUN_TESTS()` —— 排空注册表、逐条计时调用、组装结果字典(标准库规范 §8)。
pub fn kernel_run_tests(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !args.is_empty() {
        return Err(arity(host, "RUN_TESTS", args.len(), 0));
    }
    // Drain the registry. We swap with a new empty Vec so a TEST
    // that registers more tests inside its body (unusual but
    // legitimate) doesn't deadlock.
    let entries = std::mem::take(&mut host.ctx().tests);
    let mut results: Vec<Value> = Vec::with_capacity(entries.len());
    for entry in entries {
        let started = Instant::now();
        // Per §8: ERR from assertions is "non-transparent" by spec rule
        // (RUN_TESTS catches it). A properly-written TEST body either
        // returns OK(TRUE) on pass or an ERR value from ASSERT/ASSERT_EQ
        // on fail; the latter returns through Outcome::normal(err) from
        // the facade, which we read off the Outcome.value below.
        let outcome = match crate::call_callable(host, "RUN_TESTS", &entry.body, vec![]) {
            Ok(v) => Ok(v),
            Err(_e) => {
                // Diagnostic surfaced during the test (e.g. uncaught
                // E0102 from an unexpected ERR escape). Record the
                // failure as a generic ERR.
                Err(())
            }
        };
        let duration_ms = started.elapsed().as_millis() as i64;
        let dict = match outcome {
            Ok(Value::Err(payload)) => {
                // Spec §8: error field carries the payload.
                let mut d = vec![
                    (
                        Value::String("name".into()),
                        Value::String(entry.name.clone()),
                    ),
                    (Value::String("passed".into()), Value::Boolean(false)),
                    (
                        Value::String("duration_ms".into()),
                        Value::Integer(duration_ms),
                    ),
                    (Value::String("error".into()), *payload),
                ];
                // Order-stable for assertion predictability.
                d.sort_by(|a, b| {
                    let ka = match &a.0 {
                        Value::String(s) => s.as_str(),
                        _ => "",
                    };
                    let kb = match &b.0 {
                        Value::String(s) => s.as_str(),
                        _ => "",
                    };
                    ka.cmp(kb)
                });
                Value::Dict(d)
            }
            Ok(other) => {
                // Non-ERR outcome. Could be OK(TRUE) (from a passed
                // assertion), NULL, or a bare value. All count as
                // "passed" per §8's permissive contract.
                let mut d = vec![
                    (
                        Value::String("name".into()),
                        Value::String(entry.name.clone()),
                    ),
                    (Value::String("passed".into()), Value::Boolean(true)),
                    (
                        Value::String("duration_ms".into()),
                        Value::Integer(duration_ms),
                    ),
                ];
                if !matches!(other, Value::Null) {
                    d.push((Value::String("return_value".into()), other));
                }
                d.sort_by(|a, b| {
                    let ka = match &a.0 {
                        Value::String(s) => s.as_str(),
                        _ => "",
                    };
                    let kb = match &b.0 {
                        Value::String(s) => s.as_str(),
                        _ => "",
                    };
                    ka.cmp(kb)
                });
                Value::Dict(d)
            }
            Err(()) => {
                let mut d = vec![
                    (
                        Value::String("name".into()),
                        Value::String(entry.name.clone()),
                    ),
                    (Value::String("passed".into()), Value::Boolean(false)),
                    (
                        Value::String("duration_ms".into()),
                        Value::Integer(duration_ms),
                    ),
                    (
                        Value::String("error".into()),
                        Value::String("uncaught diagnostic".into()),
                    ),
                ];
                d.sort_by(|a, b| {
                    let ka = match &a.0 {
                        Value::String(s) => s.as_str(),
                        _ => "",
                    };
                    let kb = match &b.0 {
                        Value::String(s) => s.as_str(),
                        _ => "",
                    };
                    ka.cmp(kb)
                });
                Value::Dict(d)
            }
        };
        results.push(dict);
    }
    Ok(Outcome::normal(Value::Array(results)))
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

fn make_err(host: &mut dyn StdHost, code: ErrorCode, msg: String, payload: Value) -> Value {
    // Build an `Err(payload)` Value. `host.diag` would make a
    // `WlwlError` (a Rust Err), which would propagate up via
    // `?` — but we want the ERR to be a *value* that the calling
    // TEST body sees and that RUN_TESTS catches. So we use the diag
    // machinery for the canonical message format, then recover
    // the `Value::Err(payload)` the caller expects.
    let diag = host.diag(code, msg);
    // Lock the message on the diag so the err payload's `display`
    // stays in sync; we don't ship the whole diagnostic, just the
    // code + payload (matching §8 row 2-5 schema: ERR has
    // `{code, ...}`).
    let _ = diag; // suppress unused-variable; future use: enrich payload
    Value::Err(Box::new(payload))
}

fn short_circuit_err(args: &[Value]) -> Option<Value> {
    args.iter().find_map(|v| match v {
        Value::Err(_) => Some(v.clone()),
        _ => None,
    })
}

// ─────────────────────────────────────────────────────────────────────
// Shared payload builder(目前只有 EXPECT_ERR 用;ASSERT 族已搬到门面)
// ─────────────────────────────────────────────────────────────────────

fn build_payload(code: &str, cond: &Value, msg: Option<Value>, kind: &str) -> Value {
    // §8: ERR payload is a DICT `{code, kind, cond?, msg?}`.
    let mut entries = vec![
        (Value::String("code".into()), Value::String(code.into())),
        (Value::String("kind".into()), Value::String(kind.into())),
    ];
    if !matches!(cond, Value::Null | Value::Boolean(false)) {
        entries.push((Value::String("cond".into()), cond.clone()));
    }
    if let Some(m) = msg {
        entries.push((Value::String("msg".into()), m));
    }
    Value::Dict(entries)
}
