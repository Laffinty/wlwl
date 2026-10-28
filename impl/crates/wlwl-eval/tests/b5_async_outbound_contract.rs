//! [B-5] **异步出站**成员（`HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC`）的契约表。
//!
//! ## 它补的是什么
//!
//! `HTTP_REQUEST` / `PROCESS_RUN` 是**阻塞原语**：等待期间**不调度任何其他
//! 任务** ⇒ 整个 `SCOPE` 停摆 ⇒ 「一个 handler 在等 10 秒的慢速响应时，别的
//! handler 根本动不了」。
//!
//! 本项新增两个**异步**变体：出站操作跑在**工作线程**上，等待期间**当前任务
//! 被挂起** ⇒ **别的任务照常跑**。
//!
//! ## 三条硬约束（都是实测出来的，不是偏好）
//!
//! 1. **既有成员一个字都不改** —— `HTTP_GET` / `HTTP_POST` / `HTTP_REQUEST` /
//!    `PROCESS_RUN` **仍然阻塞**（一致性册：「不得修改任一既有成员的签名或
//!    可观察语义」）。
//! 2. **段首重放 ⇒ 必须幂等** —— 宿主挂起 = **不切段**格 ⇒ 恢复时**从挂起点
//!    那句话起整段重跑一遍**（并发册 §17.0）⇒ 若不记住「这个任务已经发起过
//!    什么」，**同一个请求会被发起两次**。
//! 3. **只收数组形态的 `argv`** —— `PROCESS_RUN_ASYNC` 与 `PROCESS_RUN` 同款：
//!    字符串形态是**命令注入**的入口。
//!
//! ## 时间断言为什么写成「区间」
//!
//! 时间相关的断言**不能**钉死精确值（机器负载一变就红 ⇒ 假门禁）⇒ 这里钉
//! 「量级」：慢请求**确实等了**那么久（下界），快请求**确实没被拖住**（上界）。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::time::Duration;
use wlwl_eval::Evaluator;

/// 每个路径的**命中计数**（用来钉「请求只被发起了一次」）。
///
/// ⚠ **按路径**计数 ⇒ 并行跑的用例互不干扰（每个用例用**自己的**路径）。
static HITS: std::sync::LazyLock<Mutex<HashMap<String, usize>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// 慢速端点：睡 [`SLOW_SECS`] 秒才回。
const SLOW_SECS: u64 = 10;

/// 快端点：立刻回。
const FAST_PATH: &str = "/b5-fast";

fn bump(path: &str) {
    let mut m = HITS.lock().unwrap_or_else(|e| e.into_inner());
    *m.entry(path.to_string()).or_insert(0) += 1;
}

fn hits(path: &str) -> usize {
    *HITS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(path)
        .unwrap_or(&0)
}

/// 本机夹具服务器（**只绑 `127.0.0.1`** —— 绝不访问任何外部地址）。
///
/// ⚠ 每条连接**各开一个线程** —— 否则一条慢请求会把后续所有请求一起堵住
///   （而超时用例正要靠它）。
fn start_fixture() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("fixture bind");
    let port = l.local_addr().expect("fixture addr").port();
    std::thread::spawn(move || {
        for conn in l.incoming() {
            let Ok(mut s) = conn else { break };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                bump(&path);
                let out: Vec<u8> = if path.starts_with("/b5-slow") {
                    std::thread::sleep(Duration::from_secs(SLOW_SECS));
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec()
                } else if path.starts_with("/b5-fast") {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nX-Fixture: yes\r\n\r\nok".to_vec()
                } else {
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()
                };
                let _ = s.write_all(&out);
                let _ = s.flush();
            });
        }
    });
    port
}

/// 求值一段 WLWL 源码 ⇒ 可观察的结果（值 / `!E**** 消息`）。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

/// 把 `[a, b, c]` 渲染串拆成整数。
fn ints(out: &str) -> Vec<i64> {
    assert!(out.starts_with('['), "want an array, got {out}");
    out.trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<i64>()
                .unwrap_or_else(|_| panic!("want ints, got {out}"))
        })
        .collect()
}

// ── 验收形态：一个 handler 在等 10 s 的慢速响应时，另一个 1 s 内拿到 200 ──

