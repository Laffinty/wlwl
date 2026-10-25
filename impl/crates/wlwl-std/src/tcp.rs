//! [项目扩展 · 非 v1.0 标准库公共面] `wlwl:std.net` 的传输层实现（**只做字节搬运**）。
//!
//! ## 为什么有它
//!
//! `wlwl:std.net` 的 5 个成员里 3 个是 HTTP **客户端**，另 2 个是纯函数
//! ⇒ **没有任何成员能建立或接受 TCP 连接**。加上语言侧没有 socket、没有 FFI、
//! 没有插件加载机制 ⇒ 「用 WLWL 写一个网络服务器」在语言层面**无路可走**。
//!
//! ## 形态约定（与 `wlwl:std.net` 同款）
//!
//! - 运行期可预期的失败 ⇒ **`ERR([kind:"SocketError", op, reason])` 值**，不是原生诊断。
//! - 程序员错误（元数 / 类型）⇒ `E0022` / `E0030` 诊断。
//! - 句柄是**不透明 `INTEGER`**（进程级表的下标）。表里**只放 socket 状态**，
//!   不放任何 `Value`（解释器的值模型是 `Rc` 语义，不是 `Send`）。
//! - 字节一律 **`ARRAY[INTEGER]`**（0..=255）—— 与 `std.fs::READ_BYTES` 同一约定。
//!
//! ## 等待语义（两种，**都在这里定死**）
//!
//! - `timeout_ms > 0` ⇒ **有界阻塞**：调用线程最多等这么久（与 `SLEEP` 同代价）。
//! - `timeout_ms == 0` ⇒ **挂起**：没有数据就**挂起当前任务**
//!   （`Signal::Yield(YieldReason::SocketReady(id))`），由解释器侧在 socket
//!   就绪时唤醒。**这是让并发服务器成为可能的那一半**（见缺失发现集 G-02）。
//!
//! ## 线程模型
//!
//! - 每个连接一个**读线程**：持续把字节读进环形缓冲，就绪时把 socket id 推进
//!   **就绪队列**（`push_ready`）—— 解释器线程**从不阻塞在 socket 上**。
//! - 每个监听一个**接收线程**：接受连接后放进待取队列。
//! - 解释器线程（唯一的 WLWL 求值线程）在需要时 `take_ready()` 取就绪 id，
//!   由 `wlwl-eval` 唤醒挂起的任务（那里才有调度器）。
//!
//! ## 边界（**故意不做**）
//!
//! - **不做 HTTP**：本文件里没有 `HTTP`、`GET`、`POST`、`Content-Length`、`200`
//!   等任何协议语义 —— HTTP 解析与响应构造留在 WLWL 侧。有一条测试机械地守着
//!   （`no_http_semantics_in_the_transport_layer`）。
//! - 不做 TLS / 代理 / 连接池 / 重试 / 流式响应体。

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::Duration;

use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, Signal, StdHost, Value, YieldReason};

// ── 限额 ────────────────────────────────────────────────────────────────────

/// 单次 `TCP_RECV` 最多取回的字节数。
/// 读线程的轮询间隔（关闭时要能及时退出）。
const READER_POLL_MS: u64 = 50;
/// 接收线程的轮询间隔。
const ACCEPT_POLL_MS: u64 = 2;

// ⚠ [v0.12.0 / Q1 + R4] **本文件不再有任何「实现侧上界」**。
//   * `max_bytes` **逐字兑现**「读至多 `max_bytes` 个字节」（系统册 §21.5 的
//     `TCP_RECV` 行）⇒ `max_bytes == 0` 读 **0** 字节；请求多大就读多大
//     （交付包原实现把它静默截到 8 MiB —— 规范**没有**这个上界 ⇒ 静默违约）。
//   * `timeout_ms` **没有上界**（系统册 §21.5：「传输层成员的耗时同样由网络决定,
//     **不**给上界」）⇒ 交付包原实现把它静默截到 600 s ⇒ 同样删掉。
//   * 要设上界就**先改规范**（§3.4 的例外要写明条款号与迁移写法）。

// ── 状态 ────────────────────────────────────────────────────────────────────

/// 一个连接：读线程把字节塞进 `buf`，写走 `stream`。
struct ConnState {
    id: i64,
    /// 写用（读线程持有一份 `try_clone` 的句柄 ⇒ 读写互不阻塞）。
    stream: TcpStream,
    /// 已到达但还没被取走的字节。
    buf: Mutex<VecDeque<u8>>,
    /// 「有数据 / EOF / 关闭」的通知。
    filled: Condvar,
    /// 对端已关闭（读到 0 字节）。
    eof: AtomicBool,
    /// 本端已关闭。
    closed: AtomicBool,
    /// 读超时（毫秒）；0 = 无限。
    read_timeout_ms: Mutex<u64>,
    peer: SocketAddr,
}

