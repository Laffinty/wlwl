//! `wlwl:std.http` 中间件成员契约 —— W-25 · D-3 / D-5 / D-6 / D-7。
//!
//! 覆盖（验收读数三条都钉住）：
//! 1. **限流**（D-3）：60 次 / 60 秒滑动窗口下第 61 次被拒；令牌桶模式并列；
//!    并发下计数精确（`COUNTER_INC` 原子）。
//! 2. **规则集热加载**（D-5）：mtime 变更 ⇒ 原子换引用 ⇒ 在途持有的旧数组
//!    内容不变、`version` 递增；mtime 未变 ⇒ 复用（`cached`）。
//! 3. **审计 JSON 行**（D-6）：落盘的那一行是**合法 JSON**，字段含
//!    时间 / 事件 / 客户端 IP / 路径 / 结果 / 请求 ID（时间 = Unix 纪元毫秒）。
//! 4. **真实 IP**（D-7）：可信代理才采信 XFF（最右非可信跳）；不可信远端
//!    ⇒ 无视 XFF（攻击者自带 XFF 不得当真实 IP）。
//!
//! ⚠ 中间件状态按 name 复用一张 `SHARED_DICT` 注册表。缺省注册表挂在
//!   `StdCtx.env`（**求值器级**）⇒ 单程序用例天然隔离；跨求值器 / 跨线程
//!   的用例显式传 `opts.state`（`SHARED_DICT` 句柄，进程级）。日志 sink
//!   （`LOG_FILE`）是**进程级**的 ⇒ 全部用例持 [`gate`] 串行跑，避免互污。
//! ⚠ IO 用例**一律绝对路径**（相对路径按进程 CWD 解析 ⇒ 会污染工作树）。
//! ⚠ mtime 热切换用例假定文件系统 mtime 粒度 ≤ 300ms（NTFS / ext4 满足）。

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use wlwl_eval::Evaluator;

/// 进程内状态（env 注册表 + 共享计数器）⇒ 带状态用例串行。
static GATE: Mutex<()> = Mutex::new(());

fn gate() -> MutexGuard<'static, ()> {
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn scratch(prefix: &str) -> PathBuf {
    common::scratch(prefix)
}

/// ⚠ 绝对路径、分隔符统一 `/`（与 `CAT(root, "/", rel)` 的口径一致）。
fn abs(dir: &Path, rel: &str) -> String {
    dir.join(rel).to_string_lossy().replace('\\', "/")
}

