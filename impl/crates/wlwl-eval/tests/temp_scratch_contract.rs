//! 契约测试的**临时目录**必须**构造上唯一** —— 守卫 `tests/common/mod.rs` 的取名。
//!
//! 背景(D13-078):macOS CI 随机红 `fs_contract::copy_and_move_are_for_files`
//! (pass 1 FAILED / pass 2 PASSED ⇒ flaky;ubuntu / windows 同轮**绿**)。
//! 归因链:
//!
//! 1. 旧写法用 `format!("wlwl_fs_io_{nanos}")` 取名,`nanos` **只**来自时钟
//!    ⇒ 名字是时钟的**纯函数**;
//! 2. `SystemTime::now()` 的**实际**分辨粒度远小于「纳秒」—— macOS 的
//!    `clock_gettime(CLOCK_REALTIME)` 是 **1 微秒**(`clock_getres` ⇒
//!    `tv_nsec = 1000`),Linux 是纳秒,Windows 约 100ns;
//! 3. 契约测试**并行**跑(libtest 并发度 = CPU 数)⇒ **头一波**几个线程几乎
//!    同时开始,取名落在**几微秒**内 ⇒ 撞名(实测 3 线程同刻取名:macOS 粒度
//!    **17.7%** 的轮次撞,Linux 粒度 **0%** —— 平台签名与 CI 现象吻合);
//! 4. 撞名 ⇒ 两个用例**共用**一个目录 ⇒ 后者起 scratch 时的 `remove_dir_all`
//!    抹掉前者刚写的文件 ⇒ 前者下一步 `COPY`/`MOVE` 找不到源文件 ⇒ 随机红。
//!
//! ⇒ 唯一性必须**不依赖时钟分辨率**。本册把那条不变量钉成三条:
//!
//! - **时钟冻住也必须不同**(`names_differ_even_when_the_clock_is_frozen`)——
//!   这一条是**根因的直译**:名字不能是时钟的纯函数。⚠ 它把时钟**当参数**传进去
//!   (`unique_name(prefix, nanos)`),所以这条断言在**任何平台都是确定的**——
//!   不像「并行跑 N 次看会不会撞」那种在粗时钟上才偶发、在细时钟上恒绿的写法。
//! - **并发取名两两不同**(`names_are_unique_under_concurrent_takes`)。
//! - **名字带进程 id 字段**(`names_carry_the_process_id`)⇒ 跨进程也隔离。
//!
//! ## 期望值不抄实现
//!
//! 「必须唯一」不是从实现里抄来的期望,而是**取名方案要满足的前提**:撞名的
//! 后果是「一个用例删掉另一个用例的目录」—— 这是**测试基础设施自己制造**的
//! 缺陷,不是被测代码的行为。

mod common;

use std::collections::HashSet;
use std::sync::{Arc, Barrier};

/// 并发取名的线程数。
///
/// ⚠ 取 **64** 而不是「几个」:64 个线程被同时放行,它们的取名调用挤在几微秒
/// 内 ⇒ 在任何**粗时钟**上都会撞名。线程数少的话,撞名只是「偶发」,守卫自己
/// 就成了另一个 flaky —— 守卫必须是**确定的**。
const THREADS: usize = 64;

/// 取一个临时目录,把**名字**那一段取出来(路径其余部分是共用的 temp 根)。
fn take_name() -> String {
    let dir = common::scratch("wlwl_probe");
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| panic!("scratch dir has no file name: {}", dir.display()));
    // 收尾:守卫自己不留垃圾。
    let _ = std::fs::remove_dir_all(&dir);
    name
}

/// **时钟冻住**时,两次取名也必须不同。
///
/// ⚠ 这一条是根因的直译:旧写法里名字是 `nanos` 的**纯函数** ⇒ 同一个 `nanos`
/// 必然给出同一个名字 ⇒ 并行线程只要落在同一个时钟格子里就会共用目录。
/// ⇒ 名字里必须带**与时钟无关**的成分(进程内序号)。
///
/// ⚠ 期望值不是「实现算出来什么就是什么」:这里断言的是一条**性质**
/// ——「两次取名不同」—— 它对任何满足该性质的实现都成立。
#[test]
fn names_differ_even_when_the_clock_is_frozen() {
    let a = common::unique_name("wlwl_probe", 42);
    let b = common::unique_name("wlwl_probe", 42);
    assert_ne!(
        a, b,
        "a temporary-dir name must not be a pure function of the clock.\n\
         Both calls were given the SAME timestamp (42) yet produced the same name \
         `{a}` — on macOS the wall clock only resolves to 1 microsecond, so \
         parallel test threads that land in the same microsecond share one \
         directory and delete each other's files (D13-078).\n\
         The name must carry something that is NOT the clock (a per-process \
         sequence number, a pid, …)."
    );
}

/// 同一进程内**并发**取名,名字必须两两不同。
///
/// ⚠ 缺陷形态(`format!("{prefix}_{nanos}")`)下这里会转红:64 个线程同刻取名,
/// 在 1 微秒的格子里只能挤出少数几个不同的名字。
#[test]
fn names_are_unique_under_concurrent_takes() {
    let barrier = Arc::new(Barrier::new(THREADS));
    let names: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                s.spawn(move || {
                    barrier.wait(); // 尽量让所有线程在同一时刻取名
                    take_name()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("scratch thread must not panic"))
            .collect()
    });

    assert_eq!(
        names.len(),
        THREADS,
        "the guard must actually take {THREADS} names"
    );
    let mut uniq: HashSet<&String> = HashSet::with_capacity(THREADS);
    let dups: Vec<&String> = names.iter().filter(|n| !uniq.insert(*n)).collect();
    assert!(
        dups.is_empty(),
        "temporary-dir names must be unique even when taken concurrently.\n\
         {THREADS} concurrent takes produced {} distinct names; duplicates: {dups:?}.\n\
         A clock-derived name is not a uniqueness guarantee: on macOS \
         clock_gettime(CLOCK_REALTIME) resolves to 1 microsecond, so parallel \
         test threads collide. Two cases sharing one directory delete each \
         other's files via remove_dir_all -> random failures (D13-078).",
        uniq.len()
    );
}

/// 名字必须带**进程 id 字段** ⇒ 两个进程不会共用一个目录。
///
/// ⚠ 这里钉的是「pid 作为一个**完整字段**出现」(`_<pid>_`)。数字字段内部不含
/// 下划线 ⇒ 命中即说明 pid 是独立的一段,而不是碰巧出现在纳秒串里。
#[test]
fn names_carry_the_process_id() {
    let pid = std::process::id().to_string();
    let name = {
        let dir = common::scratch("wlwl_probe");
        let n = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| panic!("scratch dir has no file name: {}", dir.display()));
        let _ = std::fs::remove_dir_all(&dir);
        n
    };
    assert!(
        name.contains(&format!("_{pid}_")),
        "a temporary-dir name must carry the process id as its own field, so two \
         processes never share one directory.\n\
         expected `_{pid}_` inside: {name}"
    );
}