/// 一个监听：接收线程把已接受的连接放进 `pending`。
struct ListenerState {
    id: i64,
    pending: Mutex<VecDeque<TcpStream>>,
    filled: Condvar,
    closed: AtomicBool,
    local: SocketAddr,
    /// [2026-10-25 / 审计 N2] 「**这个任务**在这个监听上 accept 到的连接」。
    ///
    /// ⚠ **为什么它住在监听里、而不是进程级的全局表**：键是**任务序号**，
    ///   而任务序号只在**一个求值器内**唯一 ⇒ 同一进程里跑**两个求值器**
    ///   （测试套件、嵌入 wlwl 的宿主程序）⇒ 键会撞车 ⇒ 一个求值器的 accept
    ///   拿到**另一个**求值器的连接句柄。
    ///   ⇒ 放在**监听的状态**里 ⇒ 每个 socket 独一份 ⇒ 跨求值器天然不撞
    ///   （socket id 是进程级唯一的）。
    ///
    /// ⚠ 语义：宿主挂起后恢复时挂起点被**再次求值**（并发册 §17.0 第 2 条）
    ///   —— 而 accept 的等待条件「队列里有连接」在第一次 accept 走之后
    ///   **不再成立** ⇒ 再次求值会**再挂一次** ⇒ 违反「不得再挂」⇒ 活锁。
    ///   ⇒ 记下它：再次求值时**返回同一个连接**，不重新 pop。
    memo: Mutex<HashMap<i64, i64>>,
}

enum Entry {
    Listener(Arc<ListenerState>),
    Conn(Arc<ConnState>),
}

static SOCKETS: Mutex<Option<HashMap<i64, Entry>>> = Mutex::new(None);
static NEXT_ID: AtomicI64 = AtomicI64::new(1);
/// **就绪集合**：已就绪的 socket id（socket 可能是连接，也可能是监听）。
///
/// ⚠ [2026-10-25 / 审计 N2] 这是**电平**（level）而不是**边沿**（edge）：
///   「这个 socket 现在有货 / 有连接」是**持续成立**的状态，直到条件被消费。
///   此前它是「一次性队列」⇒ 解释器侧**排干**它 ⇒ **同一进程里跑两个求值器**
///   （测试套件、嵌入 wlwl 的宿主程序）时，一个求值器会把**另一个**的
///   就绪事件**排干并丢掉** ⇒ 那个求值器的任务永远等不到唤醒
///   （实测：CI 上 accept 白等满 5 s 预算）。
///   ⇒ 现在：**排干改为「只取自己要的」**（`peek_ready` + `retire_ready`）
///   ⇒ 不是自己的事件**原样留给主人**。
static READY: LazyLock<Mutex<HashSet<i64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// 就绪集合的「有东西了」信号。
///
/// ⚠ [2026-10-25 / 审计 N3] 此前解释器侧只能**轮询**（`sleep(50ms)` 后再看一眼）
///   ⇒ 一次「就绪」最多要**白等 50 ms** 才能被看见（本仓实测：客户端在
///   33 ms 发数据，服务端 51 ms 才拿到 —— 那 18 ms 的差就是这个轮询粒度）。
///   ⇒ 改成「**条件变量**」：I/O 线程推进就绪集合时 `notify_all`，
///   解释器侧**立刻**被唤醒 ⇒ 等待延迟从「≤50 ms」降到「≈0」。
static READY_CV: Condvar = Condvar::new();

/// 把 socket id 标记为就绪（由 I/O 线程调用）。
fn push_ready(id: i64) {
    let mut q = READY.lock().unwrap_or_else(|e| e.into_inner());
    q.insert(id);
    drop(q);
    // ⚠ 必须**在放锁之后**通知：否则被唤醒的线程可能抢在 insert 之前拿到锁，
    //   看到空集合又睡回去（丢通知）。
    READY_CV.notify_all();
}

/// **看一眼**当前就绪的 socket id（**不消费**）。
///
/// ⚠ 解释器侧只**取走自己要的那几个**（`retire_ready`）⇒ 别人的就绪事件
///   **原样留着**。
pub fn peek_ready() -> Vec<i64> {
    READY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .copied()
        .collect()
}

/// 消费掉一个就绪事件（条件已被消费 ⇒ 不再是「就绪」）。
pub fn retire_ready(id: i64) {
    READY.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
}

/// 阻塞到「有人推进了就绪集合」或超时。
///
/// ⚠ [2026-10-25 / 审计 N3] 解释器侧的等待点用它**代替**「睡 50 ms 再看一眼」：
///   I/O 线程一推进就绪集合就 `notify_all` ⇒ 等待延迟 ≈ 0。
///
/// ⚠ **不看集合本身空不空** —— 它是**电平**，可能一直非空（别人的事件也在里面）
///   ⇒ 用它当「有更新了」的信号会变成忙等。
pub fn await_ready(timeout: std::time::Duration) {
    let q = READY.lock().unwrap_or_else(|e| e.into_inner());
    let _ = READY_CV
        .wait_timeout(q, timeout)
        .unwrap_or_else(|e| e.into_inner());
}

