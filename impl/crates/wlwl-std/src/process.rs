//! [v0.11.3 M6 / addendum-06] `wlwl:std.process` —— **不经 shell** 跑一个子进程。
//!
//! **R2 全员**。
//!
//! ## 为什么本成员**只**收 `ARRAY[STRING]`(业主 2026-10-08 裁决)
//!
//! `argv` **必须**是数组。拼一个字符串再让系统解析 = **命令注入**:
//! `PROCESS_RUN(["sh", "-c", "echo " + user])` 里 `user` 若是 `; rm -rf /`,
//! 它就是一条真的会被执行的命令。⇒ 本成员**没有** `SYSTEM` / `exec("sh -c")`
//! 这种形态,规范也**显式禁止**字符串实参 —— 这条是本成员存在的**安全理由**。
//!
//! ## ⚠️ 它在协作式调度下是**阻塞**的(与 `SLEEP` 同源)
//!
//! wlwl 的调度是**单线程协作式**(`addendum-01` §3.11 核过:eval 侧无
//! `thread::spawn` / rayon,调度是一个 run queue)⇒ 本成员**等子进程期间不调度
//! 任何其他任务**,等价于把整个作用域停住。
//! ⇒ 与 `std.time::SLEEP` 的阻塞代价**同源**,规范与 skill 都要写明。
//! 这也是 `addendum-05`(`std.net`)的同源隐忧:`reqwest` 阻塞 → 同样停摆气泡。
//!
//! ## 超时怎么实现而**不开线程**
//!
//! `opts.timeout` 需要「到点杀掉子进程」。朴素做法是「先读完管道再等」——
//! 子进程写满管道缓冲就**死锁**。本实现起**两个读取线程**把 stdout / stderr
//! 抽干,主线程轮询 `try_wait()`。⚠️ 这两个线程**只做 I/O**,调度器仍是
//! 单线程(`ADR-0016` 边界未动)—— 与「不做 `std.thread`」那条非目标
//! **不冲突**:那条禁的是**把线程暴露给用户**。

use crate::{ModuleSpec, StdFn};
use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.process",
    functions: &[
        ("PROCESS_RUN", process_run as StdFn),
        // [B-5] **异步**版：子进程跑在**工作线程**上，等待期间**当前任务被
        // 挂起** ⇒ 别的任务照常跑 ⇒ 出站 I/O **不冻结整个 `SCOPE`**。
        // ⚠ `PROCESS_RUN` **仍然阻塞**（既有成员的语义一个字都不改）。
        ("PROCESS_RUN_ASYNC", process_run_async as StdFn),
        ("PROCESS_ID", process_id as StdFn),
    ],
};

/// 轮询间隔:5 ms。太大则超时的精度差,太小则空转。
const POLL: Duration = Duration::from_millis(5);

/// 读取线程的**有界**等待上限,**正常路径与超时路径共用**。
///
/// ⚠ 超时路径上这个上界**不是兜底而是命脉**(`[N-011 / D13-061]`,见 `drain`):
/// 超时被杀的子进程留下的孙进程握着管道写端 ⇒ 管道不 EOF ⇒ 没有这个上界就会
/// 无限期等下去。
const JOIN_GRACE: Duration = Duration::from_millis(500);

/// 组装 `PROCESS_RUN` 要跑的那条 `Command`。
///
/// ⚠⚠ **为什么必须是独立函数、不能就地搭**:`std::process::Command`
/// **没有实现 `Clone`**(它带 pre-exec 闭包,克隆不了)⇒ 「带进程组起一次、
/// 失败再不带组起一次」这个**降级路径必须能重新造一个 `Command`**。
///
/// ⚠⚠ **这不是理论顾虑**:`D13-068` 第一版写的是 `cmd.clone()` ⇒ **在 POSIX 上
/// 根本编译不过**(`no method named 'clone' found for &mut std::process::Command`),
/// 而那段在 `#[cfg(unix)]` 里 ⇒ **Windows 的 clippy 永远看不见它**
/// —— 只能等 CI 的 ubuntu / macOS 才炸出来(2026-10-19 run 240 实测如此)。
/// ⇒ **凡是只编在某个平台上的代码,「本机门禁全绿」对它一句话都不算**。
fn build_command(
    argv: &[String],
    cwd: Option<&std::path::Path>,
    extra_env: &[(String, String)],
) -> Command {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd
}

/// 起子进程,并返回**我们自己创建的进程组 id**;拿不到就是 `None`。
///
/// 🔴 [N-037 项三 / D13-068] **契约**:返回 `Some(pgid)` **当且仅当**「这个组
/// 是本次运行自己建的、且子进程已经进组了」。`kill_tree` 全靠这条前提才敢按组
/// 杀 —— 返回 `None` 时它**必须**退回 `child.kill()`。
#[cfg(unix)]
fn spawn_owned_group(
    argv: &[String],
    cwd: Option<&std::path::Path>,
    extra_env: &[(String, String)],
) -> io::Result<(std::process::Child, Option<i32>)> {
    use std::os::unix::process::CommandExt;
    // ⚠ `process_group(0)` 的语义就是 `setpgid(0, 0)`:把子进程变成**新进程组的
    //   组长**。它 1.64 起 stable、**纯标准库** —— 设组这一步不需要 libc。
    let mut grouped = build_command(argv, cwd, extra_env);
    grouped.process_group(0);
    match grouped.spawn() {
        Ok(c) => {
            // ⚠ 组 id **就是子进程 pid**(`setpgid(0,0)` 的定义)⇒ 必须**当场**取。
            //   子进程一退出,组就可能消失,事后再问谁也答不出来。
            // ⚠ `try_from` 而不是 `as`:万一 pid 超出 `i32`(现实中不可能)就得到
            //   `None` ⇒ 降级成「只杀直接子进程」,而不是去杀一个可疑的组。
            // ⚠⚠ **必须先把 pid 取出来再构造元组**:写成
            //   `Ok((c, i32::try_from(c.id()).ok()))` 会**先移走 `c` 再借它**
            //   ⇒ `E0382 borrow of moved value`。⚠ 这条**本机 Windows 看不见**
            //   (整段在 `#[cfg(unix)]` 里),是 2026-10-19 的 unix 形状检查抓出来的
            //   —— 同一段里还有一个 `cmd.clone()` 的编译错误,两者都只有 CI 能炸。
            let pgid = i32::try_from(c.id()).ok();
            Ok((c, pgid))
        }
        Err(_) => {
            // ⚠ **降级而不是让 `PROCESS_RUN` 失败**(红线:受限环境里
            //   `PROCESS_RUN` 不该因为一个**优化**而拒绝干活)。seccomp / 沙箱 /
            //   某些容器配置会让 fork 之后的 `setpgid` 失败,cargo 把它**一并报成
            //   spawn 失败** —— 于是这里**重新造一条** `Command`(不带进程组)再起
            //   一次,并如实返回 `None`。
            //
            // ⚠⚠ **重试不会让目标程序跑两遍**:`spawn()` 返回 `Err` 意味着 fork
            //   失败、或 fork 成功了但 exec / pre-exec 的 `setpgid` 失败 ⇒ 那次
            //   fork 出来的子进程**没有执行目标程序**。成功的那一次只会返回 `Ok`。
            build_command(argv, cwd, extra_env)
                .spawn()
                .map(|c| (c, None))
        }
    }
}

