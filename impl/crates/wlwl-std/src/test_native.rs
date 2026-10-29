//! `wlwl:std.test` 的 **R2 原生内核** —— 标准库底座 v0.11 M3-3(ADR-0021)。
//!
//! 混合形态:对外契约层是 R1 门面 `wl/std/test.wll`(6 个成员都在那里导出,
//! 见标准库规范 §8)。本文件只留三件纯 wlwl 表达不了的事:
//!
//! 1. **注册表** —— `TEST` 推入 `StdCtx::tests`(`StdCtx` 住在 `wlwl-value`,
//!    本文件没有 `use` 它,所以写成代码而非 intra-doc 链接);注册表是每调用级
//!    的宿主状态,R1 侧没有对应的存储面。
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
//! ## [G-1] v0.12.2 新增的四个内核
//!
//! 需求清单 G-1 要的 7 个新成员里,有 4 个**纯 wlwl 表达不了**,故在本文件实现:
//!
//! - `EXPECT_ERR_CODE` —— 要**捕获原生诊断码**(如 `E1003` 除零)。诊断是
//!   **宿主级错误**(Rust `Err`),不是 `ERR` **值** ⇒ 门面(`FUN`)根本拿不到它,
//!   它会一路逃逸成进程级失败(实测:仓里的 `H4_expect_err_cannot_catch_native`
//!   探针就是这个缺口)。只有内核能把 `call_callable` 的 `Err` 接住。
//! - `SKIP` —— 跳过标记的**形状**必须与 `RUN_TESTS` 的识别口径同源 ⇒ 两边同住
//!   本文件(门面只改名透传)。
//! - `TEST_CASES` —— 注册 **N 条**测试,每条带**自己的实参**。实参要跟着测试体
//!   一起进注册表,而 `TestEntry.body` 只有一个 `Value` 位 ⇒ 体与实参**打包成
//!   一个二元数组**存(见 `split_entry_body`)。
//! - `TEMP_DIR` —— 必须返回**绝对路径**:`std.fs` 成员的相对路径按**进程 CWD**
//!   解析 ⇒ 相对路径会把文件写进仓库里(`fs_atomic_lock_meta_contract.rs` 踩过
//!   的既有教训)。
//!
//! 另 3 个(`ASSERT_MSG` / `ASSERT_TRUE` / `RUN_TESTS_ASYNC`)是**纯 R1**,在门面。
//!
//! ## ERR 约定
//!
//! 断言失败是**值**不是诊断:`ASSERT_*` 返回 `ERR(载荷)`,由 `RUN_TESTS`
//! 捕获并记进结果字典的 `error` 字段(标准库规范 §8)。测试体求值期间
//! 逃逸的 `ERR` 同样被捕获,该项 `passed = FALSE`。
//!
//! ## 挂起约定(D11-019)
//!
//! `ERR` 是**值**,可以被 `RUN_TESTS` 捕获;**挂起不是值**。`StdFn` 的
//! 契约要求 std 层把回调的 `Signal::Yield` 原样穿透,所以测试体挂起时
//! `RUN_TESTS` 不记账、不产出结果数组,把信号原样交回调度器 —— 绝不能把
//! 挂起点的值当成测试结果(那正是此前「`YIELD()` 之后的断言没跑却判通过」
//! 的成因,见 `crate::call_callable` 的文档)。

use wlwl_value::{union_unconsumed, Outcome, Signal, StdHost, TestEntry, UnconsumedErrs, Value};

