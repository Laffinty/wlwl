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
//! 4. 「点名」的办法：扫 `0..4096` 的句柄值，**用 `DuplicateHandle` 试**哪个
//!    是本进程的有效句柄（成功 ⇔ 有效）⇒ 再用 `NtQueryObject` 取**类型名**、
//!    `GetFinalPathNameByHandleW` 取**文件路径** ⇒ 与基线快照**做差** ⇒
//!    CI 日志里直接看得见「多出来的是什么（文件？线程？子进程？事件？）、指向哪里」。
//!
//! ## 为什么不用 `NtQuerySystemInformation(SystemExtendedHandleInformation)`
//!
//! ⚠ 实测（Windows 11，本机）：那个类给的是**全系统**的句柄表（本机 9 万多条，
//!   含别的进程的）⇒ **没有 PID 字段**没法按进程过滤 ⇒ 用不得。
//! `SystemHandleInformation` 在现代 Windows 上同样对不上（实测三种 stride 都
//!   匹配不到本进程 PID）⇒ 也用不得。
//! ⇒ 改走**用户态暴力扫描**：只依赖 `DuplicateHandle` / `NtQueryObject` /
//!   `GetFinalPathNameByHandleW` 三个公开 API ⇒ **无版本依赖、无需系统表**。
//!
//! ## 成本
//!
//! 只在 `WLWL_HANDLE_DUMP=1` 时干活 ⇒ 正常 CI 跑它 ≈ **0 成本**（立刻返回）。
//! 打开时每次 eval ≈ 0.4 ms ⇒ 20000 次 ≈ 8 s；扫描只在**命中时**才做（≈ 几 ms）。

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
    let base_snapshot = scan_handles();
    #[cfg(not(windows))]
    let base_snapshot: Vec<(usize, String, String)> = Vec::new();

    // ⚠ **无条件**把基线快照的概貌打出来 ⇒ ① 证明扫描那套 API 在真机上**能用**
    //   （类型名 / 文件路径读得出来）；② CI 日志里能直接和后面的「新增句柄」对照。
    #[cfg(windows)]
    {
        println!(
            "[handle-dump] 基线快照：{} 个可识别句柄",
            base_snapshot.len()
        );
        let hist = histogram(&base_snapshot);
        for (k, n) in hist.iter().take(12) {
            println!("[handle-dump]   {k} x {n}");
        }
        let with_path = base_snapshot.iter().filter(|e| !e.2.is_empty()).count();
        println!("[handle-dump]   其中带路径的：{with_path} 个（⇒ 路径读取可用）");
    }

    let t0 = Instant::now();
    let mut transient = 0usize;
    for i in 0..EVALS {
        run_once(&src, &dir);
        let now = open_handle_count();
        if now != baseline {
            let after = settled(500);
            if after != baseline {
                // ⇒ **真泄漏**：把句柄点名（CI 日志里直接看出持有者）。
                println!(
                    "[handle-dump] ⚠ 真泄漏命中：第 {i} 次 eval 后读数 {now}（基线 {baseline}）\
                     ，落定后仍是 {after}"
                );
                #[cfg(windows)]
                dump_diff(&base_snapshot);
                #[cfg(not(windows))]
                println!("[handle-dump] （非 Windows 平台 ⇒ 无句柄可点名）");
                let _ = std::fs::remove_dir_all(&dir);
                return;
            }
            transient += 1;
        }
    }

    println!(
        "[handle-dump] 跑完 {EVALS} 次 eval：**未命中**（瞬时偏离 {transient} 次）\
         ，elapsed={:?} ⇒ 这台机器上复现不出 ⇒ 看 CI runner 的那份日志",
        t0.elapsed()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Windows：句柄点名 ──────────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    pub type Handle = isize;

    #[link(name = "kernel32")]
    extern "system" {
        /// ⚠ 签名必须与 `common::handle_guard` 里那份**逐字一致**（同一个 Win32
        ///   API 声明两遍、类型不同 ⇒ rustc 会报 "redeclared with a different
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

    #[link(name = "ntdll")]
    extern "system" {
        pub fn NtQueryObject(
            handle: Handle,
            class: u32,
            info: *mut c_void,
            len: u32,
            ret_len: *mut u32,
        ) -> i32;
    }

    pub const OBJECT_TYPE_INFORMATION: u32 = 2;
    pub const DUPLICATE_SAME_ACCESS: u32 = 0x2;
    /// 句柄值的扫描上限（Windows 的句柄值是很小的整数 ⇒ 4096 绰绰有余）。
    pub const MAX_HANDLE_VALUE: isize = 4096;
}

#[cfg(windows)]
/// 扫出本进程所有**有效**的句柄值 ⇒ `[(句柄值, 类型名, 文件路径)]`。
///
/// ⚠ 手法：`DuplicateHandle(GetCurrentProcess(), v, …)` 成功 ⇔ `v` 是本进程的有效句柄。
/// ⚠ 结果是**真实句柄表的一个超集**（线程池的内部对象等也会被算进去）⇒
///   所以只用它**做差**（基线 vs 事后），不用它当绝对计数。
fn scan_handles() -> Vec<(usize, String, String)> {
    use win::*;

    let mut out = Vec::new();
    unsafe {
        let me = GetCurrentProcess() as Handle;
        for v in 0..MAX_HANDLE_VALUE {
            let mut dup: Handle = 0;
            if DuplicateHandle(me, v, me, &mut dup, 0, 0, DUPLICATE_SAME_ACCESS) == 0 {
                continue;
            }
            let ty = type_name(dup);
            let path = final_path(dup);
            let _ = CloseHandle(dup);
            out.push((v as usize, ty, path));
        }
    }
    out
}

#[cfg(windows)]
/// 句柄的**类型名**（`NtQueryObject` ⇒ `ObjectTypeInformation` ⇒ UNICODE_STRING）。
fn type_name(h: win::Handle) -> String {
    use std::ffi::c_void;
    use win::*;

    unsafe {
        let mut buf = [0u8; 1024];
        let mut needed: u32 = 0;
        let st = NtQueryObject(
            h,
            OBJECT_TYPE_INFORMATION,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as u32,
            &mut needed,
        );
        if st != 0 {
            return format!("<type? st={st}>");
        }
        // UNICODE_STRING { USHORT Length; USHORT MaximumLength; PWSTR Buffer; }
        let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        let buffer_ptr = *(buf.as_ptr().add(8) as *const usize);
        if len == 0 || buffer_ptr == 0 {
            return "<type?>".to_string();
        }
        let slice = std::slice::from_raw_parts(buffer_ptr as *const u16, len / 2);
        String::from_utf16_lossy(slice)
    }
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
/// 类型分布（按数量降序）。
fn histogram(snapshot: &[(usize, String, String)]) -> Vec<(String, usize)> {
    let mut kinds: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (_, ty, _) in snapshot {
        *kinds.entry(ty.clone()).or_insert(0) += 1;
    }
    let mut v: Vec<(String, usize)> = kinds.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

#[cfg(windows)]
/// 只打印**相对基线新增的**句柄 ⇒ CI 日志里直接看见「多出来的是什么」。
fn dump_diff(base: &[(usize, String, String)]) {
    let now = scan_handles();
    let base_handles: std::collections::HashSet<usize> = base.iter().map(|e| e.0).collect();
    let added: Vec<_> = now
        .iter()
        .filter(|e| !base_handles.contains(&e.0))
        .collect();
    let gone = base_handles.len() - (now.len() - added.len());
    println!(
        "[handle-dump] 基线 {} 个可识别句柄 ⇒ 现在 {} 个 ⇒ **新增 {} 个**（少了 {gone} 个）",
        base.len(),
        now.len(),
        added.len()
    );
    for (handle, ty, path) in &added {
        println!(
            "[handle-dump]   新增句柄 #{handle}  类型={ty}  路径={}",
            if path.is_empty() { "<无路径>" } else { path }
        );
    }
    if added.is_empty() {
        println!(
            "[handle-dump]   ⚠ 没有「新增句柄」⇒ 多出来的可能是**已存在的句柄被复用**\
             ⇒ 改看类型分布对比："
        );
        let hb = histogram(base);
        let hn = histogram(&now);
        for (k, n) in hn.iter().take(12) {
            let b = hb.iter().find(|e| e.0 == *k).map(|e| e.1).unwrap_or(0);
            println!("[handle-dump]     {k}: 基线 {b} ⇒ 现在 {n}");
        }
    }
}