fn with_sockets<R>(f: impl FnOnce(&mut HashMap<i64, Entry>) -> R) -> R {
    let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

// ── 诊断助手（与 net.rs / time.rs 同款）────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, want: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

/// 运行期失败 → `ERR([kind:"SocketError", op, reason])`**值**。
fn socket_error(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("SocketError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

fn int_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<i64, wlwl_error::WlwlError> {
    let Value::Integer(n) = v else {
        return Err(type_err(host, name, "integer", v));
    };
    Ok(*n)
}

fn str_arg<'a>(
    host: &mut dyn StdHost,
    name: &str,
    v: &'a Value,
) -> Result<&'a str, wlwl_error::WlwlError> {
    let Value::String(s) = v else {
        return Err(type_err(host, name, "string", v));
    };
    Ok(s.as_str())
}

/// 取出「那一次等待的截止时间」（毫秒）。
///
/// ⚠ [v0.12.0 / Q1] `0` = **无截止时间**（挂起直到条件满足）—— 系统册 §21.6.4：
/// 「`timeout_ms` 是**那一次等待的截止时间**;`0` = **无截止时间**」。
/// ⚠ **不设上界**（规范不给上界）⇒ 交付包原实现的 600 s 截断**已删**。
fn timeout_arg(
    host: &mut dyn StdHost,
    name: &str,
    ms: i64,
) -> Result<Option<i64>, wlwl_error::WlwlError> {
    if ms < 0 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{name}: expected a non-negative timeout in ms, got {ms}"),
        ));
    }
    Ok(if ms == 0 { None } else { Some(ms) })
}

/// [v0.12.0 / Q1] 装上「那一次等待」的截止时间。
///
/// ⚠ **两个预算先到者为准**（系统册 §21.6.4）：
/// - `call_site` = 调用点那个 `timeout_ms`（`None` = 无截止时间）；
/// - `conn` = 连接上的**读超时**（`TCP_SET_TIMEOUT` 设的，`None` = 无限）。
///
/// ⚠ 截止时间住在**调度器**里（`push_task_deadline`）⇒ 到点后调度器**唤醒**
///   这个任务 ⇒ 它的等待以「超时」结束 ⇒ 成员重跑时 `take_timed_out()` 命中。
///
/// ⚠ **已经在跑的等待不会被这个覆盖**：若任务已有**更紧**的截止时间 ⇒ 保留它。
fn arm_wait_deadline(
    host: &mut dyn StdHost,
    call_site: Option<i64>,
    conn: Option<i64>,
) -> Result<(), wlwl_error::WlwlError> {
    let budget = match (call_site, conn) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    let Some(ms) = budget else {
        return Ok(());
    };
    let at = wlwl_value::real_mono_ms().saturating_add(ms);
    if let Some(existing) = host.task_deadline_ms() {
        if existing <= at {
            // 已有的更紧 ⇒ 保留它（别覆盖）。
            return Ok(());
        }
    }
    host.push_task_deadline(at)?;
    Ok(())
}

// ── I/O 线程 ───────────────────────────────────────────────────────────────

/// 每个连接一个读线程：持续把字节读进缓冲，就绪时推进就绪队列。
fn spawn_reader(state: Arc<ConnState>, mut reader: TcpStream) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            if state.closed.load(Ordering::SeqCst) {
                break;
            }
            // 读超时用来「定期醒来看看关闭标志」—— 关闭要能及时生效。
            let want = *state
                .read_timeout_ms
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let _ = reader.set_read_timeout(if want == 0 {
                Some(Duration::from_millis(READER_POLL_MS))
            } else {
                Some(Duration::from_millis(want.min(READER_POLL_MS)))
            });
            match reader.read(&mut buf) {
                Ok(0) => {
                    state.eof.store(true, Ordering::SeqCst);
                    state.filled.notify_all();
                    break;
                }
                Ok(n) => {
                    {
                        let mut q = state.buf.lock().unwrap_or_else(|e| e.into_inner());
                        q.extend(&buf[..n]);
                    }
                    state.filled.notify_all();
                    push_ready(state.id);
                }
                Err(ref e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    // 超时（可能是我们自己设的轮询间隔）⇒ 回去看看关闭标志。
                    continue;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_e) => {
                    // 读出错（对端复位等）⇒ 当作 EOF 收尾：调用方看到 `eof = TRUE`，
                    // 不会以为「连接还活着但没数据」。
                    state.eof.store(true, Ordering::SeqCst);
                    state.filled.notify_all();
                    break;
                }
            }
        }
    });
}

/// 每个监听一个接收线程：接受连接后放进待取队列。
fn spawn_acceptor(state: Arc<ListenerState>, listener: TcpListener) {
    std::thread::spawn(move || loop {
        if state.closed.load(Ordering::SeqCst) {
            break;
        }
        match listener.accept() {
            Ok((stream, _peer)) => {
                {
                    let mut p = state.pending.lock().unwrap_or_else(|e| e.into_inner());
                    p.push_back(stream);
                }
                state.filled.notify_all();
                push_ready(state.id);
            }
            Err(ref e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                std::thread::sleep(Duration::from_millis(ACCEPT_POLL_MS));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                state.closed.store(true, Ordering::SeqCst);
                state.filled.notify_all();
                break;
            }
        }
    });
}