fn run(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(std::env::temp_dir());
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

/// 只锁码的期望（`!E0030` 形式）⇒ 前缀匹配。
fn is_code(expect: &str, got: &str) -> bool {
    expect.len() == 6 && expect.starts_with('!') && got.starts_with(expect)
}

/// 一张**共享**注册表（`std.sync::SHARED_DICT` 句柄）—— 跨求值器 / 跨线程
/// 的用例把它当 `opts.state` 传（句柄是进程级的）。
fn shared_registry() -> i64 {
    run(r#"IMPORT("wlwl:std.sync", ["SHARED_DICT"]); STR(SHARED_DICT())"#)
        .parse()
        .expect("the handle prints as a decimal integer")
}

// ---------------------------------------------------------------------------
// 导出面（本地守卫；共享名单断言在 `http_contract.rs`，那是集成者的表）
// ---------------------------------------------------------------------------

#[test]
fn the_export_surface_is_the_frozen_sixteen_in_order() {
    let backend = wlwl_std::resolve("wlwl:std.http").expect("std.http resolves");
    let wlwl_std::StdBackend::Lang(src) = backend else {
        panic!("std.http is a language module");
    };
    let names = wlwl_std::lang_exports(src.source);
    let want = [
        "PARSE_REQUEST",
        "BUILD_RESPONSE",
        "SERVE_STREAM",
        "ROUTE",
        "MATCH_ROUTE",
        "PARSE_COOKIES",
        "SET_COOKIE",
        "PARSE_FORM",
        "PARSE_MULTIPART",
        "STATIC_FILE",
        "GET",
        "POST",
        "REQUEST",
        "RATE_LIMITER",
        "RULESET_LOAD",
        "CLIENT_IP",
    ];
    assert_eq!(
        names, want,
        "EXPORT must stay a single line holding the frozen sixteen names in order"
    );
}

// ---------------------------------------------------------------------------
// D-3 · RATE_LIMITER
// ---------------------------------------------------------------------------

/// 验收读数 ①：60 次 / 60 秒滑动窗口 ⇒ 第 61 次 `allowed = FALSE`。
#[test]
fn sliding_window_denies_the_sixty_first_call() {
    let _g = gate();
    let src = r#"
IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET MUT(a, 0);
LET MUT(i, 0);
LET MUT(last, ["allowed": TRUE, "remaining": -1]);
WHILE(<(i, 61),
    (LET(r, RATE_LIMITER("t61", 60, 60, ["audit": FALSE]));
     IF(AT(r, "allowed", FALSE), SET(a, +(a, 1)), NULL);
     SET(last, r);
     SET(i, +(i, 1))));
CAT(STR(a), "|", STR(AT(last, "allowed", TRUE)), "|", STR(AT(last, "remaining", -1)), "|", STR(>=(AT(last, "reset", 0), 0)), "|", AT(last, "mode", ""))
"#;
    assert_eq!(run(src), "60|FALSE|0|TRUE|sliding");
}

/// 滑动窗口真的会**滑**：1 秒窗口过期后重新放行。
#[test]
fn sliding_window_expires_and_allows_again() {
    let _g = gate();
    let src = r#"
IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
IMPORT("wlwl:std.time", ["NOW"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(r1, RATE_LIMITER("tslide", 2, 1, ["audit": FALSE]));
LET(r2, RATE_LIMITER("tslide", 2, 1, ["audit": FALSE]));
LET(r3, RATE_LIMITER("tslide", 2, 1, ["audit": FALSE]));
LET(t0, NOW());
WHILE(<(NOW(), +(t0, 1100)), NULL);
LET(r4, RATE_LIMITER("tslide", 2, 1, ["audit": FALSE]));
CAT(STR(AT(r1, "allowed", FALSE)), ",", STR(AT(r2, "allowed", FALSE)), ",", STR(AT(r3, "allowed", FALSE)), ",", STR(AT(r4, "allowed", FALSE)))
"#;
    assert_eq!(run(src), "TRUE,TRUE,FALSE,TRUE");
}

/// 令牌桶与滑动窗口**并列**：容量耗尽 ⇒ 拒；按 `refill` 补给后再放行。
#[test]
fn token_bucket_consumes_denies_and_refills() {
    let _g = gate();
    let src = r#"
IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
IMPORT("wlwl:std.time", ["NOW"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(r1, RATE_LIMITER("ttok", 3, 60, ["mode": "token_bucket", "refill": 1, "audit": FALSE]));
LET(r2, RATE_LIMITER("ttok", 3, 60, ["mode": "token_bucket", "refill": 1, "audit": FALSE]));
LET(r3, RATE_LIMITER("ttok", 3, 60, ["mode": "token_bucket", "refill": 1, "audit": FALSE]));
LET(r4, RATE_LIMITER("ttok", 3, 60, ["mode": "token_bucket", "refill": 1, "audit": FALSE]));
LET(t0, NOW());
WHILE(<(NOW(), +(t0, 1100)), NULL);
LET(r5, RATE_LIMITER("ttok", 3, 60, ["mode": "token_bucket", "refill": 1, "audit": FALSE]));
CAT(STR(AT(r1, "allowed", FALSE)), ",", STR(AT(r2, "allowed", FALSE)), ",", STR(AT(r3, "allowed", FALSE)), ",", STR(AT(r4, "allowed", FALSE)), ",", STR(AT(r5, "allowed", FALSE)), "|", STR(AT(r1, "remaining", -1)), "|", AT(r1, "mode", ""))
"#;
    assert_eq!(run(src), "TRUE,TRUE,TRUE,FALSE,TRUE|2|token_bucket");
}

/// `refill = 0` ⇒ 桶**永不**补给（时间流逝也不放行）。
#[test]
fn token_bucket_with_zero_refill_never_refills() {
    let _g = gate();
    let src = r#"
IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
IMPORT("wlwl:std.time", ["NOW"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(r1, RATE_LIMITER("ttok0", 2, 60, ["mode": "token_bucket", "refill": 0, "audit": FALSE]));
LET(r2, RATE_LIMITER("ttok0", 2, 60, ["mode": "token_bucket", "refill": 0, "audit": FALSE]));
LET(r3, RATE_LIMITER("ttok0", 2, 60, ["mode": "token_bucket", "refill": 0, "audit": FALSE]));
LET(t0, NOW());
WHILE(<(NOW(), +(t0, 1100)), NULL);
LET(r4, RATE_LIMITER("ttok0", 2, 60, ["mode": "token_bucket", "refill": 0, "audit": FALSE]));
CAT(STR(AT(r1, "allowed", FALSE)), ",", STR(AT(r2, "allowed", FALSE)), ",", STR(AT(r3, "allowed", FALSE)), ",", STR(AT(r4, "allowed", FALSE)))
"#;
    assert_eq!(run(src), "TRUE,TRUE,FALSE,FALSE");
}

/// 同名复用同一桶（`limit = 1` 时第二次调用必须被拒）。
#[test]
fn the_same_name_reuses_one_bucket() {
    let _g = gate();
    let src = r#"
IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(r1, RATE_LIMITER("tname", 1, 60, ["audit": FALSE]));
LET(r2, RATE_LIMITER("tname", 1, 60, ["audit": FALSE]));
CAT(STR(AT(r1, "allowed", FALSE)), "|", STR(AT(r2, "allowed", FALSE)), "|", STR(AT(r2, "remaining", -1)))
"#;
    assert_eq!(run(src), "TRUE|FALSE|0");
}

/// 参数 / 类型 / 未知键的失败口径：元数与类型 ⇒ `E0030`；可预期的参数错
/// ⇒ `ERR` **值**（`HttpError`），**不新增诊断码**。
#[test]
fn rate_limiter_failure_surface() {
    let _g = gate();
    // 类型错 ⇒ E0030（既有诊断）
    let got = run(r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); RATE_LIMITER("tbad", "60", 60)"#);
    assert!(is_code("!E0030", &got), "limit must be INTEGER, got {got}");
    let got = run(r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); RATE_LIMITER("tbad", 60, 60, 5)"#);
    assert!(is_code("!E0030", &got), "opts must be DICT, got {got}");
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 60, 60, ["audit": "yes"])); LET(_, r); IF(IS_ERR(r), "ERR", "OK")"#,
    );
    assert_eq!(
        got, "ERR",
        "an opts value of the wrong type must be an ERR value"
    );
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 60, 60, ["state": "x"])); LET(_, r); IF(IS_ERR(r), "ERR", "OK")"#,
    );
    assert_eq!(got, "ERR", "state must be an INTEGER handle");
    // 可预期的参数错 ⇒ ERR 值（op = RATE_LIMITER）
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 0, 60, ["audit": FALSE])); LET(_, r); IF(IS_ERR(r), "ERR", "OK")"#,
    );
    assert_eq!(got, "ERR", "limit < 1 must be an ERR value");
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 5, 0, ["audit": FALSE])); LET(_, r); IF(IS_ERR(r), "ERR", "OK")"#,
    );
    assert_eq!(got, "ERR", "window < 1 must be an ERR value");
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 5, 60, ["bogus": 1])); LET(_, r); IF(IS_ERR(r), "ERR", "OK")"#,
    );
    assert_eq!(got, "ERR", "unknown option must be an ERR value");
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 5, 60, ["mode": "bogus"])); LET(_, r); IF(IS_ERR(r), "ERR", "OK")"#,
    );
    assert_eq!(got, "ERR", "unknown mode must be an ERR value");
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 5, 60, ["mode": "sliding", "refill": 1])); LET(_, r); IF(IS_ERR(r), "ERR", "OK")"#,
    );
    assert_eq!(got, "ERR", "refill with sliding must be an ERR value");
    // ERR 载荷形状：op 逐字是成员名
    let got = run(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]); LET(r, RATE_LIMITER("tbad", 0, 60, ["audit": FALSE])); LET(_, r); AT(ERR_PAYLOAD(r), "op", "")"#,
    );
    assert_eq!(got, "RATE_LIMITER");
}

