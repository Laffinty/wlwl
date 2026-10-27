//! [W-27 / v0.12.1] `wlwl:std.fs` 的**原子写 / 文件锁 / 目录元数据** —— 契约测试。
//!
//! 覆盖 `WRITE_FILE_ATOMIC` / `LOCK_FILE` / `UNLOCK_FILE` / `FILE_MTIME` /
//! `DIR_ENTRIES` 五个成员(需求清单 E-1 / E-2 / E-3)。
//!
//! ## 一、期望值从哪来
//!
//! **不是从实现输出抄的** —— 那等于把实现钉成「永远正确」。四类来源:
//!
//! 1. **POSIX / Win32 的平台事实**:原子替换在 Windows 上是
//!    `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`、在
//!    POSIX 上是 `rename(2)`;`flock` 的锁挂在**打开文件描述**上而 `fcntl`
//!    记录锁挂在**进程**上;`FILETIME` 是 1601 起的 100 ns 刻数而 Unix 纪元是
//!    1970 ⇒ 差 369 年。三条都要求实现**跨平台同口径**。
//! 2. **既有成员的边界**:`LIST_DIR` **排序**、`WALK_DIR(follow = FALSE)`
//!    **不跟随符号链接**、`FILE_SIZE` **跟随**符号链接 —— `DIR_ENTRIES` 与
//!    `FILE_MTIME` 各按其中一条落位。
//! 3. **本仓既有的失败口径**:运行期失败 ⇒ `ERR([kind:"FsError", op, reason])`
//!    **值**;元数 / 类型错 ⇒ 原生诊断 `E0022` / `E0030`。
//! 4. **文件系统自身的口径**:目录迭代序未定义 ⇒ 必须排序;文件时间戳有
//!    粗粒度 ⇒ 断言必须带**容差**而不是逐毫秒相等。
//!
//! ## 二、**反向守卫**(本文件最重要的一条)
//!
//! `a_reader_never_sees_a_half_written_target` 用一个**并发读者**证明
//! 「原子」这件事:读者在写者反复替换的整个期间里,读到的永远是**一个完整
//! 的版本**(或读不到 —— 那也算中间态)。
//!
//! ⇒ 把实现里的「原子替换」改成「**先删目标再写**」(Windows 上 rename 不能
//!   覆盖目标时的退化写法),这条**必须变红**。
//!
//! ## 三、句柄纪律
//!
//! `LOCK_FILE` 的返回值是**不透明整数**。本文件只把它原样传给
//! `UNLOCK_FILE`,**从不**假设它是什么(它不是 fd、不是路径、跨进程也不通用)。
//!
//! ⚠ **IO 用例一律用绝对路径**:fs 成员的相对路径按**进程 CWD** 解析
//! (`with_base_dir` 只影响 `IMPORT` 的模块解析)⇒ 相对路径会把文件写进仓库里。

mod common;

use std::path::{Path, PathBuf};
// ⚠ 并发读者那条用例只在 POSIX 上编译(见它的说明)⇒ 这两个 import 也是。
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(unix)]
use std::sync::{Arc, Mutex};
use wlwl_eval::Evaluator;

/// IO 用例共用的 IMPORT 头。
const IO: &str = concat!(
    r#"IMPORT("wlwl:std.fs", ["WRITE_FILE_ATOMIC", "LOCK_FILE", "UNLOCK_FILE", "#,
    r#""FILE_MTIME", "DIR_ENTRIES", "LIST_DIR", "READ_FILE", "REMOVE"]); "#,
);

/// 一个临时目录(已创建好)。
fn scratch() -> PathBuf {
    common::scratch("wlwl_fs_atomic_lock_meta")
}

