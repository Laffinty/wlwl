//! ⚠⚠ **CI 专用临时探针**（拿到结论后**即删**）。
//!
//! ## 为什么需要它
//!
//! CI run #335 的 windows job 红在
//! `resource_release_contract::a_failing_file_read_leaves_no_handle_behind`
//! （`delta = 1`）⇒ 已定性为**真泄漏**（读数不回落，不是测量噪声），
//! 但 ⚠ **本机复现不出**（本机 5 个变体各 6000 次 eval ⇒ 零命中）。
//! ⇒ 决定性的一步只能在 **CI 的 windows runner** 上做：**把那个多出来的句柄点名**。
//!
//! ## 它做什么
//!
//! 1. 复刻那条用例的形状：`IMPORT("wlwl:std.fs", ["READ_FILE"])` +
//!    `PRINT(READ_FILE(<不存在的文件>))`（**必须失败** ⇒ 走错误路径）。
//! 2. **每次 eval 后立刻**读一次进程句柄数（`GetProcessHandleCount`）。
//! 3. 一旦读数偏离基线 ⇒ **等它落定**（连续两次读数相同）：
//!    * 落定后**仍偏离** ⇒ **真泄漏** ⇒ 立刻**把句柄点名**；
//!    * 落定后回到基线 ⇒ 只是噪声 ⇒ 继续跑。
//! 4. 「点名」= 扫 `0..4096` 的句柄值，用 `DuplicateHandle` 试哪个是本进程的
//!    **有效**句柄（成功 ⇔ 有效），再用 `GetFinalPathNameByHandleW` 取**文件路径**
//!    ⇒ 与基线快照**做差** ⇒ 注解里直接看得见「多出来的那个是什么、指向哪里」。
//!
//! ## ⚠⚠ **绝对不要**在这里调 `NtQueryObject`
//!
//! **实测事故（本轮自己造成的）**：第一版探针用
//! `NtQueryObject(ObjectTypeInformation)` 取**对象类型名**（`File` / `Event` /
//! `Key` …）⇒ 本机跑得好好的（7.7 s，212 个句柄全读出类型）⇒ ⚠ **但它在
//! CI 的 windows runner 上挂死了 2.4 小时**（步骤从 `02:06` 一直 `in_progress`
//! 到 `04:30` 仍未结束）。
//!
//! **根因（公开资料坐实）**：`NtQueryObject` 在**同步管道句柄 + 有挂起读操作**时
//! **会永久挂死**，而且**无法终止那个线程**（强杀可能蓝屏）；`GetFileType`
//! 之类「判断它是不是管道」的替代尝试同样会死锁。
//! ⇒ runner 上有大量这种管道句柄（runner 自己与它派的子进程通信）⇒
//! **任何逐个句柄查类型名的写法在 CI 上都是定时炸弹**。
//!
//! ⇒ 本探针**只**用两个安全 API：`DuplicateHandle`（判有效）与
//! `GetFinalPathNameByHandleW`（取路径，非文件句柄直接返回错误）。
//! ⇒ 代价：**拿不到对象类型名**。但「多出来的是**文件**（带路径）还是
//! **非文件内核对象**」这个二分对定位已经足够决定性。
//!
//! ## 两道超时保险（都是事故之后加的）
//!
//! * 扫描本身跑在**独立线程 + `recv_timeout`** 里 ⇒ 万一再踩到会阻塞的 API，
//!   探针**不会挂死**（最多泄漏 1 个线程，进程结束即回收）。
//! * `ci.yml` 里那一步带 `timeout-minutes` ⇒ 硬上限，**不可能再烧几小时 runner**。
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
//! 只在 `WLWL_HANDLE_DUMP=1` 时干活 ⇒ 正常 CI 跑它 ≈ **0 成本**（立刻返回）。

mod common;

use std::time::{Duration, Instant};

use common::handle_guard::open_handle_count;

/// eval 次数（**每次 eval 后都要读一次数** ⇒ 不能批量读）。
const EVALS: usize = 20_000;