/// **并发下计数必须精确**（`COUNTER_INC` 原子）：8 线程 × 2500 次 + 前后
/// 各一次读 ⇒ 总入账恰好 20002。反向守卫：非原子计数 ⇒ 本用例**必须红**。
///
/// ⚠ **为什么用令牌桶（`refill = 0`）而不是滑动窗口**：
///   滑动窗口的 `remaining` 是「最近 `window` 秒的**窗口和**」⇒ 期望值
///   **依赖墙钟**。这 20002 次调用本身要跑 **~60 s**（本机实测 61.5 s）
///   ⇒ 机器一慢，最早的那些调用就**滑出窗口** ⇒ 期望值失真（本机实测
///   `980510 ≠ 979998`，少 512 次 —— 正是 ~1.5 s 的那批调用）。
///   ⚠ 那**不是丢更新**，是**窗口过期** ⇒ 违反 §8.3 第 8 条「用确定性计数、
///   不用墙钟」（一致性册 §6.2 V-01）⇒ 这就是登记 ⑰ 那次偶发红的真身。
///   令牌桶（`refill = 0`）把全部计数放进**同一个**计数器 ⇒ 总量与墙钟无关。
///   ⇒ 修完之后，本用例**只有一种**变红的理由：计数真的丢了（原先有两种：
///   丢更新 **或** 窗口过期 ⇒ 它当不了守卫）。
#[test]
fn concurrent_counting_is_exact_under_threads() {
    let _g = gate();
    let name = format!("tconc{}", std::process::id());
    // 跨线程共享的注册表（进程级句柄）⇒ 4 个求值器同 name 复用同一计数。
    let h = shared_registry();
    // 先单线程跑一次：完成注册表里该 name 的桶创建（避开并发首呼的创建竞态）。
    let warm = format!(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
LET(r, RATE_LIMITER("{name}", 1000000, 60, ["mode": "token_bucket", "refill": 0, "state": {h}, "audit": FALSE]));
STR(AT(r, "remaining", 0))"#
    );
    assert_eq!(run(&warm), "999999", "the warmup call claims one slot");
    let mut joins = vec![];
    for _ in 0..8 {
        let name = name.clone();
        joins.push(std::thread::spawn(move || {
            let src = format!(
                r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
LET MUT(i, 0);
LET MUT(ok, 0);
WHILE(<(i, 2500),
    (LET(r, RATE_LIMITER("{name}", 1000000, 60, ["mode": "token_bucket", "refill": 0, "state": {h}, "audit": FALSE]));
     IF(AT(r, "allowed", FALSE), SET(ok, +(ok, 1)), NULL);
     SET(i, +(i, 1))));
STR(ok)"#
            );
            run(&src)
        }));
    }
    for j in joins {
        assert_eq!(
            j.join().expect("thread"),
            "2500",
            "every call is admitted (limit is huge)"
        );
    }
    // 1 次预热 + 20000 次并发 + 1 次读回 = 20002 次入账 ⇒ remaining 精确。
    let read = format!(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
LET(r, RATE_LIMITER("{name}", 1000000, 60, ["mode": "token_bucket", "refill": 0, "state": {h}, "audit": FALSE]));
STR(AT(r, "remaining", 0))"#
    );
    assert_eq!(run(&read), "979998", "no lost updates under concurrency");
}