use std::sync::atomic::{AtomicU64, Ordering};
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
    // [X-06 / v1.0 spec §8.6.3] 结果容器持有的未消费集合 = 各条目集合的
    // **并集**(容器装盛 ⇒ 转发给该容器)。
    let mut result_set: UnconsumedErrs = Vec::new();
    for entry in entries {
        let started = Instant::now();
        // Per §8: ERR from assertions is "non-transparent" by spec rule
        // (RUN_TESTS catches it). A properly-written TEST body either
        // returns OK(TRUE) on pass or an ERR value from ASSERT/ASSERT_EQ
        // on fail; the latter returns through Outcome::normal(err) from
        // the facade, which we read off the Outcome.value below.
        //
        // [D11-019] `call_callable` 现在原样回传 `Outcome`,所以这里必须
        // 显式处理**挂起**:测试体里出现 `YIELD()`(或落空的
        // `CHANNEL_RECV`)时,信号要交回调度器,本轮已攒的结果作废。
        // 此前信号在 `call_callable` 里被丢弃,挂起点传回的 `NULL` 被
        // 当作「非 ERR ⇒ 通过」,于是体里 `YIELD()` 之后的断言一次都没跑
        // 却记成 `passed = TRUE`(实测:`ASSERT(FALSE)` 的用例判通过)。
        let called = {
            // [G-1] `TEST_CASES` 注册的体是「体 + 实参」的二元数组 ⇒ 这里拆开。
            let (callable, call_args) = split_entry_body(&entry.body);
            crate::call_callable(host, "RUN_TESTS", &callable, call_args)
        };
        if let Ok(o) = &called {
            if o.signal != Signal::None {
                // 挂起优先于记账:原样把信号带回,值无意义(调度器在
                // 段边界丢弃它,或在最后一段把它当作任务终值)。
                //
                // ⚠ [X-06 / v1.0 spec §8.6] 未消费状态是**集合**,这里给
                // **空集合**:挂起**不是**丢弃点(§8.6.2 的三处都不含它)⇒
                // 「本轮已攒的结果作废」**不产生** `E0103`。
                return Ok(Outcome {
                    value: Value::Null,
                    signal: o.signal.clone(),
                    unconsumed: Vec::new(),
                });
            }
        }
        // [X-06 / v1.0 spec §8.6.3] 测试体返回值的未消费集合 ⇒ **转发**给
        // 本成员的接收位置(§8.6.3「读入其它任何位置 ⇒ 由接收方承担」)⇒
        // 最终落在 `RUN_TESTS` 的结果容器上。
        //
        // ⚠ `ERR` 的那一支**不转发**:成员把它的**载荷**取走了(取走型消费
        // 者,§8.6.1)⇒ 那个 `ERR` 已不在未消费状态。
        let (outcome, body_set): (Result<Value, ()>, UnconsumedErrs) = match called {
            Ok(o) => {
                let Outcome {
                    value, unconsumed, ..
                } = o;
                let set = if matches!(&value, Value::Err(_)) {
                    Vec::new()
                } else {
                    unconsumed
                };
                (Ok(value), set)
            }
            Err(_e) => {
                // Diagnostic surfaced during the test (e.g. uncaught
                // E0102 from an unexpected ERR escape). Record the
                // failure as a generic ERR.
                (Err(()), Vec::new())
            }
        };
        // 本条测试体带回来的集合 ⇒ 并进结果容器的集合(§8.6.3)。
        result_set = union_unconsumed(&result_set, &body_set);
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
                Value::dict(d)
            }
            Ok(other) => {
                // [G-1] **跳过**:测试体返回了 `SKIP(reason)` 的标记值。
                //
                // ⚠ 跳过记录比通过 / 失败记录**多出** `skipped`(与 `reason`)两个键;
                //   通过 / 失败两种记录的键集与取值**逐字未变**。
                // ⚠ `passed` 取 `FALSE`:跳过的测试**没有跑断言** ⇒ 它不是「通过」。
                //   ⇒ CI 必须**先看 `skipped`** 再看 `passed`(见门面文件头)。
                if let Some(reason) = skip_reason_of(&other) {
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
                        (Value::String("skipped".into()), Value::Boolean(true)),
                    ];
                    if !matches!(reason, Value::Null) {
                        d.push((Value::String("reason".into()), reason));
                    }
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
                    Value::dict(d)
                } else {
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
                    Value::dict(d)
                }
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
                Value::dict(d)
            }
        };
        results.push(dict);
    }
    Ok(Outcome::with_unconsumed(Value::Array(results), result_set))
}

// ─────────────────────────────────────────────────────────────────────
// [G-1] 新增内核(需求清单 G-1)
// ─────────────────────────────────────────────────────────────────────

