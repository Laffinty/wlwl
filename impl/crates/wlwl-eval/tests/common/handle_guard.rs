//! [项目扩展 / G-11] 「句柄泄漏」的**声明式**守卫夹具。
//!
//! ## 为什么要有它（缺口 G-11）
//!
//! 语言核心册 §19.4 / §19.5 要求「程序终止时全部资源必须被释放」，
//! 而它的**可测形式**是「**句柄数回到基线**」。
//! 上游的 `resource_release_contract.rs` 已经把这个判据做出来了
//! （Windows 走 `GetProcessHandleCount`，POSIX 走 `/proc/self/fd`），
//! 但**它内联在那个测试文件里** ⇒ 新增一种原生资源族时**只能把这一段抄一遍**
//! （本项目加 TCP socket 族时就是这么干的）。
//!
//! ⇒ 本模块把「**怎么数**」与「**串行闸**」抽出来，并给出一个**声明式**写法：
//! **「怎么建、怎么关」两个闭包**，其余交给夹具。
//!
//! ## 用法：新增一种原生资源族时的**全部**代码
//!
//! ```rust
//! common::handle_guard::assert_no_handle_leak(
//!     "socket family",
//!     || { /* 建：开 N 个 socket */ },
//!     || { /* 关：把它们关掉 */ },
//! );
//! ```
//!
//! ## ⚠️ 串行闸**不是**可选的洁癖
//!
//! 句柄计数是**进程级**的 ⇒ 同一条测试二进制里的用例**默认并行**跑 ⇒
//! 一条用例在数句柄时，另一条可能正在 spawn 子进程 ⇒ 差值里混进**别人的**句柄
//! ⇒ 偶发红。⇒ 本模块的闸是**进程内共享的一把锁**（`static Mutex`）
//! ⇒ 所有用它的人**自动**串行 ⇒ 不必每个用例各写一把。
//!
//! ## ⚠️⚠️ 但闸**只挡得住也用了闸的用例**
//!
//! 同一条测试二进制里，**没取闸**的用例照样能污染读数（它开着句柄时，
//! 本用例的基线就偏了 —— 哪怕那个用例后来把句柄关了，读数会**下降**，
//! 同样判红）。
//!
//! ⇒ **两条路二选一**：
//! ① **独占一个测试二进制**（本项目 socket 族走这条：`socket_handle_guard.rs`）；
//! ② 同文件里**所有会开句柄的用例都取同一把闸**（上游
//! `resource_release_contract.rs` 走这条：它文件里每条用例都取 `SERIAL`）。
//!
//! ⚠ **实测踩过**：socket 族的守卫最初放在 `tcp_contract.rs` 里 ⇒ 那条二进制
//!   还有 4 条用例会开监听器/连接 ⇒ 并行跑时**偶发红**（单独跑绿、全量跑红）
//!   ⇒ 挪到独立文件才稳。
//!
//! ## ⚠️ 测不出来 ≠ 没问题
//!
//! 平台不支持计数时,`open_handle_count()` 返回**哨兵** `usize::MAX`。
//!
//! ⚠ **历史缺陷(已修)**:哨兵原先只被「返回」,**没有任何一处检查它**
//!   ⇒ 基线与事后读数**都是**哨兵 ⇒ `baseline == after` **恒真**
//!   ⇒ 守卫在**什么都没测到**的情况下**报绿** —— 与本节原先声称的
//!   「任何断言都会变红」**正好相反**。
//! ⇒ 现在哨兵**一律判红**:`assert_no_handle_leak*` 在**开跑前的探针**、
//!   **基线**、**事后读数**三处都**显式**检查哨兵 ⇒ 「数不出来」
//!   ⇒ **红** ⇒ **绝不**可能因为「两边都是哨兵」而蒙混过关。
//!
//! ⚠ 判**红**而不是「响亮地跳过」:这条守卫的**全部价值**就在于
//!   「数不出来 ⇒ 资源释放**未被证明**」;跳过会让「未证明」长得像「已证明」。
//!   (反向守卫:计数不可用时**必须红** —— 哨兵 ≠ 通过。)

use std::sync::{Mutex, MutexGuard};

/// 计数**读不出来**时的哨兵。
///
/// ⚠ 它是 [`open_handle_count`] 的**返回契约**的一部分（`resource_release_contract.rs`
///   也按这个值判「数不出来」）⇒ **不得**改成别的形状。
pub const UNREADABLE: usize = usize::MAX;