/// 建临时目录 **并取串行闸**。
///
/// ⚠ **为什么每条用例都要取闸**:同一条测试二进制里的用例**默认并行**跑,
///   而「进程打开的句柄数」是**进程级**读数 ⇒ 别的用例正开着文件时,本仓
///   `handle_guard` 那条守卫会读到**偏掉的基线** ⇒ **假红**(实测踩过:
///   `baseline=84 after=91` ⇒ 那 7 个句柄是**别的用例**的,不是泄漏)。
///   ⇒ 本文件**所有**用例都取**同一把**闸(`common::handle_guard::serial_gate`)
///   ⇒ 守卫那条用例(`assert_no_handle_leak`)**自己**取闸 ⇒ 不能在这里取
///   (同一把 `Mutex` 不可重入 ⇒ 会自锁)⇒ 那条用例用 [`scratch`]。
fn gated_scratch() -> (PathBuf, std::sync::MutexGuard<'static, ()>) {
    let gate = common::handle_guard::serial_gate();
    // ⚠ 这里必须用**不取闸**的 `scratch()`：同一把 `Mutex` 不可重入 ⇒
    // 若在这里再取一次闸就会自锁（闸已在上一行取过）。
    let dir = scratch();
    (dir, gate)
}

/// 跑一段 wlwl 源码,返回它的显示值(诊断 ⇒ `!CODE message`)。
fn run(src: &str, dir: &Path) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(dir.to_path_buf());
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

/// ⚠ **IO 用例必须用绝对路径**(见模块说明)。
fn abs(dir: &Path, rel: &str) -> String {
    dir.join(rel).to_string_lossy().replace('\\', "/")
}

/// 目录里现在有哪些文件(排序)—— 「不留残留」这条契约的观测口径。
fn sorted_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// 当前墙钟(Unix 纪元起的毫秒)。
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════
// 原子写
// ═══════════════════════════════════════════════════════════════════════