/// `EXPECT_ERR_CODE(fn, code)` —— 调用 `fn`,**捕获原生诊断码**。
///
/// 规范 §8 的 `EXPECT_ERR` 只能吃 `ERR` **值**;**原生诊断**(如 `E1003` 除零、
/// `E0030` 类型错)是**宿主级错误**(Rust `Err`),不是值 ⇒ 它会一路逃逸成
/// 进程级失败(仓里的 `H4_expect_err_cannot_catch_native` 探针钉的就是这个缺口)。
/// 本内核把 `call_callable` 的 `Err` **接住** ⇒ 原生码**不逃逸**。
///
/// 口径:
/// - `fn` 抛出**期望的**诊断码 ⇒ `OK([code, message])`;
/// - `fn` 抛出**别的**诊断码 ⇒ `ERR([code: E0049, kind, expected, actual, message])`;
/// - `fn` **没抛**诊断 ⇒ `ERR([code: E0049, kind, expected])`。
///
/// ⚠ 失败口径**复用既有码 `E0049`**(`EXPECT_ERR` 族那一支)⇒ **不新增诊断码**
///   (规范 N-3);`kind` 把它与 `EXPECT_ERR` 的失败区分开。
/// ⚠ `fn` 的返回值**不参与判定** —— 判据只有「有没有原生诊断」与「它的码」。
pub fn kernel_expect_err_code(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "EXPECT_ERR_CODE", args.len(), 2));
    }
    let code = match &args[1] {
        Value::String(s) => s.clone(),
        other => return Err(type_err(host, "EXPECT_ERR_CODE", "string", other)),
    };
    if !matches!(args[0], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(type_err(host, "EXPECT_ERR_CODE", "function", &args[0]));
    }
    let called = crate::call_callable(host, "EXPECT_ERR_CODE", &args[0], vec![]);
    match called {
        Ok(outcome) => {
            // [D11-019] 挂起**不是**「没抛诊断」⇒ 原样穿透,交回调度器。
            if outcome.signal != Signal::None {
                return Ok(Outcome {
                    value: Value::Null,
                    signal: outcome.signal.clone(),
                    unconsumed: Vec::new(),
                });
            }
            // 没有原生诊断 ⇒ 与期望不符。
            //
            // ⚠ §8.6.3:测试体带回来的**未消费集合**(例如体里留下了一个没人
            //   消费的 `ERR` 值)⇒ **转发**给本成员的接收位置 ⇒ 不能在这里
            //   悄悄吞掉(吞掉 = 把「没人消费的 ERR」变成静默)。
            Ok(Outcome::with_unconsumed(
                make_err(
                    host,
                    ErrorCode::E0049,
                    format!("EXPECT_ERR_CODE failed: no diagnostic was raised (expected {code})"),
                    expect_err_code_failed(&code, None, None),
                ),
                outcome.unconsumed,
            ))
        }
        Err(e) => {
            let diag = e.diagnostic();
            let actual = diag.code.as_str().to_string();
            let message = diag.message.clone();
            if actual == code {
                Ok(Outcome::normal(Value::Ok(Box::new(
                    expect_err_code_caught(&code, &message),
                ))))
            } else {
                Ok(Outcome::normal(make_err(
                    host,
                    ErrorCode::E0049,
                    format!("EXPECT_ERR_CODE failed: expected {code}, got {actual} ({message})"),
                    expect_err_code_failed(&code, Some(&actual), Some(&message)),
                )))
            }
        }
    }
}

/// `SKIP(reason)` —— 跳过当前测试。
///
/// 返回一个**跳过标记**(字典 `kind = "test_skipped"`);`RUN_TESTS` 认出它就把
/// 那条测试记成**跳过**(`skipped = TRUE`)。⚠ 标记的形状只在本文件里定义
/// (构造与识别同源)⇒ 门面只改名透传。
///
/// ⚠ `reason` 是**可选**的(0 个实参 ⇒ `NULL`);给了就必须是**字符串**
///   (CI 要能直接读它)。
pub fn kernel_skip(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() > 1 {
        return Err(arity(host, "SKIP", args.len(), 1));
    }
    let reason = match args.first() {
        None => Value::Null,
        Some(Value::String(s)) => Value::String(s.clone()),
        Some(other) => return Err(type_err(host, "SKIP", "string", other)),
    };
    Ok(Outcome::normal(skip_marker(reason)))
}

/// `TEST_CASES(name, cases, body)` —— **参数化**注册。
///
/// `cases` 是**数组的数组**:每个内层数组是**一条用例的实参列表**。每个用例
/// 注册成一条独立的测试,名字是 `<name>[<下标>]` ⇒ 与 `TEST` 的注册语义兼容
/// (同一条注册表、同一种记录形状、同一个 `RUN_TESTS` 排空流程)。
///
/// ⚠ 体与实参**打包**进 `TestEntry.body`(一个二元数组)—— `TestEntry` 只有一个
///   `Value` 位,而实参要跟着体一起进注册表 ⇒ 见 `split_entry_body`。
/// ⚠ 实参的个数与体的形参个数**不**在这里校验:体是闭包时形参数可读,但体也
///   可能是 native fn(数不可读)⇒ 统一交给调用时的元数诊断(E0022)。
pub fn kernel_test_cases(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 3 {
        return Err(arity(host, "TEST_CASES", args.len(), 3));
    }
    let name = match &args[0] {
        Value::String(s) => s.clone(),
        other => return Err(type_err(host, "TEST_CASES", "string", other)),
    };
    let cases = match &args[1] {
        Value::Array(v) => v.clone(),
        other => return Err(type_err(host, "TEST_CASES", "array", other)),
    };
    if !matches!(args[2], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(type_err(host, "TEST_CASES", "function", &args[2]));
    }
    for (i, case) in cases.iter().enumerate() {
        let elements = match case {
            Value::Array(v) => v.clone(),
            other => {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!(
                        "TEST_CASES: case {i} must be an array, got {}",
                        crate::value_kind(other)
                    ),
                ))
            }
        };
        host.ctx().tests.push(TestEntry {
            name: format!("{name}[{i}]"),
            body: Value::Array(vec![args[2].clone(), Value::Array(elements)]),
        });
    }
    Ok(Outcome::normal(Value::Null))
}