/// Windows 上**没有**进程组 ⇒ 一律 `None`。
///
/// ⚠ Windows 的等价机制是 Job Object,而 `taskkill /T /F` 已经按**进程树**解决
///   了同一件事(见 `kill_tree`)⇒ 这里不需要、也没有引入任何 Windows 侧机制。
///   返回 `None` ⇒ `kill_tree` 在 Windows 上**逐字保持原样**。
#[cfg(windows)]
fn spawn_owned_group(
    argv: &[String],
    cwd: Option<&std::path::Path>,
    extra_env: &[(String, String)],
) -> io::Result<(std::process::Child, Option<i32>)> {
    build_command(argv, cwd, extra_env)
        .spawn()
        .map(|c| (c, None))
}

/// 超时要杀的**进程组** id —— `None` 表示「没有可按组杀的组」。
///
/// 🔴 [N-037 项三 / D13-068] **红线函数**:它单独决定「要不要按组杀」。打错组
/// 的代价是**杀掉不属于本次运行的进程** —— 那比多留一个孤儿坏得多。⇒ 三个条件
/// 缺一不可,任何一条不满足都退回 `child.kill()`(只杀直接子进程,即修复前的
/// 行为):
///
/// 1. **`Some(pgid)`** —— 必须是 spawn 时**我们自己**记下的那个组。没拿到组信息
///    就绝不走按组杀:POSIX 里 `killpg(0, s)` 的语义是「调用者自己的组」⇒ 一旦
///    传了 `0`,整台机器上**同组的进程**(包括整个 wlwl 运行时)一起死。
/// 2. **`pgid > 1`** —— `1` 是 init 的组,不是我们的。
/// 3. **`pgid != own_pgrp`** —— ⚠⚠ **最后一道闸**:即使 `process_group` 声称
///    生效,也再核对一次「这个组不是我自己所在的组」。设错一次就是杀掉整个运行时。
///
/// ⚠ 做成**纯函数**(把 `own_pgrp` 当参数)是刻意的:按组杀那条路径只在 POSIX 上
///   存在,而这个判定是本条最该被守住的逻辑 ⇒ 做成纯函数就能**在任何平台上被单测
///   钉住**(`#[cfg(any(unix, test))]`,见 `target_group` 的单测)。
#[cfg(any(unix, test))]
fn target_group(owned_group: Option<i32>, own_pgrp: i32) -> Option<i32> {
    match owned_group {
        Some(pgid) if pgid > 1 && pgid != own_pgrp => Some(pgid),
        _ => None,
    }
}

/// 本进程所在的进程组 id。
#[cfg(unix)]
fn own_pgrp() -> i32 {
    // ⚠ `getpgrp()` 没有失败语义(它不设 errno、也不返回 -1)⇒ 没有错误分支。
    //
    // ⚠⚠ **这里不能写 `i32::from(unsafe { libc::getpgrp() })`**:`getpgrp()` 的返回
    //   类型 `libc::pid_t` 在所有支持的 POSIX 目标上**就是 `i32`** ⇒ 那层转换是
    //   **同类型转换**,被 `clippy::useless_conversion` 判红(2026-10-19 run 241
    //   实测:ubuntu 与 macOS 双双在 clippy 步骤红,Windows 绿)。
    //   ⚠ **「收敛成 `i32`」这条契约改由函数签名保证**:哪天 `pid_t` 不是 `i32`
    //   了,这一行**直接编译不过** —— 那正是我们想要的(显式失败,而不是静默截断)。
    //   ⚠ 这条同形问题**本机查不出来**:`#[cfg(unix)]` 的代码在 Windows 的门禁里
    //   **根本不存在**,而裸 `rustc` 又查不出 lint ⇒ 必须用 clippy 跑形状检查。
    unsafe { libc::getpgrp() }
}

/// 杀掉一个**进程组**(`killpg`,即 `kill(-pgid, SIGKILL)`)。
///
/// ⚠ 标准库**没有**这个能力 —— `Child::kill()` 在 POSIX 上只给直接子进程发信号,
///   这正是 `[N-037 项三]` 要修的东西。⇒ 这一处是全仓唯一用到 libc 的地方。
#[cfg(unix)]
fn kill_group(pgid: i32) {
    // ⚠ 失败(组已空 / 权限不足)时返回 -1,这里**刻意丢弃**:此时仍要继续走
    //   `kill_tree` 里那句 `child.kill()` —— 那才是**保证**生效的那一次。
    // ⚠ `SIGKILL` 与 `child.kill()` 在 POSIX 上同义(后者内部也是发 `SIGKILL`),
    //   所以按组杀**不会**改变「超时是硬杀、不是协商退出」这个既有语义。
    let _ = unsafe { libc::killpg(pgid, libc::SIGKILL) };
}