/// 跑一次 ⇒ 返回 `(成功?, 诊断消息)`。
fn run_once(src: &str, dir: &std::path::Path) -> (bool, String) {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    let mut ev = wlwl_eval::Evaluator::new().with_base_dir(dir.to_path_buf());
    match ev.eval(&ast) {
        Ok(_) => (true, String::new()),
        Err(e) => (false, e.diagnostic().message.clone()),
    }
}

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
/// ⚠ 注解 message **不能含换行** ⇒ 多条压成一行；⚠ 一律 **ASCII**
/// （Windows runner 的 locale 不是 UTF-8 ⇒ 非 ASCII 会变乱码）。
fn annotation(title: &str, msg: &str) {
    let ascii: String = msg
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    let one_line: String = ascii.replace(['\r', '\n'], " ");
    println!("::warning title={title}::{one_line}");
}

#[test]
fn ci_only_handle_dump_probe() {
    if std::env::var("WLWL_HANDLE_DUMP").as_deref() != Ok("1") {
        println!("[handle-dump] 未设置 WLWL_HANDLE_DUMP=1 ⇒ 跳过（正常 CI 不受影响）");
        return;
    }

    let dir = common::scratch("wlwl_handle_dump");
    let missing = dir
        .join("no-such-file.txt")
        .to_string_lossy()
        .replace('\\', "/");
    let src =
        format!("IMPORT(\"wlwl:std.fs\", [\"READ_FILE\"]);\nPRINT(READ_FILE(\"{missing}\"));\n");

    // 热身：把懒初始化的一次性句柄排除在基线之外。
    let (ok, msg) = run_once(&src, &dir);
    assert!(
        !ok,
        "the probe's program must FAIL (that is the point): {msg}"
    );
    println!("[handle-dump] 程序确实失败：{msg}");

    let baseline = settled(1_000);
    println!("[handle-dump] baseline handle count = {baseline}");

    #[cfg(windows)]
    let base_snapshot = guarded_scan(120).unwrap_or_default();
    #[cfg(not(windows))]
    let base_snapshot: Vec<(usize, String)> = Vec::new();
    // ⚠ 非 Windows 平台**没有句柄表可扫** ⇒ 这张快照在那边只被下面的
    //   `#[cfg(windows)]` 块用到 ⇒ 在非 Windows 侧它是**未使用变量** ⇒
    //   CI 带 `-D warnings` ⇒ 会**直接红**（实测：run #337 的 ubuntu / macOS
    //   就红在这一条）⇒ 这里显式吞掉它。
    let _ = &base_snapshot;

    #[cfg(windows)]
    {
        let with_path = base_snapshot.iter().filter(|e| !e.1.is_empty()).count();
        println!(
            "[handle-dump] 基线快照：{} 个有效句柄，其中带路径的 {} 个",
            base_snapshot.len(),
            with_path
        );
    }

    let t0 = Instant::now();
    let mut transient = 0usize;
    for i in 0..EVALS {
        run_once(&src, &dir);
        let now = open_handle_count();
        if now != baseline {
            let after = settled(500);
            if after != baseline {
                // ⇒ **真泄漏**：把句柄点名。
                println!(
                    "[handle-dump] ⚠ 真泄漏命中：第 {i} 次 eval 后读数 {now}（基线 {baseline}）\
                     ，落定后仍是 {after}"
                );
                #[cfg(windows)]
                let detail = match guarded_scan(120) {
                    Some(now_snapshot) => describe_diff(&base_snapshot, &now_snapshot),
                    None => "SCAN-TIMEOUT (guarded scan did not finish in 120s)".to_string(),
                };
                #[cfg(not(windows))]
                let detail: String = "non-Windows: nothing to name".to_string();
                annotation(
                    "handle-leak (windows runner)",
                    &format!(
                        "LEAK-HIT eval={i} count={now} baseline={baseline} settled={after} :: {detail}"
                    ),
                );
                let _ = std::fs::remove_dir_all(&dir);
                return;
            }
            transient += 1;
        }
    }

    println!(
        "[handle-dump] 跑完 {EVALS} 次 eval：**未命中**（瞬时偏离 {transient} 次）\
         ，elapsed={:?} ⇒ 这台机器上复现不出",
        t0.elapsed()
    );
    #[cfg(windows)]
    let extra = format!(
        "snapshot={} with_path={}",
        base_snapshot.len(),
        base_snapshot.iter().filter(|e| !e.1.is_empty()).count()
    );
    #[cfg(not(windows))]
    let extra = "non-Windows".to_string();
    annotation(
        "handle-leak probe (no hit)",
        &format!("NO-HIT evals={EVALS} baseline={baseline} transient={transient} {extra}"),
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Windows：安全的句柄点名 ────────────────────────────────────────────────

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
        pub fn GetFinalPathNameByHandleW(
            handle: Handle,
            path: *mut u16,
            len: u32,
            flags: u32,
        ) -> u32;
    }

    pub const DUPLICATE_SAME_ACCESS: u32 = 0x2;
    /// 句柄值的扫描上限（Windows 的句柄值是很小的整数 ⇒ 4096 绰绰有余）。
    pub const MAX_HANDLE_VALUE: isize = 4096;
}