/// [2026-10-25 / 审计 N2] 「**这个任务**刚才 accept 到的那个连接」。
///
/// ⚠ **为什么需要它**：宿主挂起后恢复时，挂起点被**再次求值**（并发册 §17.0
///   第 2 条：「等待条件既已满足，它**必须**立即完成；实现**不得**让它
///   再次挂起」）。而 `TCP_ACCEPT` 的等待条件是「**队列里有连接**」——
///   连接一旦被 accept 走，条件**就不再成立** ⇒ 再次求值会**再挂一次**
///   ⇒ 违反「不得再挂」⇒ 任务原地活锁。
///   实测（修复前）：accept 烧光自己的超时预算（5 s）⇒ 返回 `NULL` ⇒
///   调用方拿 `NULL` 去 `TCP_RECV` ⇒ `E0030`；而第一次 accept 到的那个
///   连接**被覆盖、再也无法关闭**（泄漏）。
/// ⇒ 记下它：再次求值时**返回同一个连接**，不重新 pop。
///
/// ⚠ **它住在监听的状态里**（`ListenerState::memo`），**不是**进程级全局表 ——
///   键是任务序号，只在一个求值器内唯一；全局表会让同一进程里的两个求值器
///   撞车（实测：CI 上两个并行用例互换了连接句柄）。
///
/// ⚠ 连接被 `TCP_CLOSE` 时清掉 ⇒ 不会随连接数无界增长。
fn memo_get(state: &Arc<ListenerState>, key: i64) -> Option<i64> {
    state
        .memo
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .copied()
}

fn memo_set(state: &Arc<ListenerState>, key: i64, conn: i64) {
    state
        .memo
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, conn);
}

/// 清掉与某个 listener 相关的备忘（**挂起前**调用：一次新的等待 ⇒ 旧的作废）。
fn memo_clear_for_listener(state: &Arc<ListenerState>, key: i64) {
    state
        .memo
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&key);
}

/// 这个连接句柄**还在**吗？（被 `TCP_CLOSE` 关掉了 ⇒ 备忘作废）
fn conn_alive(cid: i64) -> bool {
    let guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
    matches!(
        guard.as_ref().and_then(|m| m.get(&cid)),
        Some(Entry::Conn(_))
    )
}

/// 清掉与某个连接相关的备忘（**连接关闭时**调用）。
///
/// ⚠ 监听们**不知道**彼此 ⇒ 扫一遍（监听通常只有一两个）。
fn memo_clear_for_conn(conn: i64) {
    let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(map) = guard.as_mut() else { return };
    for entry in map.values() {
        let Entry::Listener(l) = entry else { continue };
        let mut m = l.memo.lock().unwrap_or_else(|e| e.into_inner());
        let stale: Vec<i64> = m
            .iter()
            .filter(|(_, c)| **c == conn)
            .map(|(k, _)| *k)
            .collect();
        for k in stale {
            m.remove(&k);
        }
    }
}

// ── 成员实现 ────────────────────────────────────────────────────────────────

/// `TCP_LISTEN(host, port) -> INTEGER`
pub fn tcp_listen(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "TCP_LISTEN", args.len(), 2));
    }
    let host_str = str_arg(host, "TCP_LISTEN", &args[0])?.to_string();
    let port = int_arg(host, "TCP_LISTEN", &args[1])?;
    if !(0..=65535).contains(&port) {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("TCP_LISTEN: port out of range: {port}"),
        ));
    }
    let bind_host = if host_str.is_empty() {
        "0.0.0.0".to_string()
    } else {
        host_str
    };
    let addr = format!("{bind_host}:{port}");
    let resolved = match addr.to_socket_addrs() {
        Ok(mut it) => it.next(),
        Err(e) => {
            return Ok(Outcome::normal(socket_error(
                "TCP_LISTEN",
                format!("cannot resolve {addr:?}: {e}"),
            )))
        }
    };
    let Some(resolved) = resolved else {
        return Ok(Outcome::normal(socket_error(
            "TCP_LISTEN",
            format!("no address for {addr:?}"),
        )));
    };
    let listener = match TcpListener::bind(resolved) {
        Ok(l) => l,
        Err(e) => {
            return Ok(Outcome::normal(socket_error(
                "TCP_LISTEN",
                format!("cannot bind {resolved}: {e}"),
            )))
        }
    };
    // 非阻塞 ⇒ 接收线程可以定期醒来检查关闭标志。
    let _ = listener.set_nonblocking(true);
    let local = match listener.local_addr() {
        Ok(a) => a,
        Err(e) => {
            return Ok(Outcome::normal(socket_error(
                "TCP_LISTEN",
                format!("cannot read the local address: {e}"),
            )))
        }
    };
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let state = Arc::new(ListenerState {
        id,
        pending: Mutex::new(VecDeque::new()),
        filled: Condvar::new(),
        closed: AtomicBool::new(false),
        local,
        memo: Mutex::new(HashMap::new()),
    });
    spawn_acceptor(state.clone(), listener);
    with_sockets(|map| {
        map.insert(id, Entry::Listener(state));
    });
    Ok(Outcome::normal(Value::Integer(id)))
}

