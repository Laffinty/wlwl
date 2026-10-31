//! ⚠⚠ **CI 专用临时探针**（拿到结论后**即删**）。
//!
//! ## 为什么需要它
//!
//! CI 的 windows job 偶发红在
//! `resource_release_contract::a_failing_file_read_leaves_no_handle_behind`
//! （`delta = 1`）⇒ 已定性为**真泄漏**（读数不回落，不是测量噪声），
//! 但 ⚠ **本机复现不出**（本机 5 个变体各 6000 次 eval ⇒ 零命中）。
//! ⇒ 决定性的一步只能在 **CI 的 windows runner** 上做。
//!
//! ## 它做什么：**变体矩阵**（只依赖安全的句柄计数器）
//!
//! 对下面每个变体，各自：热身 → 取**落定**基线 → 连跑 `K` 次 → 取**落定**读数
//! ⇒ `delta = 后 − 前` ⇒ **哪个变体 delta ≠ 0，泄漏就发生在那一小段操作里**。
//!
//! | 变体 | 含义 |
//! |---|---|
//! | `import_only` | 只有 `IMPORT`（不碰文件） |
//! | `read_missing_print` | `IMPORT` + `PRINT(READ_FILE(不存在))` ⇒ **ERR 逃逸到顶层** |
//! | `read_missing_let` | `IMPORT` + `LET(e, READ_FILE(不存在))` ⇒ **ERR 被丢弃** |
//! | `read_existing` | `IMPORT` + 读**存在**的文件（成功路径） |
//! | `write_bad_dir` | `IMPORT` + `WRITE_FILE(不存在的目录/…)`（另一条 IO 失败路径） |
//! | `reuse_evaluator` | **复用同一个 `Evaluator`** 反复 eval 读失败 ⇒ 分「构造/析构」与「每次 eval」 |
//!
//! ## ⚠⚠ 为什么**不再**去「把句柄点名」
//!
//! 前两版都试过，两版都**在 CI runner 上翻车**：
//!
//! 1. **版 1（`NtQueryObject` 取类型名）** ⇒ **挂死 2.4 小时**。该 API 在
//!    **同步管道句柄 + 有挂起读操作**时永久挂死，且线程杀不掉；连「先判断它是不是
//!    管道」的替代尝试（`GetFileType`）同样死锁。
//! 2. **版 2（只用 `DuplicateHandle` ＋ `GetFinalPathNameByHandleW`）** ⇒ 仍然
//!    **`SCAN-TIMEOUT`（120 s 没扫完）**。⇒ 说明 `GetFinalPathNameByHandleW`
//!    对 runner 上那类句柄**同样会阻塞**（它内部查 `FileNameInformation`，
//!    与 `NtQueryObject` 同一类阻塞面）。
//!
//! ⇒ **结论：在 runner 上逐个句柄查询这条路根本走不通**。本版改为**只用
//!   `GetProcessHandleCount` 这一个绝对安全的调用**（`common::handle_guard::
//!   open_handle_count`）⇒ 拿不到「那个句柄是什么」，但能拿到
//!   「**哪一段操作**让它多出来一个」—— 而后者才是能直接定位到代码的东西。
//!
//! ⚠ 仍保留一条**有界**的诊断：`dup_only` 只用 `DuplicateHandle`（不查路径）
//! 枚举一次，放在**可放弃的线程**里最多等 15 s ⇒ 用来回答「到底是
//! `DuplicateHandle` 还是路径查询被阻塞」。⚠ 它**不参与判定**，只做记录。
//!
//! ## 结论进**注解**，不只 println
//!
//! GitHub 的**作业日志需要登录**才能读（`GET /actions/jobs/<id>/logs` = 403、
//! artifact = 401）⇒ 只 print 的话证据留在「要登录才看得见」的地方。
//! **注解是匿名可读的**（`GET /repos/<o>/<r>/check-runs/<id>/annotations`
//! 无需 token）⇒ 把结论压成一行放进 `::warning` 注解。
//!
//! ## 成本
//!
//! 只在 `WLWL_HANDLE_DUMP=1` 时干活；`K` 次 eval × 6 个变体 ≈ **几秒**。
//! `ci.yml` 那一步另带 `timeout-minutes` 硬上限。