/// **核心验收**：慢 handler 等 10 s 的响应，**快 handler 1 s 内拿到 200**。
///
/// ⚠ 这就是需求清单 B-5 的验收形态（计划 §4）：
///   - 慢 handler 的任务被**挂起**（不是阻塞）⇒ 运行时**不冻结**；
///   - 快 handler 因此**照常跑完**，拿到 `200`。
///
/// ⚠ 判据写成「区间」：快 handler ≤ 1 s、慢 handler ≥ 9 s。
#[test]
fn a_slow_handler_does_not_freeze_the_other_handlers() {
    let port = start_fixture();
    let slow_url = format!("http://127.0.0.1:{port}/b5-slow-1");
    let fast_url = format!("http://127.0.0.1:{port}{FAST_PATH}-1");
    let src = format!(
        r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]);
IMPORT("wlwl:std.time", ["MONOTONIC"]);
LET(res, SCOPE(FUN(() , (
  LET(slow, SPAWN(FUN(() , (
    LET(t0, MONOTONIC());
    YIELD();
    LET(r, HTTP_REQUEST_ASYNC("GET", "{slow_url}"));
    ["t": -(MONOTONIC(), t0), "r": r]
  ))));
  LET(fast, SPAWN(FUN(() , (
    LET(t0, MONOTONIC());
    YIELD();
    LET(r, HTTP_REQUEST_ASYNC("GET", "{fast_url}"));
    ["t": -(MONOTONIC(), t0), "r": r]
  ))));
  LET(fr, AWAIT(fast));
  LET(sr, AWAIT(slow));
  ["fast": fr, "slow": sr]
))));
[res["fast"]["t"], res["fast"]["r"]["status"], res["slow"]["t"], res["slow"]["r"]["status"]]"#,
    );
    let out = actual(&src);
    let v = ints(&out);
    assert_eq!(v.len(), 4, "want four readings, got {out}");
    let (fast_elapsed, fast_status, slow_elapsed, slow_status) = (v[0], v[1], v[2], v[3]);
    assert_eq!(
        fast_status, 200,
        "the fast handler must get its 200, got {out}"
    );
    assert!(
        fast_elapsed < 1000,
        "the fast handler must NOT wait for the slow one ({fast_elapsed}ms), got {out}"
    );
    assert_eq!(
        slow_status, 200,
        "the slow handler must still get its 200, got {out}"
    );
    assert!(
        slow_elapsed >= (SLOW_SECS as i64 * 1000) - 500,
        "the slow handler must really wait ~{SLOW_SECS}s (waited {slow_elapsed}ms), got {out}"
    );
}

/// **同一个慢请求只被发起一次**（幂等：段首重放 ⇒ 不重复发起）。
///
/// ⚠ 这是**反向守卫 ①** 的正向锚点：把幂等判定去掉 ⇒ 这里立刻变红
///   （计数从 1 变成 N）。
#[test]
fn the_request_is_issued_exactly_once_even_though_the_task_is_replayed() {
    let port = start_fixture();
    let url = format!("http://127.0.0.1:{port}/b5-slow-2");
    let src = format!(
        r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (
  LET(r, HTTP_REQUEST_ASYNC("GET", "{url}"));
  r
))))))"#,
    );
    let out = actual(&src);
    assert!(
        out.contains("200"),
        "the async request must succeed, got {out}"
    );
    // ⚠ **等一下**再数：若有**多出来**的那一次请求（反向守卫 ① 的形态），它是在
    //   **工作线程**上发起的 ⇒ 断言若**抢跑**，会在它落地之前就数完 ⇒ 假绿。
    //   ⇒ 静置 250 ms（连接建立是毫秒级）⇒ 计数才是真的。
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert_eq!(
        hits("/b5-slow-2"),
        1,
        "the request must be issued exactly once — the task is replayed after the \
         suspension, and the idempotency memo is what keeps it from being re-issued"
    );
}

// ── 返回值形状：与阻塞的兄弟成员**同形** ──────────────────────────────────

