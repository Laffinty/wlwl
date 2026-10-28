//! [W-15 / B-7] `wlwl:std.signal` —— **OS 信号** + **优雅停机**。
//!
//! ## 这个命名空间是什么
//!
//! 两个成员，**一个文件、一个命名空间**：
//!
//! 1. `ON_SIGNAL(sig, handler)` —— 登记某个**信号**的处理器（一个 WLWL 闭包）。
//! 2. `STOP()` —— **请求**优雅停机（**不**立刻杀进程）。
//!
//! ## 停机序列（`STOP()` 只**请求**，收尾由宿主在**安全点**执行）
//!
//! ```text
//! 停止 accept → 等在途请求（可配上限时）→ 落盘 → 退出
//! ```
//!
//! ⚠ **`STOP()` 本身只置标志**：它**不**退出进程、**不**取消在途任务。
//!   「谁在什么时候收尾」是**宿主**（求值器）的事 —— 见
//!   [`stop_requested`] / [`service_pending`] 这两个接缝。
//!
//! ## 平台
//!
//! * Unix：`libc::signal`（`SIGINT` / `SIGTERM`）—— `libc` 已是 `wlwl-std` 的
//!   **直接依赖**（`Cargo.toml` 的 `[target.'cfg(unix)'.dependencies]`）⇒ 零新增依赖。
//! * Windows：手写 `extern "system" { SetConsoleCtrlHandler }`（照仓里既有的
//!   手写 FFI 做法，见 `wlwl-std/src/fs.rs` 的 Win32 调用）。
//!
//! ## ⚠ 信号处理器里**只碰原子量**
//!
//! C 层的信号处理器**不能**碰解释器（`Value` 是 `Rc`，不是 `Send`；解释器
//! 不是 `Send`）⇒ OS 事件先落成**待处理位**（`PENDING`），由**主线程**在
//! 安全点取走并跑用户 handler。
//!
//! ## 失败口径
//!
//! * 程序员错误（元数 / 类型 / 认不得的信号名）⇒ `E0022` / `E0030` **诊断**。
//! * 运行期可预期的失败（信号装不上）⇒ `ERR([kind:"SignalError", …])` **值**。

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_error::WlwlResult;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.signal",
    functions: &[("ON_SIGNAL", on_signal as StdFn), ("STOP", stop as StdFn)],
};

// ── 信号种类 ──────────────────────────────────────────────────────────────────

/// 一个可登记的信号。
///
/// ⚠ **平台名与 WLWL 名的对应**：
///
/// | WLWL 名 | Unix | Windows |
/// |---|---|---|
/// | `SIGINT` / `CTRL_C_EVENT` | `SIGINT` | `CTRL_C_EVENT` |
/// | `SIGTERM` / `CTRL_CLOSE_EVENT` | `SIGTERM` | `CTRL_CLOSE_EVENT` |
///
/// ⇒ Windows 上**没有** `SIGTERM` ⇒ `SIGTERM` 映射到 `CTRL_CLOSE_EVENT`
///   （“控制台关闭”那一族）；Unix 上**没有**控制台事件 ⇒ `CTRL_C_EVENT`
///   映射到 `SIGINT`、`CTRL_CLOSE_EVENT` 映射到 `SIGTERM`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    /// 中断（`SIGINT` / `CTRL_C_EVENT`）。
    Interrupt,
    /// 终止（`SIGTERM` / `CTRL_CLOSE_EVENT`）。
    Terminate,
}

impl SignalKind {
    /// 待处理位掩码里对应的那一位。
    const fn bit(self) -> u32 {
        match self {
            SignalKind::Interrupt => PENDING_INTERRUPT,
            SignalKind::Terminate => PENDING_TERMINATE,
        }
    }

    /// 这个信号在**本平台**上的名字（诊断文案用）。
    pub fn platform_name(self) -> &'static str {
        match (self, cfg!(windows)) {
            (SignalKind::Interrupt, true) => "CTRL_C_EVENT",
            (SignalKind::Interrupt, false) => "SIGINT",
            (SignalKind::Terminate, true) => "CTRL_CLOSE_EVENT",
            (SignalKind::Terminate, false) => "SIGTERM",
        }
    }
}