/// ⚠ **成功路径**:内容写对了,且目录里**只有目标那一个文件**。
///
/// 「无残留」是本成员的**契约**(调用方看得见目录里多了一个文件)⇒
/// 临时文件的名字必须被实现**清理掉**。
#[test]
fn write_file_atomic_writes_the_content_and_leaves_no_temporary_file() {
    let (dir, _gate) = gated_scratch();
    let target = abs(&dir, "a.txt");
    assert_eq!(
        run(
            &format!("{IO}WRITE_FILE_ATOMIC(\"{target}\", \"hello\")"),
            &dir
        ),
        "NULL",
        "WRITE_FILE_ATOMIC must succeed on a fresh path"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "hello",
        "the content must land byte-for-byte"
    );
    assert_eq!(
        sorted_names(&dir),
        vec!["a.txt".to_string()],
        "WRITE_FILE_ATOMIC must not leave a temporary file behind"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **替换既有文件**:旧内容必须**整个**被换掉(不是「截断后接着写」)。
#[test]
fn write_file_atomic_replaces_the_whole_old_content() {
    let (dir, _gate) = gated_scratch();
    // 旧内容**更长** ⇒ 若实现是「截断 + 追加」,长度会露馅。
    std::fs::write(dir.join("a.txt"), b"OLD-AND-MUCH-LONGER").unwrap();
    let target = abs(&dir, "a.txt");
    assert_eq!(
        run(
            &format!("{IO}WRITE_FILE_ATOMIC(\"{target}\", \"NEW\")"),
            &dir
        ),
        "NULL"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("a.txt")).unwrap(),
        "NEW",
        "the old content must be gone, not merged"
    );
    assert_eq!(sorted_names(&dir), vec!["a.txt".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **失败路径**:运行期失败 ⇒ **`ERR` 值**(不是诊断),目标原封不动,
/// 且**不留残留**。
///
/// 两条子路径各覆盖一段:
/// - ① 父目录不存在 ⇒ **建临时文件**那一步就失败(目标压根没被碰过);
/// - ② 目标是**已存在的目录** ⇒ 临时文件**已经写完**了,是**替换**那一步失败
///   ⇒ 这一条才真正覆盖「提交失败 ⇒ 清理临时文件」。
#[test]
fn a_failed_atomic_write_is_an_err_value_and_leaves_nothing_behind() {
    let (dir, _gate) = gated_scratch();

    // ① 父目录不存在。
    let missing = abs(&dir, "no_such_dir/a.txt");
    // ⚠ 用 `IS_ERR` / `ERR_PAYLOAD` **消费**那个 ERR 值 —— 不消费的话它会
    //   一路逃到顶层,变成诊断 `E0102`(那是语言的规矩,不是本成员的失败)。
    assert_eq!(
        run(
            &format!("{IO}IS_ERR(WRITE_FILE_ATOMIC(\"{missing}\", \"x\"))"),
            &dir
        ),
        "TRUE",
        "a runtime failure must be an ERR value, not a diagnostic"
    );
    let payload = run(
        &format!("{IO}ERR_PAYLOAD(WRITE_FILE_ATOMIC(\"{missing}\", \"x\"))"),
        &dir,
    );
    assert!(payload.contains("kind: FsError"), "{payload}");
    assert!(payload.contains("op: WRITE_FILE_ATOMIC"), "{payload}");
    assert!(
        sorted_names(&dir).is_empty(),
        "a failed write must not create anything: {:?}",
        sorted_names(&dir)
    );

    // ② 目标是个**已存在的目录** ⇒ 替换失败(临时文件已经写完了)。
    std::fs::create_dir_all(dir.join("d")).unwrap();
    assert_eq!(
        run(
            &format!(
                "{IO}IS_ERR(WRITE_FILE_ATOMIC(\"{}\", \"x\"))",
                abs(&dir, "d")
            ),
            &dir
        ),
        "TRUE",
        "replacing a directory with a file must fail as an ERR value"
    );
    assert!(
        dir.join("d").is_dir(),
        "a failed atomic write must not touch the target"
    );
    assert_eq!(
        sorted_names(&dir),
        vec!["d".to_string()],
        "a failed commit must clean up its temporary file: {:?}",
        sorted_names(&dir)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠🔴 **反向守卫**:读者**永远**看不到中间态。
///
/// 这是「原子」这件事**唯一**可观察的证据:写者反复替换目标,读者在同一个
/// 期间里不停地读 ⇒ 读到的永远是**一个完整的版本**。
///
/// ⇒ 把实现改成「**先删目标再写**」,读者会看到「文件不见了」或「内容只写了一半」
///   ⇒ 这条**必须变红**。
///
/// ⚠ **为什么读者必须是另一个线程**:单线程写不出「中间态」——写完再读永远
///   读得到完整内容。
/// ⚠ **为什么载荷要够大**:退化写法里「目标被删掉 / 被截断」的那段窗口与
///   **写入耗时**成正比 ⇒ 载荷太小就抓不到。
///
/// ⚠⚠ **为什么这条只在 POSIX 上跑**:Windows 的 `MoveFileExW` **不许替换
///   「正被别处打开」的文件**(实测:读者握着目标时替换返回
///   `ERROR_ACCESS_DENIED` ⇒ 这次写会**失败**)⇒ 「读者与写者同时在场」这个
///   场景在 Windows 上**根本构造不出来** ⇒ 这条断言在 Windows 上没有可观测
///   的对象(写先失败了)。
///   ⇒ Windows 侧由另外两条**确定性**证据承担同一件事:
///   ① `impl/crates/wlwl-std/src/fs.rs` 里「提交前目标一个字都没动过」
///      (提交点观测)与「提交失败 ⇒ 目标保持旧内容 + 无残留」两条单测;
///   ② 本文件里「成功路径不留残留」那条 —— 退化实现会把临时文件留在目录里
///      ⇒ **它必红**(两个平台都红)。
#[cfg(unix)]
#[test]
fn a_reader_never_sees_a_half_written_target() {
    const SIZE: usize = 32 * 1024;
    const ROUNDS: usize = 40;
    /// 一次写允许重试的次数(Windows 上目标被读者握着时替换会失败)。
    const WRITE_ATTEMPTS: usize = 60;

    let payload_a = "A".repeat(SIZE);
    let payload_b = "B".repeat(SIZE);

    let (dir, _gate) = gated_scratch();
    let target = dir.join("data.bin");
    std::fs::write(&target, payload_a.as_bytes()).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let bad: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let reader = {
        let (stop, reads, bad) = (stop.clone(), reads.clone(), bad.clone());
        let target = target.clone();
        let payload_a = payload_a.clone();
        let payload_b = payload_b.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match std::fs::read(&target) {
                    Ok(content) => {
                        reads.fetch_add(1, Ordering::Relaxed);
                        if content != payload_a.as_bytes() && content != payload_b.as_bytes() {
                            bad.lock().unwrap().push(format!(
                                "the reader saw a partial target ({} bytes)",
                                content.len()
                            ));
                        }
                    }
                    Err(e) => {
                        // ⚠ **Windows 专属的例外**:替换进行中,读者**可能**被
                        //   短暂挡一下(`ERROR_SHARING_VIOLATION`)⇒ 那不是中间态
                        //   (实测:Windows 的 `MoveFileExW` 那一路读数里
                        //   sharing=0、missing=0、partial=0)。
                        // ⚠ 「文件不见了」**不算**例外 —— 那正是退化实现露出来的
                        //   中间态。
                        if cfg!(windows) && e.raw_os_error() == Some(32) {
                            reads.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                        bad.lock()
                            .unwrap()
                            .push(format!("the reader could not read the target: {e}"));
                    }
                }
                // ⚠ **每读一次歇一下**:Windows 上目标被读者握着时替换会失败
                //   ⇒ 读者占着的时间越少,写者越容易一次就成(整条用例才跑得快)。
                //   ⚠ 歇的间隔(1 ms)必须与「退化实现露出的窗口」同量级
                //   ⇒ 载荷 32 KiB 的直写窗口(~50 µs)… hmm, 那比 1 ms 小两个量级
                //   ⇒ 靠**轮数**补:一轮写 ≈ 几 ms(fsync)⇒ 一轮下来读者能采到
                //   好几个样 ⇒ 40 轮累计几百次采样 ⇒ 撞上的概率≈1。
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        })
    };

    let target_str = abs(&dir, "data.bin");
    for i in 0..ROUNDS {
        let payload = if i % 2 == 0 { &payload_a } else { &payload_b };
        // ⚠ **允许重试**:Windows 上 `MoveFileExW` **不许替换「正被别处打开」的
        //   文件**(实测)⇒ 读者握着目标时,这一次写会失败 ⇒ 那是**平台限制**,
        //   不是本成员的缺陷 ⇒ 这里重试到成功为止。
        //   ⚠ 退化实现(先删目标再写)**不会**失败 ⇒ 它照样一路写下去 ⇒
        //   读者照样会抓到中间态 ⇒ **这条守卫不会被重试稀释掉**。
        let mut last = String::new();
        let mut ok = false;
        for attempt in 0..WRITE_ATTEMPTS {
            last = run(
                &format!("{IO}WRITE_FILE_ATOMIC(\"{target_str}\", \"{payload}\")"),
                &dir,
            );
            if last == "NULL" {
                ok = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            let _ = attempt;
        }
        assert!(ok, "round {i} must succeed in the end: {last}");
    }

    stop.store(true, Ordering::Relaxed);
    reader.join().expect("the reader thread must not panic");

    let bad = bad.lock().unwrap();
    assert!(
        bad.is_empty(),
        "an atomic write must never expose an intermediate state:\n    {}",
        bad.join("\n    ")
    );
    let reads = reads.load(Ordering::Relaxed);
    assert!(
        reads >= 10,
        "the reader must actually have run (only {reads} reads) — \
         otherwise this guard proved nothing"
    );
    // ⚠ 收尾:读完之后内容必须是**某一个完整版本**。
    let final_content = std::fs::read(&target).unwrap();
    assert!(
        final_content == payload_a.as_bytes() || final_content == payload_b.as_bytes(),
        "the final content must be one of the complete versions"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════
// 文件锁
// ═══════════════════════════════════════════════════════════════════════

/// ⚠ **互斥**:同进程两个锁 ⇒ 第二个**必须被拒**。
///
/// ⚠ POSIX 侧若用 `fcntl` 记录锁就**过不了**这条(记录锁是**按进程**的 ⇒
/// 第二次加锁直接成功)⇒ 这条是「必须用 `flock`」的守卫。
/// ⚠ Windows 侧 `LockFileEx` 的锁挂在**句柄**上 ⇒ 两次独立的打开就是两把锁。
#[test]
fn a_second_lock_on_the_same_file_is_refused_until_the_first_is_released() {
    let (dir, _gate) = gated_scratch();
    let path = abs(&dir, "lock");

    let h1 = run(&format!("{IO}LOCK_FILE(\"{path}\")"), &dir);
    assert!(
        h1.parse::<i64>().is_ok(),
        "LOCK_FILE must return an opaque integer handle: {h1}"
    );

    let h2 = run(&format!("{IO}LOCK_FILE(\"{path}\")"), &dir);
    assert_eq!(
        run(&format!("{IO}IS_ERR(LOCK_FILE(\"{path}\"))"), &dir),
        "TRUE",
        "a second exclusive lock on the same file must be refused"
    );
    let payload = run(&format!("{IO}ERR_PAYLOAD(LOCK_FILE(\"{path}\"))"), &dir);
    assert!(payload.contains("kind: FsError"), "{payload}");
    assert!(payload.contains("op: LOCK_FILE"), "{payload}");
    assert!(
        payload.contains("already locked"),
        "the refusal must say \"already locked\", not \"I/O error\": {payload}"
    );
    let _ = h2;

    // 释放之后 ⇒ 能再拿到(锁真的被放掉了)。
    assert_eq!(run(&format!("{IO}UNLOCK_FILE({h1})"), &dir), "NULL");
    let h3 = run(&format!("{IO}LOCK_FILE(\"{path}\")"), &dir);
    assert!(
        h3.parse::<i64>().is_ok(),
        "the lock must be re-acquirable after UNLOCK_FILE: {h3}"
    );
    assert_eq!(run(&format!("{IO}UNLOCK_FILE({h3})"), &dir), "NULL");
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **幂等**:重复释放**不报错**;但一个**从未发出过**的句柄必须被拒。
#[test]
fn unlock_file_is_idempotent_but_rejects_a_handle_that_was_never_issued() {
    let (dir, _gate) = gated_scratch();
    let path = abs(&dir, "lock");
    let h = run(&format!("{IO}LOCK_FILE(\"{path}\")"), &dir);
    assert!(h.parse::<i64>().is_ok(), "{h}");
    assert_eq!(run(&format!("{IO}UNLOCK_FILE({h})"), &dir), "NULL");
    assert_eq!(
        run(&format!("{IO}UNLOCK_FILE({h})"), &dir),
        "NULL",
        "releasing the same handle twice must be idempotent"
    );
    let bad = run(&format!("{IO}UNLOCK_FILE(987654)"), &dir);
    assert_eq!(
        run(&format!("{IO}IS_ERR(UNLOCK_FILE(987654))"), &dir),
        "TRUE",
        "a handle that was never issued must be refused"
    );
    let payload = run(&format!("{IO}ERR_PAYLOAD(UNLOCK_FILE(987654))"), &dir);
    assert!(payload.contains("kind: FsError"), "{payload}");
    assert!(payload.contains("op: UNLOCK_FILE"), "{payload}");
    let _ = bad;
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **不漏句柄**:加锁 + 解锁之后,进程打开的句柄数必须**回到基线**。
///
/// ⚠ 用 `common::handle_guard` 的**声明式**夹具(它自带串行闸 —— 句柄计数是
///   **进程级**的,不串行会被同文件里别的用例污染)。
#[test]
fn locking_and_unlocking_does_not_leak_a_handle() {
    // ⚠ **这条不能取串行闸**:`assert_no_handle_leak` **自己**取同一把闸
    //   (`Mutex` 不可重入 ⇒ 这里再取一次就自锁)。
    let dir = scratch();
    let path = abs(&dir, "lock");
    common::handle_guard::assert_no_handle_leak(
        "wlwl:std.fs LOCK_FILE family",
        || run(&format!("{IO}LOCK_FILE(\"{path}\")"), &dir),
        |h| {
            let _ = run(&format!("{IO}UNLOCK_FILE({h})"), &dir);
        },
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════
// 修改时间
// ═══════════════════════════════════════════════════════════════════════

/// ⚠ **口径**:`FILE_MTIME` = **Unix 纪元起的毫秒**(两个平台一致)。
///
/// ⚠ **必须带容差**:文件系统的时间戳**粒度**是粗的(NTFS 100 ns、ext4 1 ns、
///   但 FAT 是 2 s)⇒ 落盘时刻可能比「写之前」**早**一个粒度 ⇒ 断言
///   「不早于写前」必须留 1 s 的余量,否则在粗粒度文件系统上会偶发红。
/// ⚠ **不许在未来**:那说明口径错了(比如把 `FILETIME` 的刻数当毫秒)。
#[test]
fn file_mtime_is_unix_milliseconds_and_tracks_the_write() {
    let (dir, _gate) = gated_scratch();
    let target = abs(&dir, "a.txt");
    let before = now_ms();
    assert_eq!(
        run(&format!("{IO}WRITE_FILE_ATOMIC(\"{target}\", \"x\")"), &dir),
        "NULL"
    );
    let after = now_ms();
    let got: i64 = run(&format!("{IO}FILE_MTIME(\"{target}\")"), &dir)
        .parse()
        .unwrap_or_else(|e| panic!("FILE_MTIME must return an integer: {e}"));
    assert!(
        got <= after + 1000,
        "the mtime must not be in the future: {got} > {after}"
    );
    assert!(
        got + 1000 >= before,
        "the mtime must not be older than the write: {got} < {before} (tolerance 1000ms)"
    );
    // ⚠ 口径必须是「Unix 纪元」⇒ 落在 2020-01-01 之后
    // (若误把 `FILETIME` 刻数当毫秒,这里会是一个 1601 年附近的数)。
    assert!(
        got > 1_577_836_800_000,
        "the epoch must be Unix, not FILETIME: {got}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **文件不存在** ⇒ `ERR` 值(不是诊断、不是 0)。
#[test]
fn file_mtime_on_a_missing_file_is_an_err_value() {
    let (dir, _gate) = gated_scratch();
    assert_eq!(
        run(
            &format!(
                "{IO}IS_ERR(FILE_MTIME(\"{}\"))",
                abs(&dir, "no_such_file.txt")
            ),
            &dir
        ),
        "TRUE",
        "a missing file must be an ERR value, not a diagnostic"
    );
    let payload = run(
        &format!(
            "{IO}ERR_PAYLOAD(FILE_MTIME(\"{}\"))",
            abs(&dir, "no_such_file.txt")
        ),
        &dir,
    );
    assert!(payload.contains("kind: FsError"), "{payload}");
    assert!(payload.contains("op: FILE_MTIME"), "{payload}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════
// 目录项
// ═══════════════════════════════════════════════════════════════════════

/// ⚠ **形状**:`{name, size, mtime, is_dir}`;**排序**稳定;目录项的
/// `size` / `mtime` 给 `0`。
#[test]
fn dir_entries_reports_a_sorted_dict_per_entry() {
    let (dir, _gate) = gated_scratch();
    std::fs::write(dir.join("b.txt"), b"bb").unwrap();
    std::fs::write(dir.join("a.txt"), b"a").unwrap();
    std::fs::create_dir_all(dir.join("c")).unwrap();
    let d = abs(&dir, "");

    assert_eq!(
        run(&format!("{IO}LEN(DIR_ENTRIES(\"{d}\"))"), &dir),
        "3",
        "DIR_ENTRIES must report one item per directory entry"
    );

    // ⚠ 排序:按**名字**逐字节(OS 的目录迭代序未定义 ⇒ 不排序就没有确定性)。
    let first = run(&format!("{IO}INDEX_GET(DIR_ENTRIES(\"{d}\"), 0)"), &dir);
    assert!(first.starts_with("[name: a.txt,"), "{first}");
    assert!(first.contains("size: 1"), "{first}");
    assert!(first.contains("is_dir: FALSE"), "{first}");

    let second = run(&format!("{IO}INDEX_GET(DIR_ENTRIES(\"{d}\"), 1)"), &dir);
    assert!(second.starts_with("[name: b.txt,"), "{second}");
    assert!(second.contains("size: 2"), "{second}");
    assert!(second.contains("is_dir: FALSE"), "{second}");

    let third = run(&format!("{IO}INDEX_GET(DIR_ENTRIES(\"{d}\"), 2)"), &dir);
    assert!(third.starts_with("[name: c,"), "{third}");
    // ⚠ 目录项的 size / mtime 一律 0。
    assert!(third.contains("size: 0"), "{third}");
    assert!(third.contains("mtime: 0"), "{third}");
    assert!(third.contains("is_dir: TRUE"), "{third}");

    // ⚠ **排序稳定**:再跑一次,次序必须一模一样。
    let again = run(&format!("{IO}DIR_ENTRIES(\"{d}\")"), &dir);
    let once = run(&format!("{IO}DIR_ENTRIES(\"{d}\")"), &dir);
    assert_eq!(again, once, "DIR_ENTRIES must be deterministic");

    let _ = std::fs::remove_dir_all(&dir);
}

/// ⚠ **目录不存在** ⇒ `ERR` 值(与 `LIST_DIR` 同一口径)。
#[test]
fn dir_entries_on_a_missing_directory_is_an_err_value() {
    let (dir, _gate) = gated_scratch();
    assert_eq!(
        run(
            &format!("{IO}IS_ERR(DIR_ENTRIES(\"{}\"))", abs(&dir, "no_such_dir")),
            &dir
        ),
        "TRUE",
        "a missing directory must be an ERR value, not a diagnostic"
    );
    let payload = run(
        &format!(
            "{IO}ERR_PAYLOAD(DIR_ENTRIES(\"{}\"))",
            abs(&dir, "no_such_dir")
        ),
        &dir,
    );
    assert!(payload.contains("kind: FsError"), "{payload}");
    assert!(payload.contains("op: DIR_ENTRIES"), "{payload}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════
// 失败口径:元数 / 类型错 ⇒ 既有诊断(E0022 / E0030),**不新增诊断码**
// ═══════════════════════════════════════════════════════════════════════

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "write_file_atomic_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["WRITE_FILE_ATOMIC"]); WRITE_FILE_ATOMIC("a")"#,
        expect: "!E0022 WRITE_FILE_ATOMIC: function expects 2 argument(s), got 1",
    },
    Case {
        name: "write_file_atomic_path_type_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["WRITE_FILE_ATOMIC"]); WRITE_FILE_ATOMIC(5, "x")"#,
        expect: "!E0030 WRITE_FILE_ATOMIC: expected a string, got integer",
    },
    Case {
        name: "write_file_atomic_content_type_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["WRITE_FILE_ATOMIC"]); WRITE_FILE_ATOMIC("a", 5)"#,
        expect: "!E0030 WRITE_FILE_ATOMIC: expected a string, got integer",
    },
    Case {
        name: "lock_file_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["LOCK_FILE"]); LOCK_FILE("a", "b")"#,
        expect: "!E0022 LOCK_FILE: function expects 1 argument(s), got 2",
    },
    Case {
        name: "lock_file_type_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["LOCK_FILE"]); LOCK_FILE(5)"#,
        expect: "!E0030 LOCK_FILE: expected a string, got integer",
    },
    Case {
        name: "unlock_file_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["UNLOCK_FILE"]); UNLOCK_FILE()"#,
        expect: "!E0022 UNLOCK_FILE: function expects 1 argument(s), got 0",
    },
    Case {
        name: "unlock_file_type_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["UNLOCK_FILE"]); UNLOCK_FILE("x")"#,
        expect: "!E0030 UNLOCK_FILE: expected an integer, got string",
    },
    Case {
        name: "file_mtime_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["FILE_MTIME"]); FILE_MTIME("a", "b")"#,
        expect: "!E0022 FILE_MTIME: function expects 1 argument(s), got 2",
    },
    Case {
        name: "file_mtime_type_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["FILE_MTIME"]); FILE_MTIME(5)"#,
        expect: "!E0030 FILE_MTIME: expected a string, got integer",
    },
    Case {
        name: "dir_entries_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["DIR_ENTRIES"]); DIR_ENTRIES("a", "b")"#,
        expect: "!E0022 DIR_ENTRIES: function expects 1 argument(s), got 2",
    },
    Case {
        name: "dir_entries_type_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["DIR_ENTRIES"]); DIR_ENTRIES(5)"#,
        expect: "!E0030 DIR_ENTRIES: expected a string, got integer",
    },
];

#[test]
fn the_failure_surface_is_the_existing_one() {
    let (dir, _gate) = gated_scratch();
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = run(c.src, &dir);
            if got == c.expect {
                None
            } else {
                Some(format!(
                    "{name}:\n      expected: {want}\n      actual:   {got}",
                    name = c.name,
                    want = c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "W-27 contract drift:\n    {}",
        bad.join("\n    ")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