/// `TCP_ACCEPT(listener, timeout_ms) -> INTEGER`
///
/// - 有待取的连接 ⇒ 返回连接句柄。
/// - `timeout_ms > 0` 且超时 ⇒ `NULL`（**正常结果**）。
/// - `timeout_ms == 0` 且没有连接 ⇒ **挂起当前任务**（`Signal::Yield(SocketReady)`），
///   socket 就绪时由解释器唤醒。
pub fn tcp_accept(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "TCP_ACCEPT", args.len(), 2));
    }
    let lid = int_arg(host, "TCP_ACCEPT", &args[0])?;
    let timeout_ms = int_arg(host, "TCP_ACCEPT", &args[1])?;
    let timeout = timeout_arg(host, "TCP_ACCEPT", timeout_ms)?;

    // ── [v0.12.0 / Q1] 等待一律是**宿主挂起** ──────────────────────────
    //
    // 系统册 §21.6.4（Q1 裁决后）：「等待期间**挂起当前任务**……**别的任务照常
    // 运行**」⇒ 并发册 §17.0 的第四类挂起点「宿主挂起（原生成员等待 socket）」
    // ⇒ **不阻塞整个运行时**（交付包原实现是「`timeout_ms > 0` ⇒ 阻塞冻结整个
    // `SCOPE`」—— 那是 Q1 裁决**之前**的旧口径，已作废）。
    //
    // ⚠ [2026-10-25 / 审计 N2] 本次是不是「**挂起点的再次求值**」？
    //   （并发册 §17.0 第 2 条：恢复时挂起点被再次求值，**不得再挂一次**。）
    let resume = host.resume_hint();
    let task_key = host.current_task_key();

    // ⚠ **重跑入口**：宿主挂起 = **不切段**格 ⇒ 恢复时**段首重放**（§17.0）
    //   ⇒ 这段代码会**从头再跑一遍**。
    //
    // ⚠ [2026-10-25 / 审计 N1] 「已超时」的检查**必须**排在「`pending` 里有没有
    //   连接」**后面**：否则连接在截止时间**之前**就到了，却因截止时间先到而
    //   被**丢弃**（返回 `NULL` ⇒ 调用方以为「没人连」）。
    let state = {
        let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(map) = guard.as_mut() else {
            return Ok(Outcome::normal(socket_error(
                "TCP_ACCEPT",
                format!("unknown listener handle: {lid}"),
            )));
        };
        match map.get(&lid) {
            Some(Entry::Listener(s)) => s.clone(),
            Some(Entry::Conn(_)) => {
                return Ok(Outcome::normal(socket_error(
                    "TCP_ACCEPT",
                    format!("handle {lid} is not a listener"),
                )))
            }
            None => {
                return Ok(Outcome::normal(socket_error(
                    "TCP_ACCEPT",
                    format!("unknown listener handle: {lid}"),
                )))
            }
        }
    };

    // ① **恢复重放**：本任务刚才 accept 到的那个连接还在 ⇒ **返回同一个连接**，
    //    不重新 pop（否则连接会被覆盖、再也无法关闭）。
    //
    // ⚠ 备忘住在**这个监听**的状态里 ⇒ 键（任务序号）只需在一个求值器内唯一。
    if resume.is_some() {
        if let Some(key) = task_key {
            if let Some(memo_conn) = memo_get(&state, key) {
                // ⚠ 那个连接**还在**吗？（被关掉了 ⇒ 备忘作废）
                if conn_alive(memo_conn) {
                    // ⚠ 我**就是**挂起点（socket 相同）⇒ 本次求值完成 ⇒ 清掉提示。
                    if resume == Some(lid) {
                        host.clear_resume_hint();
                    }
                    return Ok(Outcome::normal(Value::Integer(memo_conn)));
                }
            }
        }
    }

    // ② 条件已满足？⇒ **直接交付**（不挂起，也不装截止时间）。
    let stream = {
        let mut p = state.pending.lock().unwrap_or_else(|e| e.into_inner());
        p.pop_front()
    };
    let Some(stream) = stream else {
        // ③ 真的没有连接 ⇒ 这时才问「是不是等超时了」。
        if host.take_timed_out() {
            return Ok(Outcome::normal(Value::Null));
        }
        // ⚠ [2026-10-25 / 审计 N2] 恢复重放时**不得再挂一次**（§17.0 第 2 条）
        //   —— 而这里既没有连接、也没有备忘 ⇒ 那是病态形态 ⇒ **不挂**，
        //   返回「没人连」的正常值，让调用方自己处置（挂下去就是原地活锁）。
        if resume == Some(lid) {
            return Ok(Outcome::normal(Value::Null));
        }
        // ④ 条件不满足 ⇒ 装上「那一次等待」的截止时间 ⇒ **挂起**。
        //
        // ⚠ 监听没有「连接读超时」⇒ 这里只有**调用点**这一个预算。
        //   `timeout_ms == 0` = **无截止时间**（挂起直到条件满足）。
        //
        // ⚠ [2026-10-25 / 审计 N2] 挂起 ⇒ **清掉**本任务在本 listener 上的
        //   备忘（一次新的等待 ⇒ 上一次的连接不再有效 ⇒ 否则 accept 循环会
        //   永远拿到同一个连接）。
        if let Some(key) = task_key {
            memo_clear_for_listener(&state, key);
        }
        arm_wait_deadline(host, timeout, None)?;
        return Ok(Outcome::with_signal(
            Value::Null,
            Signal::Yield(YieldReason::SocketReady(lid)),
        ));
    };
    // 建立连接状态 + 读线程。
    let _ = stream.set_nonblocking(false);
    let peer = stream
        .peer_addr()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
    let reader = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => {
            return Ok(Outcome::normal(socket_error(
                "TCP_ACCEPT",
                format!("cannot clone the accepted socket: {e}"),
            )))
        }
    };
    let cid = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let conn = Arc::new(ConnState {
        id: cid,
        stream,
        buf: Mutex::new(VecDeque::new()),
        filled: Condvar::new(),
        eof: AtomicBool::new(false),
        closed: AtomicBool::new(false),
        // ⚠ [v0.12.0 / R3] 连接的**读超时**默认 = **无限**（`0`）
        //   ⇒ 只有**显式** `TCP_SET_TIMEOUT` 才给读设上界。
        //   （交付包原实现默认 30 s ⇒ 那是一条规范里**没有**的静默截断。）
        read_timeout_ms: Mutex::new(0),
        peer,
    });
    spawn_reader(conn.clone(), reader);
    with_sockets(|map| {
        map.insert(cid, Entry::Conn(conn));
    });
    // ⚠ [2026-10-25 / 审计 N2] 记下「这个任务 accept 到的这个连接」⇒
    //   恢复重放时**返回同一个连接**（不重新 pop ⇒ 连接不被覆盖、仍可关闭）。
    if let Some(key) = task_key {
        memo_set(&state, key, cid);
    }
    // ⚠ 条件被**消费**了（连接被取走）⇒ 就绪标记作废。
    // ⚠ **但队列里还有连接时不能作废** —— 否则下一次 accept 会挂上去，而
    //   「接收线程」只在接受**新**连接时才推事件 ⇒ 那个连接就永远等不到唤醒。
    {
        let still_pending = !state
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty();
        if !still_pending {
            retire_ready(lid);
        }
    }
    // ⚠ 我**就是**挂起点（socket 相同）⇒ 本次求值完成 ⇒ 清掉恢复提示。
    if resume == Some(lid) {
        host.clear_resume_hint();
    }
    Ok(Outcome::normal(Value::Integer(cid)))
}

