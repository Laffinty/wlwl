// [v0.7 Phase B5a-3 Path B] concurrency conformance suite driver.
//
// Each `impl/tests/concurrency/*.wll` file is a v0.7 WIP conformance
// fixture for one of the plan §6.2 concurrency categories. The
// harness shells out to the workspace-built `wlwl run` binary and
// asserts the exit code is 0 (happy path) and that stdout contains
// the expected deterministic text.
//
// The fixture list itself is the constraint: removing a `.wll`
// file without updating WLT_FILES makes the test fail. This is the
// same contract as the spec v0.4 §16.5 conformance driver in
// `conformance.rs`.
//
// Deviations:
//   P7-B5a3-001  Path B chosen 2026-09-22 as the mid-body suspend
//                strategy (over Path A's full stack-machine rewrite
//                of `eval_expr` and Path C's re-run + yield-counter
//                hack). See `docs/history/20260915-22.md` for rationale.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = impl/crates/wlwl-cli
    //   parent.parent -> impl/
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR should be impl/crates/wlwl-cli")
        .join("tests")
        .join("concurrency")
}

fn wlwl_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        return PathBuf::from(p);
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let candidate =
            here.join("target")
                .join(profile)
                .join(if cfg!(windows) { "wlwl.exe" } else { "wlwl" });
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("wlwl binary not found -- run `cargo build -p wlwl-cli --bin wlwl` first");
}

fn fixture(name: &str) -> PathBuf {
    fixture_root().join(name)
}

const WLT_FILES: &[&str] = &[
    // Phase B5a-3 Path B: mid-body YIELD actually suspends the
    // task body, the scheduler runs a sibling between the
    // pre-yield and post-yield segments, and the post-yield
    // mutation is observable.
    "yield_midbody.wll",
    // Phase D: producer / consumer pair using CHANNEL. Uses
    // CHANNEL_TRY_SEND / CHANNEL_TRY_RECV (deviation P7-D2-001 —
    // mid-body suspend on SEND/RECV is deferred to v0.7.1).
    "channel_basic.wll",
    // Phase E-B / E3 (plan §5.4.1): SCOPE surfaces the first
    // child ERR when the fn body itself does not consume it.
    "scope_cancel_siblings.wll",
    // Phase E-B / E3 (plan §5.4.1): SCOPE returns the consumed
    // value (negative case) — sibling ERRs that were explicitly
    // consumed by the fn body must NOT override SCOPE's return.
    "scope_consume_does_not_change_return.wll",
    // Phase E-C / E4 (blocking) — 4 ERR consumer registry
    // cross-task conformance fixtures (plan §6.0 row "ERR
    // consumer 跨 task 回归(§3 Phase E4 阻塞项)", 8 unit + 4
    // concurrency + 8 snapshot):
    "err_consumer_unwrap_or.wll",
    "err_consumer_is_err.wll",
    "err_consumer_payload.wll",
    "err_consumer_unwrap.wll",
    // Phase F-C / F5 — 3 SHIELD conformance fixtures (plan §3 F5
    // sub-bullets 1+3+4; minimum 3 fixtures per the spec):
    "shield_basic.wll",
    "shield_nested.wll",
    "shield_then_error.wll",
    // [v0.10.1 / R10-014] 无缓冲通道的两种 spawn 顺序。
    //
    // 两条必须同时在列表里:之前只有「消费者先」能跑通,而它**没有**夹具
    // 覆盖 —— 于是「生产者先」坏掉时没有对照,没人看得出那是个 bug。
    // 补上配对逻辑之后,顺序才真的与结果无关,这两条一起把这件事钉死。
    "unbuffered_producer_first.wll",
    "unbuffered_consumer_first.wll",
    // [2026-10-28 / 需求清单 B-2] `TCP_ACCEPT` 的**惊群**:同一个监听句柄上
    // **多个 acceptor** ⇒ 一次就绪事件**只唤醒一个** ⇒ 落选者**留在名单里**
    // 继续等下一次就绪事件 ⇒ 落选者**不拿到 NULL**、**不报错**、**保持存活**,
    // 并能**继续接受后续连接**。
    //
    // ⚠ 这份夹具**独立**跑(并发套件不给它客户端)⇒ 断言的是「两个 acceptor
    //   等**同一个**监听句柄、**都干净地超时**、谁也没死」。
    // ⚠ **惊群本体**（有客户端 ⇒ 落选者被唤醒却没货 ⇒ 拿到 `NULL` ⇒ 喂给
    //   `TCP_RECV` ⇒ `E0030`）由 `crates/wlwl-eval/tests/
    //   b2_accept_wakeup_contract.rs` 驱动**同一份**夹具（它把监听端口替换成
    //   一个空闲端口、并从外部发起两个连接）。
    "tcp_accept_single_wakeup.wll",
    // 活锁护栏:`unbuffered_repeat_rendezvous_is_reported.wll` **预期失败**
    // (exit 1 + E0064),所以它由下面那条专门的测试驱动,不放进
    // `all_concurrency_fixtures_run_clean` 这类「必须跑通」的循环。
];

