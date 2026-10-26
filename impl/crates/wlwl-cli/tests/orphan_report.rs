//! [G-24] 没人 `AWAIT` 的任务失败**必须被报告**。
//!
//! ## 这条契约在钉什么
//!
//! 任务的失败只存在它自己的 `TaskResult::Failed` 里 ⇒ **只有 `AWAIT` 它的人才会看到**。
//! fire-and-forget 的任务（**服务器里「每连接一个任务」正是这种**）失败会**完全静默**：
//! 没有诊断、没有 stderr、没有退出码。
//!
//! 本项目实测：并发服务器的第一版，handler 闭包里少了一条
//! `IMPORT("wlwl:std.str", ["CONTAINS_SUB"])` ⇒ handler 每次都报 `E0020`，
//! 而 **stderr 完全空白**、端口在听、客户端一直超时 ⇒ 症状与「网络卡了」无法区分。
//!
//! ## 两条断言
//!
//! 1. 没人 `AWAIT` ⇒ stderr 出现报告行，**且程序照常跑完**（退出码 0）——
//!    服务器必须继续服务别的连接。
//! 2. 有人 `AWAIT` ⇒ 失败由 `AWAIT` 在**调用点**抛出（退出码非 0），
//!    且**不再**出现「没人管」的报告行（避免同一失败报两次）。

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};

fn wlwl_exe() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    let exe_name = if cfg!(windows) { "wlwl.exe" } else { "wlwl" };
    let candidates = [
        std::env::current_dir()
            .unwrap()
            .join("target/debug")
            .join(exe_name),
        PathBuf::from(format!("target/debug/{exe_name}")),
        PathBuf::from(exe_name),
    ];
    candidates
        .into_iter()
        .find(|c| c.exists())
        .unwrap_or_else(|| panic!("wlwl binary not found; build it first with `cargo build`"))
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wlwl_g24_{tag}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn write_source(dir: &std::path::Path, name: &str, src: &str) -> PathBuf {
    let p = dir.join(name);
    let mut f = fs::File::create(&p).expect("create source");
    f.write_all(src.as_bytes()).expect("write source");
    p
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(wlwl_exe())
        .args(args)
        .output()
        .expect("failed to spawn wlwl")
}

#[test]
fn an_unawaited_task_failure_is_reported_and_the_program_keeps_running() {
    let dir = scratch_dir("orphan");
    let src = r#"
IMPORT("wlwl:std.io", ["PRINT"]);
SCOPE(FUN(() , (
    LET(t, SPAWN(FUN(() , UNDEFINED_NAME_HERE)));
    0
)));
PRINT("program-finished");
"#;
    let p = write_source(&dir, "orphan.wll", src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        out.status.success(),
        "a fire-and-forget failure must NOT abort the program; status={:?} stderr={stderr}",
        out.status
    );
    assert!(
        stdout.contains("program-finished"),
        "the program must run to completion; stdout={stdout}"
    );
    assert!(
        stderr.contains("unhandled task failure"),
        "the orphan failure must be reported; stderr={stderr}"
    );
    assert!(
        stderr.contains("E0020"),
        "the report must name the diagnostic; stderr={stderr}"
    );
}

#[test]
fn an_awaited_task_failure_is_raised_at_the_await_site_and_not_reported_twice() {
    let dir = scratch_dir("awaited");
    let src = r#"
IMPORT("wlwl:std.io", ["PRINT"]);
SCOPE(FUN(() , (
    LET(t, SPAWN(FUN(() , UNDEFINED_NAME_HERE_TOO)));
    LET(v, AWAIT(t));
    0
)));
PRINT("unreachable");
"#;
    let p = write_source(&dir, "awaited.wll", src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        !out.status.success(),
        "an awaited failure must still fail the program; stderr={stderr}"
    );
    assert!(
        stderr.contains("E0020"),
        "the AWAIT site must raise the diagnostic; stderr={stderr}"
    );
    assert!(
        !stderr.contains("unhandled task failure"),
        "the failure is claimed by the AWAIT — it must not be reported twice; stderr={stderr}"
    );
}

/// [v0.12.0 / D-16] **同一个失败不许被报两次** —— 即使它在**被认领之前**就已经失败了。
///
/// ## 这条钉的是什么
///
/// 「有没有人 `AWAIT` 它」在**失败发生的那一刻**是**判不准**的：
/// `AWAIT` 不把调用者停在 `AwaitingChild` 上（它内联驱动调度循环），
/// 那一刻的「有人在等」信号只剩「当前这个 `AWAIT` 正在驱动的那个目标」。
///
/// ⇒ 下面的程序里，`a` 在 **`AWAIT(b)` 的驱动窗口内**失败（那一刻**没人**在等 `a`），
/// 随后 **`AWAIT(a)`** 才认领它 ⇒ 若判定发生在失败的那一刻，`a` 会被**误判**成孤儿
/// ⇒ 报一行 stderr，**再**由 `AWAIT(a)` 在调用点抛一次 ⇒ **同一个失败报两次**。
///
/// ⇒ 并发册 §17.9：两条路径**互斥** —— 被 `AWAIT` 的失败**不重复报告**。
/// ⇒ 正确做法：**判定延后**（失败先挂起，`AWAIT` 认领时摘掉，程序收尾时报剩下的）。
#[test]
fn a_failure_claimed_later_by_an_await_is_not_reported_as_an_orphan() {
    let dir = scratch_dir("late-claim");
    let src = r#"
IMPORT("wlwl:std.io", ["PRINT"]);
SCOPE(FUN(() , (
    LET(a, SPAWN(FUN(() , UNDEFINED_NAME_LATE_CLAIM)));
    LET(b, SPAWN(FUN(() , 0)));
    AWAIT(b);
    LET(v, AWAIT(a));
    0
)));
PRINT("unreachable");
"#;
    let p = write_source(&dir, "late_claim.wll", src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        !out.status.success(),
        "the AWAIT site must raise the diagnostic; stderr={stderr}"
    );
    assert!(
        stderr.contains("E0020"),
        "the AWAIT site must raise the diagnostic; stderr={stderr}"
    );
    assert!(
        !stderr.contains("unhandled task failure"),
        "the failure IS claimed by the later AWAIT — it must not be reported as an orphan \
         (that would report the same failure twice); stderr={stderr}"
    );
}