mod common;

use std::time::{Duration, Instant};

use common::handle_guard::open_handle_count;

/// 每个变体连跑多少次 eval。⚠ 每个变体自带**落定基线** ⇒ 前一个变体漏出来的
/// 那一个会被基线吸收 ⇒ 各变体的 delta 互不污染。
const K: usize = 1_000;

/// 读数**落定**（连续两次相同才返回）。
fn settled(timeout_ms: u64) -> usize {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut last = open_handle_count();
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let now = open_handle_count();
        if now == last || Instant::now() >= deadline {
            return now;
        }
        last = now;
    }
}

/// 发一条 **workflow annotation**（`::warning title=…::…`），见文件头的「结论进注解」。
///
/// ⚠ 注解 message **不能含换行** ⇒ 压成一行；⚠ 一律 **ASCII**
/// （Windows runner 的 locale 不是 UTF-8 ⇒ 非 ASCII 会变乱码）。
fn annotation(title: &str, msg: &str) {
    let ascii: String = msg
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    let one_line: String = ascii.replace(['\r', '\n'], " ");
    println!("::warning title={title}::{one_line}");
}

/// 跑一次程序（**每次新建 `Evaluator`**）⇒ 返回 `(成功?, 诊断消息)`。
fn run_fresh(src: &str, dir: &std::path::Path) -> (bool, String) {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    let mut ev = wlwl_eval::Evaluator::new().with_base_dir(dir.to_path_buf());
    match ev.eval(&ast) {
        Ok(_) => (true, String::new()),
        Err(e) => (false, e.diagnostic().message.clone()),
    }
}

/// 一次测量：热身 → 落定基线 → 连跑 `K` 次 → 落定读数 ⇒ `(delta, 基线, 读数)`。
fn measure<F: FnMut()>(mut body: F) -> (i64, usize, usize) {
    // 热身 ⇒ 把懒初始化的一次性句柄排除在基线之外。
    body();
    body();
    let before = settled(1_000);
    for _ in 0..K {
        body();
    }
    let after = settled(1_500);
    (after as i64 - before as i64, before, after)
}

