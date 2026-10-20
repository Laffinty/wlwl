//! 契约测试共用的**临时目录**工具。
//!
//! ⚠🔴 **[D13-078] 临时目录名不能只由时钟决定。**
//!
//! 旧写法是 `format!("wlwl_xxx_{nanos}")`(`nanos` 取自 `SystemTime::now()`)。
//! 那**不是**唯一性保证,只是「大概不撞」—— 而「大概」在 CI 上是**不够**的:
//!
//! - `SystemTime::now()` 的**实际**分辨粒度远小于它的单位。macOS 的
//!   `clock_gettime(CLOCK_REALTIME)` 是 **1 微秒**(`clock_getres` 给出
//!   `tv_nsec = 1000`);Linux 是纳秒,Windows 约 100ns(实测 1000 次采样只有
//!   355 个不同的值、645 次相邻重复)。
//! - 契约测试是**并行**跑的(libtest 默认并发度 = CPU 数)⇒ **头一波**有 2~3 个
//!   线程**几乎同时**开始 ⇒ 它们的第一次取名落在**几微秒**内。
//!
//! 实测(同刻取名,按平台粒度量化,每档 300 轮):
//!
//! | 线程数 | Windows ≈100ns | **macOS 1µs** | Linux 1ns |
//! |---|---|---|---|
//! | 3(= runner 核数,头一波) | 2 轮(0.7%) | **53 轮(17.7%)** | 0 轮 |
//! | 13(= `fs_contract.rs` 的 IO 用例数) | 38 轮(12.7%) | **230 轮(76.7%)** | 43 轮 |
//!
//! ⇒ **只有 macOS 会常撞** —— 这与「CI 上 macOS 红、ubuntu/windows 绿」完全吻合。
//!
//! 撞名的后果不是「目录里多几个文件」,而是**互相 `remove_dir_all`**:一个用例
//! 起 scratch 时会把**另一个正在跑**的用例的目录整个删掉 ⇒ 那个用例下一步的
//! `COPY`/`MOVE` 找不到源文件 ⇒ 随机红(实测复刻:同名 scratch 起来后,前一个用例
//! 刚写下的 `a.bin` **消失**)。CI 上的签名正是
//! `fs_contract::copy_and_move_are_for_files` 偶发失败。
//!
//! ⇒ 名字必须**构造上唯一**,不依赖时钟分辨率:**进程 id**(跨进程)+
//! **原子计数**(同进程内并发)+ **纳秒**(跨运行残留)。
//!
//! ⚠ 同一个坑本仓库在 CLI 侧**已经踩过一次并修过** ——
//! `wlwl-cli/tests/lexer_macro_coverage.rs` 里那段注释写得很直白:
//! 「临时目录**必须按名字分开**……共用一个目录时一边刚写下的 `main.wll`
//! 会被另一边读走」。那次只修了 CLI 侧,`wlwl-eval` 侧这 22 处是**残留副本**。

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// 进程内自增的取名序号。
///
/// ⚠ **这一根「保险丝」是唯一性的真正来源**:时钟粒度再粗(1 微秒)、再多线程
/// 同时取名,同一进程内的第 N 次与第 N+1 次也**必然**不同。
static SEQ: AtomicU64 = AtomicU64::new(0);

/// 当前的墙钟纳秒。
pub fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// 一个**构造上唯一**的名字。
///
/// ⚠ `nanos` 由调用方传入 —— 这样契约能**把时钟冻住**来验证「名字**不是**时钟的
/// 纯函数」这条不变量(见 `temp_scratch_contract.rs`)。若把时钟写死在这里,
/// 这条不变量就**没法验**了。
///
/// 名字形状:`<prefix>_<pid>_<seq>_<nanos>` —— 三个成分各管一段:
/// `pid` 管跨进程、`seq` 管同进程并发、`nanos` 管跨运行残留。
pub fn unique_name(prefix: &str, nanos: u128) -> String {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{}_{seq}_{nanos}", std::process::id())
}

/// 一个**构造上唯一**的临时目录(已创建好,可直接往里写文件)。
pub fn scratch(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(unique_name(prefix, now_nanos()));
    // 残留清理:同一 pid + 同一 seq 理论上不会重现,但 pid 会被复用 ⇒ 留着这一步
    // 才能保证「同名目录一定是空的」。
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}