#[cfg(windows)]
/// 扫出本进程所有**有效**的句柄值 ⇒ `[(句柄值, 文件路径或空串)]`。
///
/// ⚠ **只用安全 API**（`DuplicateHandle` + `GetFinalPathNameByHandleW`）——
///   ⚠ **绝不要**在这里加 `NtQueryObject`（见文件头的事故记录：它在 CI runner
///   的同步管道句柄上永久挂死）。代价是拿不到类型名。
fn scan_handles() -> Vec<(usize, String)> {
    use win::*;

    let mut out = Vec::new();
    unsafe {
        let me = GetCurrentProcess() as Handle;
        for v in 0..MAX_HANDLE_VALUE {
            let mut dup: Handle = 0;
            if DuplicateHandle(me, v, me, &mut dup, 0, 0, DUPLICATE_SAME_ACCESS) == 0 {
                continue;
            }
            let path = final_path(dup);
            let _ = CloseHandle(dup);
            out.push((v as usize, path));
        }
    }
    out
}

#[cfg(windows)]
/// 文件句柄指向的路径（非文件句柄 ⇒ 空串）。
fn final_path(h: win::Handle) -> String {
    use win::*;

    unsafe {
        let mut buf = [0u16; 2048];
        let n = GetFinalPathNameByHandleW(h, buf.as_mut_ptr(), buf.len() as u32, 0);
        if n == 0 || n as usize > buf.len() {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

#[cfg(windows)]
/// **超时保护**的扫描：跑在独立线程里，最多等 `timeout_secs`。
///
/// ⚠ 为什么必须：`NtQueryObject` 那类 API 会**永久挂死**且线程杀不掉 ⇒
///   把扫描关进「可放弃」的线程里 ⇒ 万一再踩到，探针只是**拿不到证据**，
///   不会像第一版那样把 CI 步骤挂死几个小时。
/// ⚠ 超时后那个线程仍在（Rust 杀不掉线程）⇒ 进程结束时随之回收。
fn guarded_scan(timeout_secs: u64) -> Option<Vec<(usize, String)>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scan_handles());
    });
    rx.recv_timeout(Duration::from_secs(timeout_secs)).ok()
}

#[cfg(windows)]
/// 与基线**做差** ⇒ 一段**单行**摘要（进注解）。
fn describe_diff(base: &[(usize, String)], now: &[(usize, String)]) -> String {
    let base_handles: std::collections::HashSet<usize> = base.iter().map(|e| e.0).collect();
    let added: Vec<&(usize, String)> = now
        .iter()
        .filter(|e| !base_handles.contains(&e.0))
        .collect();
    println!(
        "[handle-dump] 基线 {} 个有效句柄 ⇒ 现在 {} 个 ⇒ **新增 {} 个**",
        base.len(),
        now.len(),
        added.len()
    );
    if added.is_empty() {
        println!(
            "[handle-dump]   ⚠ 没有「新增句柄」⇒ 多出来的可能是**已存在的句柄被复用**\
             ⇒ 按句柄值比对不出来"
        );
        return format!(
            "NO-ADDED-HANDLE baseline={} now={} (a reused handle value)",
            base.len(),
            now.len()
        );
    }
    let mut parts: Vec<String> = Vec::new();
    for (handle, path) in &added {
        let kind = if path.is_empty() {
            "<non-file kernel object>"
        } else {
            path.as_str()
        };
        println!("[handle-dump]   新增句柄 #{handle}  {kind}");
        parts.push(format!("#{handle} {kind}"));
    }
    format!("ADDED={} :: {}", added.len(), parts.join(" | "))
}