/// WLWL 侧认得的信号名 ⇒ 种类。
///
/// ⚠ 认不得 ⇒ `None`（**不**猜）—— 猜了就等于把「写错了信号名」藏起来。
fn parse_kind(name: &str) -> Option<SignalKind> {
    match name.trim().to_ascii_uppercase().as_str() {
        "SIGINT" | "CTRL_C" | "CTRL_C_EVENT" => Some(SignalKind::Interrupt),
        "SIGTERM" | "CTRL_CLOSE" | "CTRL_CLOSE_EVENT" => Some(SignalKind::Terminate),
        _ => None,
    }
}

/// 认得的信号名清单（诊断文案用）。
const KNOWN_NAMES: &str = "SIGINT / CTRL_C_EVENT / SIGTERM / CTRL_CLOSE_EVENT";

// ── 状态 ──────────────────────────────────────────────────────────────────────

/// 进程级状态。
///
/// ⚠ **线程局部**（不是进程全局的 `static`）：登记的 handler 是 **WLWL 闭包**
///   （`Value` 里面有 `Rc` ⇒ **不是 `Send`**）⇒ 放进 `static` 会编译不过
///   （`static` 要求 `Sync`）⇒ 用**线程局部存储** —— 解释器本来就是**单线程**
///   的，handler 只属于**登记它的那条线程**。
struct State {
    /// 已登记的处理器（信号 ⇒ WLWL 闭包）。
    handlers: HashMap<SignalKind, Value>,
    /// 已经装过 OS 处理器的信号（**每种只装一次**）。
    installed: std::collections::HashSet<SignalKind>,
    /// 停机**是否被请求过**。
    stop_requested: bool,
    /// 停机请求的时刻（单调毫秒；`None` = 没请求过）。
    stop_requested_at: Option<i64>,
}