/// `TCP_RECV(conn, max_bytes, timeout_ms) -> DICT`
///
/// 返回 `["bytes": ARRAY[INTEGER], "text": STRING, "eof": BOOLEAN]`。
///
/// ⚠ [v0.12.0 / Q1] 等待**一律是宿主挂起**（系统册 §21.6.4）：没有数据 ⇒
///   **挂起当前任务**，别的任务照常运行；`timeout_ms` = **那一次等待**的截止时间，
///   `0` = **无截止时间**（挂起直到条件满足）。
pub fn tcp_recv(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity(host, "TCP_RECV", args.len(), 3));
    }
    let cid = int_arg(host, "TCP_RECV", &args[0])?;
    let max_bytes = int_arg(host, "TCP_RECV", &args[1])?;
    let timeout_ms = int_arg(host, "TCP_RECV", &args[2])?;
    let timeout = timeout_arg(host, "TCP_RECV", timeout_ms)?;
    if max_bytes < 0 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("TCP_RECV: expected a non-negative max_bytes, got {max_bytes}"),
        ));
    }
    // ⚠ [v0.12.0 / R4] `max_bytes` **逐字兑现**「读至多 `max_bytes` 个字节」
    //   （系统册 §21.5）⇒ **不**抬到 1（`max_bytes == 0` ⇒ 读 **0** 字节）、
    //   **不**静默截到 8 MiB（规范**没有**那个上界）。
    let max = max_bytes as usize;

    // [v0.12.0 / Q1] **重跑入口**：宿主挂起 = **不切段**格 ⇒ 恢复时**段首重放**
    //   （并发册 §17.0）⇒ 这段代码会**从头再跑一遍**。
    //
    // ⚠ [2026-10-25 / 审计 N1] 「已超时」的检查**必须**排在「缓冲里有没有货」
    //   **后面**：此前顺序相反 ⇒ 数据在截止时间**之前**就到了缓冲区，却因为
    //   截止时间先到而被**丢弃**（返回「空字节 + `eof = FALSE`」⇒ 已经到达的
    //   数据没了）。超时形状表示的是「**这一次没等到**」；货已经在了就不该是它。
    let state = {
        let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(map) = guard.as_mut() else {
            return Ok(Outcome::normal(socket_error(
                "TCP_RECV",
                format!("unknown connection handle: {cid}"),
            )));
        };
        match map.get(&cid) {
            Some(Entry::Conn(s)) => s.clone(),
            Some(Entry::Listener(_)) => {
                return Ok(Outcome::normal(socket_error(
                    "TCP_RECV",
                    format!("handle {cid} is a listener, not a connection"),
                )))
            }
            None => {
                return Ok(Outcome::normal(socket_error(
                    "TCP_RECV",
                    format!("unknown connection handle: {cid}"),
                )))
            }
        }
    };

    // ① 条件已满足？⇒ **直接交付**（不挂起，也不装截止时间）。
    {
        let mut q = state.buf.lock().unwrap_or_else(|e| e.into_inner());
        if !q.is_empty() {
            let n = q.len().min(max);
            let bytes: Vec<u8> = q.drain(..n).collect();
            let eof = state.eof.load(Ordering::SeqCst);
            let empty_now = q.is_empty();
            drop(q);
            // 数据到手 ⇒ 那个「已超时」标志**作废**（否则下一次等待会立刻超时）。
            let _ = host.take_timed_out();
            // ⚠ [2026-10-25 / 审计 N2] 条件被**消费**了 ⇒ 就绪标记作废
            //   （否则下一次等待会被这个陈旧标记**立刻唤醒** ⇒ 空转）。
            if empty_now {
                retire_ready(cid);
            }
            return Ok(Outcome::normal(recv_value(&bytes, eof)));
        }
        if state.eof.load(Ordering::SeqCst) {
            let _ = host.take_timed_out();
            retire_ready(cid);
            return Ok(Outcome::normal(recv_value(&[], true)));
        }
    }

    // ② 缓冲里确实没有 ⇒ 这时才问「是不是等超时了」。
    //
    // ⚠ 超时 ⇒ **「暂时没有」形状**（空字节 + `eof = FALSE`）—— **不是** `ERR`，
    //   也**不是** EOF（§21.6.2：「超时**不得**被当成 EOF」）。
    if host.take_timed_out() {
        return Ok(Outcome::normal(recv_value(&[], false)));
    }

    // ② 条件不满足 ⇒ 装上「那一次等待」的截止时间 ⇒ **挂起**。
    //
    // ⚠ **两个预算先到者为准**（系统册 §21.6.4）：调用点的 `timeout_ms` 与
    //   连接上的读超时（`TCP_SET_TIMEOUT` 設的）⇒ 取更早者。
    //   两者都是「无」⇒ **无截止时间**（挂起直到条件满足）。
    let conn_budget = {
        let v = *state
            .read_timeout_ms
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if v == 0 {
            None
        } else {
            Some(v as i64)
        }
    };
    arm_wait_deadline(host, timeout, conn_budget)?;
    Ok(Outcome::with_signal(
        Value::Null,
        Signal::Yield(YieldReason::SocketReady(cid)),
    ))
}