#[test]
fn all_concurrency_fixtures_present() {
    assert!(
        !WLT_FILES.is_empty(),
        "spec says at least one concurrent fixture"
    );
    for f in WLT_FILES {
        let p = fixture(f);
        assert!(p.exists(), "missing concurrency fixture: {}", p.display());
    }
}

fn run_wlwl(path: &Path) -> std::process::Output {
    let bin = wlwl_binary();
    Command::new(&bin)
        .arg("run")
        .arg(path)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `wlwl run {}`: {}", path.display(), e))
}

/// Phase B5a-3 Path B: the body that yields once, then mutates a
/// shared cell, must produce the deterministic post-yield value (42)
/// after the scheduler has interleaved the other task between the
/// two segments. If mid-body YIELD were a no-op (the v0.6
/// workaround), the test would either hang on the AWAIT or emit
/// a different non-deterministic ordering.
#[test]
fn yield_midbody_runs_to_completion_with_expected_stdout() {
    let path = fixture("yield_midbody.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "before-yield\nafter-resume\n42\ndone\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase D: a flat sequential CHANNEL round-trip. We exercise
/// CHANNEL_NEW / CHANNEL_TRY_SEND / CHANNEL_TRY_RECV end-to-end
/// without mid-body suspend (deviation P7-D2-001). The stdout is
/// the deterministic sequence "1\n2\n3\ndone\n".
#[test]
fn channel_basic_producer_consumer_round_trip() {
    let path = fixture("channel_basic.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "1\n2\n3\ndone\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// 把一段 WLWL 源码写进临时目录,返回它的路径。
///
/// 用系统临时目录而不是夹具目录:夹具目录是**被跟踪**的,测试往里写文件会
/// 弄脏工作区,而且下一次跑会撞上上一次的残留。
fn scratch_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("wlwl-concurrency-scratch");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let p = dir.join(name);
    std::fs::write(&p, source).expect("scratch fixture written");
    p
}

/// [v0.10.1 / R10-014] 无缓冲通道上，**两种** spawn 顺序都必须跑通。
///
/// spec §17.2.1：「同一通道上互为对端的挂起收发双方**不**构成死锁」。
///
/// 修之前只有消费者先那条能过（`CHANNEL_SEND` 有 pair-first，`CHANNEL_RECV`
/// 缺对偶），生产者先会撞上 E0065 假死锁。这里同时跑两份夹具：只跑一条顺序
/// 等于没测——「能跑」的那条本来就一直能跑。
#[test]
fn unbuffered_rendezvous_is_independent_of_spawn_order() {
    for (fixture_name, order) in [
        ("unbuffered_producer_first.wll", "producer spawned first"),
        ("unbuffered_consumer_first.wll", "consumer spawned first"),
    ] {
        let path = fixture(fixture_name);
        assert!(path.exists(), "missing fixture: {}", path.display());
        let out = run_wlwl(&path);
        let code = out.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            code, 0,
            "{fixture_name} ({order}) must run clean, got exit {code}:\n  stderr={stderr}"
        );
        assert_eq!(
            stdout, "1\n",
            "{fixture_name} ({order}) printed the wrong value"
        );
    }
}

/// [v0.10.1 / R10-014] 活锁护栏：同一对任务在无缓冲通道上第二次 rendezvous
/// 必须报 `E0064`，**不能挂住**。
///
/// 这条守的是失败模式本身。补上配对逻辑之后这类程序会无限循环：通道操作挂起
/// 时 `running_env` 被丢弃，任务从段首重跑，循环计数回到初值。没有护栏时它
/// 静默挂死——而挂死比报错糟得多。
///
/// 顺带把两个**真死锁**也钉住，确保护栏没有把该报的吞掉、也没有抢在它们
/// 前面误报。
#[test]
fn r10_014_livelock_guard_reports_instead_of_hanging() {
    let path = fixture("unbuffered_repeat_rendezvous_is_reported.wll");
    let out = run_wlwl(&path);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(
        out.status.code(),
        Some(1),
        "a repeated rendezvous must exit 1 with a diagnostic, not hang:\n  stderr={stderr}"
    );
    assert!(
        stderr.contains("E0064"),
        "expected the E0064 livelock guard, got:\n  stderr={stderr}"
    );
}

/// [v0.10.1 / R10-014] 护栏**不得**误伤真死锁。
///
/// 一个挂在通道上、没有对端可唤醒的任务是货真价实的死锁，仍然报 E0053；
/// 两个互相排队的发送者仍然报 E0065。护栏只管「配上了但推不动」，
/// 不许伸手去管「压根没人来」。
#[test]
fn r10_014_guard_does_not_swallow_real_deadlocks() {
    // 单发送者、无人接收 -> E0053（legacy no-peer）。
    let single = scratch_program(
        "single_sender.wll",
        "SCOPE(FUN(() , LET(ch, CHANNEL_NEW(0)); \
         LET(a, SPAWN(FUN(() , CHANNEL_SEND(ch, 1)))); AWAIT(a)));\n",
    );
    let out = run_wlwl(&single);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(1), "stderr={stderr}");
    assert!(
        stderr.contains("E0053"),
        "a lone parked sender is a real deadlock and must stay E0053:\n  stderr={stderr}"
    );

    // 两个发送者互相排队 -> E0065（L1 死锁环）。
    let pair = scratch_program(
        "two_senders.wll",
        "SCOPE(FUN(() , LET(ch, CHANNEL_NEW(0)); \
         LET(a, SPAWN(FUN(() , CHANNEL_SEND(ch, 1)))); \
         LET(b, SPAWN(FUN(() , CHANNEL_SEND(ch, 2)))); \
         AWAIT(a); AWAIT(b)));\n",
    );
    let out = run_wlwl(&pair);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(1), "stderr={stderr}");
    assert!(
        stderr.contains("E0065"),
        "two queued senders are a real L1 deadlock and must stay E0065:\n  stderr={stderr}"
    );
}