/// 滑动窗口模式下的并发覆盖 —— **只断言与墙钟无关的事实**：限量大时每次
/// 调用都必须被放行（不许误拒）；并发首呼的**跨秒建桶竞态**也不许把调用
/// 变成 `ERR` 或误拒。
///
/// ⚠ 这里**不**断言「总入账恰好 N」—— 滑窗的 `remaining` 是**窗口和**
/// ⇒ 那个期望值依赖墙钟（见上一条用例的注释）⇒ 精确性只在令牌桶那条里钉。
#[test]
fn concurrent_sliding_window_never_denies_while_the_limit_is_huge() {
    let _g = gate();
    let name = format!("tcondsl{}", std::process::id());
    let h = shared_registry();
    let mut joins = vec![];
    for _ in 0..4 {
        let name = name.clone();
        joins.push(std::thread::spawn(move || {
            let src = format!(
                r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
LET MUT(i, 0);
LET MUT(ok, 0);
WHILE(<(i, 500),
    (LET(r, RATE_LIMITER("{name}", 1000000, 60, ["state": {h}, "audit": FALSE]));
     IF(AT(r, "allowed", FALSE), SET(ok, +(ok, 1)), NULL);
     SET(i, +(i, 1))));
STR(ok)"#
            );
            run(&src)
        }));
    }
    for j in joins {
        assert_eq!(
            j.join().expect("thread"),
            "500",
            "every sliding-window call is admitted (the limit is huge)"
        );
    }
}