impl State {
    fn new() -> Self {
        State {
            handlers: HashMap::new(),
            installed: std::collections::HashSet::new(),
            stop_requested: false,
            stop_requested_at: None,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new());
}

/// 读-改-写线程局部状态。
fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// 待处理信号位掩码（OS ⇒ 主线程的**唯一**通道）。
///
/// ⚠ 必须是**原子量**：C 层的信号处理器只许碰它。
static PENDING: AtomicU32 = AtomicU32::new(0);

const PENDING_INTERRUPT: u32 = 1 << 0;
const PENDING_TERMINATE: u32 = 1 << 1;

/// 跑过的用户 handler 次数（**诊断 / 测试**用）。
static HANDLER_RUNS: AtomicU64 = AtomicU64::new(0);

// ── OS 层 ─────────────────────────────────────────────────────────────────────

/// C 层的信号处理器 —— **只碰原子量**。
///
/// ⚠ POSIX 规定：信号处理器里只许做 *async-signal-safe* 的事 ⇒ 这里
///   **只有一次原子读改写**，没有分配、没有加锁、没有碰解释器。
#[cfg(unix)]
extern "C" fn os_signal_handler(sig: libc::c_int) {
    let bit = match sig {
        libc::SIGINT => PENDING_INTERRUPT,
        libc::SIGTERM => PENDING_TERMINATE,
        _ => return,
    };
    PENDING.fetch_or(bit, Ordering::SeqCst);
}

/// 装上 OS 层的信号处理器（Unix：`libc::signal`）。
#[cfg(unix)]
fn install_os_handler(kind: SignalKind) -> Result<(), String> {
    let (sig, handler) = match kind {
        SignalKind::Interrupt => (libc::SIGINT, os_signal_handler as libc::sighandler_t),
        SignalKind::Terminate => (libc::SIGTERM, os_signal_handler as libc::sighandler_t),
    };
    // SAFETY: `handler` 是本文件里那个 `extern "C" fn(c_int)` —— 与
    // `libc::signal` 要求的 `sighandler_t` 同形；`sig` 是上面两个常量之一。
    // ⚠ `libc::signal` 在 POSIX 上返回**旧**处理器 ⇒ 这里**刻意丢弃**
    //   （装过一次之后不再关心旧的）。
    unsafe {
        libc::signal(sig, handler);
    }
    Ok(())
}

/// Windows 控制台事件处理器 —— **只碰原子量**。
///
/// ⚠ Windows 的控制台处理器跑在**另一个线程**上 ⇒ 它**更**不能碰解释器。
#[cfg(windows)]
extern "system" fn os_console_handler(kind: u32) -> i32 {
    // `CTRL_C_EVENT` / `CTRL_BREAK_EVENT` / `CTRL_CLOSE_EVENT`
    const CTRL_C_EVENT: u32 = 0;
    const CTRL_BREAK_EVENT: u32 = 1;
    const CTRL_CLOSE_EVENT: u32 = 2;
    let bit = match kind {
        CTRL_C_EVENT | CTRL_BREAK_EVENT => PENDING_INTERRUPT,
        CTRL_CLOSE_EVENT => PENDING_TERMINATE,
        _ => return 0,
    };
    PENDING.fetch_or(bit, Ordering::SeqCst);
    // 返回非 0 ⇒ 「这个事件我处理了」（系统不再走默认动作）。
    1
}

#[cfg(windows)]
extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

/// 装上 OS 层的信号处理器（Windows：`SetConsoleCtrlHandler`）。
#[cfg(windows)]
fn install_os_handler(kind: SignalKind) -> Result<(), String> {
    // ⚠ 控制台处理器是**整个进程**一个 ⇒ 两种信号共用**同一个**回调
    //   （回调自己按事件类型分派）⇒ 只装一次。
    // SAFETY: `handler` 是本文件里那个 `extern "system" fn(u32) -> i32` ——
    // 与 Win32 要求的签名同形；`add = 1` 表示“登记”。
    let ok = unsafe { SetConsoleCtrlHandler(Some(os_console_handler), 1) };
    if ok == 0 {
        return Err("SetConsoleCtrlHandler failed".to_string());
    }
    let _ = kind;
    Ok(())
}

/// 非 Unix / 非 Windows 平台：**装不上**（运行期可预期的失败 ⇒ `ERR` 值）。
#[cfg(not(any(unix, windows)))]
fn install_os_handler(_kind: SignalKind) -> Result<(), String> {
    Err("signal handlers are not supported on this platform".to_string())
}

// ── 成员 ──────────────────────────────────────────────────────────────────────

/// `ON_SIGNAL(sig, handler) -> NULL`
///
/// 登记一个信号的处理器。**同一种信号登记多次 ⇒ 后一次覆盖前一次**
/// （表里只有一个槽位 —— 与「一个 handler 对一个信号」的心智模型一致）。
///
/// ⚠ 装 OS 处理器**只装一次**（每种信号一次）⇒ 反复登记不会反复改信号处置。
pub fn on_signal(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "ON_SIGNAL";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), "2"));
    }
    let name = str_arg(host, NAME, &args[0])?;
    let handler = args[1].clone();
    if !is_callable(&handler) {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "{NAME}: expected a function handler, got {}",
                kind_of(&handler)
            ),
        ));
    }
    let Some(kind) = parse_kind(&name) else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{NAME}: unknown signal {name:?} (expected one of {KNOWN_NAMES})"),
        ));
    };
    let installed = with_state(|st| {
        if st.installed.insert(kind) {
            // ⚠ **先登记、后装**：装失败时 handler 仍然留着（调用方能看到
            //   这次登记没生效）—— 失败以 `ERR` 值返回，**不**改已有登记。
            match install_os_handler(kind) {
                Ok(()) => Ok(()),
                Err(why) => {
                    st.installed.remove(&kind);
                    Err(why)
                }
            }
        } else {
            Ok(())
        }
    });
    if let Err(why) = installed {
        return Ok(Outcome::normal(signal_err(
            NAME,
            format!("cannot install the {} handler: {why}", kind.platform_name()),
        )));
    }
    with_state(|st| {
        st.handlers.insert(kind, handler);
    });
    Ok(Outcome::normal(Value::Null))
}