/// [v0.10.1 / R10-014] 护栏只对**无缓冲**通道生效。
///
/// 缓冲通道上的 park/wake 循环是正常用法：满缓冲时发送者停泊、接收者取走
/// 腾位，一次流水线里可以发生任意多次。要是护栏不分青红皂白，连合法的
/// 缓冲流水线都会被误报。
#[test]
fn r10_014_guard_does_not_apply_to_buffered_channels() {
    let path = scratch_program(
        "buffered_many.wll",
        // 缓冲足够大时 SEND 不会停泊，两轮都不会进护栏。
        "SCOPE(FUN(() , LET(ch, CHANNEL_NEW(8)); \
         LET(p, SPAWN(FUN(() , CHANNEL_SEND(ch, 1); CHANNEL_SEND(ch, 2); \
         CHANNEL_CLOSE(ch); 0))); \
         LET(c, SPAWN(FUN(() , [CHANNEL_RECV(ch), CHANNEL_RECV(ch)]))); \
         PRINT(AWAIT(c)); AWAIT(p)));\n",
    );
    let out = run_wlwl(&path);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(
        out.status.code(),
        Some(0),
        "a buffered channel must not trip the unbuffered-rendezvous guard:\n  stderr={stderr}"
    );
    assert!(
        !stderr.contains("E0064"),
        "E0064 fired on a buffered channel:\n  stderr={stderr}"
    );
}