// ── [v0.12.0 / R-6 · S-2] 孤儿任务的 **`ERR` 终值** ──────────────────────
//
// ## 这条钉的是什么
//
// 语言核心册 §8.6.2 的丢弃点**恰好三处**:① 语句值丢弃;② 绑定被覆盖;
// ③ **作用域结束时仍有绑定**持有未消费 `ERR`。
// ⇒ 「**任务终值**」既不是绑定、也不是语句值 ⇒ **三处都不覆盖它**。
//
// ⇒ 一个以 `ERR` 值收尾的孤儿任务以前**不留任何痕迹**:没有 stderr、没有
// 诊断、退出码 0 ⇒ 症状与「网络卡了」**无法区分** —— 这正是并发册 §17.9
// 明写「无人 `AWAIT` 的失败**不得**完全静默」要防的那件事。
//
// ⇒ 修法（**实现侧补报**，S-2 已裁决）：任务以 `ERR` 值收尾 ⇒ 登记为待报;
// 程序收尾时按 §17.9 的**报告行形态**报一行;被 `AWAIT` 认领的那一份
// **摘掉** ⇒ 两条路径**互斥**、**不报两次**。

#[test]
fn an_unawaited_task_ending_with_an_err_value_is_reported_and_the_program_keeps_running() {
    let dir = scratch_dir("orphan-err-value");
    let src = r#"
IMPORT("wlwl:std.io", ["PRINT"]);
SCOPE(FUN(() , (
    LET(t, SPAWN(FUN(() , ERR("orphan-terminal-err"))));
    0
)));
PRINT("program-finished");
"#;
    let p = write_source(&dir, "orphan_err_value.wll", src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        out.status.success(),
        "an orphan ERR value must NOT abort the program; status={:?} stderr={stderr}",
        out.status
    );
    assert!(
        stdout.contains("program-finished"),
        "the program must run to completion; stdout={stdout}"
    );
    assert!(
        stderr.contains("unhandled task failure"),
        "the orphan ERR value must be reported; stderr={stderr}"
    );
    assert!(
        stderr.contains("orphan-terminal-err"),
        "the report must carry the payload; stderr={stderr}"
    );
    // 与「绑定持有 `ERR`」那族**同款**形态 ⇒ 运维只要 grep 一条串。
    assert!(
        stderr.contains("E0103"),
        "the report must name the code; stderr={stderr}"
    );
}

#[test]
fn an_awaited_task_ending_with_an_err_value_is_raised_at_the_await_site_and_not_reported_twice() {
    let dir = scratch_dir("awaited-err-value");
    let src = r#"
IMPORT("wlwl:std.io", ["PRINT"]);
SCOPE(FUN(() , (
    LET(t, SPAWN(FUN(() , ERR("awaited-terminal-err"))));
    LET(v, AWAIT(t));
    0
)));
PRINT("unreachable");
"#;
    let p = write_source(&dir, "awaited_err_value.wll", src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        !out.status.success(),
        "an awaited ERR value must still fail the program at the AWAIT site; stderr={stderr}"
    );
    assert!(
        stderr.contains("awaited-terminal-err"),
        "the AWAIT site must surface the payload; stderr={stderr}"
    );
    assert!(
        !stderr.contains("unhandled task failure"),
        "the ERR value IS claimed by the AWAIT — it must not be reported as an orphan \
         (that would report the same ERR twice); stderr={stderr}"
    );
}

/// 反向守卫:**绑定**持有 `ERR` 的孤儿任务**只报一行** —— 它的终值不是 `ERR`
/// (终值是 `0`)⇒ 两个机制**不得**同时报同一件事。
#[test]
fn an_orphan_task_holding_an_err_in_a_binding_is_reported_exactly_once() {
    let dir = scratch_dir("orphan-bound-err");
    let src = r#"
IMPORT("wlwl:std.io", ["PRINT"]);
SCOPE(FUN(() , (
    LET(t, SPAWN(FUN(() , (
        LET(x, ERR("orphan-bound-err"));
        0
    ))));
    0
)));
PRINT("program-finished");
"#;
    let p = write_source(&dir, "orphan_bound_err.wll", src);
    let out = run_cli(&["run", p.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let hits = stderr.matches("orphan-bound-err").count();
    assert_eq!(
        hits, 1,
        "the orphan ERR must be reported exactly once; stderr={stderr}"
    );
    assert!(
        out.status.success(),
        "an orphan failure must NOT abort the program; status={:?} stderr={stderr}",
        out.status
    );
}