/// `STOP() -> NULL`
///
/// **请求**优雅停机。
///
/// ⚠ **只请求**：本成员**不**退出进程、**不**取消在途任务、**不**改退出码。
///   真正的收尾序列（**停止 accept → 等在途请求 → 落盘 → 退出**）由**宿主**
///   在**安全点**执行 —— 宿主通过 [`stop_requested`] 读这个标志。
pub fn stop(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "STOP";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), "0"));
    }
    with_state(|st| {
        if !st.stop_requested {
            st.stop_requested = true;
            st.stop_requested_at = Some(wlwl_value::real_mono_ms());
        }
    });
    Ok(Outcome::normal(Value::Null))
}

// ── 宿主接缝（求值器侧用） ───────────────────────────────────────────────────

/// 停机**是否被请求过**。
///
/// ⇒ 这是「停止 accept」那一段的**接缝**：原生接受循环（`std.net` 的
///   `TCP_SERVE`）在**下一次 accept 之前**查它 ⇒ 查到了就不再接新连接。
pub fn stop_requested() -> bool {
    with_state(|st| st.stop_requested)
}

/// 停机请求的时刻（单调毫秒；`None` = 没请求过）。
pub fn stop_requested_at_ms() -> Option<i64> {
    with_state(|st| st.stop_requested_at)
}

/// 「等在途请求」的**上限**（毫秒）。
///
/// ⇒ 来自环境变量 `WLWL_STOP_TIMEOUT_MS`；缺省 **5000 ms**。
///   ⚠ 认不得 / 负数 ⇒ 回落缺省（**不**让一个坏环境变量把上限变成 0）。
pub fn stop_timeout_ms() -> i64 {
    const DEFAULT_MS: i64 = 5_000;
    let raw = std::env::var("WLWL_STOP_TIMEOUT_MS").ok();
    match raw {
        Some(s) => match s.trim().parse::<i64>() {
            Ok(n) if n >= 0 => n,
            _ => DEFAULT_MS,
        },
        None => DEFAULT_MS,
    }
}

/// 停机的上限**是否已经过了**。
///
/// ⇒ `false` = 还没过（继续等在途请求）；`true` = 过了（**别再等**了）。
pub fn stop_deadline_exceeded() -> bool {
    let Some(at) = stop_requested_at_ms() else {
        return false;
    };
    wlwl_value::real_mono_ms().saturating_sub(at) >= stop_timeout_ms()
}

/// 取走**待处理**的信号（OS ⇒ 主线程的那一档）。
///
/// ⚠ **取走即清**（`fetch_and`）⇒ 同一个信号不会跑两次 handler。
pub fn take_pending_signals() -> Vec<SignalKind> {
    let bits = PENDING.swap(0, Ordering::SeqCst);
    let mut out = Vec::new();
    if bits & PENDING_INTERRUPT != 0 {
        out.push(SignalKind::Interrupt);
    }
    if bits & PENDING_TERMINATE != 0 {
        out.push(SignalKind::Terminate);
    }
    out
}

/// 送一个信号进来（**等价于 OS 送来那个信号**）。
///
/// ⚠ 真实世界里这一步由 **OS** 完成（C 层的处理器 ⇒ `PENDING` 位）；
///   本函数是同一件事的**可编程入口** —— 契约测试用它钉「信号 ⇒ 主线程跑
///   handler」这条链，而**不**依赖平台信号工具（`kill` 在 Windows 上不存在）。
pub fn deliver(kind: SignalKind) {
    PENDING.fetch_or(kind.bit(), Ordering::SeqCst);
}