// ---------------------------------------------------------------------------
// D-5 · RULESET_LOAD
// ---------------------------------------------------------------------------

/// 规则文件文法：每行一条；空行 / `#` 注释跳过；CRLF 的 `\r` 剥掉。
#[test]
fn ruleset_load_parses_lines_and_skips_noise() {
    let _g = gate();
    let dir = scratch("wlwl_rules_parse");
    let p = abs(&dir, "rules.txt");
    std::fs::write(&p, "rule-a\n\n# comment\nrule-b\r\n").unwrap();
    let src = format!(
        r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
IMPORT("wlwl:std.str", ["CAT", "JOIN"]);
LET(r, RULESET_LOAD("{p}", ["audit": FALSE]));
CAT(JOIN(AT(r, "rules", []), ","), "|", STR(AT(r, "version", 0)), "|", STR(>(AT(r, "mtime", 0), 0)))"#
    );
    assert_eq!(run(&src), "rule-a,rule-b|1|TRUE");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 验收读数 ②：mtime 变更 ⇒ 原子换引用 ⇒ **在途持有的旧数组内容不变**、
/// `version` 递增；mtime 未变 ⇒ 复用（`version` 不动）。
#[test]
fn ruleset_hot_swap_keeps_the_inflight_array() {
    let _g = gate();
    let dir = scratch("wlwl_rules_swap");
    let p = abs(&dir, "rules.txt");
    let src = format!(
        r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
IMPORT("wlwl:std.fs", ["WRITE_FILE"]);
IMPORT("wlwl:std.time", ["NOW"]);
IMPORT("wlwl:std.str", ["CAT", "JOIN"]);
LET(_, WRITE_FILE("{p}", "rule-a\nrule-b\n"));
LET(r1, RULESET_LOAD("{p}", ["audit": FALSE]));
LET(t0, NOW());
WHILE(<(NOW(), +(t0, 1100)), NULL);
LET(_, WRITE_FILE("{p}", "rule-x\n"));
LET(r2, RULESET_LOAD("{p}", ["audit": FALSE]));
LET(r3, RULESET_LOAD("{p}", ["audit": FALSE]));
CAT(JOIN(AT(r1, "rules", []), ","), "|", JOIN(AT(r2, "rules", []), ","), "|", STR(AT(r1, "version", 0)), ",", STR(AT(r2, "version", 0)), "|", STR(AT(r3, "version", 0)), "|", STR(>=(AT(r2, "mtime", 0), AT(r1, "mtime", 0))))"#
    );
    assert_eq!(run(&src), "rule-a,rule-b|rule-x|1,2|2|TRUE");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 文件不在 ⇒ `ERR` **值**（op = RULESET_LOAD）；未知键 ⇒ `ERR`；类型错 ⇒ `E0030`。
#[test]
fn ruleset_failure_surface() {
    let _g = gate();
    let dir = scratch("wlwl_rules_err");
    let missing = abs(&dir, "absent.txt");
    let p = abs(&dir, "rules.txt");
    std::fs::write(&p, "rule-a\n").unwrap();
    let got = run(&format!(
        r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(r, RULESET_LOAD("{missing}", ["audit": FALSE]));
LET(_, r);
IF(IS_ERR(r), CAT("ERR|", AT(ERR_PAYLOAD(r), "op", "")), "OK")"#
    ));
    assert_eq!(got, "ERR|RULESET_LOAD");
    let got = run(&format!(
        r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
LET(r, RULESET_LOAD("{p}", ["bogus": 1]));
LET(_, r);
IF(IS_ERR(r), "ERR", "OK")"#
    ));
    assert_eq!(got, "ERR", "unknown option must be an ERR value");
    let got = run(r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]); RULESET_LOAD(5)"#);
    assert!(is_code("!E0030", &got), "path must be STRING, got {got}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 规则热切换对**并发读者**永远呈现完整的一代（原子换引用）。
/// 反向守卫：「原地 / 分步改旧数组」⇒ 本用例**必须红**。
#[test]
fn ruleset_hot_swap_never_exposes_a_torn_entry() {
    let _g = gate();
    let dir = scratch("wlwl_rules_race");
    let p = abs(&dir, "rules.txt");
    let h = shared_registry();
    let a_set: Vec<String> = (1..=200).map(|i| format!("a{i}")).collect();
    let b_set: Vec<String> = (1..=200).map(|i| format!("b{i}")).collect();
    let a_join = a_set.join(",");
    let b_join = b_set.join(",");
    std::fs::write(&p, a_set.join("\n")).unwrap();
    // 预热：建好缓存与注册表。
    let warm = format!(
        r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
LET(r, RULESET_LOAD("{p}", ["state": {h}, "audit": FALSE]));
STR(LEN(AT(r, "rules", [])))"#
    );
    assert_eq!(run(&warm), "200");
    // 写者：交替写入两代规则并触发热切换（每次写前等 mtime 粒度）。
    let writer = std::thread::spawn({
        let p = p.clone();
        move || {
            let src = format!(
                r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
IMPORT("wlwl:std.fs", ["WRITE_FILE"]);
IMPORT("wlwl:std.time", ["NOW"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(p, "{p}");
LET MUT(k, 0);
LET MUT(ver, 0);
WHILE(<(k, 6),
    (LET(t0, NOW());
     WHILE(<(NOW(), +(t0, 300)), NULL);
     LET(prefix, IF(==(%(k, 2), 0), "b", "a"));
     LET MUT(c, "");
     LET MUT(j, 1);
     WHILE(<(j, 201), (SET(c, CAT(c, prefix, STR(j), "\n")); SET(j, +(j, 1))));
     LET(_, WRITE_FILE(p, c));
     LET(r, RULESET_LOAD(p, ["state": {h}, "audit": FALSE]));
     SET(ver, AT(r, "version", 0));
     SET(k, +(k, 1))));
STR(ver)"#
            );
            run(&src)
        }
    });
    // 读者：持续读缓存；任何一刻都必须拿到**完整**的某一代。
    let mut readers = vec![];
    for _ in 0..3 {
        let p = p.clone();
        let a_join = a_join.clone();
        let b_join = b_join.clone();
        readers.push(std::thread::spawn(move || {
            let src = format!(
                r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
IMPORT("wlwl:std.time", ["NOW"]);
IMPORT("wlwl:std.str", ["CAT", "JOIN"]);
LET(p, "{p}");
LET(t0, NOW());
LET MUT(bad, 0);
LET MUT(seen, 0);
WHILE(<(NOW(), +(t0, 4000)),
    (LET(r, RULESET_LOAD(p, ["state": {h}, "audit": FALSE]));
     LET(j, JOIN(AT(r, "rules", []), ","));
     IF(||(==("{a_join}", j), ==("{b_join}", j)), NULL, SET(bad, +(bad, 1)));
     SET(seen, +(seen, 1))));
CAT(STR(bad), "|", STR(seen))"#
            );
            run(&src)
        }));
    }
    let ver = writer.join().expect("writer");
    // 预热 = 1 号；6 次热切换每次至少一个重读拿新号 ⇒ 末次读回 ≥ 7。
    assert!(
        ver.parse::<i64>().unwrap_or(0) >= 7,
        "six reloads on top of the warmup load, got ver={ver}"
    );
    for r in readers {
        let out = r.join().expect("reader");
        let (bad, seen) = out.split_once('|').expect("bad|seen");
        assert_eq!(bad, "0", "a torn/partial ruleset entry was observed");
        assert!(seen.parse::<i64>().unwrap_or(0) > 0, "reader made no reads");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// D-6 · 审计 JSON 行（不导出的 `_AUDIT` 经三个新成员触发）
// ---------------------------------------------------------------------------

/// 验收读数 ③：审计行是**合法 JSON**（`std.json::PARSE` 无错）且字段含
/// 时间（Unix 毫秒，> 0）/ 事件 / 客户端 IP / 路径 / 结果 / 请求 ID。
#[test]
fn audit_line_is_json_with_the_six_fields() {
    let _g = gate();
    let dir = scratch("wlwl_audit");
    let ap = abs(&dir, "audit.log");
    let src = format!(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
IMPORT("wlwl:std.log", ["LOG_FILE"]);
IMPORT("wlwl:std.fs", ["READ_FILE"]);
IMPORT("wlwl:std.json", ["PARSE"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(_, LOG_FILE("{ap}"));
LET(r, RATE_LIMITER("taudit", 5, 60, ["client_ip": "203.0.113.7", "path": "/admin", "request_id": "req-42"]));
LET(txt, READ_FILE("{ap}"));
LET(line, TRIM(AT(SPLIT(txt, "\n"), 0, "")));
LET(obj, PARSE(line));
LET(_, obj);
IF(IS_ERR(obj),
   "PARSE_FAIL",
   (LET(f, AT(obj, "fields", DICT()));
    CAT(STR(==(TYPE(obj), "DICT")), "|", AT(obj, "level", ""), "|", AT(obj, "msg", ""),
        "|", STR(>(AT(f, "time", 0), 0)), "|", AT(f, "event", ""), "|", AT(f, "client_ip", ""),
        "|", AT(f, "path", ""), "|", AT(f, "result", ""), "|", AT(f, "request_id", ""))))"#
    );
    assert_eq!(
        run(&src),
        "TRUE|INFO|http.audit|TRUE|rate_limit|203.0.113.7|/admin|allowed|req-42"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `audit = FALSE` ⇒ 一行都不落。
#[test]
fn audit_can_be_switched_off() {
    let _g = gate();
    let dir = scratch("wlwl_audit_off");
    let ap = abs(&dir, "audit.log");
    let src = format!(
        r#"IMPORT("wlwl:std.http", ["RATE_LIMITER"]);
IMPORT("wlwl:std.log", ["LOG_FILE"]);
IMPORT("wlwl:std.fs", ["READ_FILE"]);
LET(_, LOG_FILE("{ap}"));
LET(r, RATE_LIMITER("tauditoff", 5, 60, ["audit": FALSE]));
READ_FILE("{ap}")"#
    );
    assert_eq!(run(&src), "");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `RULESET_LOAD` 的审计 `result` 口径：`loaded` → `reloaded` → `cached`。
#[test]
fn audit_ruleset_result_tokens() {
    let _g = gate();
    let dir = scratch("wlwl_audit_rs");
    let ap = abs(&dir, "audit.log");
    let p = abs(&dir, "rules.txt");
    std::fs::write(&p, "rule-a\n").unwrap();
    let src = format!(
        r#"IMPORT("wlwl:std.http", ["RULESET_LOAD"]);
IMPORT("wlwl:std.log", ["LOG_FILE"]);
IMPORT("wlwl:std.fs", ["READ_FILE", "WRITE_FILE"]);
IMPORT("wlwl:std.json", ["PARSE"]);
IMPORT("wlwl:std.time", ["NOW"]);
IMPORT("wlwl:std.str", ["CAT"]);
LET(_, LOG_FILE("{ap}"));
LET(_, RULESET_LOAD("{p}", ["client_ip": "203.0.113.7", "path": "/x", "request_id": "r1"]));
LET(t0, NOW());
WHILE(<(NOW(), +(t0, 1100)), NULL);
LET(_, WRITE_FILE("{p}", "rule-b\n"));
LET(_, RULESET_LOAD("{p}", ["client_ip": "203.0.113.7", "path": "/x", "request_id": "r2"]));
LET(_, RULESET_LOAD("{p}", ["client_ip": "203.0.113.7", "path": "/x", "request_id": "r3"]));
LET(txt, READ_FILE("{ap}"));
LET MUT(out, []);
FOR(line, SPLIT(txt, "\n"),
    (LET(t, TRIM(line));
     IF(==(t, ""), NULL,
        (LET(obj, PARSE(t));
         LET(_, obj);
         IF(IS_ERR(obj), SET(out, PUSH(out, "PARSE_FAIL")), SET(out, PUSH(out, AT(AT(obj, "fields", DICT()), "result", ""))))))));
CAT(AT(out, 0, ""), "|", AT(out, 1, ""), "|", AT(out, 2, ""), "|", STR(LEN(out)))"#
    );
    assert_eq!(run(&src), "loaded|reloaded|cached|3");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// D-7 · CLIENT_IP
// ---------------------------------------------------------------------------

struct IpCase {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const IP_CASES: &[IpCase] = &[
    IpCase {
        name: "trusted_proxy_takes_rightmost_untrusted_hop",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": "203.0.113.7, 10.0.0.1"], "10.0.0.1", ["10.0.0.1"])"#,
        expect: "203.0.113.7",
    },
    IpCase {
        name: "untrusted_remote_ignores_xff",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": "203.0.113.7"], "198.51.100.9", ["10.0.0.1"])"#,
        expect: "198.51.100.9",
    },
    IpCase {
        name: "rightmost_untrusted_hop_wins",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": "1.2.3.4, 5.6.7.8, 10.0.0.1"], "10.0.0.1", ["10.0.0.1"])"#,
        expect: "5.6.7.8",
    },
    IpCase {
        name: "all_hops_trusted_takes_leftmost",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": "10.0.0.2, 10.0.0.1"], "10.0.0.1", ["10.0.0.1", "10.0.0.2"])"#,
        expect: "10.0.0.2",
    },
    IpCase {
        name: "no_xff_returns_remote",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(DICT(), "198.51.100.9", ["10.0.0.1"])"#,
        expect: "198.51.100.9",
    },
    IpCase {
        name: "empty_xff_returns_remote",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": ""], "10.0.0.1", ["10.0.0.1"])"#,
        expect: "10.0.0.1",
    },
    IpCase {
        name: "request_dict_form_reads_req_headers",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["method": "GET", "path": "/", "headers": ["x-forwarded-for": "203.0.113.7"]], "10.0.0.1", ["10.0.0.1"])"#,
        expect: "203.0.113.7",
    },
    IpCase {
        name: "multi_value_xff_header_is_flattened_in_order",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": ["203.0.113.7", "10.0.0.9, 10.0.0.1"]], "10.0.0.1", ["10.0.0.1"])"#,
        expect: "10.0.0.9",
    },
    IpCase {
        name: "trusted_match_is_case_insensitive_but_return_is_verbatim",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": "2001:DB8::1"], "::AB", ["::ab"])"#,
        expect: "2001:DB8::1",
    },
    IpCase {
        name: "remote_with_port_is_a_different_literal_and_never_trusted",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": "203.0.113.7"], "10.0.0.1:8080", ["10.0.0.1"])"#,
        expect: "10.0.0.1:8080",
    },
    IpCase {
        name: "xff_array_type_error_is_e0030",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(["x-forwarded-for": 5], "10.0.0.1", ["10.0.0.1"])"#,
        expect: "!E0030",
    },
    IpCase {
        name: "trusted_proxies_must_be_an_array",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(DICT(), "10.0.0.1", "10.0.0.1")"#,
        expect: "!E0030",
    },
    IpCase {
        name: "trusted_proxies_elements_must_be_strings",
        src: r#"IMPORT("wlwl:std.http", ["CLIENT_IP"]); CLIENT_IP(DICT(), "10.0.0.1", [5])"#,
        expect: "!E0030",
    },
];

#[test]
fn client_ip_matches_d7_semantics() {
    // ⚠ CLIENT_IP 恒产出审计行（LOG）⇒ 持 gate，别与 LOG_FILE 用例互污。
    let _g = gate();
    let bad: Vec<String> = IP_CASES
        .iter()
        .filter_map(|c| {
            let got = run(c.src);
            let ok = is_code(c.expect, &got) || got == c.expect;
            if ok {
                None
            } else {
                Some(format!(
                    "{}:\n      want: {}\n      got:  {}",
                    c.name, c.expect, got
                ))
            }
        })
        .collect();
    assert!(bad.is_empty(), "CLIENT_IP drift:\n  {}", bad.join("\n  "));
}