/// 杀掉子进程**及其整棵后代**。
///
/// ⚠️ 这不是洁癖 —— `child.kill()` 在 Windows 上**只杀直接子进程**:
/// `cmd /C ping` 里 `cmd` 被杀了而 `ping` 还活着,而 `ping` **继承了管道句柄**
/// ⇒ 读线程等不到 EOF。实测:300 ms 的超时因此走了 **29.26 s**。
/// 「杀掉这个子进程」在语义上就该含它的后代,所以走 `taskkill /T /F`。
///
/// 🔴 [N-037 项三 / D13-068] **POSIX 侧此前完全没有这一层**:`taskkill` 是
/// Windows 专属,而 POSIX 上只剩 `child.kill()` —— 它只给**直接子进程**发信号。
/// `sh -c 'sleep 300'` 里 `sh` 死了、`sleep` **还活着**,并继续握着继承来的
/// stdout / stderr **写端**。
/// ⚠ `[N-011 / D13-061]` 把 `drain` 改成有界之后,「**我们的线程挂死**」不再
///   发生,但**孤儿进程本身还在跑**:它一直占 CPU 到自己结束,在 wlwl 退出之后
///   也照样占着那些管道。⇒ 这一层补在**各平台自己的原生机制**上:
///
/// | | 设组 | 杀整组 |
/// |---|---|---|
/// | Windows | 不适用(等价物是 Job Object,此处用不上) | `taskkill /T /F`(**逐字不变**) |
/// | POSIX | `CommandExt::process_group(0)`,**纯标准库** | `killpg`(要 libc),标准库没有 |
///
/// ⚠ **只有 `target_group` 判定出 `Some` 才按组杀**,拿不到组信息一律退回
///   `child.kill()` —— 打错进程组比留一个孤儿坏得多(红线)。
fn kill_tree(child: &mut std::process::Child, owned_group: Option<i32>) {
    if cfg!(windows) {
        let pid = child.id();
        // `taskkill` 失败也不该把调用卡住 ⇒ 立刻放弃,退回 `child.kill()`。
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    if let Some(pgid) = target_group(owned_group, own_pgrp()) {
        kill_group(pgid);
    }
    // ⚠ Windows 没有进程组 ⇒ 这个参数在这条分支上只用于让两平台共享签名。
    #[cfg(not(unix))]
    let _ = owned_group;
    // ⚠ 无条件、且**在**按组杀之后:它是「无论如何都杀掉直接子进程」的那道底线。
    let _ = child.kill();
}

/// 收一个读取线程的输出,**两条路径走同一条有界逻辑**。
///
/// ⚠ **必须有界**:即便杀了进程树,一个「不听话」的**孙进程**仍可能握着管道
/// 写端,无条件 `join()` 会把「超时返回」变成「等到孙进程自己结束」
/// —— 那超时就没了。
/// ⇒ 轮询 `is_finished()` 到 `JOIN_GRACE` 为止;仍未结束就**丢弃句柄**
/// (`JoinHandle` drop = 分离线程),让它自生自灭。已读到的部分拿不到就返回空
/// —— 超时路径上 stdout / stderr 本来就不给调用方。
///
/// ⚠🔴 **[N-011 / D13-061] 原实现把超时路径排除在这个上界之外。**
/// 形参 `already_timed_out` 当时只控制「**要不要轮询**」,于是
/// `already_timed_out == true` 时**跳过整个轮询、直接 `join()`**
/// —— 而超时路径恰恰是**最可能挂住**的那条:`kill_tree` 只保证子进程
/// (Windows 上连整棵进程树)被杀掉,**管不到「继承了管道写端、又活着的
/// 孙进程」**。⚠ **POSIX** 上 `child.kill()` 只发信号给直接子进程 ⇒ `sh` 死了
/// 而它的孙进程还握着 stdout / stderr 写端 ⇒ 管道**永不 EOF** ⇒ `join()`
/// **无限期**等下去 ⇒ 「超时返回」退化成「等到孙进程自己结束」。
/// ⚠ Windows 上这条路被 `taskkill /T /F` 掩盖了(它杀整棵进程树),所以**同一个
/// 缺陷在 Windows 上跑不出来**,契约在 Windows 上恒绿。
/// ⇒ 形参**已删除**:它曾经描述的意图(两条路径不同)恰恰就是缺陷本身,而两条
/// 路径真正需要的性质(有界)是**同一条**。
///
/// ⚠⚠ **按组杀不能撤掉这个上界**(`[N-037 项三 / D13-068]`):`kill_tree` 现在在
/// POSIX 上会 `killpg` 整个组,但那只覆盖「**还在这个组里**的进程」。一个
/// 主动 `setsid()`、或者在**开了 job control** 的 shell(`sh -m`)里起的后台
/// 孙进程**会逃出这个组** ⇒ 它照样能握着管道写端不放。
/// ⇒ 两条防线是**互补**的:`killpg` 负责「别留孤儿」,这里的**有界等待**负责
/// 「就算有进程逃出去了也别把我们自己拖死」。
fn drain(h: std::thread::JoinHandle<Vec<u8>>) -> Vec<u8> {
    let deadline = Instant::now() + JOIN_GRACE;
    while !h.is_finished() && Instant::now() < deadline {
        std::thread::sleep(POLL);
    }
    if h.is_finished() {
        // 已结束 ⇒ `join()` 立即返回,拿得到完整缓冲。
        h.join().unwrap_or_default()
    } else {
        // ⚠ **关键**:这里**必须**返回而不是 `join()` —— 句柄随作用域结束被丢弃
        //   = 分离线程,让它自生自灭(见本函数注)。
        Vec::new()
    }
}

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, want: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

fn ok(v: Value) -> Result<Outcome, WlwlError> {
    Ok(Outcome::normal(v))
}

/// 进程失败 → `ERR([kind:"ProcessError", op, reason])`**值**。
///
/// ⚠ `pub(crate)`：[B-5] 的异步变体在**另一个模块**里把这层形状复用过去
///   （`wlwl_std::net::async_op` 的产物 → 值）。
pub(crate) fn proc_err(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("ProcessError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

/// 一次子进程运行的**产物** —— **纯数据**（`Send`）。
///
/// ⚠ **为什么不是 `Value`**：`Value` 的字典载荷是 `Rc<DictInner>` ⇒
///   **`Value` 不是 `Send`** ⇒ [B-5] 的异步变体**不能**把结果从**工作线程**
///   搬回**求值线程** ⇒ 线程之间只搬**纯数据**，`Value` 在求值线程上现搭
///   （见 [`process_outcome_value`]）。
#[derive(Debug, Clone)]
pub(crate) struct ProcessOutcome {
    pub code: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// 把 [`ProcessOutcome`] 搭成**成员返回值**（`DICT {code, stdout, stderr}`）。
pub(crate) fn process_outcome_value(o: ProcessOutcome) -> Value {
    Value::dict(vec![
        (Value::String("code".into()), Value::Integer(o.code)),
        (
            Value::String("stdout".into()),
            // 🔴 [D13-064 · N-037 项一]这一行**是有损替换**,而成员文档与规范
            // §19 **从未写过** ⇒ 调用方会以为 stdout 是**原始字节**。
            // ⚠ 它与全局「不做有损替换」条款(§2.3 / §11.3-3)**冲突**:非 UTF-8
            // 字节在这里被换成 `U+FFFD`,**不可逆**。
            //
            // ⚠ **为什么不改成 `ARRAY[INTEGER]` 并集**(像 `HTTP_*` 的 body 那样,
            //   见 `D13-043`):成员表把 `stdout` / `stderr` 定为 `STRING`,
            //   改类型是**破坏性变更**,且不在本批范围。⇒ 本批**如实成文**,
            //   变更路径登记待独立立项。
            // ⚠ **要拿字节就用文件**:`WRITE_BYTES` + `READ_BYTES` 是无损的那条路。
            Value::String(String::from_utf8_lossy(&o.stdout).into_owned()),
        ),
        (
            Value::String("stderr".into()),
            Value::String(String::from_utf8_lossy(&o.stderr).into_owned()),
        ),
    ])
}

/// `PROCESS_RUN(argv, opts?) -> DICT`
///
/// 返 `{code, stdout, stderr}`。`code` 是退出码;**信号杀死**的平台差异用
/// `code = -1` 表达(`kill` 之后拿不到真实码),并在 `stderr` 留痕。
///
/// `opts` 可含:
/// - `timeout` `INTEGER` 毫秒 ⇒ 超时**杀掉**子进程并返回 `ERR`(`kind` =
///   `ProcessError`),**不挂死**;
/// - `cwd` `STRING` 工作目录;
/// - `env` `DICT` **追加**到继承来的环境(同名键覆盖)—— 不是「替换整份环境」。
pub fn process_run(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PROCESS_RUN";
    let a = parse_proc_args(host, NAME, &args)?;
    match run_process(&a.argv, a.timeout_ms, a.cwd.as_deref(), &a.extra_env) {
        Ok(o) => ok(process_outcome_value(o)),
        Err(reason) => ok(proc_err(NAME, reason)),
    }
}

/// `PROCESS_RUN` / `PROCESS_RUN_ASYNC` **共用**的实参解析结果。
struct ProcArgs {
    argv: Vec<String>,
    timeout_ms: Option<i64>,
    cwd: Option<String>,
    extra_env: Vec<(String, String)>,
}

/// 解析 `argv` 与 `opts` —— **两个成员共用** ⇒ 口径逐字一致。
///
/// ⚠️ 只收数组。**没有**字符串形态 —— 那是命令注入的入口(见文件头)。
fn parse_proc_args(
    host: &mut dyn StdHost,
    name: &str,
    args: &[Value],
) -> Result<ProcArgs, WlwlError> {
    if args.is_empty() || args.len() > 2 {
        return Err(arity(host, name, args.len(), 1));
    }
    let Value::Array(items) = &args[0] else {
        return Err(type_err(host, name, "an argv array of strings", &args[0]));
    };
    let mut argv: Vec<String> = Vec::with_capacity(items.len());
    for it in items {
        match it {
            Value::String(s) => argv.push(s.clone()),
            other => return Err(type_err(host, name, "an argv array of strings", other)),
        }
    }
    if argv.is_empty() {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{name}: argv must have at least one element (the program)"),
        ));
    }

    let mut timeout_ms: Option<i64> = None;
    let mut cwd: Option<String> = None;
    let mut extra_env: Vec<(String, String)> = Vec::new();
    match args.get(1) {
        None | Some(Value::Null) => {}
        Some(Value::Dict(entries)) => {
            for (k, v) in entries.iter() {
                let (Value::String(key), _) = (k, v) else {
                    return Err(type_err(host, name, "an opts dict with string keys", k));
                };
                match key.as_str() {
                    "timeout" => match v {
                        Value::Integer(ms) if *ms >= 0 => timeout_ms = Some(*ms),
                        other => {
                            return Err(type_err(
                                host,
                                name,
                                "opts.timeout to be a non-negative integer",
                                other,
                            ))
                        }
                    },
                    "cwd" => match v {
                        Value::String(s) => cwd = Some(s.clone()),
                        other => {
                            return Err(type_err(host, name, "opts.cwd to be a string", other))
                        }
                    },
                    "env" => match v {
                        Value::Dict(kvs) => {
                            for (ek, ev) in kvs.iter() {
                                match (ek, ev) {
                                    (Value::String(k), Value::String(val)) => {
                                        extra_env.push((k.clone(), val.clone()))
                                    }
                                    _ => {
                                        return Err(type_err(
                                            host,
                                            name,
                                            "opts.env to be a dict of strings",
                                            ek,
                                        ))
                                    }
                                }
                            }
                        }
                        other => return Err(type_err(host, name, "opts.env to be a dict", other)),
                    },
                    other => {
                        return Err(host.diag(
                            ErrorCode::E0030,
                            format!("{name}: unknown opts key `{other}` (timeout / cwd / env)"),
                        ))
                    }
                }
            }
        }
        Some(other) => return Err(type_err(host, name, "an opts dictionary", other)),
    }
    Ok(ProcArgs {
        argv,
        timeout_ms,
        cwd,
        extra_env,
    })
}

/// `PROCESS_RUN_ASYNC(argv, opts?) -> DICT`
///
/// **异步**版的 [`process_run`]：子进程在**工作线程**上跑，等待期间**当前任务
/// 被挂起**（不是阻塞整个运行时）⇒ **别的任务照常运行**。
///
/// - 返回值与 `PROCESS_RUN` **同形**：`DICT {code, stdout, stderr}`；
///   运行期失败 ⇒ `ERR([kind:"ProcessError", op, reason])`**值**（不是诊断）。
/// - `opts` 与 `PROCESS_RUN` **逐字同形**（`timeout` / `cwd` / `env`；
///   **未知键 ⇒ `E0030`**，不静默忽略 —— 拼错的上界会变成「没有上界」）。
/// - ⚠️ **只收数组形态的 `argv`**（同 `PROCESS_RUN`）：字符串形态是命令注入
///   的入口，本成员**没有**那种形态。
///
/// ## 与 `PROCESS_RUN` 的差别（**唯一**的一条）
///
/// `PROCESS_RUN` **阻塞调用线程** ⇒ 整个 `SCOPE` 停摆；本成员**挂起当前任务**
/// ⇒ 别的任务照常跑。**既有成员的语义一个字都不改**。
pub fn process_run_async(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PROCESS_RUN_ASYNC";
    let a = parse_proc_args(host, NAME, &args)?;
    // 幂等用的**实参指纹**（同一次调用重跑时逐字相同）。
    let signature = format!(
        "{NAME}|{:?}|{:?}|{:?}|{:?}",
        a.argv, a.timeout_ms, a.cwd, a.extra_env
    );
    crate::net::async_op::run(host, NAME, signature, move || crate::net::Payload::Proc {
        op: NAME,
        r: run_process(&a.argv, a.timeout_ms, a.cwd.as_deref(), &a.extra_env),
    })
}

/// 子进程运行的**核心**（**不碰宿主**）：起进程、抽干两个管道、等它结束。
///
/// ⚠ 返回**纯数据** / **失败原因**（`String`）—— **不是** `Value`：
///   `Value` 的字典载荷是 `Rc<DictInner>` ⇒ **`Value` 不是 `Send`**
///   ⇒ [B-5] 的异步变体**不能**把结果从**工作线程**搬回**求值线程**
///   ⇒ 线程之间只搬纯数据，`Value` 在求值线程上现搭
///   （见 [`process_outcome_value`]）。
fn run_process(
    argv: &[String],
    timeout_ms: Option<i64>,
    cwd: Option<&str>,
    extra_env: &[(String, String)],
) -> Result<ProcessOutcome, String> {
    // 🔴 [N-037 项三 / D13-068] 起子进程时**顺手记下自己建的进程组**。
    // ⚠ 记的是 `None` 的那一次 = 「没拿到组信息」⇒ `kill_tree` 只杀直接子进程。
    // ⚠ 正常路径**不**按组杀:一个程序合法地起后台进程然后自己退出,那是它的
    //   语义,不是泄漏(泄漏的定义是「我们超时杀了它」之后还留着)。
    let (mut child, owned_group) =
        match spawn_owned_group(argv, cwd.map(std::path::Path::new), extra_env) {
            Ok(pair) => pair,
            Err(e) => return Err(format!("spawn {}: {e}", argv[0])),
        };
    // 抽干两个管道,否则子进程写满缓冲就死锁(「先读后等」是错的)。
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms as u64));
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {}
            Err(e) => {
                // 🔴 [D13-064 · N-037 项二]此前这里有两处缺陷,`review-1`
                // **只报了第一处**:
                //
                // ① **不回收 wait 状态** —— `Ok(Some(s))` 那条会 `wait()` 到,
                //    这条**直接 return**,`Child` 被丢弃。⚠ `std::process::Child`
                //    的 `Drop` **不** wait ⇒ Unix 上留下**僵尸进程**直到本进程
                //    退出。⇒ 先 kill 再 wait,把它收干净。
                // ② ⚠⚠ **无界 `join()`** —— 与 `[N-011 / D13-061]` 是同一族:
                //    `try_wait` 报错时子进程**可能还活着**并握着管道写端 ⇒
                //    `out_handle.join()` 无限期等下去。**review-1 没提这一条。**
                //    ⇒ 走同一条**有界** drain(缓冲反正要丢弃,因为这里返 `ERR`)。
                //
                // ⚠⚠ [N-037 项三 / D13-068]这里**刻意仍**只 `child.kill()`,没走
                //   `kill_tree`:换成 `kill_tree` 会让 **Windows** 也多跑一次
                //   `taskkill /T /F` —— 而本批的红线是「Windows 侧逐字不变」,
                //   且这条臂在本仓**造不出来**(`try_wait` 返 `Err`)。⇒ POSIX 上
                //   这里仍留有同族的孙进程泄漏面,登记为已知边界,不偷跑。
                let _ = child.kill();
                let _ = child.wait();
                let _ = drain(out_handle);
                let _ = drain(err_handle);
                return Err(format!("wait {}: {e}", argv[0]));
            }
        }
        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                // ⚠⚠ 超时路径上子进程**一定还活着**(上一轮 `try_wait` 刚返回
                //   `Ok(None)`),而且**我们还没有 reap 过它** ⇒ 它的 pid 这期间
                //   **不可能被系统回收给别人**。这正是按组杀不会打到陌生进程组的
                //   前提之一(另一道是 `target_group`)。⚠ 子进程此刻退出也只会变成
                //   **僵尸**,僵尸仍然占着 pid。
                kill_tree(&mut child, owned_group);
                let _ = child.wait();
                timed_out = true;
                break None;
            }
        }
        std::thread::sleep(POLL);
    };

    // ⚠ 两条都用同一条有界 drain:超时路径**也**必须有界(见 `drain` 的
    //   `[N-011 / D13-061]` 注 —— 那里曾经把超时路径排除在上界之外)。
    let stdout = drain(out_handle);
    let stderr = drain(err_handle);
    if timed_out {
        let secs = timeout_ms.unwrap_or(0) as f64 / 1000.0;
        return Err(format!("timed out after {secs}s: {}", argv.join(" ")));
    }
    let code = status.and_then(|s| s.code()).unwrap_or(-1);
    Ok(ProcessOutcome {
        code: i64::from(code),
        stdout,
        stderr,
    })
}