/// 跑掉**待处理**信号的处理器（**主线程**侧的那一半）。
///
/// ⇒ 返回 `(信号, handler 自己的失败)`：handler 抛错**不**中断停机序列
///   （信号处理器失败不该把进程带崩）。
///
/// ⚠ **必须**在主线程上调用（handler 是 WLWL 代码）。
pub fn service_pending(host: &mut dyn StdHost) -> Vec<(SignalKind, WlwlError)> {
    let mut out = Vec::new();
    for kind in take_pending_signals() {
        let handler = with_state(|st| st.handlers.get(&kind).cloned());
        let Some(handler) = handler else {
            // ⚠ 没有登记 ⇒ 这个信号**没人管**⇒ 不猜、不代跑。
            continue;
        };
        match host.call(&handler, Vec::new(), "signal handler") {
            Ok(_) => {
                HANDLER_RUNS.fetch_add(1, Ordering::SeqCst);
            }
            Err(e) => {
                HANDLER_RUNS.fetch_add(1, Ordering::SeqCst);
                out.push((kind, e));
            }
        }
    }
    out
}

/// 跑过的用户 handler 次数（**诊断 / 测试**用）。
pub fn handler_runs() -> u64 {
    HANDLER_RUNS.load(Ordering::SeqCst)
}

/// 「落盘」那一段 —— 把**标准输出 / 标准错误**刷出去。
///
/// ⚠ **幂等**（可以反复调）；只刷标准流 —— 文件句柄由各自模块负责。
pub fn flush_for_shutdown() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}

// ── 诊断 / 值级错误助手 ──────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: &str) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn str_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<String, WlwlError> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::CellStr(c) => Ok(c.as_str()),
        other => Err(host.diag(
            ErrorCode::E0030,
            format!("{name}: expected a string, got {}", kind_of(other)),
        )),
    }
}

/// 这个值**能不能被调用**（闭包 / 原生函数）。
fn is_callable(v: &Value) -> bool {
    matches!(v, Value::Closure { .. } | Value::NativeFn { .. })
}

/// 值的种类名（诊断措辞用）。
fn kind_of(v: &Value) -> &'static str {
    wlwl_value::type_name(v)
}

/// 运行期可预期的失败 ⇒ `ERR([kind:"SignalError", op, reason])` **值**。
fn signal_err(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("SignalError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一个**什么都不做**的宿主（只需要 `call` / `diag`）。
    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut wlwl_value::StdCtx {
            unimplemented!("not used by these tests")
        }
        fn call(
            &mut self,
            _f: &Value,
            _args: Vec<Value>,
            _name: &str,
        ) -> Result<Outcome, WlwlError> {
            unimplemented!("not used by these tests")
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

    #[test]
    fn signal_names_map_to_kinds() {
        assert_eq!(parse_kind("SIGINT"), Some(SignalKind::Interrupt));
        assert_eq!(parse_kind("sigint"), Some(SignalKind::Interrupt));
        assert_eq!(parse_kind("CTRL_C_EVENT"), Some(SignalKind::Interrupt));
        assert_eq!(parse_kind("SIGTERM"), Some(SignalKind::Terminate));
        assert_eq!(parse_kind("CTRL_CLOSE_EVENT"), Some(SignalKind::Terminate));
        assert_eq!(parse_kind("SIGKILL"), None);
        assert_eq!(parse_kind(""), None);
    }

    #[test]
    fn stop_only_requests_and_never_kills() {
        // ⚠ 本用例是 B-7 的**反向守卫**：把 `STOP()` 改成「立刻杀」⇒
        //   这个测试进程会**当场退出** ⇒ 门禁红。
        let mut host = NullHost;
        let out = stop(&mut host, Vec::new()).expect("STOP must not fail");
        assert!(matches!(out.value, Value::Null));
        assert!(stop_requested(), "STOP() must set the stop flag");
        // 「不立刻杀」的另一半：**本进程还活着**（能跑到下一行就是证据）。
        assert!(stop_timeout_ms() >= 0);
    }

    #[test]
    fn a_pending_signal_is_taken_exactly_once() {
        deliver(SignalKind::Interrupt);
        let got = take_pending_signals();
        assert_eq!(got, vec![SignalKind::Interrupt]);
        assert!(
            take_pending_signals().is_empty(),
            "the pending bit must be cleared once it is taken"
        );
    }
}