/// 异步变体的返回值与 `HTTP_REQUEST` **逐字同形**。
#[test]
fn the_async_result_has_the_same_shape_as_the_blocking_one() {
    let port = start_fixture();
    let url = format!("http://127.0.0.1:{port}/b5-fast-shape");
    let src = format!(
        r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST", "HTTP_REQUEST_ASYNC"]);
LET(a, HTTP_REQUEST("GET", "{url}"));
LET(b, SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (HTTP_REQUEST_ASYNC("GET", "{url}"))))))));
[a, b]"#,
    );
    let out = actual(&src);
    // 两个 DICT 的渲染必须逐字相同（同样的 status / headers / body）。
    // ⚠ `Value::display` 把 DICT 渲染成 `[键: 值, …]` ⇒ 分隔符是 `], [`。
    let inner = out
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split_once("], [")
        .unwrap_or_else(|| panic!("want two dicts, got {out}"));
    assert_eq!(
        inner.0, inner.1,
        "the async result must have the same shape as the blocking one, got {out}"
    );
    assert!(
        out.contains("status: 200"),
        "the result must be the usual DICT, got {out}"
    );
}

// ── 失败口径：运行期失败是 `ERR` **值**，元数/类型错是既有诊断码 ────────────

/// 运行期失败（连不上）⇒ `ERR` **值**（不是诊断）。
#[test]
fn a_runtime_failure_is_an_err_value_not_a_diagnostic() {
    // 端口 1 上没有人监听 ⇒ 连接被拒 ⇒ `ERR([kind:"HttpError", …])`。
    //
    // ⚠ `IS_ERR(...)` **直接**包住那次调用 ⇒ `ERR` **当场被消费** ⇒ 不会触发
    //   §12.6 的「未消费的 `ERR` 被丢弃」（`E0103`）。
    let src = r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (
  IS_ERR(HTTP_REQUEST_ASYNC("GET", "http://127.0.0.1:1/nope"))
))))))"#;
    let out = actual(src);
    assert_eq!(
        out, "TRUE",
        "a runtime failure must be an ERR value, got {out}"
    );
}

/// 元数错 ⇒ `E0022`；类型错 ⇒ `E0030`（**既有**码；本轮不新增诊断码）。
#[test]
fn arity_and_type_errors_keep_the_existing_codes() {
    let arity = actual(
        r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]);
HTTP_REQUEST_ASYNC("GET")"#,
    );
    assert!(
        arity.starts_with("!E0022"),
        "a wrong argument count must stay E0022, got {arity}"
    );
    let ty = actual(
        r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]);
HTTP_REQUEST_ASYNC(1, "http://127.0.0.1/x")"#,
    );
    assert!(
        ty.starts_with("!E0030"),
        "a wrong argument type must stay E0030, got {ty}"
    );
    let proc_ty = actual(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN_ASYNC"]);
PROCESS_RUN_ASYNC("echo hi")"#,
    );
    assert!(
        proc_ty.starts_with("!E0030"),
        "a string argv must stay E0030 (that form is a command-injection entry), got {proc_ty}"
    );
}

/// `opts` 的**未知键 ⇒ `E0030`**（不静默忽略 —— 拼错的上界会变成「没有上界」）。
#[test]
fn an_unknown_opts_key_is_an_error_not_silently_ignored() {
    let out = actual(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN_ASYNC"]);
PROCESS_RUN_ASYNC(["cmd", "/C", "echo hi"], ["timeuot": 10])"#,
    );
    assert!(
        out.starts_with("!E0030"),
        "an unknown opts key must be E0030, got {out}"
    );
}

// ── `PROCESS_RUN_ASYNC` ────────────────────────────────────────────────────

/// `PROCESS_RUN_ASYNC` 的返回值与 `PROCESS_RUN` **同形**。
#[test]
fn the_async_process_result_has_the_same_shape_as_the_blocking_one() {
    let argv: Vec<&str> = if cfg!(windows) {
        vec!["cmd", "/C", "echo", "hello"]
    } else {
        vec!["sh", "-c", "echo hello"]
    };
    let argv_wl = argv
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN", "PROCESS_RUN_ASYNC"]);
LET(a, PROCESS_RUN([{argv_wl}]));
LET(b, SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (PROCESS_RUN_ASYNC([{argv_wl}]))))))));
[a, b]"#,
    );
    let out = actual(&src);
    let inner = out
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split_once("], [")
        .unwrap_or_else(|| panic!("want two dicts, got {out}"));
    assert_eq!(
        inner.0, inner.1,
        "the async process result must have the same shape as the blocking one, got {out}"
    );
    assert!(
        out.contains("code: 0"),
        "the process must succeed, got {out}"
    );
}