/// `PROCESS_ID() -> INTEGER` —— 本进程 id。
pub fn process_id(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "PROCESS_ID";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), 0));
    }
    ok(Value::Integer(i64::from(std::process::id())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_value::StdCtx;

    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut StdCtx {
            panic!("std.process must not touch StdCtx")
        }
        fn call(&mut self, _f: &Value, _a: Vec<Value>, _n: &str) -> Result<Outcome, WlwlError> {
            panic!("std.process has no member that calls back into the host")
        }
        fn diag(&mut self, code: ErrorCode, message: String) -> WlwlError {
            wlwl_error::WlwlDiagnostic::new(
                code,
                message,
                wlwl_error::Location::point("<test>", 0, 0),
            )
            .into()
        }
    }

    fn argv_of(v: &str) -> Value {
        Value::Array(vec![Value::String(v.into())])
    }

    #[test]
    fn runs_a_real_command_and_captures_stdout() {
        let mut h = NullHost;
        // ⚠🔴 [D13-057]这里必须给**三个以上** argv 元素。
        //   POSIX 分支原先写 `["sh", "-c echo hello"]` —— 那是**一个**元素,
        //   `sh` 认不出 `-c`(它带空格和后缀),直接以退出码 **2** 失败。
        //   Windows 分支的 `["cmd", "/C echo hello"]` 同样是**一个**元素,只是
        //   `cmd /C <整串>` 恰好也能跑,所以那条分支一直是绿的 ——
        //   **两边的 argv 形状都写错了,只有 Windows 那边的错被容忍了**。
        let argv: Vec<&str> = if cfg!(windows) {
            vec!["cmd", "/C", "echo", "hello"]
        } else {
            vec!["sh", "-c", "echo hello"]
        };
        let out = process_run(
            &mut h,
            vec![
                Value::Array(argv.into_iter().map(|s| Value::String(s.into())).collect()),
                Value::dict(vec![]),
            ],
        )
        .expect("spawn");
        let Value::Dict(d) = out.value else { panic!() };
        let get = |k: &str| {
            d.iter()
                .find(|(key, _)| matches!(key, Value::String(s) if s == k))
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        let Value::Integer(code) = get("code") else {
            panic!()
        };
        assert_eq!(code, 0, "a successful command exits 0");
        let Value::String(stdout) = get("stdout") else {
            panic!()
        };
        assert!(stdout.contains("hello"), "stdout = {stdout:?}");
    }

    /// **本成员存在的安全理由**:元组里的元字符**不被**任何 shell 解释。
    /// 若实现哪天改成拼字符串,这条会红。
    #[test]
    fn argv_elements_are_never_shell_interpreted() {
        let mut h = NullHost;
        // ⚠⚠ [D13-057]POSIX 分支原先写 `["sh", "-c", "printf %s a; echo b"]` ——
        //   `a; echo b` 被放进了 **shell 脚本本体**,于是 `;` 是**真的**命令分隔符,
        //   输出 `a\nb`。⚠ **那条分支根本没在测这个性质**:被测的
        //   `a; echo b` 从来没有作为 **argv 元素**到达过。
        //   ⇒ 正确形状是把载荷放进 `$1`,让它**只展开一次**:
        //   `sh -c 'printf %s "$1"' sh <arg>`。
        //   (同一个错在 `process_contract.rs` 的同款用例里也有一份,
        //    见 `D13-055` / `D13-056` —— 两处是同一个错误的两个副本。)
        let (cmd, args): (&str, Vec<&str>) = if cfg!(windows) {
            ("cmd", vec!["/C", "echo", "a; echo b"])
        } else {
            ("sh", vec!["-c", r#"printf %s "$1""#, "sh", "a; echo b"])
        };
        let argv = Value::Array(
            std::iter::once(cmd.to_string())
                .chain(args.into_iter().map(String::from))
                .map(Value::String)
                .collect(),
        );
        let out = process_run(&mut h, vec![argv]).expect("spawn");
        let Value::Dict(d) = out.value else { panic!() };
        let stdout = d
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "stdout"))
            .map(|(_, v)| v.clone())
            .unwrap();
        let Value::String(s) = stdout else { panic!() };
        // 无论哪个平台,`a; echo b` 必须作为**单个参数**原样到达。
        assert!(s.contains("a; echo b"), "argv element was mangled: {s:?}");
    }

    /// `try_wait` 出错时必须**回收 wait 状态**,且读线程**有界**收尾。
    ///
    /// ⚠⚠ **这是「形状守卫」不是「行为守卫」**(`D13-064 / N-037` 项二):那条
    /// 分支**在测试里不可达** —— `try_wait` 返回 `Err` 需要平台级的非常规
    /// 状况,本仓**造不出来**。这正是它当初能长期存在的原因。
    /// ⇒ 能钉的只有**代码形状**。⚠ 守卫的固有上限已如实登记:有人把缺陷换个
    /// 写法重新引入(而不复现旧形状),它不会响。
    ///
    /// ⚠⚠ needle **必须运行时拼**。写成字面量的话,这个测试**自己**就含有那个
    /// 字符串 ⇒ `src.contains(..)` 命中测试自身 ⇒ 断言永假。
    /// (本条守卫在成型过程中已经踩了两次:① 自引用;② 固定字节数切片切在
    /// UTF-8 字符中间 panic。**形状守卫比它守的缺陷还容易坏** —— 这本身就是
    /// 「用简单断言」的理由。)
    #[test]
    fn the_wait_error_branch_reaps_and_is_bounded() {
        let src = include_str!("process.rs");
        for handle in ["out", "err"] {
            let old = format!("let _ = {handle}_handle.join();");
            assert!(
                !src.contains(&old),
                "`{old}` is the defect shape D13-064 removed: on the `try_wait` error \
                 arm the child may still be alive holding the pipe write end, so an \
                 unbounded join turns a rare error into a hang (same class as D13-061)"
            );
        }
        // `child.wait()` 必须出现在该分支里。⚠ 同样运行时拼,理由同上。
        let reap = format!("let _ = child.{}();", "wait");
        let arm_anchor = format!("Err(e) => {{{}", ""); // 仅用于把范围收窄到那条臂
        let start = src
            .find(&arm_anchor)
            .expect("the try_wait error arm exists")
            + arm_anchor.len();
        let arm = &src[start..];
        let end = arm
            .find("return ok(proc_err(")
            .expect("the arm's return statement exists");
        assert!(
            arm[..end].contains(&reap),
            "the `try_wait` error arm must reap the wait status — `Child`'s Drop does \
             NOT wait, so skipping it leaks a zombie on Unix (D13-064)"
        );
    }

    /// 🔴 **[N-037 项三 / D13-068] 头号红线:绝不误杀不属于本次运行的进程组。**
    ///
    /// 「按组杀」这条路径只在 POSIX 上存在,而本机是 Windows ⇒ 把「要不要按组杀」
    /// 抽成**纯函数** `target_group(owned_group, own_pgrp)`,于是这条守卫
    /// **在两个平台上都跑得到**,而且它守的正是红线本身,不是「某个字符串还在」。
    ///
    /// 逐条对应 `target_group` 的三个条件:
    /// - `None` ⇒ 我们**没拿到**组信息 ⇒ 一律不按组杀(`killpg(0, s)` 在 POSIX 里
    ///   是「调用者自己的组」,传错一次就是**整个运行时陪葬**);
    /// - `pgid == own_pgrp` ⇒ 这个组是**我们自己**所在的 ⇒ 万一设组失败而子进程
    ///   仍留在本进程组里,这条挡住我们自杀;
    /// - `pgid <= 1` ⇒ `0` 是自己的组、`1` 是 init 的组,都不是我们的;
    /// - 其余才按组杀。
    #[test]
    fn a_group_is_only_killed_when_this_run_created_it() {
        const OWN: i32 = 4242; // 本进程所在的组(假装)
        for (given, why) in [
            (None, "no group id recorded => we did not create it"),
            (
                Some(OWN),
                "it IS our own group => killing it kills the whole runtime",
            ),
            (Some(0), "pgid 0 means the caller's own group in POSIX"),
            (Some(1), "pgid 1 is init's group"),
            (Some(-7), "a negative id is not a group we ever created"),
        ] {
            assert_eq!(
                target_group(given, OWN),
                None,
                "{why}: target_group({given:?}, {OWN}) must decline to kill by group"
            );
        }
        // 反向:一个显然属于本次运行的组**必须**被按组杀,否则就是没修(N-037 项三)。
        assert_eq!(
            target_group(Some(7777), OWN),
            Some(7777),
            "a group we created ourselves is exactly the one N-037 item 3 exists to kill"
        );
    }

    /// spawn 侧:**只有真的建了组才记 `Some`**。
    ///
    /// ⚠ 端到端的「超时后不留活着的孙进程」那条契约在 `process_contract.rs`,
    ///   并且**只编在 POSIX 上**(Windows 的 `taskkill /T /F` 已经按进程树解决
    ///   同一件事,那条用例在那里恒绿)。本条是它的**平台无关那一半**:Windows 上
    ///   必须**拿不到**组 id ⇒ `kill_tree` 走不到按组杀那条分支 ⇒ Windows 行为
    ///   逐字不变(红线二)。
    #[test]
    fn a_spawn_only_records_a_group_where_one_exists() {
        let argv: Vec<String> = if cfg!(windows) {
            vec!["cmd".into(), "/C".into(), "exit".into(), "0".into()]
        } else {
            vec!["sh".into(), "-c".into(), "exit 0".into()]
        };
        let (mut child, group) = spawn_owned_group(&argv, None, &[]).expect("spawn");
        let pid = child.id();
        let _ = child.wait();
        if cfg!(windows) {
            assert!(
                group.is_none(),
                "Windows has no process group here — recording one would let the \
                 group-kill branch run on a platform that never asked for it"
            );
        } else {
            // `setpgid(0, 0)` ⇒ 组 id 就是子进程 pid。
            assert_eq!(
                group,
                Some(i32::try_from(pid).expect("pid fits in i32")),
                "the recorded group id must be the child's own pid (setpgid(0,0))"
            );
        }
    }

    /// ⚠ **形状守卫**(`N-037 项三 / D13-068`):按组杀必须**被闸门挡着**,且
    /// 必须**有回退**。
    ///
    /// ⚠ 它守不了「按组杀真的杀到了孙进程」—— 那只能在 POSIX 上真跑,见
    /// `process_contract.rs` 的 `a_timeout_leaves_no_living_grandchild`。
    /// 它守的是**红线那一侧**:「谁可以决定按组杀」。
    ///
    /// ⚠⚠ needle **全部运行时拼**(仓库既有纪律):写成字面量的话,这些字符串就在
    /// 测试自身里 ⇒ `contains` 命中自己 ⇒ 断言永假、静默空转。
    #[test]
    fn the_group_kill_stays_behind_the_gate_and_keeps_a_fallback() {
        /// 取某个 `fn name(` 的函数体(到下一个顶层 `fn` / 空行前的 `}` 之前)。
        /// ⚠ needle **运行时拼**:写成字面量的话,这些字符串就在测试自身里 ⇒
        ///   `contains` 命中自己 ⇒ 断言永假、静默空转(本仓已踩过两次)。
        fn body_of<'a>(src: &'a str, name: &str) -> &'a str {
            let head = format!("fn {name}(");
            let start = src.find(&head).expect("the fn exists") + head.len();
            let rest = &src[start..];
            let end = rest.find("\nfn ").unwrap_or(rest.len());
            &rest[..end]
        }

        let src = include_str!("process.rs");
        let raw = format!("libc::{}", "killpg"); // 真正的系统调用点
        let helper = format!("{}(pgid)", "kill_group"); // 受控的包装
        let gate = format!("{}(owned_group, own_pgrp())", "target_group");

        let kill_tree = body_of(src, "kill_tree");
        // ① 红线:按组杀只由 `target_group` 的结果驱动。
        assert!(
            kill_tree.contains(&gate),
            "kill_tree must decide the group through target_group — a raw group kill \
             here would fire even when no group was created (N-037 item 3)"
        );
        // ② ⚠ 闸门必须**真的通向一次杀**。⚠ 这一条是**试出来**的:先前只钉 ①⑤,
        //    把 `kill_group(pgid)` 换成 `let _ = pgid;`(即「拿到组却什么都不杀」)
        //    之后守卫**仍然全绿** —— 那等于装饰。
        assert!(
            kill_tree.contains(&helper),
            "the gated branch in kill_tree must actually call the group-kill helper — \
             deciding the group and then not killing it is the defect N-037 item 3 \
             exists to remove"
        );
        // ③ 系统调用不许内联进 `kill_tree`(必须待在受控包装里)。
        assert!(
            !kill_tree.contains(&raw),
            "the raw group signal must not be inlined into kill_tree"
        );
        // ④ 回退底线:`child.kill()` 必须**无条件**留在最后。
        let fallback = format!("let _ = child.{}();", "kill");
        assert!(
            kill_tree.contains(&fallback),
            "kill_tree must keep the unconditional child.kill() — it is the one kill \
             that is guaranteed to land (degraded spawn, empty group, failed group kill)"
        );
        // ⑤ 全链闭合:闸门 → 包装 → 系统调用。系统调用全文件**只出现一次**。
        assert_eq!(
            src.matches(&raw).count(),
            1,
            "the group signal must live in exactly one place (the wrapper)"
        );
        assert!(
            body_of(src, "kill_group").contains(&raw),
            "the group-signal wrapper must be the thing that emits it — otherwise the \
             chain gate -> wrapper -> signal is broken somewhere in the middle"
        );
        // ⑥ spawn 侧真的设了组(否则「记下来的组」根本不存在)。
        // ⚠ needle 带**接收者**(`grouped.`):注释里的 `CommandExt::process_group(0)`
        //   也在讲同一件事,只拿函数名去 `contains` 会命中注释 ⇒ 删掉真调用也不响。
        let setpgid = format!("grouped.process_{}(0)", "group");
        assert!(
            src.contains(&setpgid),
            "the POSIX spawn path must put the child in its own process group \
             (CommandExt::process_group(0) == setpgid(0,0)); without it there is no \
             group to kill and N-037 item 3 is a no-op"
        );
    }

    #[test]
    fn a_string_argv_is_rejected() {
        let mut h = NullHost;
        let e = process_run(&mut h, vec![Value::String("sh -c ls".into())]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        assert!(e.diagnostic().message.contains("argv array"));
    }

    #[test]
    fn an_empty_argv_is_rejected() {
        let mut h = NullHost;
        let e = process_run(&mut h, vec![Value::Array(vec![])]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn a_spawn_failure_is_an_err_value_not_a_diagnostic() {
        let mut h = NullHost;
        let out = process_run(
            &mut h,
            vec![argv_of("wlwl_definitely_no_such_program_zzz_42")],
        )
        .expect("returns a value, not an Err");
        let Value::Err(payload) = out.value else {
            panic!("a failed spawn must be an ERR value")
        };
        let Value::Dict(d) = *payload else { panic!() };
        let kind = d
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "kind"))
            .map(|(_, v)| v.clone());
        assert_eq!(kind, Some(Value::String("ProcessError".into())));
    }

    /// ⚠️ 这条曾经**真的抓到过一个缺陷**:300 ms 的超时走了 **29.26 s**。
    /// 根因是 `child.kill()` 在 Windows 上只杀直接子进程(`cmd`),它 spawn 的
    /// `ping` 仍活着且**继承了管道句柄** ⇒ 读线程等不到 EOF,`join()` 拖住了
    /// 整个超时。修法是「杀进程树」+「join 必须有界」。
    ///
    /// 子命令只跑 5 s(不是 30 s) —— 回归时这条在 ~5 s 内失败,而不是让整个
    /// `cargo test` 多等半分钟。
    #[test]
    fn a_timeout_kills_the_child_instead_of_hanging() {
        let mut h = NullHost;
        let argv = if cfg!(windows) {
            Value::Array(vec![
                Value::String("cmd".into()),
                Value::String("/C ping -n 6 127.0.0.1 > NUL".into()),
            ])
        } else {
            Value::Array(vec![
                Value::String("sleep".into()),
                Value::String("5".into()),
            ])
        };
        let opts = Value::dict(vec![(Value::String("timeout".into()), Value::Integer(300))]);
        let started = Instant::now();
        let out = process_run(&mut h, vec![argv, opts]).expect("returns");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "a 300 ms timeout took {elapsed:?} — the child (or its descendants) \
             were not killed, or the pipe drain was unbounded"
        );
        let Value::Err(payload) = out.value else {
            panic!("a timeout must be an ERR value")
        };
        let Value::Dict(d) = *payload else { panic!() };
        let reason = d
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "reason"))
            .map(|(_, v)| v.clone())
            .unwrap();
        let Value::String(reason) = reason else {
            panic!()
        };
        assert!(reason.contains("timed out"), "reason = {reason:?}");
    }

    /// ⚠🔴 **[N-011 / D13-061] `drain` 对「不会结束的读取线程」也必须有界。**
    ///
    /// ⚠ 这条**不需要真的 spawn 子进程**就能钉住性质:缺陷的形状是
    /// 「`already_timed_out == true` 时跳过轮询、直接 `join()`」⇒ **拿一个不会
    /// 结束的读取线程调 `drain`,就会一直等下去**。这里用「睡 5 s 才结束」的线程
    /// 做替身 —— 与真缺陷**同形**(孙进程握着管道写端 ⇒ 读取线程不结束)。
    /// ⚠ 它**本机可跑、与平台无关**,所以它是本条的**负向验证闸门**:退回缺陷
    /// 形态时这条会红(actual 见交付报告)。
    /// 端到端那条在 `process_contract.rs`:
    /// `a_timeout_with_a_living_grandchild_returns_promptly` —— ⚠ 那条在
    /// **Windows 上恒绿**(`taskkill /T /F` 把孙进程也杀了),只有 POSIX 才验得到
    /// 真形态 ⇒ 本条才是本机/Windows 上真正守得住的那一半。
    #[test]
    fn drain_is_bounded_even_when_the_reader_never_finishes() {
        let h = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(5));
            Vec::new()
        });
        let started = Instant::now();
        let out = drain(h);
        let elapsed = started.elapsed();
        // 未结束 ⇒ 已读到的部分**拿不到** ⇒ 空(超时路径上 stdout/stderr 本来
        // 就不给调用方,见调用点)。
        assert!(
            out.is_empty(),
            "an unfinished reader has no deliverable buffer, got {} bytes",
            out.len()
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "drain waited {elapsed:?} on a reader that never finishes — the bounded \
             wait regressed; on the timeout path this is the hang"
        );
    }

    /// 反向守卫:上界**不能**退化成「永远返回空」—— 读取线程**已结束**时必须拿得到
    /// 完整缓冲(正常路径上 stdout / stderr 是要交给调用方的)。
    #[test]
    fn drain_returns_the_buffer_when_the_reader_has_finished() {
        let h = std::thread::spawn(|| b"hello".to_vec());
        assert_eq!(drain(h), b"hello".to_vec());
    }

    #[test]
    fn unknown_opts_key_is_a_diagnostic() {
        let mut h = NullHost;
        let opts = Value::dict(vec![(Value::String("shell".into()), Value::Boolean(true))]);
        let e = process_run(&mut h, vec![argv_of("cmd"), opts]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        assert!(e.diagnostic().message.contains("unknown opts key"));
    }

    #[test]
    fn process_id_is_the_current_process() {
        let mut h = NullHost;
        let Value::Integer(id) = process_id(&mut h, vec![]).unwrap().value else {
            panic!()
        };
        assert_eq!(id, i64::from(std::process::id()));
    }
}