#[test]
fn ci_only_handle_variant_matrix() {
    if std::env::var("WLWL_HANDLE_DUMP").as_deref() != Ok("1") {
        println!("[handle-matrix] 未设置 WLWL_HANDLE_DUMP=1 ⇒ 跳过（正常 CI 不受影响）");
        return;
    }

    let dir = common::scratch("wlwl_handle_matrix");
    let missing = dir
        .join("no-such-file.txt")
        .to_string_lossy()
        .replace('\\', "/");
    let present = dir.join("present.txt").to_string_lossy().replace('\\', "/");
    std::fs::write(&present, "hello").expect("present file");
    let bad_target = dir
        .join("no-such-dir")
        .join("out.txt")
        .to_string_lossy()
        .replace('\\', "/");

    let import_only = String::from("IMPORT(\"wlwl:std.fs\", [\"READ_FILE\", \"WRITE_FILE\"]);\n");
    let read_missing_print = format!("{import_only}PRINT(READ_FILE(\"{missing}\"));\n");
    let read_missing_let = format!("{import_only}LET(e, READ_FILE(\"{missing}\"));\n");
    let read_existing = format!("{import_only}PRINT(READ_FILE(\"{present}\"));\n");
    let write_bad_dir = format!("{import_only}LET(w, WRITE_FILE(\"{bad_target}\", \"x\"));\n");

    let t0 = Instant::now();
    let mut lines: Vec<String> = Vec::new();

    // ── 六个变体（每个自带落定基线 ⇒ 互不污染）───────────────────────────
    {
        let (d, b, a) = measure(|| {
            run_fresh(&import_only, &dir);
        });
        lines.push(format!("import_only d={d} ({b}->{a})"));
    }
    {
        let (d, b, a) = measure(|| {
            run_fresh(&read_missing_print, &dir);
        });
        lines.push(format!("read_missing_print d={d} ({b}->{a})"));
    }
    {
        let (d, b, a) = measure(|| {
            run_fresh(&read_missing_let, &dir);
        });
        lines.push(format!("read_missing_let d={d} ({b}->{a})"));
    }
    {
        let (d, b, a) = measure(|| {
            run_fresh(&read_existing, &dir);
        });
        lines.push(format!("read_existing d={d} ({b}->{a})"));
    }
    {
        let (d, b, a) = measure(|| {
            run_fresh(&write_bad_dir, &dir);
        });
        lines.push(format!("write_bad_dir d={d} ({b}->{a})"));
    }
    {
        // ⚠ 复用**同一个** `Evaluator` 反复 eval ⇒ 分「构造/析构」与「每次 eval」。
        //   基线在**构造之后**取 ⇒ 构造本身的影响被吸收 ⇒ 只看 eval。
        let ast = wlwl_parser::parse(&read_missing_print, "t.wll").expect("src must parse");
        let mut ev = wlwl_eval::Evaluator::new().with_base_dir(dir.clone());
        let _ = ev.eval(&ast);
        let _ = ev.eval(&ast);
        let before = settled(1_000);
        for _ in 0..K {
            let _ = ev.eval(&ast);
        }
        let after = settled(1_500);
        lines.push(format!(
            "reuse_evaluator d={} ({before}->{after})",
            after as i64 - before as i64
        ));
    }

    // ── 有界诊断：`DuplicateHandle` 枚举本身会不会阻塞？ ─────────────────
    #[cfg(windows)]
    let dup = match dup_only_guarded(15) {
        Some(n) => format!("dup_only=ok({n})"),
        None => String::from("dup_only=TIMEOUT(15s)"),
    };
    #[cfg(not(windows))]
    let dup = String::from("dup_only=n/a");

    let summary = format!(
        "evals_per_variant={K} elapsed_ms={} :: {} :: {}",
        t0.elapsed().as_millis(),
        lines.join(" | "),
        dup
    );
    println!("[handle-matrix] {summary}");
    annotation("handle-leak matrix (windows runner)", &summary);

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Windows：`DuplicateHandle` 枚举（**有界**，15 s 上限）────────────────

#[cfg(windows)]
mod win {
    pub type Handle = isize;

    #[link(name = "kernel32")]
    extern "system" {
        /// ⚠ 签名必须与 `common::handle_guard` 里那份**逐字一致**（同一个 Win32
        ///   API 声明两遍、类型不同 ⇒ rustc 报 "redeclared with a different
        ///   signature"）。
        pub fn GetCurrentProcess() -> *mut core::ffi::c_void;
        pub fn DuplicateHandle(
            src_proc: Handle,
            src: Handle,
            dst_proc: Handle,
            dst: *mut Handle,
            access: u32,
            inherit: i32,
            options: u32,
        ) -> i32;
        pub fn CloseHandle(h: Handle) -> i32;
    }

    pub const DUPLICATE_SAME_ACCESS: u32 = 0x2;
    /// 句柄值的扫描上限（Windows 的句柄值是很小的整数 ⇒ 4096 绰绰有余）。
    pub const MAX_HANDLE_VALUE: isize = 4096;
}

#[cfg(windows)]
/// 只用 `DuplicateHandle` 枚举**有效句柄值的个数**。
///
/// ⚠ **不查路径、不查类型** —— 那两个都会在 runner 的管道句柄上阻塞（见文件头）。
fn count_valid_handles() -> usize {
    use win::*;

    let mut n = 0usize;
    unsafe {
        let me = GetCurrentProcess() as Handle;
        for v in 0..MAX_HANDLE_VALUE {
            let mut dup: Handle = 0;
            if DuplicateHandle(me, v, me, &mut dup, 0, 0, DUPLICATE_SAME_ACCESS) != 0 {
                let _ = CloseHandle(dup);
                n += 1;
            }
        }
    }
    n
}

#[cfg(windows)]
/// **超时保护**的枚举：跑在独立线程里，最多等 `timeout_secs`。
///
/// ⚠ 为什么必须：会阻塞的原生调用**杀不掉线程** ⇒ 关进「可放弃」的线程里
///   ⇒ 最坏情况只是**拿不到这个数**，不会像第一版那样把 CI 步骤挂死几小时。
fn dup_only_guarded(timeout_secs: u64) -> Option<usize> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(count_valid_handles());
    });
    rx.recv_timeout(Duration::from_secs(timeout_secs)).ok()
}