/// `PROCESS_RUN_ASYNC` 的**超时** ⇒ `ERR` **值**（同 `PROCESS_RUN`）。
#[test]
fn an_async_process_timeout_is_an_err_value() {
    let argv: Vec<&str> = if cfg!(windows) {
        vec!["cmd", "/C", "ping -n 30 127.0.0.1 > nul"]
    } else {
        vec!["sh", "-c", "sleep 30"]
    };
    let argv_wl = argv
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (
  IS_ERR(PROCESS_RUN_ASYNC([{argv_wl}], ["timeout": 300]))
))))))"#,
    );
    let out = actual(&src);
    assert_eq!(
        out, "TRUE",
        "a timed-out async process must be an ERR value, got {out}"
    );
}

/// 一个**慢子进程**不冻结别的任务（与 HTTP 那边同款判据）。
#[test]
fn a_slow_child_process_does_not_freeze_the_other_tasks() {
    let slow: Vec<&str> = if cfg!(windows) {
        vec!["cmd", "/C", "ping -n 4 127.0.0.1 > nul"]
    } else {
        vec!["sh", "-c", "sleep 3"]
    };
    let slow_wl = slow
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let fast_wl = if cfg!(windows) {
        r#""cmd", "/C", "echo", "hi""#.to_string()
    } else {
        r#""sh", "-c", "echo hi""#.to_string()
    };
    let src = format!(
        r#"IMPORT("wlwl:std.process", ["PROCESS_RUN_ASYNC"]);
IMPORT("wlwl:std.time", ["MONOTONIC"]);
LET(res, SCOPE(FUN(() , (
  LET(slow, SPAWN(FUN(() , (
    LET(t0, MONOTONIC());
    YIELD();
    LET(r, PROCESS_RUN_ASYNC([{slow_wl}]));
    -(MONOTONIC(), t0)
  ))));
  LET(fast, SPAWN(FUN(() , (
    LET(t0, MONOTONIC());
    YIELD();
    LET(r, PROCESS_RUN_ASYNC([{fast_wl}]));
    -(MONOTONIC(), t0)
  ))));
  LET(fr, AWAIT(fast));
  LET(sr, AWAIT(slow));
  [fr, sr]
))));
res"#,
    );
    let out = actual(&src);
    let v = ints(&out);
    assert_eq!(v.len(), 2, "want two readings, got {out}");
    let (fast_elapsed, slow_elapsed) = (v[0], v[1]);
    assert!(
        fast_elapsed < 1000,
        "the fast child must not wait for the slow one ({fast_elapsed}ms), got {out}"
    );
    assert!(
        slow_elapsed >= 2000,
        "the slow child must really run (~3s), waited {slow_elapsed}ms, got {out}"
    );
}

// ── 任务体外调用 ⇒ `E0014`（指名成员） ────────────────────────────────────

/// 宿主挂起点成员在**任务体外**被调用 ⇒ `E0014`（指名该成员）。
#[test]
fn calling_the_async_member_outside_a_task_body_is_e0014() {
    let out = actual(
        r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]);
HTTP_REQUEST_ASYNC("GET", "http://127.0.0.1:1/x")"#,
    );
    assert!(
        out.starts_with("!E0014"),
        "a host-suspend member outside a task body must be E0014, got {out}"
    );
    assert!(
        out.contains("HTTP_REQUEST_ASYNC"),
        "the diagnostic must name the member, got {out}"
    );
}

/// 计数用的**自检**（夹具本身没坏 ⇒ 下面的计数断言才有意义）。
#[test]
fn the_fixture_counts_what_it_serves() {
    let port = start_fixture();
    let url = format!("http://127.0.0.1:{port}/b5-fast-selfcheck");
    let src = format!(
        r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (HTTP_REQUEST_ASYNC("GET", "{url}")))))))"#,
    );
    let out = actual(&src);
    assert!(out.contains("200"), "the fixture must serve, got {out}");
    assert_eq!(
        hits("/b5-fast-selfcheck"),
        1,
        "the fixture's counter must work"
    );
}

/// 兜底：`hits` 的读取本身不许 panic（空表也要能读）。
#[test]
fn an_unseen_path_has_zero_hits() {
    assert_eq!(hits("/b5-never-used"), 0);
}