/// 串行闸：**进程内共享**（`Mutex::new` 是 `const` ⇒ 可以做 `static`）。
///
/// ⚠ `lock()` 的 `Err`（中毒）被**吞掉** —— 中毒只说明**上一条**用例炸了，
/// 不该让本条也炸成「因为上一条炸了」。
pub static SERIAL: Mutex<()> = Mutex::new(());

/// 拿串行闸的锁。
pub fn serial_gate() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// 当前进程打开的句柄数。
///
/// * POSIX：`/proc/self/fd`（Linux）/ `/dev/fd`（macOS）—— 纯标准库。
/// * Windows：kernel32 的 `GetProcessHandleCount` —— **原始 FFI，不引依赖**
///   （本仓的依赖表里没有 windows-sys；加一个只为了数句柄不值得）。
///
/// ⚠ 失败时返回 `usize::MAX`（哨兵 ＝ **数不出来**）⇒ **调用方必须自己判它**：
///   [`assert_no_handle_leak`] 判它 ⇒ 守卫**变红**；`resource_release_contract.rs`
///   也判它（`handle_delta` 返 `i64::MAX`）。
/// ⚠ **不要**把哨兵当成一个普通计数去比较 —— 两边都是哨兵时「相等」是**恒真**
///   ⇒ 那正是本模块修掉的那个假绿。
pub fn open_handle_count() -> usize {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetProcessHandleCount(process: *mut core::ffi::c_void, count: *mut u32) -> i32;
            fn GetCurrentProcess() -> *mut core::ffi::c_void;
        }
        let mut count: u32 = 0;
        // SAFETY: 两个函数都是 kernel32 的 Win32 API，签名与调用约定正确；
        // `count` 指向一个合法的 u32。
        let ok = unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) };
        if ok == 0 {
            return usize::MAX;
        }
        count as usize
    }
    #[cfg(unix)]
    {
        // Linux 有 /proc/self/fd；macOS 是 /dev/fd。⇒ 两个都试，先到先算。
        for dir in ["/proc/self/fd", "/dev/fd"] {
            if let Ok(rd) = std::fs::read_dir(dir) {
                return rd.count();
            }
        }
        usize::MAX
    }
}

/// 读数必须**不是哨兵** —— 哨兵意味着「数不出来」⇒ 这条守卫**判不了**
/// ⇒ **判红**（哨兵 ≠ 通过）。
///
/// ⚠ 为什么单独抽一个函数：三处检查（开跑前的探针 / 基线 / 事后读数）必须
///   **同一口径** ⇒ 集中在一处写 ⇒ 免得后加的那处忘了判 ⇒ 又变成假绿。
///
/// ⚠ 判**红**而不是「跳过」：跳过 ⇒ 「未证明」看起来像「已证明」⇒
///   那正是本模块修掉的那个假绿（只是换了个方向）。
fn require_countable(what: &str, phase: &str, count: usize) -> usize {
    assert!(
        count != UNREADABLE,
        "{what}: {phase}读不到打开的句柄数（计数返回哨兵 usize::MAX）⇒ \
         这条守卫**测不了** ⇒ 判红（哨兵 ≠ 通过；不是「静默通过」）"
    );
    count
}

/// 句柄数**落定**（连续两次读数相同）—— 用来取一个可信的基线。
///
/// ⚠ 为什么需要：原生资源的释放**常常是异步的**（例如本项目的 socket 族：
///   `TCP_CLOSE_LISTENER` 只是**唤醒** acceptor 线程，socket 句柄由那个线程
///   drop 掉 ⇒ 关闭调用**返回时**句柄还在）⇒ 立刻取基线会把「还没落定的
///   异步释放」算进基线 ⇒ 后面的判据就永远对不上。
/// ⇒ 这里等它**不再变化**为止（最多 `timeout_ms`）。
///
/// ⚠ **不**在这里过滤哨兵：哨兵是「数不出来」这一**事实** ⇒ 交由调用方
///   （[`require_countable`]）判红 —— 免得「取不到就返回空」把「测不了」
///   变成静默漂移。
fn settled_handle_count(timeout_ms: u64) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let mut last = open_handle_count();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(25));
        let now = open_handle_count();
        if now == last || std::time::Instant::now() >= deadline {
            return now;
        }
        last = now;
    }
}