/// `["bytes": ARRAY[INTEGER], "text": STRING, "eof": BOOLEAN]`
fn recv_value(bytes: &[u8], eof: bool) -> Value {
    Value::dict(vec![
        (
            Value::String("bytes".into()),
            Value::Array(
                bytes
                    .iter()
                    .map(|b| Value::Integer(i64::from(*b)))
                    .collect::<Vec<Value>>(),
            ),
        ),
        (
            Value::String("text".into()),
            Value::String(String::from_utf8_lossy(bytes).to_string()),
        ),
        (Value::String("eof".into()), Value::Boolean(eof)),
    ])
}

/// `TCP_SEND(conn, payload) -> INTEGER`
pub fn tcp_send(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "TCP_SEND", args.len(), 2));
    }
    let cid = int_arg(host, "TCP_SEND", &args[0])?;
    let payload: Vec<u8> = match &args[1] {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                let Value::Integer(b) = it else {
                    return Err(type_err(host, "TCP_SEND", "array of integers", it));
                };
                if !(0..=255).contains(b) {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("TCP_SEND: byte out of range: {b}"),
                    ));
                }
                out.push(*b as u8);
            }
            out
        }
        other => {
            return Err(type_err(
                host,
                "TCP_SEND",
                "string or array of integers",
                other,
            ))
        }
    };

    let state = {
        let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(map) = guard.as_mut() else {
            return Ok(Outcome::normal(socket_error(
                "TCP_SEND",
                format!("unknown connection handle: {cid}"),
            )));
        };
        match map.get(&cid) {
            Some(Entry::Conn(s)) => s.clone(),
            _ => {
                return Ok(Outcome::normal(socket_error(
                    "TCP_SEND",
                    format!("handle {cid} is not a connection"),
                )))
            }
        }
    };
    let mut stream = &state.stream;
    match stream.write_all(&payload) {
        Ok(()) => Ok(Outcome::normal(Value::Integer(payload.len() as i64))),
        Err(e) => Ok(Outcome::normal(socket_error(
            "TCP_SEND",
            format!("write failed: {e}"),
        ))),
    }
}