/// Phase E-B / E3 (plan §5.4.1): SCOPE(fn) surfaces the first
/// uncaught child ERR when the fn body does not consume it. Path
/// B runs children synchronously, so the "cancel siblings" side
/// effect has no in-flight sibling to interrupt (P7-E3-001
/// documents this). This fixture locks the ERR-propagation half
/// of the contract.
#[test]
fn scope_cancel_siblings_surfaces_first_err() {
    let path = fixture("scope_cancel_siblings.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "ERR-surfaced\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase E-B / E3 (plan §5.4.1, negative case): when the fn body
/// consumes the child ERR via UNWRAP_OR, SCOPE returns the
/// consumed value, not the child ERR. This locks the invariant
/// that SCOPE does NOT override fn-body-consumed ERRs.
#[test]
fn scope_consume_does_not_change_return() {
    let path = fixture("scope_consume_does_not_change_return.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "consumed\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase E-C / E4 (blocking, plan §6.0): UNWRAP_OR cross-task.
/// Child returns ERR; parent AWAITs and UNWRAP_OR consumes the
/// ERR (§12.7 / Appendix B.17 — UNWRAP_OR is in the ERR consumer
/// registry) and returns the default. End-to-end exercise of the
/// §8.3 consumer registry through a SCOPE/AWAIT boundary.
#[test]
fn err_consumer_unwrap_or_cross_task() {
    let path = fixture("err_consumer_unwrap_or.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "default\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase E-C / E4 (blocking): IS_ERR cross-task. Child returns
/// ERR; parent AWAITs and IS_ERR observes the ERR (TRUE per
/// §12.2 — IS_ERR is observation only, payload untouched).
#[test]
fn err_consumer_is_err_cross_task() {
    let path = fixture("err_consumer_is_err.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "TRUE\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase E-C / E4 (blocking): ERR_PAYLOAD cross-task. Child
/// returns ERR(["kind": "child-err"]); parent AWAITs and
/// ERR_PAYLOAD extracts the inner Dict; AT_K reads the "kind"
/// field. Locks plan §5.4.1 invariant "ERR payload 中的 kind
/// 字段在跨 task 边界后保持原值".
#[test]
fn err_consumer_payload_cross_task() {
    let path = fixture("err_consumer_payload.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "child-err\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase E-C / E4 (blocking): UNWRAP cross-task. Child returns
/// ERR; parent AWAITs and UNWRAP panics with E0100 (spec §12.4
/// PANIC). This locks the consumer-registry invariant that
/// UNWRAP does NOT participate in §8.2 transparent propagation —
/// it converts the ERR into a fatal diagnostic. Exit code must
/// be non-zero; stderr must contain the E0100 code + the "UNWRAP
/// called on ERR value" message.
#[test]
fn err_consumer_unwrap_cross_task_panics_e0100() {
    let path = fixture("err_consumer_unwrap.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_ne!(code, 0, "UNWRAP on ERR must PANIC, got exit 0");
    assert!(
        stderr.contains("E0100"),
        "expected E0100 in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("UNWRAP called on ERR value"),
        "expected the UNWRAP PANIC message in stderr, got: {stderr}"
    );
}

/// Phase F-C / F5 sub-bullets 1+3: SHIELD(fn) is nestable; a
/// TASK_CANCEL_PARENT inside SHIELD records a pending cancel
/// that fires only after the OUTERMOST SHIELD exits. After the
/// outer SHIELD exits, TASK_IS_CANCELLED reads the deferred
/// cancel. Expected stdout: "shielded".
#[test]
fn shield_basic_pending_cancel_fires_at_outermost_exit() {
    let path = fixture("shield_basic.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "shielded\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase F-C / F5 sub-bullet 1: 3 nested SHIELDs. A cancel at
/// the innermost layer must defer until the outermost exits.
/// TASK_IS_CANCELLED reads inside inner SHIELDs observe FALSE
/// (pending not fired yet); only after the outermost exit does
/// the read return TRUE. Expected stdout: "nested-shielded".
#[test]
fn shield_nested_pending_fires_only_at_outermost() {
    let path = fixture("shield_nested.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "nested-shielded\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// Phase F-C / F5 sub-bullet 4: SHIELD does NOT absorb the fn
/// body's own ERR. TRY(ERR(...)) inside SHIELD early-RETURNs
/// with the ERR, which propagates through SHIELD unchanged and
/// surfaces at the outer SCOPE boundary. Expected stdout:
/// "ERR-propagated".
#[test]
fn shield_then_error_propagates_via_5_4() {
    let path = fixture("shield_then_error.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "ERR-propagated\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}

// ────────────────────────────────────────────────────────────────────────
// [v1.0 V-04] 挂起点的**切段 / 不切段**两格 —— 并发册 §17.0 的两个金标准示例。
//
// 规范(并发册 §17.0 的挂起点三类表)把「切段 / 不切段」写成**可观察行为**:
// `YIELD()` 切段(段首**不**重跑),阻塞型 `CHANNEL_SEND` / `CHANNEL_RECV` 不切段
// (段首**重放**)。一致性册 §6.2 的 **V-04** 要求「附录 H 的小步语义与参考实现的
// 实际调度行为一致」,它的验证手段逐字写的是「逐条迁移规则对一条可观测行为;
// **挂起点分类**(切段 / 不切段)用『恢复后段首是否重放』判定」。
//
// ⇒ 这一格此前**零覆盖**:既没有断言「YIELD 之前的代码不重跑」,也没有断言
// 「阻塞恢复时段首重放一次」—— 现有生产者/消费者夹具里**一条 `PRINT` 都没有**,
// 「不死锁」证不了「重放」。
//
// **成对**钉,两条互为**反向守卫**:只钉「切段时 A 出现一次」而不钉「不切段时 A
// 出现两次」,任一侧改动都不会让测试变红。
//
// 期望值**手写自规范**:并发册 §17.0 的两段示例各自标注了输出(`7 / 1` 与
// `A / A / B / 7`)。断言用**精确 stdout 相等**(不是 `contains`)—— 「A 打印两次」
// 只能靠精确序列钉住,子串匹配钉不住「恰好两次」。
//
// 程序写在**临时目录**(`scratch_program`),不落进被跟踪的夹具目录 ⇒ 不必动
// `WLT_FILES` 清单(那份清单的约束是「夹具文件不许悄悄消失」,而这里没有新增夹具文件)。
// ────────────────────────────────────────────────────────────────────────

/// 切段:`YIELD()` 之后恢复从**下一段**开始 ⇒ 段首的 `PRINT("A")` **不**重跑。
///
/// 规范(并发册 §17.0)的第一段示例:输出 `7 / 1`;这里把被打印的值换成 `A` 与
/// `1`,判据不变 —— **段首那行只出现一次**。
#[test]
fn yield_suspension_does_not_replay_the_segment_head() {
    let src = r#"
SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() ,
    LET(x, 7);
    YIELD();
    PRINT("A");
    1
  )));
  LET(v, AWAIT(t));
  PRINT(v)
));
"#;
    let path = scratch_program("yield_segment_split.wll", src);
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    // 切段的**可观察判据**:段首那行 `PRINT("A")` 只出现**一次**。
    let expected = "A\n1\n";
    assert_eq!(
        stdout, expected,
        "a YIELD suspension must resume at the NEXT segment (the segment head must not replay)\n\
         got:      {stdout:?}\n  expected: {expected:?}"
    );
}

/// 不切段:阻塞型 `CHANNEL_SEND` 之后恢复从**当前段首**重跑 ⇒ `PRINT("A")` **重放**
/// (出现**两次**)。
///
/// 规范(并发册 §17.0)的第二段示例:输出 `A / A / B / 7`。
#[test]
fn blocking_channel_suspension_replays_the_segment_head_exactly_once() {
    let src = r#"
SCOPE(FUN(() ,
  LET(ch, CHANNEL_NEW(0));
  LET(s, SPAWN(FUN(() ,
    PRINT("A");
    CHANNEL_SEND(ch, 1);
    PRINT("B");
    7
  )));
  LET(r, SPAWN(FUN(() , CHANNEL_RECV(ch))));
  LET(v, AWAIT(s));
  PRINT(v)
));
"#;
    let path = scratch_program("blocking_segment_replay.wll", src);
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    // 不切段的**可观察判据**:段首那行 `PRINT("A")` 出现**两次**(重放一次)。
    let expected = "A\nA\nB\n7\n";
    assert_eq!(
        stdout, expected,
        "a blocking channel suspension must resume at the CURRENT segment head \
         (the segment head replays exactly once)\n\
         got:      {stdout:?}\n  expected: {expected:?}"
    );
}

// ────────────────────────────────────────────────────────────────────────
// [2026-10-28 / 需求清单 B-2] `TCP_ACCEPT` 的**惊群** —— 同一个监听句柄上
// **多个 acceptor**。
//
// ⚠ **这条独立跑它 ⇒ 并发套件不给它客户端** ⇒ 判据是「两个 acceptor 等**同一个**
//   监听句柄、**都干净地超时**（各拿一个 `NULL`）、谁也没死、谁也没报错」。
// ⚠ **惊群本体**（**有**客户端 ⇒ 落选者被唤醒却没货 ⇒ `TCP_ACCEPT` 返回
//   `NULL` ⇒ 调用方拿 `NULL` 去 `TCP_RECV` ⇒ `E0030` ⇒ 任务死 ⇒ `SCOPE` 提前
//   退出 ⇒ 监听句柄随进程消失 ⇒ 客户端 RST）由
//   `crates/wlwl-eval/tests/b2_accept_wakeup_contract.rs` 驱动**同一份**夹具：
//   它把夹具里的监听端口**替换**成一个空闲端口、并从**外部**发起两个连接。
//   ⇒ 两处共用**一份**场景 ⇒ 不会出现「夹具与用例各写一套、慢慢漂移」。
// ────────────────────────────────────────────────────────────────────────

/// 同一个监听句柄上**两个 acceptor**：**没有**客户端时两个都**干净地超时**。
///
/// ⚠ 期望 stdout 是 `0` / `0`（各拿一个 `NULL` = 超时的**正常值**，不是错误）。
/// ⚠ 这条守的是「**多等待者不许互相顶掉**」（审计 N4：一个 socket **可以**有
///   多个等待者）⇒ 少了任何一份登记，第二个 acceptor 就会**永远等不到**。
#[test]
fn two_acceptors_on_one_listener_both_survive_the_timeout() {
    let path = fixture("tcp_accept_single_wakeup.wll");
    let out = run_wlwl(&path);
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "expected exit 0, got {code}:\n  stderr={stderr}");
    let expected = "0\n0\n";
    assert_eq!(
        stdout, expected,
        "stdout mismatch:\n  got:      {stdout:?}\n  expected: {expected:?}"
    );
}