/// 等句柄数**回到** `target`（最多 `timeout_ms`）⇒ 返回最后的读数。
///
/// ⚠ 同上：**不**在这里过滤哨兵 —— 「数不出来」由调用方判红。
/// ⚠ 若 `target` 本身是哨兵，本函数会**立刻**返回哨兵（`now == target`）
///   ⇒ 调用方**必须**在比较之前判哨兵，否则就是那个假绿。
fn wait_for_handle_count(target: usize, timeout_ms: u64) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let now = open_handle_count();
        if now == target || std::time::Instant::now() >= deadline {
            return now;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// **声明式**的句柄泄漏守卫：`build` 建资源并返回它的**令牌**、`release` 消费令牌
/// 把资源关掉，句柄数必须**回到基线**。
///
/// ⚠ 基线在 `build` **之前**取 ⇒ 判的是「**建了再关 ⇒ 回到原样**」，
/// 不是「比某次运行少」⇒ 别的用例的噪声被基线吸收。
/// ⚠ `release` 跑完后**立刻**再数一次 ⇒ 中间不做别的 ⇒ 差值只可能来自
///   这一族资源 ⇒ 泄漏一条就是多一个句柄 ⇒ 红。
///
/// ⚠ **为什么令牌走返回值而不是闭包共享**：两个闭包若捕获同一个 `&mut`，
///   借检查器会直接拒（同时持有两个可变借用）⇒ 让 `build` **返回**令牌、
///   `release` **消费**它 ⇒ 调用点写起来是直线，状态无处可藏。
///
/// ⚠ **三步走**（都是为了对付「原生资源的释放常常是异步的」）：
/// ① **热身**一轮（建+关）⇒ 把懒初始化的一次性句柄排除在基线之外；
/// ② 等句柄数**落定**（不再变化）⇒ 记为**基线**；
/// ③ 再建一轮、关掉 ⇒ **轮询**等它回到基线 ⇒ 回不去就是泄漏 ⇒ 红。
///
/// ⚠ **三处哨兵检查**（哨兵 ＝ **数不出来** ⇒ 这条守卫**判不了** ⇒ **判红**）：
///   ⓿ 开跑前的**探针**（数不出来就别建资源）；
///   ① **基线**（基线是哨兵 ⇒ 后面每一次比较都是「哨兵 == 哨兵」⇒ 恒真）；
///   ② **事后读数**（中途数不出来 ⇒ 「回没回到基线」这个问题**没有答案**）。
/// ⇒ 三处都判 ⇒ 这条守卫**绝不可能**在「什么都没测到」的情况下报绿。
///
/// ```rust
/// common::handle_guard::assert_no_handle_leak(
///     "socket family",
///     || { let id = open_it(); id },   // 建
///     |id| close_it(id),               // 关
/// );
/// ```
pub fn assert_no_handle_leak<T, F1, F2>(what: &str, build: F1, release: F2)
where
    F1: FnMut() -> T,
    F2: FnMut(T),
{
    assert_no_handle_leak_within(what, build, release, 2_000)
}

/// [`assert_no_handle_leak`] 的显式超时版本（毫秒）。
pub fn assert_no_handle_leak_within<T, F1, F2>(what: &str, build: F1, release: F2, timeout_ms: u64)
where
    F1: FnMut() -> T,
    F2: FnMut(T),
{
    // ⚠ 串行闸**先**取：句柄计数是**进程级**的 ⇒ 闸外读会混进别的用例的句柄。
    let _gate = serial_gate();
    // ⚠ 热身要跑两轮 ⇒ 闭包是 `FnMut`（不是 `FnOnce`）⇒ 这里要 `mut` 绑定。
    let mut build = build;
    let mut release = release;
    // ⓿ 探针：这个平台**数得出**句柄数吗？数不出 ⇒ 后面每一次比较都是
    //   「哨兵 == 哨兵」⇒ 恒真 ⇒ **假绿** ⇒ 在这里就**判红**。
    require_countable(what, "开跑前的探针", open_handle_count());
    // ① 热身：懒初始化的一次性句柄不计入基线。
    {
        let token = build();
        release(token);
    }
    // ② 基线：等句柄数**落定**（异步释放要先收干净）。
    let baseline = require_countable(what, "基线", settled_handle_count(timeout_ms));
    // ③ 正式一轮：建 → 关 → 等它回到基线。
    let token = build();
    release(token);
    let after = require_countable(what, "事后", wait_for_handle_count(baseline, timeout_ms));
    assert!(
        baseline == after,
        "{what}: 句柄数没有回到基线（baseline={baseline} after={after}）⇒ 资源泄漏",
    );
}

/// 便捷形态：只有一段「建+用+关」的闭包。
///
/// ⚠ 与 [`assert_no_handle_leak`] 的判据**同一条**（回到基线）⇒ 语义等价，
///   只是写法更短（资源令牌不需要跨闭包传）。
pub fn assert_no_handle_leak_around<F>(what: &str, body: F)
where
    F: FnMut(),
{
    let mut body = body;
    assert_no_handle_leak(what, || (), |()| body());
}