/// `TCP_PEER(conn) -> DICT` —— `["host": STRING, "port": INTEGER]`。
pub fn tcp_peer(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "TCP_PEER", args.len(), 1));
    }
    let cid = int_arg(host, "TCP_PEER", &args[0])?;
    let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(map) = guard.as_mut() else {
        return Ok(Outcome::normal(socket_error(
            "TCP_PEER",
            format!("unknown connection handle: {cid}"),
        )));
    };
    let Some(Entry::Conn(s)) = map.get(&cid) else {
        return Ok(Outcome::normal(socket_error(
            "TCP_PEER",
            format!("handle {cid} is not a connection"),
        )));
    };
    Ok(Outcome::normal(Value::dict(vec![
        (
            Value::String("host".into()),
            Value::String(s.peer.ip().to_string()),
        ),
        (
            Value::String("port".into()),
            Value::Integer(i64::from(s.peer.port())),
        ),
    ])))
}

/// `TCP_LOCAL_ADDR(listener) -> DICT` —— 本地绑定地址。
pub fn tcp_local_addr(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "TCP_LOCAL_ADDR", args.len(), 1));
    }
    let lid = int_arg(host, "TCP_LOCAL_ADDR", &args[0])?;
    let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(map) = guard.as_mut() else {
        return Ok(Outcome::normal(socket_error(
            "TCP_LOCAL_ADDR",
            format!("unknown listener handle: {lid}"),
        )));
    };
    let Some(Entry::Listener(s)) = map.get(&lid) else {
        return Ok(Outcome::normal(socket_error(
            "TCP_LOCAL_ADDR",
            format!("handle {lid} is not a listener"),
        )));
    };
    Ok(Outcome::normal(Value::dict(vec![
        (
            Value::String("host".into()),
            Value::String(s.local.ip().to_string()),
        ),
        (
            Value::String("port".into()),
            Value::Integer(i64::from(s.local.port())),
        ),
    ])))
}

/// `TCP_SET_TIMEOUT(conn, ms) -> NULL` —— 读超时；`0` = 无限。
pub fn tcp_set_timeout(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "TCP_SET_TIMEOUT", args.len(), 2));
    }
    let cid = int_arg(host, "TCP_SET_TIMEOUT", &args[0])?;
    let ms = int_arg(host, "TCP_SET_TIMEOUT", &args[1])?;
    if ms < 0 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("TCP_SET_TIMEOUT: expected a non-negative duration in ms, got {ms}"),
        ));
    }
    // ⚠ [v0.12.0 / R3] 这个值**真的**给读设上界（`0` = 无限）——
    //   `TCP_RECV` 的等待预算取「连接读超时」与「调用点 `timeout_ms`」中
    //   **先到者**（系统册 §21.6.4）。交付包原实现只把它当**轮询粒度**用
    //   （且被截到 600 s）⇒ **语义空转**：设了也没用。
    // ⚠ **不设上界**（规范不给上界）⇒ 交付包原实现的 600 s 截断**已删**。
    let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(map) = guard.as_mut() else {
        return Ok(Outcome::normal(socket_error(
            "TCP_SET_TIMEOUT",
            format!("unknown connection handle: {cid}"),
        )));
    };
    let Some(Entry::Conn(s)) = map.get(&cid) else {
        return Ok(Outcome::normal(socket_error(
            "TCP_SET_TIMEOUT",
            format!("handle {cid} is not a connection"),
        )));
    };
    *s.read_timeout_ms.lock().unwrap_or_else(|e| e.into_inner()) = ms as u64;
    Ok(Outcome::normal(Value::Null))
}

/// `TCP_CLOSE(conn) -> NULL` —— 关闭连接（**幂等**）。
pub fn tcp_close(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "TCP_CLOSE", args.len(), 1));
    }
    let cid = int_arg(host, "TCP_CLOSE", &args[0])?;
    {
        let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = guard.as_mut() {
            if let Some(Entry::Conn(s)) = map.remove(&cid) {
                s.closed.store(true, Ordering::SeqCst);
                s.filled.notify_all();
                let _ = s.stream.shutdown(Shutdown::Both);
            }
        }
        // ⚠ **必须先放掉 `SOCKETS` 锁**再清备忘 —— 备忘清理自己也要拿这把锁
        //   （`std::sync::Mutex` **不可重入** ⇒ 拿着它再拿一次 = 自死锁）。
    }
    // ⚠ [2026-10-25 / 审计 N2] 连接没了 ⇒ 它的「accept 备忘」也作废
    //   （否则备忘表随连接数无界增长）。
    memo_clear_for_conn(cid);
    Ok(Outcome::normal(Value::Null))
}

/// `TCP_CLOSE_LISTENER(listener) -> NULL` —— 关闭监听（**幂等**）。
pub fn tcp_close_listener(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "TCP_CLOSE_LISTENER", args.len(), 1));
    }
    let lid = int_arg(host, "TCP_CLOSE_LISTENER", &args[0])?;
    let mut guard = SOCKETS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        if let Some(Entry::Listener(s)) = map.remove(&lid) {
            s.closed.store(true, Ordering::SeqCst);
            s.filled.notify_all();
        }
    }
    Ok(Outcome::normal(Value::Null))
}