/// `TEMP_DIR()` —— 造一个**绝对路径**的临时目录并返回它。
///
/// ⚠⚠ **必须返回绝对路径**:`std.fs` 成员的相对路径按**进程 CWD** 解析 ⇒ 相对
///   路径会把测试文件写进仓库里(`fs_atomic_lock_meta_contract.rs` 踩过的既有
///   教训)。⇒ 本成员**只**返回绝对路径,没有相对路径这一档。
/// ⚠ 目录**不会**被自动删除(标准库没有 teardown 钩子)⇒ 调用方负责清理。
pub fn kernel_temp_dir(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !args.is_empty() {
        return Err(arity(host, "TEMP_DIR", args.len(), 0));
    }
    match make_temp_dir() {
        Ok(path) => Ok(Outcome::normal(Value::String(path))),
        Err(msg) => Err(host.diag(ErrorCode::E0060, format!("TEMP_DIR: {msg}"))),
    }
}

// ─────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────

/// 跳过标记的 `kind` 值(构造与识别**同源**)。
const SKIP_KIND: &str = "test_skipped";

/// 跳过标记字典。
fn skip_marker(reason: Value) -> Value {
    Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String(SKIP_KIND.to_string()),
        ),
        (Value::String("reason".into()), reason),
    ])
}

/// 值是不是跳过标记?是 ⇒ 返回它的 `reason`(可能是 `NULL`)。
fn skip_reason_of(v: &Value) -> Option<Value> {
    let Value::Dict(d) = v else {
        return None;
    };
    match d.get(&Value::String("kind".into())) {
        Some(Value::String(s)) if s == SKIP_KIND => {}
        _ => return None,
    }
    Some(
        d.get(&Value::String("reason".into()))
            .cloned()
            .unwrap_or(Value::Null),
    )
}

/// `TEST_CASES` 注册的体 =「体 + 实参」的二元数组 ⇒ 拆开。
///
/// 普通测试(`TEST` 注册的)体是闭包 / native fn ⇒ 实参为空。
fn split_entry_body(body: &Value) -> (Value, Vec<Value>) {
    if let Value::Array(items) = body {
        if items.len() == 2 {
            if let Value::Array(args) = &items[1] {
                if matches!(items[0], Value::Closure { .. } | Value::NativeFn { .. }) {
                    return (items[0].clone(), args.clone());
                }
            }
        }
    }
    (body.clone(), Vec::new())
}

/// `EXPECT_ERR_CODE` 捕获到**期望的**诊断码时的载荷。
fn expect_err_code_caught(code: &str, message: &str) -> Value {
    Value::dict(vec![
        (
            Value::String("code".into()),
            Value::String(code.to_string()),
        ),
        (
            Value::String("message".into()),
            Value::String(message.to_string()),
        ),
    ])
}

/// `EXPECT_ERR_CODE` **没**捕获到期望诊断码时的载荷(走 `ERR` 值)。
fn expect_err_code_failed(expected: &str, actual: Option<&str>, message: Option<&str>) -> Value {
    let mut entries = vec![
        (
            Value::String("code".into()),
            Value::String("E0049".to_string()),
        ),
        (
            Value::String("kind".into()),
            Value::String("test_expect_err_code_failed".to_string()),
        ),
        (
            Value::String("expected".into()),
            Value::String(expected.to_string()),
        ),
    ];
    if let Some(a) = actual {
        entries.push((Value::String("actual".into()), Value::String(a.to_string())));
    }
    if let Some(m) = message {
        entries.push((
            Value::String("message".into()),
            Value::String(m.to_string()),
        ));
    }
    Value::dict(entries)
}

/// 造一个**绝对路径**的临时目录(已创建好,可直接往里写文件)。
fn make_temp_dir() -> Result<String, String> {
    let base = std::env::temp_dir();
    // `temp_dir()` 在极端配置下可能是**相对**的 ⇒ 兜到 CWD 之下。
    let base = if base.is_absolute() {
        base
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cannot resolve the current directory: {e}"))?
            .join(base)
    };
    // 唯一名:进程 id(跨进程)+ 原子计数(同进程并发)+ 纳秒(跨运行残留)。
    // ⚠ 不靠时钟**单独**保证唯一(时钟粒度比想象里粗 —— 见 `wlwl-eval` 的
    //   `tests/common/mod.rs` 那段实测)。
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = base.join(format!("wlwl-test-{}-{seq}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir.to_string_lossy().into_owned())
}

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
    Value::dict(entries)
}
