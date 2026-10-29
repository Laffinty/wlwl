//! [v0.11.3 M6 / ADDENDUM-05] `wlwl:std.net` —— 网络(零第三方依赖的最小面)。
//!
//! **本文件目前只含 W-02 的两个纯函数成员。** `HTTP_GET` / `HTTP_POST` /
//! `HTTP_REQUEST` 属 W-03,形态已由 [`ADR-0028`] 定死(`docs/adr/0028-std-net-shape.md`,
//! **Accepted**),但 D1「阻塞原语」是**带条件冻结**的 —— 见该 ADR §6 与守卫测试
//! `impl/crates/wlwl-eval/tests/adr_conditional_guard.rs`。
//!
//! [`ADR-0028`]: ../../../docs/adr/0028-std-net-shape.md
//!
//! ## 为什么这两个成员先落
//!
//! 它们是**纯函数**,与阻塞 / 挂起 / 超时**毫无关系**。W-03 建在 D1 之上;
//! 若 `ADR-0017` Step 3 落地导致 D1 重裁,要改的只有那一层,不是这一批。
//!
//! ## 规范与权威
//!
//! 语法与解析**逐条照 RFC 3986**:分组件走 Appendix B 的 first-match-wins
//! 正则,`URL_JOIN` 走 §5.2.2 / §5.2.3 / §5.2.4 / §5.3 的**伪代码原文**。
//! 契约表把 §5.4 的 **40 条**参考用例**逐字冻结**。
//!
//! ## 「非法 URL」的边界(规范性,2026-10-09)
//!
//! RFC 3986 的解析是**宽容**的 —— 绝大多数字符串都能解析成某种引用。所以
//! 「非法 → `E0030`」必须有一条能写进契约表的边界。本实现的判据是:
//! **不满足 `URI-reference` 的 ABNF(Appendix A)即非法**。逐条为:
//!
//! | 情形 | 结果 |
//! |---|---|
//! | 任何 ASCII 控制字符或空格(0x00–0x20 / 0x7F) | `E0030` |
//! | `:` 出现在首个 `/?#` 之前,但其前缀不是合法 `scheme` | `E0030` |
//! | 裸 `%`(后面不是两个十六进制数字) | `E0030` |
//! | 组件里有超出该组件字符集的字符 | `E0030` |
//! | `[` / `]` 不成对,或出现在 authority 之外 | `E0030` |
//! | `port` 非 `*DIGIT`,或数值 > 65535 | `E0030` |
//! | **空 host**(如 `http://`) | **接受** —— 那是合法的 authority |
//! | **无 scheme**(如 `//g`、`a/b`) | **接受** —— 那是合法的 `relative-ref` |
//!
//! 最后两行是**刻意宽松**的:把它们判成非法会让 `URL_JOIN` 无法处理
//! §5.4 里 `//g`、`""` 这类正常输入。
//!
//! ### 明确**不**校验的东西
//!
//! **IPv6 地址的完整文法。** RFC 3986 §3.2.2 自己就说 IPv6 文法「很难指定」
//! 并把它委派给 RFC 3513。本实现只做**保守**检查:`[...]` 内非空且只含
//! `[0-9A-Fa-f:.]`,或以 `v`/`V` 开头(IPvFuture)。不解析成二进制地址。
//! 这是**已知限制**,不是遗漏。
//!
//! ## `TCP_SERVE`:原生帧内的接受循环(W-12 / B-3 候选 a)
//!
//! `TCP_SERVE(listener, timeout_ms, opts, handler) -> INTEGER` 在**一个原生成员
//! 内部**推进整个接受循环:
//!
//! ```text
//! accept → 读满一次请求 → 调 handler(request) → 写回响应 → 下一条连接
//! ```
//!
//! 它替代「**静态展开的 accept 链**」(连接数写死在源码里的那种):那种写法每
//! 加一条连接就要改一次源码,且「等下一条」这件事在语言层没有位置。
//!
//! ### 契约(规范性)
//!
//! 1. **单条连接异常 ⇒ 记一笔、继续下一条** —— 对端断开 / 读失败 / handler
//!    抛错 / 响应形状不对,**都只终止那一条连接**,**绝不**终止整个循环。
//!    失败以 `ERR([kind:"SocketError", op:"TCP_SERVE", reason])` 的**形状**记录
//!    (落点:宿主的**告警槽**,软告警 ⇒ 不改变程序语义)。
//! 2. **accept 超时 ⇒ 退出循环**,返回**已成功服务的连接数**(不是错误)。调用方
//!    可以用较长的 `timeout_ms`,或自己套一层外层循环重入。
//! 3. **handler 内不得调用宿主挂起点成员**(`TCP_RECV` / `TCP_ACCEPT` /
//!    `SLEEP_ASYNC`)—— handler 跑在**原生帧**里、**不是任务体** ⇒ 没有调度器
//!    可挂。⚠ **本轮不主动报错**(「任务体内判定调用点」那条检查是另一项工作,
//!    尚未落地)⇒ 真的挂起了,落成「这一条连接作废」。
//!
//! ### 形状
//!
//! - **请求**(handler 的实参)= `DICT`:
//!   `{"method": STRING, "path": STRING, "version": STRING, "headers": DICT, "body": STRING}`
//!   —— `headers` 的**键一律小写**;`path` 是**请求目标原样**(可能带查询串);
//!   `body` 是**有损 UTF-8**。
//! - **响应**(handler 的返回值)= `DICT`:
//!   `{"status": INTEGER, "headers": DICT, "body": STRING | ARRAY[INTEGER]}`
//!   —— **三个键缺一即失败**(那一条连接作废,循环继续)。
//! - **失败** = `ERR([kind:"SocketError", op:"TCP_SERVE", reason])` **值**;
//!   **元数 / 类型错** = `E0022` / `E0030` 诊断。
//!
//! ### ⚠ 已知代价(不是缺陷,是「原生帧内推进」这条候选的形状)
//!
//! 原生帧里的等待是**阻塞调用线程** ⇒ `TCP_SERVE` 期间**别的任务跑不了**
//! (与 `HTTP_*` / `SLEEP` 同类)。这与 `TCP_ACCEPT` / `TCP_RECV` 的
//! 「**宿主挂起** ⇒ 别的任务照常运行」**不同** —— 那是「静态 accept 链」那条路的
//! 优点,也是本候选 (a) 的代价。
//!
//! ### `opts`(可选,`DICT` 或 `NULL`)
//!
//! | 键 | 缺省 | 含义 |
//! |---|---|---|
//! | `max_header_bytes` | 64 KiB | 一次请求的**头块**上限 |
//! | `max_body_bytes` | 8 MiB | 一次请求的**体**上限 |
//! | `read_timeout_ms` | `0`(无上限) | 读**一次**请求的预算(与 `TCP_RECV` 的连接读超时**缺省**同口径) |
//!
//! ⚠ **未知选项名 ⇒ `E0030`**（不静默忽略：拼错的上界会变成「没有上界」）。
//! ⚠ `opts` 必须是 `DICT` 或 `NULL` —— WLWL 的 `[]` 是**空数组**，不是空字典。
//!
//! ### ⚠ 与 `TCP_ACCEPT` 抢**同一条**连接队列
//!
//! `TCP_SERVE` 与 `TCP_ACCEPT` 从**同一个**待取队列里取连接 ⇒ 同一个监听上
//! **混用**两者 ⇒ 谁先到谁拿到那条连接。⇒ 一个监听要么交给 `TCP_SERVE`（整条
//! 循环），要么交给 `TCP_ACCEPT`（自己写链），**不要**两边都用。
//!
//! ### 监听的生命周期
//!
//! `TCP_SERVE` **不关**监听器（超时退出后监听**仍然有效**）⇒ 调用方自己
//! `TCP_CLOSE_LISTENER`（**幂等**）。

use crate::tcp::{
    accept_blocking, close_conn_raw, recv_blocking, tcp_accept, tcp_close, tcp_close_listener,
    tcp_listen, tcp_local_addr, tcp_peer, tcp_recv, tcp_send, tcp_set_timeout, write_all_bytes,
    AcceptOutcome, ReadOutcome,
};
use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, Signal, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.net",
    functions: &[
        ("URL_PARSE", url_parse as StdFn),
        ("URL_JOIN", url_join as StdFn),
        ("HTTP_GET", http_get as StdFn),
        ("HTTP_POST", http_post as StdFn),
        ("HTTP_REQUEST", http_request as StdFn),
        // [B-5] **异步**出站：请求跑在**工作线程**上，等待期间**当前任务被
        // 挂起** ⇒ 别的任务（别的 handler）照常跑 ⇒ 出站 I/O **不冻结整个
        // `SCOPE`**。⚠ `HTTP_GET` / `HTTP_POST` / `HTTP_REQUEST` **仍然阻塞**
        //   （既有成员的语义一个字都不改 —— 一致性册「不得修改任一既有成员的
        //   签名或可观察语义」）。
        ("HTTP_REQUEST_ASYNC", http_request_async as StdFn),
        // ── 传输层（TCP）──────────────────────────────────────────────────
        //
        // ⚠ **这 9 个成员不在 v1.0 冻结快照（附录 A）里** ⇒ `stdlib_appendix_a_sync`
        // 的 Guard B 会**恰好红 1 行**（`std.net` 多出这 9 个成员）。
        // 那条红灯是**预期**的，是「规范缺传输层能力」的指纹 —— 不得为了让它变绿
        // 而删成员或改快照。见 docs/gaps/gap-registry.md 的 G-01 / G-05。
        //
        // ⚠ **它们只做字节搬运**：本文件里没有 HTTP 语义（请求行/状态码/头名
        // 一律不认识）—— HTTP 解析与响应构造留在 WLWL 侧。实现见 `tcp.rs`。
        ("TCP_LISTEN", tcp_listen as StdFn),
        ("TCP_ACCEPT", tcp_accept as StdFn),
        ("TCP_RECV", tcp_recv as StdFn),
        ("TCP_SEND", tcp_send as StdFn),
        ("TCP_PEER", tcp_peer as StdFn),
        ("TCP_LOCAL_ADDR", tcp_local_addr as StdFn),
        ("TCP_SET_TIMEOUT", tcp_set_timeout as StdFn),
        ("TCP_CLOSE", tcp_close as StdFn),
        ("TCP_CLOSE_LISTENER", tcp_close_listener as StdFn),
        // ── 原生帧内的接受循环（W-12 / B-3 候选 a）─────────────────────
        //
        // ⚠ **它比上面 9 个成员更「上层」**：那 9 个只搬字节；`TCP_SERVE`
        //   在**原生帧内**推进 accept 循环 ⇒ 它**认识请求/响应的形状**
        //   （方法/路径/头/体）⇒ 那属于**协议语义** ⇒ 住在 `net.rs`，
        //   不住在 `tcp.rs`（后者有机械守卫 `no_http_semantics_in_the_transport_layer`）。
        ("TCP_SERVE", tcp_serve as StdFn),
        // ── 原生帧内的 **Keep-Alive** 循环（W-17 / C-3 / C-8 / C-9）──────────
        //
        // ⚠ **与 `TCP_SERVE` 的分工**（两者**不**互相替代）：
        //   · `TCP_SERVE(listener, …)` = **监听** → 收一条连接 → 服务**一条**请求 → 关；
        //   · `SERVE_STREAM(conn, …)` = **已接到的连接** → 循环服务**多条**请求 → 关。
        // ⇒ 后者**不 accept**、**不关**监听器；它只管**一条**连接。
        //
        // ⚠ **handler 的契约与 `TCP_SERVE` 逐字相同**：handler 跑在**原生帧**里
        //   ⇒ handler 里调宿主挂起点成员（`TCP_RECV` / `TCP_ACCEPT` /
        //   `SLEEP_ASYNC`）⇒ **既有 `E0014`**（W-38 诊断化）。
        //
        // ⚠⚠ **它**不是** `std.net` 的公共面成员** —— 它是 `std.http::SERVE_STREAM`
        //   的**私有内核**（`ADR-0021` 的分层：R1 门面 + R2 内核）⇒ 登记在
        //   `lib.rs` 的 **`wlwl:std.http`** 那一段的 `kernels` 里，形如
        //   `("_SERVE_STREAM", net::tcp_serve_stream)`。
        //   ⇒ **为什么不做成 `std.net` 的公共成员**：
        //   ① 需求清单 C-3 的落点是 **`std.http`**（§6.2 的新成员表里
        //      `std.net` **没有**这一条）⇒ 做成公共成员会**多欠一份规范侧登记**
        //      （成员表 + 附录 A 两处）；
        //   ② 诊断**要指名用户真的调过的那个成员**（规范 §0.3 治理四件套）⇒
        //      公共名是 `std.http::SERVE_STREAM`，诊断里就必须是 `SERVE_STREAM`
        //      ⇒ 若内核叫 `TCP_SERVE_STREAM`，诊断会指名一个**用户没调过**的名字。
    ],
};

// ── HTTP 的三条硬上限(ADR-0028 D3)──────────────────────────────
//
// ⚠ **越限必须是 `ERR` 而不是 OOM / 无界分配** —— 一个 `Content-Length:
// 10 GB` 的响应不该把进程打死,那是**服务端能触发的**。
/// 响应头块上限(字节)。超过即 `ERR`,不继续读到内存里。
const MAX_HEADER_BYTES: usize = 64 * 1024;
/// 响应体上限(字节)。超过即 `ERR`。
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
/// `ADR-0028` D2 的默认超时(毫秒)。调用方可覆盖,但**忘写不会永久挂住**。
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// 允许跳过的 `1xx` 中间响应条数上限(D13-050)。
///
/// ⚠ 为什么要一个上限:`1xx` 是**中间响应**,客户端必须能跳过它继续读下一个
/// 响应(RFC 9112 §15.2)。但「跳过」本身必须**有界** —— 一个只会发 `100` 的
/// 服务端不该把客户端变成永动机。⇒ 超过这个数就 `ERR`。
/// ⚠ 真正的兜底是**总闸**(`Deadline`):每跳一条都要从同一份预算里扣,
/// 所以一个慢速滴流的 `1xx` 服务端照样会在预算耗尽处失败。
const MAX_INFORMATIONAL_RESPONSES: usize = 16;

/// D13-040(N-033):一次 HTTP 请求的**总**时间闸。
///
/// ## 缺陷形态(为什么要有这个类型)
///
/// 旧的 `timeout_ms` 被当成「**每一次系统调用**的时长」而不是「整条请求的上界」:
/// 每个候选地址各拿一份完整 `dur`,`sock.read()` 每轮又各拿一份。于是
///
/// ```text
/// 总时长 = DNS(无上限) + N × dur + Σ(每次 read 的 dur)
/// ```
///
/// 一个解析出 4 个 A 记录、body 又是慢速滴流的主机,能把 30 s 的承诺变成几分钟,
/// 而 `ADR-0028` D2 写的是「超时必填 30 s」—— 那是**整条请求**的承诺。
/// 现在 DNS 之后的每一处(connect / write / 每次 read / flush)都从**同一份剩余预算**里扣。
///
/// ## ⚠ DNS 解析**不在闸内** —— 这是已知缺口,不是已解决
///
/// `ToSocketAddrs::to_socket_addrs` 是**阻塞的系统调用**,标准库**没有**可移植的
/// 取消手段(`getaddrinfo` 要么同步返回要么阻塞;另起线程 + 丢弃句柄解决不了
/// 「线程仍会挂住」这件事,还会泄漏线程)。所以「超时覆盖 DNS」这句**做不到**,
/// 闸从 DNS **返回之后**才开始计。若 DNS 真的挂住,本函数仍会挂住。
/// ⇒ 这是**需要改规范**的一条(见交付说明):要么规范承认这个缺口,
/// 要么规范要求改用可取消的解析器(那会引入第三方依赖,与 D3「零新增依赖」冲突)。
struct Deadline {
    /// 请求开始的那一刻(URL 解析之后、拨号之前)。
    start: std::time::Instant,
    /// 调用方给的**总**预算。
    total: Duration,
}

impl Deadline {
    fn new(total: Duration) -> Self {
        Self {
            start: std::time::Instant::now(),
            total,
        }
    }

    /// 剩余预算。**耗尽即 `None`**。
    ///
    /// ⚠ 这里用 `total.checked_sub(elapsed)` 而不是 `start + total` 再求差:
    /// `timeout_ms` 是用户给的 `u64`,`Instant + Duration` 在 `u64::MAX` 上会
    /// **panic**;而 `Instant::elapsed()` 与 `Duration` 的减法都不溢出。
    fn left(&self) -> Option<Duration> {
        self.total.checked_sub(self.start.elapsed())
    }

    /// 预算耗尽时给 `http_error` 的 `reason`。**刻意带 `timeout` 字样** ——
    /// 调用方看到的 `reason` 必须能区分「总预算用完」与「某个别的 I/O 错误」。
    fn exhausted_reason(&self, doing: &str) -> String {
        format!(
            "{doing}: total request budget of {} ms exhausted (the `timeout` argument is a \
             whole-request cap, not a per-read one — ADR-0028 D2)",
            self.total.as_millis()
        )
    }
}

/// HTTP 失败 → `ERR([kind:"HttpError", op, reason])`**值**,不是原生诊断。
///
/// 理由与 `std.encode` 的 `DecodeError` 同款:网络不通是**运行期可预期的
/// 业务分支**(会重试、会降级、会记日志),不是程序员把参数写错了。把它做成
/// 原生诊断会让整个运行**终止**,调用方连「重试」都做不到。
fn http_error(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("HttpError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

/// 一次 HTTP 请求的**产物** —— **纯数据**（`Send`）。
///
/// ⚠ **为什么不是 `Value`**：`Value` 的字典载荷是 `Rc<DictInner>`
///   （`wlwl-value/src/lib.rs` 的 `Value::Dict`）⇒ **`Value` 不是 `Send`**
///   ⇒ [B-5] 的异步变体**不能**把结果从**工作线程**搬回**求值线程**
///   ⇒ 线程之间只搬**纯数据**，`Value` 在求值线程上现搭
///   （见 [`http_response_value`]）。
#[derive(Debug, Clone)]
pub(crate) struct HttpResponse {
    pub status: i64,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// 把 [`HttpResponse`] 搭成**成员返回值**（`DICT {status, headers, body}`）。
///
/// ⚠ body 的形状**随内容走**（D13-043 / N-036）：合法 UTF-8 ⇒ `STRING`，
///   否则 ⇒ `ARRAY[INTEGER]`（本语言唯一无损字节载体）。
fn http_response_value(r: HttpResponse) -> Value {
    // 🔴 [D13-043(N-036):非 UTF-8 的 body **不再整条 ERR**]——
    //
    // 缺陷形态:`String::from_utf8(body_bytes)` 失败 ⇒ 整条请求 `ERR`。于是
    // 「取一个 PNG / 一个 wasm」这种最普通的用法**完全不可用** —— 这是**可用性**
    // 缺陷,不是安全缺陷(顶多丢一次请求,不会越权、不会越界)。
    //
    // 依据:规范 §2.3 / §11.6 把 `ARRAY[INTEGER]` 定为本语言**唯一无损字节载体**
    // (`STRING` 是 UTF-8,装不了任意字节),而 `std.fs::READ_BYTES` 正是逐字节
    // 无损返回。`HTTP_*` 的 body 是同一个东西,没理由比 `READ_BYTES` 更严。
    //
    // ⚠ **这是规范改动**:规范 §21 的成员表只写了 `{status, headers, body}`,
    //   **从未规定 `body` 的类型** —— 所以实现侧无冲突,但规范必须补一条
    //   「`STRING` 或 `ARRAY[INTEGER]`」才不算自说自话(见交付说明)。
    // ⚠ 形状**随内容走**:合法 UTF-8 ⇒ `STRING`(既有契约一个不改),否则 ⇒
    //   `ARRAY[INTEGER]`。不允许「无脑一律返回数组」—— 那会把每一个文本响应
    //   的调用方全部改成手动重组。
    let body = match String::from_utf8(r.body) {
        Ok(b) => Value::String(b),
        // `FromUtf8Error::into_bytes` 把**原始字节**原样取回 —— 不重扫一遍,
        // 也就不可能在这一步引入有损替换(§11.3-3「不做有损替换」)。
        Err(e) => Value::Array(
            e.into_bytes()
                .into_iter()
                .map(|b| Value::Integer(i64::from(b)))
                .collect(),
        ),
    };
    Value::dict(vec![
        (Value::String("status".into()), Value::Integer(r.status)),
        (
            Value::String("headers".into()),
            Value::dict(
                r.headers
                    .into_iter()
                    .map(|(k, v)| (Value::String(k), Value::String(v)))
                    .collect(),
            ),
        ),
        (Value::String("body".into()), body),
    ])
}

/// RFC 9110 §5.6.2 的 `tchar` —— 头字段名与方法名只能用这些字符。
///
/// ```text
/// token          = 1*tchar
/// tchar          = "!" / "#" / "$" / "%" / "&" / "'" / "*"
///                / "+" / "-" / "." / "^" / "_" / "`" / "|" / "~"
///                / DIGIT / ALPHA
///                ; any VCHAR, except delimiters
/// ```
/// 原文(RFC 9110 §5.6.2 第 15–16 行)把分隔符点名为 `DQUOTE` 与
/// `"(),/:;<=>?@[\]{}"` —— ⚠ `(`, `)`, `,`, `;`, `=` **正在其中**。
/// RFC 7230 §3.2.6 的同一条 ABNF 逐字相同(它是本仓此前引用的出处)。
///
/// ## D13-042(N-035):这里曾是**刻意收紧之前**的形态,多放进五个 sub-delim
///
/// 旧实现写的是 `is_unreserved || is_sub_delim || <16 个标点>`,而
/// `sub-delims`(RFC 3986 Appendix A)比 `tchar` **多出且只多出**
/// `(` `)` `,` `;` `=` 这五个 —— 于是它们被一并放进了 token。
/// ⚠ 这是**收紧**方向的改动(接受集合变小 5 个字符),所以它有可能让既有契约转红;
/// 真红了要**改判契约**(`D13-008`:已改实现而非冻结错误输出),不许回退实现来保绿。
///
/// ⚠ **为什么是收紧而不是放宽**:`field-name = token`(RFC 9110 §5.1)、`method = token`
/// (RFC 9112 §3.1)—— 两处文法都只认 `tchar`。放宽就得自己发明「更友好的 token」,
/// 而分号 / 等号在头字段名里出现正是响应拆分与走私的经典歧义源
/// (RFC 9112 §11.1 / §11.2);**没有规范依据支持放宽**。
/// ⚠ 本机上 Python 3.13 的 `http.client._is_legal_header_name` **不能**当第二实现:
///   它接受除 `:` 外**全部**可见 ASCII(那是 CPython `tokenize` 的 `_TOKEN`,
///   不是 HTTP token),比 RFC **宽得多** —— 故本条**只有 RFC 一个出处**。
fn is_tchar(c: u8) -> bool {
    // 15 个非字母数字 tchar 里,`-` `.` `_` `~` 已由 `is_unreserved` 覆盖
    // (RFC 3986 Appendix A 的 `unreserved` 恰好是这四个),这里只补剩下 11 个。
    is_unreserved(c)
        || matches!(
            c,
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'^' | b'`' | b'|'
        )
}

// ── 诊断助手(与 text.rs 同款)────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected string, got {}", crate::value_kind(got)),
    )
}

fn bad_url(host: &mut dyn StdHost, name: &str, url: &str, why: &str) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: not a valid URI reference: {why}: {url:?}"),
    )
}

fn str_args<'a>(
    host: &mut dyn StdHost,
    name: &str,
    args: &'a [Value],
    want: usize,
) -> wlwl_error::WlwlResult<Vec<&'a str>> {
    if args.len() != want {
        return Err(arity(host, name, args.len(), want));
    }
    let mut out = Vec::with_capacity(want);
    for v in args {
        let Value::String(s) = v else {
            return Err(type_err(host, name, v));
        };
        out.push(s.as_str());
    }
    Ok(out)
}

// ── RFC 3986 字符类(Appendix A)──────────────────────────────────

fn is_alpha(c: u8) -> bool {
    c.is_ascii_alphabetic()
}
fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}
fn is_hex(c: u8) -> bool {
    c.is_ascii_hexdigit()
}
/// `unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~"`
fn is_unreserved(c: u8) -> bool {
    is_alpha(c) || is_digit(c) || matches!(c, b'-' | b'.' | b'_' | b'~')
}
/// `sub-delims = "!" / "$" / "&" / "'" / "(" / ")" / "*" / "+" / "," / ";" / "="`
fn is_sub_delim(c: u8) -> bool {
    matches!(
        c,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}
/// `pct-encoded = "%" HEXDIG HEXDIG` —— 在**非 ASCII**字节上返回 `true`(留给
/// 上层报「非法字符」而不是误判成「百分号不完整」)。
fn is_pct_encoded(s: &[u8], i: usize) -> bool {
    matches!(s.get(i), Some(b'%') if s.get(i + 1).is_some_and(|c| is_hex(*c))
        && s.get(i + 2).is_some_and(|c| is_hex(*c)))
}

/// `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`
/// `query` / `fragment` 在 `pchar` 之外还允许 `/` 与 `?`。
fn scan_component(
    s: &[u8],
    start: usize,
    end: usize,
    extra: &[u8],
    what: &str,
) -> Result<(), String> {
    let mut i = start;
    while i < end {
        let c = s[i];
        if is_pct_encoded(s, i) {
            i += 3;
            continue;
        }
        if is_unreserved(c) || is_sub_delim(c) || c == b':' || c == b'@' || extra.contains(&c) {
            i += 1;
            continue;
        }
        if c.is_ascii_control() || c == b' ' {
            return Err(format!(
                "{what} contains a control character or space (byte {c:#04x})"
            ));
        }
        if c == b'%' {
            return Err(format!("{what} contains a bare `%` (needs two hex digits)"));
        }
        return Err(format!(
            "{what} contains `{}` which is not allowed there (byte {c:#04x})",
            c as char
        ));
    }
    Ok(())
}

// ── 解析结果 ────────────────────────────────────────────────────

/// 一个 URI 引用的五个组件 + authority 的三段拆分。
///
/// ⚠ **`Option::None` 与 `Some("")` 的区别是规范性的**(RFC 3986 §5.3 末段):
/// 分隔符**没出现**是 undefined;分隔符出现了但后面立刻是下一个分隔符或结尾
/// 是 empty。§5.4.2 的 `g?y/./x` 一类用例就依赖这个区别 —— 把 undefined 和
/// empty 混同,query 与 path 就分不开了(§5.2.4 末段的 Note 明写这个坑)。
#[derive(Debug, PartialEq, Eq)]
struct Parts {
    scheme: Option<String>,
    authority: Option<String>,
    userinfo: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}

/// 按 Appendix B 的正则做 first-match-wins 切分,再按 Appendix A 校验。
fn parse(name: &str, url: &str) -> Result<Parts, String> {
    let b = url.as_bytes();
    // 整个引用不含控制字符 / 空格 —— 先扫一遍,让后面每个分支的报错都更准。
    if let Some(c) = b.iter().find(|c| c.is_ascii_control() || **c == b' ') {
        return Err(format!(
            "contains a control character or space (byte {c:#04x})"
        ));
    }

    // ── scheme:首个 `:` 之前不得含 `/?#`,且必须匹配 `scheme` 的 ABNF ──
    let mut scheme: Option<String> = None;
    let mut rest_start = 0usize;
    if let Some(colon) = b.iter().position(|c| *c == b':') {
        if !b[..colon].iter().any(|c| matches!(c, b'/' | b'?' | b'#')) {
            let cand = &b[..colon];
            // `1a:b` 这类:`:` 在首个 `/?#` 之前,却又不是合法 scheme ⇒
            // 它**既不是** `URI`(`scheme` 首位必须是 ALPHA)**也不是**
            // `relative-ref`(`path-noscheme` 首段不得含 `:`)⇒ 整体非法。
            let ok = !cand.is_empty()
                && is_alpha(cand[0])
                && cand[1..]
                    .iter()
                    .all(|c| is_alpha(*c) || is_digit(*c) || matches!(c, b'+' | b'-' | b'.'));
            if !ok {
                return Err(format!(
                    "`{}` before `:` is not a valid scheme (must be ALPHA *(ALPHA / DIGIT / + / - / .))",
                    String::from_utf8_lossy(cand)
                ));
            }
            scheme = Some(String::from_utf8_lossy(cand).into_owned());
            rest_start = colon + 1;
        }
    }

    // ── authority:紧跟 `//` 时存在 ──
    let (authority_raw, mut i) = if b[rest_start..].starts_with(b"//") {
        let s = rest_start + 2;
        let e = s + b[s..]
            .iter()
            .position(|c| matches!(c, b'/' | b'?' | b'#'))
            .unwrap_or(b.len() - s);
        (Some(&url[s..e]), e)
    } else {
        (None, rest_start)
    };

    // ── path:到首个 `?` 或 `#` 为止 ──
    let path_end = b[i..]
        .iter()
        .position(|c| matches!(c, b'?' | b'#'))
        .map(|p| i + p)
        .unwrap_or(b.len());
    let path = url[i..path_end].to_string();
    // path 里不允许 `?` / `#`(已截断)。`pchar` **不含** `/`,但 path 是
    // `segment = *pchar` 由 `/` 连接起来的(`path-abempty = *( "/" segment )`),
    // 所以 `/` 必须**显式**放进 extra —— 漏了它连 `/pub/...` 都过不了。
    scan_component(b, i, path_end, b"/", "path")?;
    i = path_end;

    // ── query / fragment ──
    let query = if b.get(i) == Some(&b'?') {
        let s = i + 1;
        let e = s + b[s..]
            .iter()
            .position(|c| *c == b'#')
            .unwrap_or(b.len() - s);
        scan_component(b, s, e, b"/?", "query")?;
        i = e;
        Some(url[s..e].to_string())
    } else {
        None
    };

    let fragment = if b.get(i) == Some(&b'#') {
        let s = i + 1;
        scan_component(b, s, b.len(), b"/?", "fragment")?;
        Some(url[s..].to_string())
    } else {
        None
    };

    // ── authority 拆分:`[userinfo "@"] host [":" port]` ──
    let (userinfo, host, port) = match authority_raw {
        None => (None, None, None),
        Some(a) => split_authority(name, a)?,
    };

    Ok(Parts {
        scheme,
        authority: authority_raw.map(str::to_string),
        userinfo,
        host,
        port,
        path,
        query,
        fragment,
    })
}

/// authority 拆成三段:`[userinfo "@"] host [":" port]`。
type AuthParts = (Option<String>, Option<String>, Option<u16>);

fn split_authority(name: &str, a: &str) -> Result<AuthParts, String> {
    let _ = name;
    // `userinfo` 不含 `@`,所以至多一个,且它可以在 `:` 之前或之后。
    let (userinfo, hostport) = match a.find('@') {
        Some(at) => (Some(a[..at].to_string()), &a[at + 1..]),
        None => (None, a),
    };
    if let Some(u) = &userinfo {
        scan_component(u.as_bytes(), 0, u.len(), b":", "userinfo")?;
    }

    // host 之后可选 `:port`。
    let (host_s, port_s) = if hostport.starts_with('[') {
        // IP-literal:`[...]` 内不得含 `:port` 分隔的歧义 ⇒ 从**闭合**括号切。
        let close = hostport
            .find(']')
            .ok_or_else(|| "IP-literal is missing its closing `]`".to_string())?;
        let host = &hostport[..=close];
        let tail = &hostport[close + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p),
            None if tail.is_empty() => None,
            None => return Err(format!("unexpected `{tail}` after the IP-literal")),
        };
        (host, port)
    } else {
        match hostport.rfind(':') {
            Some(c) => (&hostport[..c], Some(&hostport[c + 1..])),
            None => (hostport, None),
        }
    };

    // host 校验:IP-literal 或 `reg-name`。
    if host_s.starts_with('[') {
        let inner = &host_s[1..host_s.len() - 1];
        if inner.is_empty() {
            return Err("IP-literal is empty".to_string());
        }
        // 保守检查,不解析成二进制地址(RFC 3986 §3.2.2 自己把 IPv6 文法
        // 委派给了 RFC 3513,并说它「很难指定」)。
        // - IPv6address: 只含十六进制数字 / `:` / `.`
        // - IPvFuture  : `v` 1*HEXDIG `.` 1*( unreserved / sub-delims / ":" )
        let ipv6_ok = inner.bytes().all(|c| is_hex(c) || c == b':' || c == b'.');
        let future_ok = matches!(inner.as_bytes().first(), Some(b'v' | b'V'))
            && inner[1..]
                .bytes()
                .all(|c| is_unreserved(c) || is_sub_delim(c) || c == b':');
        if !ipv6_ok && !future_ok {
            return Err(format!(
                "IP-literal `{host_s}` is not a plausible IPv6 / IPvFuture"
            ));
        }
    } else {
        // `reg-name = *( unreserved / pct-encoded / sub-delims )` —— 刻意不含
        // `:` `@` `[` `]`,与 sub-delims 的边界一致(§7.6 的 userinfo 混淆面)。
        let hb = host_s.as_bytes();
        let mut k = 0;
        while k < hb.len() {
            if is_pct_encoded(hb, k) {
                k += 3;
                continue;
            }
            let c = hb[k];
            if !is_unreserved(c) && !is_sub_delim(c) {
                return Err(format!(
                    "host contains `{c}`, which is not allowed in `reg-name`"
                ));
            }
            k += 1;
        }
    }

    let port = match port_s {
        None => None,
        Some(p) => {
            if !p.bytes().all(is_digit) {
                return Err(format!("port `{p}` is not `*DIGIT`"));
            }
            // 空端口(`http://h:/`)合法且意为「默认端口」⇒ `Some("")` 记 undefined。
            if p.is_empty() {
                None
            } else {
                // 🔴 **D13-020 / N-008:这里曾是一条 panic 面。**
                //
                // `is_digit` 只保证「**全是数字**」,不保证「**放得进 u32**」。
                // `http://a:99999999999999/` 是 14 位合法 `*DIGIT`(RFC 3986 §3.2.2),
                // 它让 `"99999999999999".parse::<u32>()` 返回 `Err`,于是旧的
                // `.expect("all digits")` **当场 panic** —— 纯输入打死整个解释器
                // 进程。规范 §21 表那行「`port` 非 `*DIGIT`,或数值 > 65535 → `E0030`」
                // 要求的是**诊断**,给 panic 是**违反规范**,不是「更严格的校验」。
                //
                // ⇒ 判据仍是**同一条** `exceeds 65535`(不新造错误类别、不静默 clamp):
                //   静默 clamp 会把 `:99999` 变成 `:34463`,调用方拿到一个**看起来
                //   合法**的端口 —— 那比报错坏得多。
                //   只把**真实值**如实带进文案,免得 `:99999999999999` 被报成
                //   `port 0` 之类的不存在值。
                //
                // ⚠ 为什么用 `u64` 而不是 `u128` / `str::parse` 兜底:`u64` 覆盖
                //   20 位十进制数,已远超 `u16` 的 5 位;真正放不下的(> 20 位)那条
                //   走 `Err` 分支,把**原始串**打进文案 —— 同样**不 panic**,
                //   同样落在既有的 `exceeds 65535` 这条路径上。
                match p.parse::<u64>() {
                    Ok(n) if n > 65535 => {
                        return Err(format!("port {n} exceeds 65535"));
                    }
                    Ok(n) => Some(n as u16),
                    // ⚠ `u64` 装不下 ⇒ 数值必然 > 65535,判据没有变。
                    Err(_) => {
                        return Err(format!("port {p} exceeds 65535"));
                    }
                }
            }
        }
    };

    Ok((userinfo, Some(host_s.to_string()), port))
}

// ── RFC 3986 §5.2.3 merge / §5.2.4 remove_dot_segments ──────────

fn merge(base_path: &str, base_has_authority: bool, ref_path: &str) -> String {
    // §5.2.3 第一条:「base **有** authority 且 path 为空」⇒ `"/" + ref_path`。
    // ⚠ 「有 authority」这个前提**不能省**:base 无 authority 且 path 为空时
    // 走第二条(结果就是 `ref_path` 本身,没有那个 `/`)。两条在
    // `URL_JOIN("http://a", "b")` 上分别是 `/b` 与 `b`,差一个斜杠。
    if base_has_authority && base_path.is_empty() {
        return format!("/{ref_path}");
    }
    let cut = base_path.rfind('/').map(|i| i + 1).unwrap_or(0);
    format!("{}{}", &base_path[..cut], ref_path)
}

fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_string();
    let mut out = String::with_capacity(path.len());
    while !input.is_empty() {
        if input.starts_with("../") {
            input.drain(..3); // A: remove that prefix
        } else if input.starts_with("./") {
            input.drain(..2); // A: remove that prefix
                              // B / C: 原文是 **replace** that prefix with "/" —— 是**替换**不是删除。
                              // 写成 drain 会把 "/./x" 变成 "x"(丢了那个斜杠),RFC §5.2.4 那张缓冲表
                              // 第 2B 行立刻对不上:`/a/b/c` + `/./../../g` 应得 `/a/b/c` + `/../../g`。
        } else if input.starts_with("/./") {
            input.replace_range(..3, "/"); // B: replace with "/"
        } else if input == "/." {
            input = "/".to_string();
        } else if input.starts_with("/../") {
            input.replace_range(..4, "/"); // C: replace with "/"
            pop_segment(&mut out);
        } else if input == "/.." {
            input = "/".to_string();
            pop_segment(&mut out);
        } else if input == "." || input == ".." {
            input.clear(); // D: remove
        } else {
            // E: 搬第一个完整路径段(含它前面的 `/`)。
            let end = match input.find('/') {
                None => input.len(),
                Some(0) => input[1..].find('/').map(|i| i + 1).unwrap_or(input.len()),
                Some(i) => i,
            };
            out.push_str(&input[..end]);
            input.drain(..end);
        }
    }
    out
}

/// 从输出缓冲区去掉**最后一个完整段**及其前面的 `/`(§5.2.4 步骤 2C)。
fn pop_segment(out: &mut String) {
    match out.rfind('/') {
        Some(i) => out.truncate(i),
        // 没有 `/` ⇒ 去掉整段。
        None => out.clear(),
    }
}

// ── RFC 3986 §5.2.2 transform + §5.3 recomposition ──────────────

/// 相对引用 → 绝对 URI。**strict 模式**:与 base 同 scheme 的引用**也**按
/// 有 scheme 处理(§5.2.2 里 `if ((not strict) and (R.scheme == Base.scheme))`)。
/// §5.4.2 的 `"http:g"` 一行给了两个结果,strict 侧是 RFC 自己列在**前面**的
/// 那个,故取它;WHATWG URL 取的是另一个(向后兼容),两者**不等价**。
fn resolve(base: &Parts, r: &Parts) -> Parts {
    let (t_scheme, t_authority, t_path, t_query);
    if let Some(s) = &r.scheme {
        t_scheme = Some(s.clone());
        t_authority = r.authority.clone();
        t_path = remove_dot_segments(&r.path);
        t_query = r.query.clone();
    } else if let Some(a) = &r.authority {
        t_authority = Some(a.clone());
        t_path = remove_dot_segments(&r.path);
        t_query = r.query.clone();
        t_scheme = base.scheme.clone();
    } else if r.path.is_empty() {
        t_authority = base.authority.clone();
        t_path = base.path.clone();
        t_query = r.query.clone().or_else(|| base.query.clone());
        t_scheme = base.scheme.clone();
    } else {
        t_authority = base.authority.clone();
        t_path = if r.path.starts_with('/') {
            remove_dot_segments(&r.path)
        } else {
            remove_dot_segments(&merge(&base.path, base.authority.is_some(), &r.path))
        };
        t_query = r.query.clone();
        t_scheme = base.scheme.clone();
    }
    Parts {
        scheme: t_scheme,
        authority: t_authority,
        userinfo: None,
        host: None,
        port: None,
        path: t_path,
        query: t_query,
        fragment: r.fragment.clone(),
    }
}

/// §5.3 Component Recomposition。
fn recompose(p: &Parts) -> String {
    let mut s = String::new();
    if let Some(sc) = &p.scheme {
        s.push_str(sc);
        s.push(':');
    }
    if let Some(a) = &p.authority {
        s.push_str("//");
        s.push_str(a);
    }
    s.push_str(&p.path);
    if let Some(q) = &p.query {
        s.push('?');
        s.push_str(q);
    }
    if let Some(f) = &p.fragment {
        s.push('#');
        s.push_str(f);
    }
    s
}

// ── 成员 ────────────────────────────────────────────────────────

/// `URL_PARSE(url) -> DICT`
///
/// 七个键:`scheme` / `userinfo` / `host` / `port` / `path` / `query` /
/// `fragment`。组件**未出现**是 `NULL`,**出现但为空**是 `""` —— §5.3 末段
/// 明写这个区别是规范性的。
///
/// ⚠ **`userinfo` 是计划外加的一个键。** 计划 §2 列的是六个键,但 RFC 3986
/// §7.6 用 `ftp://cnn.example.com&story=breaking_news@10.0.0.1/` 证明:
/// 悄悄丢掉 userinfo 会让「人类以为 host 是 cnn.example.com」而实际是
/// `10.0.0.1`。解析器把这一段吃掉,等于替调用方**藏起一个已知攻击面**。
///
/// `host` 保留 IPv6 literal 的方括号(`"[::1]"`),好让调用方能无损拼回。
pub fn url_parse(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let url = str_args(host, "URL_PARSE", &args, 1)?[0];
    let p = parse("URL_PARSE", url).map_err(|why| bad_url(host, "URL_PARSE", url, &why))?;
    let opt = |s: &Option<String>| match s {
        None => Value::Null,
        Some(v) => Value::String(v.clone()),
    };
    let dict = vec![
        (Value::String("scheme".into()), opt(&p.scheme)),
        (Value::String("userinfo".into()), opt(&p.userinfo)),
        (Value::String("host".into()), opt(&p.host)),
        (
            Value::String("port".into()),
            match p.port {
                None => Value::Null,
                Some(n) => Value::Integer(i64::from(n)),
            },
        ),
        (Value::String("path".into()), Value::String(p.path.clone())),
        (Value::String("query".into()), opt(&p.query)),
        (Value::String("fragment".into()), opt(&p.fragment)),
    ];
    Ok(wlwl_value::Outcome::normal(Value::dict(dict)))
}

/// `URL_JOIN(base, rel) -> STRING`
///
/// 把 `rel` 按 RFC 3986 §5 解析成以 `base` 为基的绝对引用,**strict 模式**
/// (见本文件 `resolve` 的说明)。不做百分号解码、不做 host 小写化、不做 scheme
/// 规范化 —— 那些是 §6.2 的**语法 / scheme 规范化**,是**另一个语义**。
pub fn url_join(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let v = str_args(host, "URL_JOIN", &args, 2)?;
    let b = parse("URL_JOIN", v[0]).map_err(|why| bad_url(host, "URL_JOIN", v[0], &why))?;
    let r = parse("URL_JOIN", v[1]).map_err(|why| bad_url(host, "URL_JOIN", v[1], &why))?;
    let out = recompose(&resolve(&b, &r));
    Ok(wlwl_value::Outcome::normal(Value::String(out)))
}

// ── HTTP/1.1 客户端(W-03,ADR-0028 D1–D4)────────────────────────

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// 把请求实参收成 `(headers, timeout_ms)`。`args` 是**已经去掉必需部分**的尾部。
fn opt_headers_timeout(
    host: &mut dyn StdHost,
    name: &str,
    args: &[Value],
) -> wlwl_error::WlwlResult<(Vec<(String, String)>, u64)> {
    let mut headers = Vec::new();
    let mut timeout = DEFAULT_TIMEOUT_MS;
    let mut i = 0;
    while i < args.len() {
        match &args[i] {
            Value::Dict(entries) => {
                for (k, v) in entries.iter() {
                    let (Value::String(k), Value::String(v)) = (k, v) else {
                        return Err(type_err(host, name, k));
                    };
                    // 头字段名是 token;值里不许有裸 CR/LF —— 后者就是头注入。
                    let kt = k.as_bytes();
                    if kt.is_empty() || !kt.iter().copied().all(is_tchar) {
                        return Err(host.diag(
                            ErrorCode::E0030,
                            format!("{name}: header name {k:?} is not a valid token"),
                        ));
                    }
                    if v.contains('\r') || v.contains('\n') {
                        return Err(host.diag(
                            ErrorCode::E0030,
                            format!("{name}: header {k:?} value contains CR or LF"),
                        ));
                    }
                    headers.push((k.clone(), v.clone()));
                }
                i += 1;
            }
            Value::Integer(ms) => {
                if *ms < 0 {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("{name}: timeout must be >= 0 ms, got {ms}"),
                    ));
                }
                timeout = *ms as u64;
                i += 1;
            }
            other => return Err(type_err(host, name, other)),
        }
    }
    Ok((headers, timeout))
}

fn method_token(host: &mut dyn StdHost, name: &str, m: &str) -> wlwl_error::WlwlResult<()> {
    if m.is_empty() || !m.as_bytes().iter().copied().all(is_tchar) {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{name}: method {m:?} is not a valid HTTP token"),
        ));
    }
    Ok(())
}

/// HTTP 的**核心**：连、写、读。返回**纯数据** —— 成功是 [`HttpResponse`]，
/// 失败是**失败原因**（`String`）。
///
/// ⚠ **为什么核心层不返回 `Value`**：见 [`HttpResponse`] 的注释
///   （`Value` 含 `Rc` ⇒ 不是 `Send` ⇒ 工作线程搬不动它）。
fn request_raw(
    name: &str,
    method: &str,
    url: &str,
    body: Option<&str>,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
) -> Result<HttpResponse, String> {
    // ⚠ **局部遮蔽**：这一层的 `http_error` 返回**失败原因**（`Err(reason)`），
    //   不是 `ERR(...)` **值** —— 值要留到**求值线程**上才搭（`Value` 不是
    //   `Send`）⇒ 下面那三十来处 `return http_error(name, …)` **一个字都不用改**。
    fn http_error(_op: &str, reason: impl Into<String>) -> Result<HttpResponse, String> {
        Err(reason.into())
    }
    // ── 1. URL ──
    let p = match parse(name, url) {
        Ok(p) => p,
        Err(why) => return http_error(name, format!("invalid URL: {why}")),
    };
    match p.scheme.as_deref() {
        Some("http") => {}
        Some(other) => {
            return http_error(
                name,
                format!(
                    "scheme {other:?} is not supported — plaintext http: only (ADR-0028 \u{00a7}0)"
                ),
            )
        }
        None => return http_error(name, "URL has no scheme; `http:` is required"),
    }
    let Some(host) = p.host.clone() else {
        return http_error(name, "URL has no host");
    };
    // ⚠ [D13-053]原先写 `p.port.map_or(80u16, |n| n)` —— 恒等闭包,clippy 1.99 的
    // `map_or_identity`(由 `-D warnings` 提升为 error)判它可简化成 `unwrap_or`。
    // 两式**语义逐字相同**(`Option<u16>` 的 `map_or` 与 `unwrap_or` 一致)。
    let port = p.port.unwrap_or(80u16);
    // `URL_PARSE` 保留 IPv6 的方括号,这里要的是裸地址。
    let dial_host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    // path 为空时请求行必须是 `/`。
    let target = if p.path.is_empty() {
        match &p.query {
            None => "/".to_string(),
            Some(q) => format!("/?{q}"),
        }
    } else {
        match &p.query {
            None => p.path.clone(),
            Some(q) => format!("{}?{q}", p.path),
        }
    };

    // ── 2. 连接(带超时)──
    //
    // D13-040:总闸从**这里**开始计(URL 解析是纯计算,不计)。⚠ DNS 本身在闸外 ——
    // `to_socket_addrs` 是不可中断的阻塞系统调用,标准库没有可移植的取消手段。
    // 详见 `Deadline` 的文档注释。
    let dur = Duration::from_millis(timeout_ms);
    let deadline = Deadline::new(dur);
    let addrs: Vec<_> = match (dial_host.as_str(), port).to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(e) => return http_error(name, format!("resolve {dial_host}:{port}: {e}")),
    };
    if addrs.is_empty() {
        return http_error(name, format!("{dial_host}:{port} resolved to no address"));
    }
    let mut sock = None;
    let mut last = String::new();
    for a in &addrs {
        // ⚠ 缺陷形态是 `connect_timeout(a, dur)` —— **每个候选地址一份完整 dur**,
        //   N 个地址就把预算放大成 N 倍。这里只给「剩下的那一份」。
        let Some(left) = deadline.left() else {
            return http_error(name, deadline.exhausted_reason("connect"));
        };
        match TcpStream::connect_timeout(a, left) {
            Ok(s) => {
                sock = Some(s);
                break;
            }
            Err(e) => last = e.to_string(),
        }
    }
    let Some(mut sock) = sock else {
        return http_error(name, format!("connect {dial_host}:{port}: {last}"));
    };
    // ⚠ 读 / 写超时**不在这里一次设定死** —— 缺陷形态的 `set_read_timeout(Some(dur))`
    //   是「每次读各有一份完整 dur」的总时长来源。每次 I/O 前重新按剩余预算设定,
    //   见下面 `Deadline::left()` 的三个调用点。
    match deadline.left() {
        Some(left) => {
            let _ = sock.set_read_timeout(Some(left));
            let _ = sock.set_write_timeout(Some(left));
        }
        None => return http_error(name, deadline.exhausted_reason("connect")),
    }

    // ── 3. 写请求 ──
    let host_hdr = if port == 80 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    let mut req = format!("{method} {target} HTTP/1.1\r\nHost: {host_hdr}\r\n");
    for (k, v) in &headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    // 明确关闭连接:这样「无 Content-Length」的响应可以用 EOF 定界,
    // 而不必去解 chunked(ADR-0028 D3:chunked 一律 `ERR`,不解码)。
    req.push_str("Connection: close\r\n\r\n");
    if let Some(b) = body {
        req.push_str(b);
    }
    // D13-040:写也吃总预算。⚠ 缺陷形态只对读做了「一次性 set」,写的超时是
    // 连接建立时那一份 —— 一个不读请求体的服务端可以让写永久阻塞。
    match deadline.left() {
        Some(left) => {
            let _ = sock.set_write_timeout(Some(left));
        }
        None => return http_error(name, deadline.exhausted_reason("write")),
    }
    if let Err(e) = sock.write_all(req.as_bytes()) {
        return http_error(name, format!("write: {e}"));
    }
    if let Err(e) = sock.flush() {
        return http_error(name, format!("flush: {e}"));
    }

    // ── 4. 读响应头(先读到 \r\n\r\n,带上限)──
    //
    // ⚠ **D13-050:这里是一个循环,不是一次读** —— `1xx` 是**中间响应**,不是终响应,
    //   必须**跳过它、继续读下一个响应**(RFC 9112 §15.2:「A client MUST be able
    //   to parse one or more 1xx responses prior to a final response」)。
    //   ⇒ 缺陷形态把 `1xx` 当终响应,于是 `103 Early Hints` 这类服务器
    //     会把**中间响应**交回调用方。
    //   ⇒ 跳过的 `1xx` 头**整块丢弃**(`consumed` 前移),头内容按 RFC 9112
    //     §15.2「客户端可以忽略 1xx 的头」**不解析** —— 定界由**终响应**决定,
    //     所以被跳过的 `1xx` 头不构成走私面。
    let mut raw: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut buf = [0u8; 4096];
    // 已经**消费掉**的字节数(只可能是若干个被跳过的 `1xx` 响应头)。
    let mut consumed = 0usize;
    let mut informational_skipped = 0usize;
    let (head_end, http10, status) = loop {
        // 在 `raw[consumed..]` 里找**这一份**响应头的结束。
        let head_end = loop {
            match find_head_end(&raw[consumed..]) {
                Some(i) => break consumed + i,
                None => {
                    if raw.len() - consumed > MAX_HEADER_BYTES {
                        return http_error(
                            name,
                            format!("response header block exceeds {MAX_HEADER_BYTES} bytes"),
                        );
                    }
                    match read_once(&mut sock, &mut buf, &deadline) {
                        Ok(0) => {
                            return http_error(
                                name,
                                format!(
                                    "connection closed with {} byte(s) of header, no complete head",
                                    raw.len() - consumed
                                ),
                            )
                        }
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                        Err(e) => return http_error(name, format!("read head: {e}")),
                    }
                }
            }
        };
        let head = match std::str::from_utf8(&raw[consumed..head_end]) {
            Ok(h) => h,
            Err(e) => return http_error(name, format!("response header is not valid UTF-8: {e}")),
        };
        // ⚠ 只取第一行(状态行);`lines` 在这里**不留活** —— 它借着 `raw`,
        //   而下一轮循环要可变地写 `raw`(NLL:本轮结束即失效)。
        let status_line = head.split("\r\n").next().unwrap_or("");
        let mut sp = status_line.splitn(3, ' ');
        let version = sp.next().unwrap_or("");
        let status_txt = sp.next().unwrap_or("");
        let _reason = sp.next().unwrap_or("");
        if version != "HTTP/1.1" && version != "HTTP/1.0" {
            return http_error(name, format!("unexpected status line {status_line:?}"));
        }
        let Ok(status) = status_txt.parse::<i64>() else {
            return http_error(
                name,
                format!("status code {status_txt:?} is not an integer"),
            );
        };
        if (100..200).contains(&status) {
            // `1xx` = 中间响应 ⇒ **丢掉这一整块**,接着读下一个响应。
            informational_skipped += 1;
            if informational_skipped > MAX_INFORMATIONAL_RESPONSES {
                return http_error(
                    name,
                    format!(
                        "more than {MAX_INFORMATIONAL_RESPONSES} informational (1xx) responses — refusing to keep reading"
                    ),
                );
            }
            consumed = head_end + 4;
            continue;
        }
        break (head_end, version == "HTTP/1.0", status);
    };

    // ── 5. 解析状态行 + 头 ──
    let head = match std::str::from_utf8(&raw[consumed..head_end]) {
        Ok(h) => h,
        Err(e) => return http_error(name, format!("response header is not valid UTF-8: {e}")),
    };
    let mut lines = head.split("\r\n");
    let _status_line = lines.next().unwrap_or("");

    // ⚠ **D13-050:按状态码定界**(RFC 9112 §6.3 第 1 条)——
    //
    //   「Any response to a HEAD request and any response with a 1xx, 204, or
    //   304 status code is always terminated by the first empty line after the
    //   header fields, **regardless of the header fields present in the
    //   message**, and thus cannot contain a message body or trailer section.」
    //
    // ⇒ `204` / `304` / `HEAD` 的响应 **body 恒空**、**忽略 `Content-Length`**。
    //   缺陷形态是「完全不看状态码」⇒ `204` + `Content-Length: 5` 会把 body
    //   **真的读出来**,而 RFC 说这些响应本就无 body。
    let bodyless = status == 204 || status == 304 || method.eq_ignore_ascii_case("HEAD");

    let mut out_headers: Vec<(String, String)> = Vec::new();
    let mut content_length: Option<u64> = None;
    let mut connection_close = false;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        // obs-fold(行首空白)是**已知的响应拆分/走私向量** ⇒ 拒绝,不合并。
        if line.starts_with(' ') || line.starts_with('\t') {
            return http_error(name, format!("obs-fold in response header: {line:?}"));
        }
        let Some((k, v)) = line.split_once(':') else {
            return http_error(name, format!("malformed header line {line:?}"));
        };
        let v = v.trim();
        if v.contains('\r') || v.contains('\n') {
            return http_error(name, format!("header {k:?} value contains CR or LF"));
        }
        let lk = k.to_ascii_lowercase();
        match lk.as_str() {
            "content-length" => {
                // 多个 Content-Length —— 即便值相同也拒:「同一个头出现两次」
                // 本身就是走私信号,不同实现取第一个或最后一个会分叉。
                if content_length.is_some() {
                    return http_error(name, "duplicate Content-Length header");
                }
                // ⚠ **D13-050:`204` / `304` / `HEAD` ⇒ 忽略 `Content-Length` 的值**,
                //   连「不是数字」都不报 —— 这些响应的 body 恒空,头里的
                //   `Content-Length` **不参与定界**。
                // ⚠ 但**重复出现**仍然拒:那是**头块层面**的走私信号,与状态码无关。
                if !bodyless {
                    let Ok(n) = v.parse::<u64>() else {
                        return http_error(name, format!("Content-Length {v:?} is not a number"));
                    };
                    content_length = Some(n);
                }
            }
            "transfer-encoding" => {
                return http_error(
                    name,
                    format!("Transfer-Encoding {v:?} is not supported — chunked is a smuggling vector, refused rather than decoded (ADR-0028 D3)"),
                );
            }
            "connection" if v.to_ascii_lowercase().contains("close") => {
                connection_close = true;
            }
            _ => {}
        }
        out_headers.push((k.to_string(), v.to_string()));
    }

    // ── 6. 响应体:Content-Length 或 EOF 二者必居其一,否则拒绝 ──
    let mut body_bytes: Vec<u8> = raw[head_end + 4..].to_vec();
    let want = if bodyless {
        // ⚠ **D13-050:`204` / `304` / `HEAD` ⇒ body 恒空**。
        //   `Some(0)` 让下面的循环立刻收工,末尾的 `truncate(0)` 把
        //   「同一段 TCP 里紧跟着的下一个响应字节」一并丢掉。
        //   ⚠ **不看 `Content-Length`** —— 哪怕头里写了非零值。
        Some(0)
    } else {
        match content_length {
            Some(n) => {
                if n > MAX_BODY_BYTES as u64 {
                    return http_error(
                        name,
                        format!("Content-Length {n} exceeds the {MAX_BODY_BYTES} byte cap"),
                    );
                }
                Some(n as usize)
            }
        // ── D13-041(N-034):HTTP/1.0 的 close 定界 ──
        //
        // 缺陷形态只认 `Connection: close` 这**一个**信号,把 HTTP/1.0 的
        // 「默认即 close 定界」整个丢掉了 —— `version` 解析出来只用于
        // §5 的版本校验,之后再无第二次使用。
        //
        // 依据:HTTP/1.0 **没有** keep-alive,RFC 9112 Appendix C.2.2 的原话是
        // 「In HTTP/1.0, each connection is established by the client prior to
        // the request and **closed by the server after sending the response**」;
        // RFC 9112 §6.3 第 8 条(定界的判定优先级,响应专用)也说
        // 「Otherwise, this is a response message without a declared message body
        // length, so the message body length is determined by the number of octets
        // received prior to the server closing the connection」。
        //
        // ⚠ **HTTP/1.1 保持严格,这是刻意的、比 RFC 更紧的一刀**:RFC 9112 §6.3
        // 第 8 条其实**不分版本**,而规范 §21.4 的拒绝表为了反走私要求
        // 「既无 Content-Length 又无 Connection: close → ERR」。本次只给
        // **HTTP/1.0** 开口子(那里 close 定界是**唯一**可能的框架,拒绝它没有
        // 任何安全收益,只会让一堆老服务器不可达);HTTP/1.1 的那条 ERR 契约
        // `no_length_is_err_value` **原样保留**,不顺手放宽。
        // ⚠ 放宽的那一侧仍受 `MAX_BODY_BYTES` 与总闸(`Deadline`)双重约束,
        //   所以「恶意 HTTP/1.0 服务端无限流」不会变成无界分配或永久挂住。
        // ⚠ **D13-050 的 `204` / `304` / `HEAD` 走上面那个 `bodyless` 分支**,
        //   不落到这里 —— 这三条「无 CL 无 close ⇒ ERR」的路径对它们不适用。
        None if connection_close || http10 => None, // 读到 EOF
        None => {
            return http_error(
                name,
                "response has neither Content-Length nor Connection: close — refusing to guess the body length (ADR-0028 D3)",
            )
        }
        }
    };
    loop {
        let done = match want {
            Some(w) => body_bytes.len() >= w,
            None => false,
        };
        if done {
            break;
        }
        if body_bytes.len() > MAX_BODY_BYTES {
            return http_error(
                name,
                format!("response body exceeds {MAX_BODY_BYTES} bytes"),
            );
        }
        match read_once(&mut sock, &mut buf, &deadline) {
            Ok(0) => break,
            Ok(n) => body_bytes.extend_from_slice(&buf[..n]),
            Err(e) => return http_error(name, format!("read body: {e}")),
        }
    }
    if let Some(w) = want {
        // ⚠ **D13-050:`bodyless`(`204` / `304` / `HEAD`)⇒ body 恒空**,
        //   紧跟着响应头的那些字节**不属于**本次 body(那可能是**下一个响应**)
        //   ⇒ 直接丢掉,**不做**「长度不符」的判断。
        //   ⚠ 缺陷形态在这里会报「body truncated: got 5 of 0 byte(s)」——
        //   那正是「`204` 把 body 真的读出来了」的另一种表现。
        if !bodyless && body_bytes.len() != w {
            return http_error(
                name,
                format!("body truncated: got {} of {w} byte(s)", body_bytes.len()),
            );
        }
        body_bytes.truncate(w);
    }

    // ── D13-043(N-036):body 的**形状**规则见 [`http_response_value`] ──
    //
    // ⚠ 这一层只搬**字节**，**不**做 UTF-8 判定（那是求值线程上的事 ——
    //   `Value` 不是 `Send`，工作线程搭不出 `Value`）。
    Ok(HttpResponse {
        status,
        headers: out_headers,
        body: body_bytes,
    })
}

/// HTTP 的**全部实现**（成员层）。返回 `Value` —— 成功是 `DICT`,失败是
/// `ERR(...)` 值。
fn request(
    name: &str,
    method: &str,
    url: &str,
    body: Option<&str>,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
) -> Value {
    match request_raw(name, method, url, body, headers, timeout_ms) {
        Ok(r) => http_response_value(r),
        Err(reason) => http_error(name, reason),
    }
}

/// D13-040(N-033):一次**从总闸里扣**的读。
///
/// 缺陷形态是 `sock.read(buf)` 配一个建连时设死的 `set_read_timeout(Some(dur))`
/// —— 每次读都拿到一份**完整**的 `dur`,于是一个慢速滴流的服务端可以无限续命。
/// 现在每次读之前都把 socket 的读超时**重设**为剩余预算。
///
/// ⚠ 预算耗尽时**不能**去 `set_read_timeout(Some(Duration::ZERO))`:零值在
/// Windows 与 Linux 上都让 `setsockopt` 报 `EINVAL`,而调用点用的是
/// `let _ =`,那个失败会被**静默吞掉** —— 于是「超时」悄悄退化成
/// 「无超时」(这正是缺陷形态本身)。所以先判 `left()`,为 `None` 就直接返回
/// 一个 `TimedOut` 错误,根本不碰 socket 的选项。
fn read_once(sock: &mut TcpStream, buf: &mut [u8], deadline: &Deadline) -> std::io::Result<usize> {
    let Some(left) = deadline.left() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            deadline.exhausted_reason("read"),
        ));
    };
    let _ = sock.set_read_timeout(Some(left));
    sock.read(buf)
}

fn find_head_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn http_get(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.is_empty() || args.len() > 3 {
        return Err(arity(host, "HTTP_GET", args.len(), 1));
    }
    let Value::String(url) = &args[0] else {
        return Err(type_err(host, "HTTP_GET", &args[0]));
    };
    let (headers, timeout) = opt_headers_timeout(host, "HTTP_GET", &args[1..])?;
    Ok(wlwl_value::Outcome::normal(request(
        "HTTP_GET", "GET", url, None, headers, timeout,
    )))
}

pub fn http_post(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() < 2 || args.len() > 4 {
        return Err(arity(host, "HTTP_POST", args.len(), 2));
    }
    let Value::String(url) = &args[0] else {
        return Err(type_err(host, "HTTP_POST", &args[0]));
    };
    let Value::String(body) = &args[1] else {
        return Err(type_err(host, "HTTP_POST", &args[1]));
    };
    let (headers, timeout) = opt_headers_timeout(host, "HTTP_POST", &args[2..])?;
    Ok(wlwl_value::Outcome::normal(request(
        "HTTP_POST",
        "POST",
        url,
        Some(body),
        headers,
        timeout,
    )))
}

pub fn http_request(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() < 2 || args.len() > 5 {
        return Err(arity(host, "HTTP_REQUEST", args.len(), 2));
    }
    let Value::String(method) = &args[0] else {
        return Err(type_err(host, "HTTP_REQUEST", &args[0]));
    };
    let Value::String(url) = &args[1] else {
        return Err(type_err(host, "HTTP_REQUEST", &args[1]));
    };
    method_token(host, "HTTP_REQUEST", method)?;
    let mut body: Option<&str> = None;
    let mut i = 2;
    if let Some(Value::String(b)) = args.get(2) {
        body = Some(b);
        i = 3;
    }
    let (headers, timeout) = opt_headers_timeout(host, "HTTP_REQUEST", &args[i..])?;
    Ok(wlwl_value::Outcome::normal(request(
        "HTTP_REQUEST",
        method,
        url,
        body,
        headers,
        timeout,
    )))
}

// ── B-5:异步出站操作(工作线程 + 就绪集合 + 幂等备忘)────────────────────
//
// ## 它是什么
//
// `HTTP_REQUEST_ASYNC` / `PROCESS_RUN_ASYNC` 两个成员**共用**这一层：
// **把一次出站操作放到工作线程上跑**，**挂起当前任务**（不是阻塞整个运行时）
// ⇒ 别的任务（别的 handler）在这段时间里**照常跑**。
//
// ## 三条必须写下来的语义
//
// 1. **不冻结整个 `SCOPE`**：等待期间**别的任务照常运行** ⇒ 出站 I/O 不再把
//    整个运行时停摆（那是 `HTTP_REQUEST` / `PROCESS_RUN` 的既有代价，本轮
//    **一个字都不改**）。
// 2. **段首重放 ⇒ 必须幂等**（并发册 §17.0）：宿主挂起 = **不切段**格 ⇒ 恢复时
//    **从挂起点那句话起整段重跑一遍** ⇒ 若不记住「这个任务已经发起过什么」，
//    **同一个请求会被发起两次** ⇒ 幂等备忘（[`async_op::run`] 里的 `MEMO`）
//    是**硬要求**，不是优化。
// 3. **产物必须是 `Send` 的纯数据**：`Value` 的字典载荷是 `Rc<DictInner>`
//    ⇒ `Value` 不是 `Send` ⇒ 工作线程**搬不动**它 ⇒ 线程之间只搬纯数据，
//    `Value` 在**求值线程**上现搭（[`async_op::payload_to_value`]）。
//
// ## 两条唤醒路径
//
// - **事件驱动**（宿主支持 [`YieldReason::AsyncReady`] 时）：任务挂在
//   `AsyncReady(操作 id)` 上 ⇒ 工作线程完成 ⇒ 推进**就绪集合** ⇒ 宿主把它
//   唤醒 ⇒ 等待延迟 ≈ 0。接线需求见交付的「登记清单」。
// - **定时器轮询**（宿主**还没**接上那一族时的回退）：任务挂在
//   `SleepUntil(绝对时刻)` 上 ⇒ 定时器到点把它叫醒 ⇒ 成员再看一眼产物。
//   ⇒ 延迟 ≤ 轮询间隔（起步 5 ms、翻倍、封顶 200 ms）。
//
// ⚠ 两条路径**只影响延迟**，**不影响语义**（幂等、结果形状、失败口径都一样）。

/// 一次异步出站操作的**产物** —— **纯数据**（`Send`）。
#[derive(Debug, Clone)]
pub(crate) enum Payload {
    /// 一次 HTTP 请求（`op` = 成员名 ⇒ 失败值的 `op` 字段）。
    Http {
        op: &'static str,
        r: Result<HttpResponse, String>,
    },
    /// 一次子进程运行（`op` = 成员名 ⇒ 失败值的 `op` 字段）。
    Proc {
        op: &'static str,
        r: Result<crate::process::ProcessOutcome, String>,
    },
    /// 工作线程**panic**了（兜底用 —— 见 `async_op::issue` 的注）。
    Failed { op: &'static str, reason: String },
}

/// 异步出站成员的**共用台账**：工作线程、就绪集合、幂等备忘。
///
/// ⚖ **`pub` 的理由**：求值器侧（`wlwl-eval`）的「异步就绪泵」要读/消费这里的
///   就绪集合（`peek_ready` / `retire_ready` / `await_ready`）—— 它**不能**碰
///   调度器（解释器的值模型是 `Rc`，不是 `Send`）⇒ 那是**宿主 I/O 线程 →
///   解释器**的通道，与 `wlwl_std::tcp` 的就绪队列**同款**。
///   ⚠ 本模块里**带私有类型的**那些项（`run` / `Payload` / …）一律是
///   `pub(crate)` ⇒ 不构成对外的私有类型泄漏。
pub mod async_op {
    use super::{http_error, http_response_value, Payload};
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::{Arc, Condvar, LazyLock, Mutex};
    use std::time::Duration;
    use wlwl_value::{Outcome, Signal, StdHost, Value, YieldReason};

    /// 轮询回退的**起始**间隔（毫秒）。
    const POLL_START_MS: i64 = 5;
    /// 轮询回退的**上限**间隔（毫秒）。
    const POLL_MAX_MS: i64 = 200;

    /// 一次异步操作的**状态**。
    struct OpState {
        /// 完成后的产物（`None` = 还在跑）。
        result: Option<Payload>,
        /// 产物**已经交给程序**了。
        delivered: bool,
        /// 本任务在这个操作上**挂起过** ⇒ 交付时清恢复提示。
        parked: bool,
        /// 下一次轮询的间隔（回退路径的退避）。
        poll_ms: i64,
    }

    /// 一次异步操作。
    struct Op {
        id: i64,
        state: Mutex<OpState>,
    }

    /// **幂等备忘**的键：`(任务键, 实参指纹)`。
    type MemoKey = (Option<i64>, String);

    /// **幂等备忘**：`(任务键, 签名)` → 本次操作。
    ///
    /// ⚠ **键是「任务键 + 实参指纹」**：任务键（`host.current_task_key()`）把
    ///   **两个任务**的同一次调用分开；实参指纹把**同一个任务**里的**两次不同
    ///   调用**分开。
    /// ⚠ **跨求值器的已知边界**（与 `TCP_ACCEPT` 的备忘同族）：任务键只在
    ///   **一个求值器内**唯一 ⇒ 同一进程里跑**两个求值器**、且**实参完全
    ///   相同**时，两边的操作会共用同一格 ⇒ 第二个求值器会拿到第一个的产物。
    ///   ⇒ 实参指纹把这条边界**收窄**到「实参逐字相同」这一种情形。
    static MEMO: LazyLock<Mutex<HashMap<MemoKey, Arc<Op>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    /// **就绪集合**：已完成、尚未被观察的操作 id。
    ///
    /// ⚠ 这是**电平**（level）而不是**边沿**（edge）：「这个操作**已经**完成」
    ///   是**持续成立**的状态，直到它被消费。与 `tcp::READY` 同款。
    static READY: LazyLock<Mutex<HashSet<i64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

    /// 就绪集合的「有东西了」信号（条件变量 ⇒ 等待延迟 ≈ 0）。
    static READY_CV: Condvar = Condvar::new();

    /// 操作 id 的发号器。
    ///
    /// ⚠ **起点刻意很高**（`1 << 32`）—— 与 **socket id** 空间错开。
    ///   为什么：`resume_hint` 是**一个** `Option<i64>` 槽 ⇒ socket 等待与异步
    ///   等待**共用**它 ⇒ 若两个空间的 id 撞上，成员就分不清「这个提示是
    ///   socket 的还是异步操作的」⇒ 异步操作 id 从一个**很高的**起点发号。
    static NEXT_ID: AtomicI64 = AtomicI64::new(1 << 32);

    /// 把操作 id 标记为就绪（由**工作线程**调用）。
    fn push_ready(id: i64) {
        let mut q = READY.lock().unwrap_or_else(|e| e.into_inner());
        q.insert(id);
        drop(q);
        // ⚠ 必须**在放锁之后**通知（与 `tcp::push_ready` 同款）：否则被唤醒的
        //   线程可能抢在 insert 之前拿到锁、看到空集合又睡回去 ⇒ **丢通知**。
        READY_CV.notify_all();
    }

    /// **看一眼**当前就绪的操作 id（**不消费**）。
    ///
    /// ⚠ **宿主侧的泵**（求值器接线）用这三个函数把「工作线程完成」翻译成
    ///   「唤醒任务」⇒ 与 `wlwl_std::tcp` 的就绪队列**同款**通道。
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
    /// ⚠ **不看集合本身空不空** —— 它是**电平**，可能一直非空 ⇒ 用它当
    ///   「有更新了」的信号会变成忙等（与 `tcp::await_ready` 同款）。
    pub fn await_ready(timeout: Duration) {
        let q = READY.lock().unwrap_or_else(|e| e.into_inner());
        let _ = READY_CV
            .wait_timeout(q, timeout)
            .unwrap_or_else(|e| e.into_inner());
    }

    /// 把产物搭成**成员返回值**（在**求值线程**上跑 ⇒ 这里才碰 `Rc`）。
    fn payload_to_value(p: &Payload) -> Value {
        match p {
            Payload::Http { op, r } => match r {
                Ok(resp) => http_response_value(resp.clone()),
                Err(reason) => http_error(op, reason.clone()),
            },
            Payload::Proc { op, r } => match r {
                Ok(out) => crate::process::process_outcome_value(out.clone()),
                Err(reason) => crate::process::proc_err(op, reason.clone()),
            },
            Payload::Failed { op, reason } => Value::Err(Box::new(Value::dict(vec![
                (
                    Value::String("kind".into()),
                    Value::String("AsyncError".into()),
                ),
                (Value::String("op".into()), Value::String((*op).into())),
                (
                    Value::String("reason".into()),
                    Value::String(reason.clone()),
                ),
            ]))),
        }
    }

    /// 起一个异步操作：**工作线程**跑 `work`，完成后把产物放进槽、
    /// 推进就绪集合。
    fn issue(
        key: MemoKey,
        op_name: &'static str,
        work: impl FnOnce() -> Payload + Send + 'static,
    ) -> Arc<Op> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let op = Arc::new(Op {
            id,
            state: Mutex::new(OpState {
                result: None,
                delivered: false,
                parked: false,
                poll_ms: POLL_START_MS,
            }),
        });
        {
            let mut memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
            memo.insert(key, op.clone());
        }
        let worker = op.clone();
        std::thread::spawn(move || {
            // ⚠ **工作线程不许把 panic 带进台账**：panic ⇒ 产物永远不到 ⇒ 任务会
            //   **一直轮询下去**（挂死）⇒ 兜住它，当成一次失败。
            let payload = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
                Ok(p) => p,
                Err(boxed) => {
                    let reason = match boxed.downcast_ref::<&str>() {
                        Some(s) => (*s).to_string(),
                        None => match boxed.downcast_ref::<String>() {
                            Some(s) => s.clone(),
                            None => "the worker thread panicked".to_string(),
                        },
                    };
                    Payload::Failed {
                        op: op_name,
                        reason,
                    }
                }
            };
            {
                let mut st = worker.state.lock().unwrap_or_else(|e| e.into_inner());
                st.result = Some(payload);
            }
            // ⚠ 完成 ⇒ 推进就绪集合（事件驱动那条路的**唤醒源**）。
            push_ready(id);
        });
        op
    }

    /// 把当前任务**挂起**在这次异步操作上。
    fn park(host: &mut dyn StdHost, op: &Op) -> Outcome {
        // ① 宿主支持「异步就绪」这一族 ⇒ 挂在新变体上（完成时由宿主唤醒）。
        if host.supports_async_ready_park() {
            return Outcome::with_signal(
                Value::Null,
                Signal::Yield(YieldReason::AsyncReady(op.id)),
            );
        }
        // ② 回退：**定时器轮询**（宿主还没接上那一族时的形态）。
        //
        // ⚠ **退避**：间隔从 5 ms 起步、每次翻倍、封顶 200 ms ⇒
        //   - 快请求**立刻**交付（第一次轮询就在 5 ms 之后）；
        //   - 慢请求**少**几次重放（段首重放 ⇒ 每次重放都重跑这一段 ⇒
        //     重放**不是**免费的）。
        let wait = {
            let mut st = op.state.lock().unwrap_or_else(|e| e.into_inner());
            let w = st.poll_ms;
            st.poll_ms = (st.poll_ms * 2).min(POLL_MAX_MS);
            w
        };
        let at = wlwl_value::real_mono_ms().saturating_add(wait);
        Outcome::with_signal(Value::Null, Signal::Yield(YieldReason::SleepUntil(at)))
    }

    /// 异步出站成员的**共用流程**。
    ///
    /// ⚠ **幂等是硬要求**（并发册 §17.0）：宿主挂起后恢复时，挂起点被**再次
    ///   求值** ⇒ 从挂起点那句话起**整段重跑一遍** ⇒ 若不记住「这个任务已经
    ///   发起过什么」，**同一个请求会被发起两次**。
    ///
    /// - `signature` = 实参的指纹（**同一次调用重跑时逐字相同**）；
    /// - `work` = 真正干活的那段（**只在发起那一次跑**）。
    pub(crate) fn run(
        host: &mut dyn StdHost,
        op_name: &'static str,
        signature: String,
        work: impl FnOnce() -> Payload + Send + 'static,
    ) -> wlwl_error::WlwlResult<Outcome> {
        let task_key = host.current_task_key();
        let resume = host.resume_hint();
        let key = (task_key, signature);

        // ① 备忘：这个任务在这个签名上**已经发起过**什么？
        let existing = {
            let memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
            memo.get(&key).cloned()
        };
        if let Some(op) = existing {
            let mut st = op.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.delivered {
                // ①a 上一次的结果**已经交付**给程序了。
                //    - 本次是**重放**（恢复提示在）⇒ 把同一个结果**再交一次**
                //      （**绝不重发**）；
                //    - 否则 ⇒ 这是**新的一次调用** ⇒ 落到 ③（另起一个操作）。
                if resume.is_some() {
                    if let Some(p) = st.result.clone() {
                        drop(st);
                        retire_ready(op.id);
                        return Ok(Outcome::normal(payload_to_value(&p)));
                    }
                }
            } else {
                // ② 还没交付 ⇒ 我**就是**那一次调用（**绝不重复发起**）。
                if let Some(p) = st.result.clone() {
                    st.delivered = true;
                    let was_parked = st.parked;
                    drop(st);
                    if was_parked {
                        // 我**就是**挂起点 ⇒ 本次求值完成 ⇒ 清掉恢复提示
                        //（不清 ⇒ 后面的挂起点成员看不到提示 ⇒ 活锁）。
                        host.clear_resume_hint();
                    }
                    retire_ready(op.id);
                    return Ok(Outcome::normal(payload_to_value(&p)));
                }
                // 还在跑 ⇒ 挂起（**绝不重复发起**）。
                st.parked = true;
                drop(st);
                return Ok(park(host, &op));
            }
        }

        // ③ 新的一次调用 ⇒ 起工作线程。
        let op = issue(key, op_name, work);
        {
            let mut st = op.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(p) = st.result.clone() {
                // 极快的操作**已经**完成 ⇒ 直接交付（不必白挂一次）。
                st.delivered = true;
                drop(st);
                retire_ready(op.id);
                return Ok(Outcome::normal(payload_to_value(&p)));
            }
            st.parked = true;
        }
        Ok(park(host, &op))
    }
}

/// `HTTP_REQUEST_ASYNC(method, url, body?, headers?, timeout_ms?) -> DICT`
///
/// **异步**版的 [`http_request`]：请求在**工作线程**上跑，等待期间**当前任务
/// 被挂起**（不是阻塞整个运行时）⇒ **别的任务照常运行**。
///
/// - 返回值与 `HTTP_REQUEST` **同形**：`DICT {status, headers, body}`；
///   运行期失败 ⇒ `ERR([kind:"HttpError", op, reason])`**值**（不是诊断）。
/// - 实参形状与 `HTTP_REQUEST` **逐字同形**（`body` 可选、其后是可选的
///   `headers` 字典 / `timeout_ms` 整数，顺序不限）。
/// - ⚠ **元数 / 类型错 ⇒ `E0022` / `E0030`**（既有码；本轮**不新增诊断码**）。
///
/// ## 与 `HTTP_REQUEST` 的差别（**唯一**的一条）
///
/// `HTTP_REQUEST` **阻塞调用线程** ⇒ 整个 `SCOPE` 停摆；本成员**挂起当前任务**
/// ⇒ 别的 handler 照常服务。**既有成员的语义一个字都不改**（一致性册：
/// 「不得修改任一既有成员的签名或可观察语义」）。
pub fn http_request_async(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "HTTP_REQUEST_ASYNC";
    if args.len() < 2 || args.len() > 5 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let Value::String(method) = &args[0] else {
        return Err(type_err(host, NAME, &args[0]));
    };
    let Value::String(url) = &args[1] else {
        return Err(type_err(host, NAME, &args[1]));
    };
    method_token(host, NAME, method)?;
    let mut body: Option<&str> = None;
    let mut i = 2;
    if let Some(Value::String(b)) = args.get(2) {
        body = Some(b);
        i = 3;
    }
    let (headers, timeout) = opt_headers_timeout(host, NAME, &args[i..])?;
    // 幂等用的**实参指纹**（同一次调用重跑时逐字相同）。
    let signature = format!(
        "{NAME}|{method}|{url}|{}|{headers:?}|{timeout}",
        body.unwrap_or("")
    );
    // 工作线程要**拥有**实参 ⇒ 这里**复制**一份（`Value` 那边没有 `Clone`
    // 负担：这些都是 `String`）。
    let m = method.to_string();
    let u = url.to_string();
    let b = body.map(str::to_string);
    async_op::run(host, NAME, signature, move || Payload::Http {
        op: NAME,
        r: request_raw(NAME, &m, &u, b.as_deref(), headers, timeout),
    })
}

// ── TCP_SERVE:原生帧内的接受循环(W-12 / B-3 候选 a)────────────────────
//
// ## 它是什么
//
// `TCP_SERVE(listener, timeout_ms, opts, handler) -> INTEGER` 在**一个原生成员
// 内部**推进整个接受循环：
//
// ```text
// accept → 读满一次请求 → 调 handler(request) → 写回响应 → 下一条连接
// ```
//
// ⇒ **替代**「静态展开的 accept 链」（`TCP_ACCEPT` 写死在源码里、连接数写死）。
//
// ## 三条必须写下来的语义
//
// 1. **单条连接异常 ⇒ 记一笔、继续下一条**（对端断开 / 读失败 / handler 抛错
//    / 响应形状不对）—— **绝不**终止整个循环。
// 2. **accept 超时 ⇒ 退出循环**，返回**已成功服务的连接数**。调用方可用较长的
//    `timeout_ms`，或自己套一层外层循环重入。
// 3. **handler 内不得调用宿主挂起点成员**（`TCP_RECV` / `TCP_ACCEPT` /
//    `SLEEP_ASYNC`）—— handler 跑在**原生帧**里、**不是任务体** ⇒ 没有调度器
//    可挂。⚠ **本轮不主动报错**（「任务体内判定调用点」那条检查是另一项工作，
//    尚未落地）⇒ 真的挂起了，落成「这一条连接作废」。
//
// ## ⚠ 它**阻塞整个运行时**
//
// 原生帧里的等待是**阻塞调用线程**（不是 `TCP_ACCEPT` / `TCP_RECV` 的
// 「宿主挂起」⇒ 别的任务照常运行）⇒ `TCP_SERVE` 期间**别的任务跑不了**。
// 这是「原生帧内推进循环」这条候选 (a) 的**已知代价**，与 `HTTP_*` / `SLEEP`
// 同类（它们也冻结整个 `SCOPE`）。
//
// ## 形状
//
// - 请求（给 handler 的实参）= `DICT`：
//   `{"method": STRING, "path": STRING, "version": STRING, "headers": DICT, "body": STRING}`
//   —— `headers` 的**键一律小写**（查表不用猜大小写）；`body` 是**有损 UTF-8**
//   （二进制体不可靠 ⇒ 要字节请走别的路）。
// - 响应（handler 的返回值）= `DICT`：
//   `{"status": INTEGER, "headers": DICT, "body": STRING | ARRAY[INTEGER]}`
//   —— **三个键缺一即失败**（见下面第 1 条）。
// - 失败 = `ERR([kind:"SocketError", op:"TCP_SERVE", reason])` **值**（不是诊断）。
//
// ## 为什么住在 `net.rs` 而不是 `tcp.rs`
//
// `tcp.rs` 有机械守卫 `no_http_semantics_in_the_transport_layer`（**不许**出现
// HTTP / GET / POST / Content-Length / status 这些字样）⇒ 那 9 个传输层成员
// 只搬字节。`TCP_SERVE` **认识请求与响应的形状** ⇒ 它属于协议语义 ⇒ 住这里。

/// `TCP_SERVE` 的可选项（缺省值见 [`serve_opts`]）。
struct ServeOpts {
    /// 一次请求的**头块**上限（字节）。
    max_header_bytes: usize,
    /// 一次请求的**体**上限（字节）。
    max_body_bytes: usize,
    /// 读**一次**请求的预算（毫秒）；`0` = **无上限**。
    read_timeout_ms: u64,
}

/// `TCP_SERVE` 的运行期失败 ⇒ `ERR([kind:"SocketError", op:"TCP_SERVE", reason])`。
///
/// ⚠ **形状与 `tcp.rs` 的 `socket_error` 同款**（`kind` / `op` / `reason`）——
///   但 `op` 恒为 `"TCP_SERVE"`。
/// ⚠ **元数 / 类型错不走这里** ⇒ 那些是 `E0022` / `E0030` 诊断。
fn serve_error(reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("SocketError".into()),
        ),
        (
            Value::String("op".into()),
            Value::String("TCP_SERVE".into()),
        ),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

/// 整数实参（类型错 ⇒ `E0030`）。
fn int_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<i64, wlwl_error::WlwlError> {
    let Value::Integer(n) = v else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{name}: expected integer, got {}", crate::value_kind(v)),
        ));
    };
    Ok(*n)
}

/// 类型错 ⇒ `E0030`（带**期望**措辞）。
fn serve_type_err(
    host: &mut dyn StdHost,
    name: &str,
    want: &str,
    got: &Value,
) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

/// 解析 `TCP_SERVE` 的可选项。
///
/// ⚠ **缺省值**：
/// - `max_header_bytes` ⇒ [`MAX_HEADER_BYTES`]（64 KiB，与 HTTP 客户端同一顶）；
/// - `max_body_bytes` ⇒ [`MAX_BODY_BYTES`]（8 MiB，同上）；
/// - `read_timeout_ms` ⇒ `0` = **无上限**（与 `TCP_RECV` 的连接读超时
///   **缺省**同口径：只有**显式** `TCP_SET_TIMEOUT` 才给读设上界）。
///
/// ⚠ **未知选项名 ⇒ `E0030`**（不静默忽略：拼错的上界会变成「没有上界」）。
fn serve_opts(host: &mut dyn StdHost, v: &Value) -> wlwl_error::WlwlResult<ServeOpts> {
    let mut opts = ServeOpts {
        max_header_bytes: MAX_HEADER_BYTES,
        max_body_bytes: MAX_BODY_BYTES,
        read_timeout_ms: 0,
    };
    // ⚠ `opts` 认**两种**字典表示（普通 `DICT` 与字典累加器）—— 否则累加器
    //   会被判成「类型不对」，而它的类型名也是 `"dict"` ⇒ 报错会指向错处。
    let Some(entries) = dict_entries_of(v) else {
        if matches!(v, Value::Null) {
            return Ok(opts);
        }
        return Err(serve_type_err(
            host,
            "TCP_SERVE",
            "a DICT of options or NULL",
            v,
        ));
    };
    for (k, val) in entries.iter() {
        let Value::String(k) = k else {
            return Err(serve_type_err(host, "TCP_SERVE", "a STRING option name", k));
        };
        match k.as_str() {
            "max_header_bytes" => {
                let n = int_arg(host, "TCP_SERVE", val)?;
                if n <= 0 {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("TCP_SERVE: max_header_bytes must be positive, got {n}"),
                    ));
                }
                opts.max_header_bytes = n as usize;
            }
            "max_body_bytes" => {
                let n = int_arg(host, "TCP_SERVE", val)?;
                if n < 0 {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("TCP_SERVE: max_body_bytes must not be negative, got {n}"),
                    ));
                }
                opts.max_body_bytes = n as usize;
            }
            "read_timeout_ms" => {
                let n = int_arg(host, "TCP_SERVE", val)?;
                if n < 0 {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("TCP_SERVE: read_timeout_ms must not be negative, got {n}"),
                    ));
                }
                opts.read_timeout_ms = n as u64;
            }
            other => {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!(
                        "TCP_SERVE: unknown option {other:?} (expected one of \
                         `max_header_bytes`, `max_body_bytes`, `read_timeout_ms`)"
                    ),
                ))
            }
        }
    }
    Ok(opts)
}

/// `TCP_SERVE(listener, timeout_ms, opts, handler) -> INTEGER`
///
/// 返回**成功服务**的连接数（写回了响应、连接正常收尾的那几条）。
///
/// ⚠ **单条连接失败不计入返回值**，但会被**记一笔**（见 `record_serve_failure`）。
/// ⚠ **accept 超时 ⇒ 退出循环**并返回计数（不是错误）。
/// ⚠ **监听句柄不对 ⇒ 返回 `ERR([kind:"SocketError", op:"TCP_SERVE", reason])`**
///   （这是**调用点**的错误，不是某一条连接的失败）。
/// ⚠ **元数 / 类型错 ⇒ `E0022` / `E0030`**（诊断，不是 `ERR` 值）。
pub fn tcp_serve(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 4 {
        return Err(arity(host, "TCP_SERVE", args.len(), 4));
    }
    let lid = int_arg(host, "TCP_SERVE", &args[0])?;
    let timeout_ms = int_arg(host, "TCP_SERVE", &args[1])?;
    if timeout_ms < 0 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("TCP_SERVE: expected a non-negative timeout in ms, got {timeout_ms}"),
        ));
    }
    let opts = serve_opts(host, &args[2])?;
    // ⚠ handler 必须**可调用** ⇒ 类型错 ⇒ `E0030`（不是运行期失败）。
    match &args[3] {
        Value::Closure { .. } | Value::NativeFn { .. } => {}
        other => {
            return Err(serve_type_err(
                host,
                "TCP_SERVE",
                "a function handler",
                other,
            ));
        }
    }
    let handler = args[3].clone();

    // ⚠ `timeout_ms == 0` = **无截止时间**（与 `TCP_ACCEPT` 同口径：那是
    //   「那一次等待的截止时间」，`0` = 无截止时间）。
    let budget = if timeout_ms == 0 {
        None
    } else {
        Some(Duration::from_millis(timeout_ms as u64))
    };

    let mut served = 0i64;
    loop {
        match accept_blocking(lid, budget) {
            Ok(AcceptOutcome::Conn(cid)) => match serve_one(host, &handler, cid, &opts) {
                Ok(()) => served += 1,
                // ⚠ **记一笔、继续下一条** —— 绝不终止整个循环。
                Err(reason) => record_serve_failure(host, &reason),
            },
            // ⚠ **超时即退出循环**（返回已服务连接数）；监听被关掉也一样收工。
            Ok(AcceptOutcome::TimedOut) | Ok(AcceptOutcome::Closed) => break,
            // ⚠ 句柄不对 ⇒ **调用点**的错误 ⇒ 直接回 `ERR` 值。
            Err(why) => return Ok(Outcome::normal(serve_error(why))),
        }
    }
    Ok(Outcome::normal(Value::Integer(served)))
}

/// 服务**一条**连接：读一次请求 → 调 handler → 写回响应。
///
/// ⚠ **无论成败都关掉这条连接**（⇒ 不泄漏句柄）。
/// ⚠ 失败 ⇒ 尽力回一个 `500` —— 但**只在响应还没开始写**的时候回（写了一半
///   再追一个 `500` ⇒ 客户端会看到「半截响应 + 500」两截东西）。
fn serve_one(
    host: &mut dyn StdHost,
    handler: &Value,
    cid: i64,
    opts: &ServeOpts,
) -> Result<(), String> {
    // ① 读 + ② 调 handler + ③ 校验响应形状 —— 失败时**一个字节都还没写**。
    let (response, version) = match prepare_response(host, handler, cid, opts) {
        Ok(r) => r,
        Err(reason) => {
            let _ = write_failure_response(cid, &reason);
            close_conn_raw(cid);
            return Err(reason);
        }
    };
    // ④ 写回响应（**已经开始写** ⇒ 失败只能作废这一条，不能再追一个 `500`）。
    let wire = render_response(&response, &version);
    let written = write_all_bytes(cid, &wire).map(|_| ());
    close_conn_raw(cid);
    written
}

/// 一条连接的**前半段**：读一次请求 → 调 handler → 校验响应形状。
///
/// 返回 `(响应, 请求的版本号)` —— 版本号用来回响应行。
fn prepare_response(
    host: &mut dyn StdHost,
    handler: &Value,
    cid: i64,
    opts: &ServeOpts,
) -> Result<(ServeResponse, String), String> {
    // ① 读满一次请求。
    let request = read_request(cid, opts)?;
    // ② 调 handler（**原生帧内** ⇒ 同步调用）。
    let outcome = host
        .call(handler, vec![request.value], "TCP_SERVE handler")
        .map_err(|e| format!("the handler failed: {}", e.diagnostic().message))?;
    // ⚠ **挂起信号**：handler 跑在原生帧里 ⇒ 没有调度器可挂 ⇒ 「不得再挂」
    //   只能落成「这一条连接作废」。
    if outcome.signal != Signal::None {
        return Err(
            "the handler suspended — a handler must not call a host-suspending member \
             (TCP_RECV / TCP_ACCEPT / SLEEP_ASYNC): the handler runs in a native frame, \
             not in a task body"
                .into(),
        );
    }
    // ③ 校验响应形状（缺键 / 类型不对 ⇒ 这一条作废）。
    let response = parse_response(&outcome.value)?;
    Ok((response, request.version))
}

/// 一次请求（读出来的形状 + 回响应时要用的版本号）。
struct ServeRequest {
    /// 给 handler 的实参（请求 `DICT`）。
    value: Value,
    /// 请求行里的版本（回响应时原样用）。
    version: String,
}

/// 读满**一次**请求：头块（到空行为止）+ 体（按 `Content-Length`）。
fn read_request(cid: i64, opts: &ServeOpts) -> Result<ServeRequest, String> {
    // ⚠ **一次请求一份预算**（不是每次读一份）—— 与 HTTP 客户端的「总闸」同款。
    let deadline = if opts.read_timeout_ms == 0 {
        None
    } else {
        Some(Instant::now() + Duration::from_millis(opts.read_timeout_ms))
    };
    let mut raw: Vec<u8> = Vec::with_capacity(8 * 1024);
    // ① 头块：读到空行（`\r\n\r\n`）为止。
    let head_end = loop {
        if let Some(i) = find_head_end(&raw) {
            break i;
        }
        if raw.len() > opts.max_header_bytes {
            return Err(format!(
                "the request header block exceeds the {} byte cap",
                opts.max_header_bytes
            ));
        }
        read_chunk(cid, deadline, &mut raw)?;
    };
    let head = std::str::from_utf8(&raw[..head_end])
        .map_err(|e| format!("the request header is not valid UTF-8: {e}"))?;

    // ② 请求行。
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let version = parts.next().unwrap_or("").to_string();
    if method.is_empty() || path.is_empty() || version.is_empty() || parts.next().is_some() {
        return Err(format!("malformed request line {request_line:?}"));
    }
    if !method.bytes().all(is_tchar) {
        return Err(format!(
            "the request method {method:?} is not a valid token"
        ));
    }
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(format!("unsupported request version {version:?}"));
    }

    // ③ 头字段。
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut content_length: Option<u64> = None;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        // obs-fold（行首空白）是**已知的拆分/走私向量** ⇒ 拒绝，不合并。
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(format!("obs-fold in the request header: {line:?}"));
        }
        let Some((k, v)) = line.split_once(':') else {
            return Err(format!("malformed request header line {line:?}"));
        };
        let v = v.trim();
        if v.contains('\r') || v.contains('\n') {
            return Err(format!("the request header {k:?} value contains CR or LF"));
        }
        let lk = k.to_ascii_lowercase();
        match lk.as_str() {
            "content-length" => {
                // 同一个头出现两次 —— 即便值相同也拒（走私信号）。
                if content_length.is_some() {
                    return Err("duplicate Content-Length header".into());
                }
                let Ok(n) = v.parse::<u64>() else {
                    return Err(format!("Content-Length {v:?} is not a number"));
                };
                content_length = Some(n);
            }
            // ⚠ chunked 是走私向量 ⇒ 拒绝，不解码（与 HTTP 客户端同口径）。
            "transfer-encoding" => {
                return Err(
                    "Transfer-Encoding is not supported — chunked is a smuggling vector, \
                     refused rather than decoded"
                        .into(),
                )
            }
            _ => {}
        }
        headers.push((lk, v.to_string()));
    }

    // ④ 体：按 `Content-Length` 读满。
    let want = content_length.unwrap_or(0);
    if want > opts.max_body_bytes as u64 {
        return Err(format!(
            "the request body of {want} bytes exceeds the {} byte cap",
            opts.max_body_bytes
        ));
    }
    let mut body = raw[head_end + 4..].to_vec();
    while (body.len() as u64) < want {
        read_chunk(cid, deadline, &mut body)?;
    }
    // ⚠ 多出来的字节属于**下一次**请求 ⇒ 丢掉（本成员一条连接只服务一次）。
    body.truncate(want as usize);

    // ⑤ 组装给 handler 的请求 `DICT`。
    let header_dict = Value::dict(
        headers
            .iter()
            .map(|(k, v)| (Value::String(k.clone()), Value::String(v.clone())))
            .collect(),
    );
    let value = Value::dict(vec![
        (Value::String("method".into()), Value::String(method)),
        (Value::String("path".into()), Value::String(path)),
        (
            Value::String("version".into()),
            Value::String(version.clone()),
        ),
        (Value::String("headers".into()), header_dict),
        (
            Value::String("body".into()),
            Value::String(String::from_utf8_lossy(&body).to_string()),
        ),
    ]);
    Ok(ServeRequest { value, version })
}

/// 读一块并追加到 `out`。
fn read_chunk(cid: i64, deadline: Option<Instant>, out: &mut Vec<u8>) -> Result<(), String> {
    // ⚠ 预算**只出不进** ⇒ 每次读都拿「剩下的那一份」。
    let timeout = deadline.map(|dl| {
        let now = Instant::now();
        if now >= dl {
            Duration::ZERO
        } else {
            dl - now
        }
    });
    match recv_blocking(cid, 64 * 1024, timeout) {
        Ok(ReadOutcome::Bytes(b)) => {
            out.extend_from_slice(&b);
            Ok(())
        }
        // ⚠ 对端关闭 ⇒ 这一条连接作废（**不是**超时）。
        Ok(ReadOutcome::Eof) => {
            Err("the peer closed the connection before the request was complete".into())
        }
        // ⚠ 超时**不是** EOF（与 `TCP_RECV` 同口径）⇒ 这一条连接作废。
        Ok(ReadOutcome::TimedOut) => Err("timed out waiting for the request".into()),
        Err(e) => Err(e),
    }
}

/// handler 的响应字典（校验过的形状）。
struct ServeResponse {
    status: i64,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

/// 校验 handler 的返回值 ⇒ 响应。
///
/// ⚠ **缺 `status` / `headers` / `body` 任一 ⇒ 失败**（这一条连接作废）。
fn parse_response(value: &Value) -> Result<ServeResponse, String> {
    if let Value::Err(payload) = value {
        return Err(format!(
            "the handler returned an ERR value: {}",
            payload.display()
        ));
    }
    let entries = dict_entries_of(value).ok_or_else(|| {
        format!(
            "the handler returned {} instead of a response DICT",
            crate::value_kind(value)
        )
    })?;
    let status_v = entry_get(&entries, "status").ok_or("the handler's response has no `status`")?;
    let headers_v =
        entry_get(&entries, "headers").ok_or("the handler's response has no `headers`")?;
    let body_v = entry_get(&entries, "body").ok_or("the handler's response has no `body`")?;
    let Value::Integer(status) = status_v else {
        return Err(format!(
            "the handler's response `status` must be an integer, got {}",
            crate::value_kind(status_v)
        ));
    };
    let headers = parse_response_headers(headers_v)?;
    let body = match body_v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                let Value::Integer(b) = it else {
                    return Err(format!(
                        "the handler's response `body` array must hold integers, got {}",
                        crate::value_kind(it)
                    ));
                };
                if !(0..=255).contains(b) {
                    return Err(format!(
                        "the handler's response `body` byte is out of range: {b}"
                    ));
                }
                out.push(*b as u8);
            }
            out
        }
        other => {
            return Err(format!(
                "the handler's response `body` must be a STRING or ARRAY[INTEGER], got {}",
                crate::value_kind(other)
            ))
        }
    };
    Ok(ServeResponse {
        status: *status,
        headers,
        body,
    })
}

/// 校验响应里的 `headers`（`DICT`；值是 `STRING` 或 `INTEGER`）。
fn parse_response_headers(v: &Value) -> Result<Vec<(String, String)>, String> {
    let entries = dict_entries_of(v).ok_or_else(|| {
        format!(
            "the handler's response `headers` must be a DICT, got {}",
            crate::value_kind(v)
        )
    })?;
    let mut out = Vec::with_capacity(entries.len());
    for (k, val) in entries {
        let Value::String(name) = k else {
            return Err(format!(
                "the handler's response header name must be a STRING, got {}",
                crate::value_kind(&k)
            ));
        };
        // ⚠ 头名必须是 token、值里不许有裸 CR/LF —— 否则就是头注入。
        if name.is_empty() || !name.bytes().all(is_tchar) {
            return Err(format!(
                "the handler's response header name {name:?} is not a valid token"
            ));
        }
        let text = match val {
            Value::String(s) => s.clone(),
            Value::Integer(n) => n.to_string(),
            other => {
                return Err(format!(
                    "the handler's response header {name:?} must be a STRING or INTEGER, got {}",
                    crate::value_kind(&other)
                ))
            }
        };
        if text.contains('\r') || text.contains('\n') {
            return Err(format!(
                "the handler's response header {name:?} value contains CR or LF"
            ));
        }
        out.push((name.clone(), text));
    }
    Ok(out)
}

/// 响应 ⇒ 线上的字节。
fn render_response(resp: &ServeResponse, version: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(256 + resp.body.len());
    let line = match reason_phrase(resp.status) {
        Some(reason) => format!("{version} {} {reason}\r\n", resp.status),
        None => format!("{version} {}\r\n", resp.status),
    };
    out.extend_from_slice(line.as_bytes());
    // ⚠ `Content-Length` 缺失 ⇒ 补一个（否则对端**无法**给响应定界）。
    if !resp
        .headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-length"))
    {
        out.extend_from_slice(format!("Content-Length: {}\r\n", resp.body.len()).as_bytes());
    }
    for (k, v) in &resp.headers {
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&resp.body);
    out
}

/// 常见状态码的理由短语（**不是**权威表 —— 只是让响应行像样）。
fn reason_phrase(status: i64) -> Option<&'static str> {
    Some(match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => return None,
    })
}

/// 尽力把失败回给对端（`500`）—— 对端已经走了就写不进去（忽略那个失败）。
fn write_failure_response(cid: i64, reason: &str) -> Result<(), String> {
    let body = reason.as_bytes();
    let mut wire = Vec::with_capacity(128 + body.len());
    wire.extend_from_slice(b"HTTP/1.1 500 Internal Server Error\r\n");
    wire.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    wire.extend_from_slice(b"Connection: close\r\n\r\n");
    wire.extend_from_slice(body);
    write_all_bytes(cid, &wire).map(|_| ())
}

/// 记一笔失败 ⇒ **继续下一条**（不得终止整个循环）。
///
/// ⚠ 失败以 `ERR([kind:"SocketError", op:"TCP_SERVE", reason])` 的**形状**记录；
///   宿主侧能落到的位置是**告警槽**（软告警 ⇒ 不改变程序语义）。
/// ⚠ **没有新增诊断码** —— 用既有的 `E0063`（网络错误）标这一笔。
fn record_serve_failure(host: &mut dyn StdHost, reason: &str) {
    let err = serve_error(reason);
    let payload = match &err {
        Value::Err(inner) => inner.display(),
        _ => reason.to_string(),
    };
    host.ctx().warn(ErrorCode::E0063, payload);
}

/// 取字典的**插入序条目**（`DICT` 与字典累加器两种表示都认）。
fn dict_entries_of(v: &Value) -> Option<Vec<(Value, Value)>> {
    match v {
        Value::Dict(inner) => Some(inner.entries().to_vec()),
        Value::DictCell(cell) => Some(cell.inner_rc().entries().to_vec()),
        _ => None,
    }
}

/// 按键取条目（**首个**同名键 —— 与字典的 `lookup` 口径一致）。
fn entry_get<'a>(entries: &'a [(Value, Value)], key: &str) -> Option<&'a Value> {
    entries
        .iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s.as_str() == key))
        .map(|(_, v)| v)
}

// ── TCP_SERVE_STREAM：一条连接上的 Keep-Alive 循环（W-17 / C-3 / C-8 / C-9）──
//
// ## 为什么整个循环必须在一个**原生成员**内部推进
//
// handler 必须跑在**原生帧**里（⇒ handler 里调宿主挂起点成员 ⇒ 既有 `E0014`，
// 见 W-38）⇒ 原生帧里**没有调度器可挂** ⇒ 「读下一条请求」这件事在**语言层
// 没有位置**（`TCP_RECV` 是宿主挂起点成员，只能出现在任务体里）⇒ 循环只能
// 在原生成员内部推进 —— 与 `TCP_SERVE` **同款形状**。
//
// ## 四个分级预算（C-8）与两个上限（C-9）
//
// | 段 | 选项 | 计量起点 | 超了怎么办 |
// |---|---|---|---|
// | 首字节 | `first_byte_ms` | 开始等这条请求 | **408** + 关连接 |
// | 头完成 | `headers_ms` | **首字节**到手 | **408** + 关连接 |
// | 体读取 | `body_ms` | **头块**结束 | **408** + 关连接 |
// | 响应写出 | `response_ms` | **请求读完** | 放弃这次响应 + 关连接 |
// | 整条请求 | `total_ms` | 开始等这条请求 | 剩下的段**都不再等** |
//
// | 上限 | 选项 | 超了怎么办 |
// |---|---|---|
// | 头块 | `max_header_bytes` | **431** + 关连接 |
// | 请求体 | `max_body_bytes` | **413** + 关连接 |
//
// ⚠ **上限是在读取过程中就判的**，不是「读完再判」：
//   · 头块：每读一块就查一次 ⇒ **永远不会**把超限的头块读进内存；
//   · 体：`Content-Length` 超上限 ⇒ **一个字节都不读**（直接 413）。
//
// ⚠ **首字节预算与空闲超时是一对**：`first_byte_ms` 管「这条请求的首字节」，
//   `idle_timeout_ms` 管「**两条请求之间**」的空闲窗口 ⇒ 等首字节时**两者
//   取先到者**。区别只在**后果**：已经服务过请求 ⇒ 这是**空闲** ⇒ **静默**
//   关连接（不 unsolicited 回 408 —— 客户端正闲着，多一条 unsolicited 响应
//   只会把它弄糊涂）；还没服务过 ⇒ 回 **408** 再关。
//
// ## ⚠ 已知缺口（不是缺陷，是传输层现状）
//
// `tcp.rs` 的 `write_all_bytes` **没有**超时（它不在本项的改动面内）⇒ 卡在
// **一块之内**的写**不可打断** ⇒ `response_ms` 是「**块与块之间**」兑现的。

/// `TCP_SERVE_STREAM` 的选项（缺省见 [`StreamOpts::default`]）。
#[derive(Debug, Clone, Copy)]
struct StreamOpts {
    /// 头块上限（字节）—— 超限 ⇒ **431**。
    max_header_bytes: usize,
    /// 请求体上限（字节）—— 超限 ⇒ **413**。
    max_body_bytes: usize,
    /// **连接空闲超时**（毫秒）：两条请求**之间**等下一条请求首字节的上限；`0` = 无上限。
    idle_timeout_ms: u64,
    /// **首字节**预算（毫秒）：等一条请求首字节的上限；`0` = 无上限。
    first_byte_ms: u64,
    /// **头完成**预算（毫秒）：从首字节到头块结束；`0` = 无上限。
    headers_ms: u64,
    /// **体读取**预算（毫秒）：从头块结束到体读完；`0` = 无上限。
    body_ms: u64,
    /// **响应写出**预算（毫秒）：从开始写到写完；`0` = 无上限。
    response_ms: u64,
    /// **整条请求**的总预算（毫秒）：从开始等首字节到响应写完；`0` = 无上限。
    total_ms: u64,
}

impl Default for StreamOpts {
    /// ⚠ **上限有缺省、预算没有**：上限缺了就是「无界地读进内存」（那是**服务端
    ///   能触发的**资源耗尽）⇒ 缺省取 `TCP_SERVE` 的同一顶；而预算是**调用方
    ///   的承诺**，缺省 `0` = 不额外设限（与 `TCP_SERVE` 的 `read_timeout_ms`
    ///   **同口径**：只有**显式**写出来才有限）。
    fn default() -> Self {
        Self {
            max_header_bytes: MAX_HEADER_BYTES,
            max_body_bytes: MAX_BODY_BYTES,
            idle_timeout_ms: 0,
            first_byte_ms: 0,
            headers_ms: 0,
            body_ms: 0,
            response_ms: 0,
            total_ms: 0,
        }
    }
}

/// `TCP_SERVE_STREAM` 的选项名（诊断文案用）。
const STREAM_OPT_NAMES: &str = "`max_header_bytes`, `max_body_bytes`, `idle_timeout_ms`, \
     `first_byte_ms`, `headers_ms`, `body_ms`, `response_ms`, `total_ms`";

/// 解析 `TCP_SERVE_STREAM` 的可选项。
///
/// ⚠ **未知选项名 ⇒ `E0030`**（不静默忽略：拼错的上界会变成「没有上界」）。
/// ⚠ **负数 ⇒ `E0030`**（负的时长没有可表达的语义，不猜）。
fn stream_opts(host: &mut dyn StdHost, v: &Value) -> wlwl_error::WlwlResult<StreamOpts> {
    const NAME: &str = "SERVE_STREAM";
    let mut opts = StreamOpts::default();
    // ⚠ `opts` 认**两种**字典表示（普通 `DICT` 与字典累加器）—— 与 `TCP_SERVE` 同款。
    let Some(entries) = dict_entries_of(v) else {
        if matches!(v, Value::Null) {
            return Ok(opts);
        }
        return Err(serve_type_err(host, NAME, "a DICT of options or NULL", v));
    };
    for (k, val) in entries.iter() {
        let Value::String(k) = k else {
            return Err(serve_type_err(host, NAME, "a STRING option name", k));
        };
        // ⚠ 时长类：负数 ⇒ `E0030`；`0` = 无上限。
        let ms = |host: &mut dyn StdHost, key: &str| -> wlwl_error::WlwlResult<u64> {
            let n = int_arg(host, NAME, val)?;
            if n < 0 {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!("{NAME}: {key} must not be negative, got {n}"),
                ));
            }
            Ok(n as u64)
        };
        match k.as_str() {
            "max_header_bytes" => {
                let n = int_arg(host, NAME, val)?;
                if n <= 0 {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("{NAME}: max_header_bytes must be positive, got {n}"),
                    ));
                }
                opts.max_header_bytes = n as usize;
            }
            "max_body_bytes" => {
                let n = int_arg(host, NAME, val)?;
                if n < 0 {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("{NAME}: max_body_bytes must not be negative, got {n}"),
                    ));
                }
                opts.max_body_bytes = n as usize;
            }
            "idle_timeout_ms" => opts.idle_timeout_ms = ms(host, "idle_timeout_ms")?,
            "first_byte_ms" => opts.first_byte_ms = ms(host, "first_byte_ms")?,
            "headers_ms" => opts.headers_ms = ms(host, "headers_ms")?,
            "body_ms" => opts.body_ms = ms(host, "body_ms")?,
            "response_ms" => opts.response_ms = ms(host, "response_ms")?,
            "total_ms" => opts.total_ms = ms(host, "total_ms")?,
            other => {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!(
                        "{NAME}: unknown option {other:?} (expected one of {STREAM_OPT_NAMES})"
                    ),
                ))
            }
        }
    }
    Ok(opts)
}

/// 一个绝对截止时间（`ms == 0` ⇒ **无上限**）。
fn deadline_of(ms: u64, from: Instant) -> Option<Instant> {
    if ms == 0 {
        None
    } else {
        Some(from + Duration::from_millis(ms))
    }
}

/// 两个截止时间取**先到者**（`None` = 无上限）。
fn earlier(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, y) => x.or(y),
    }
}

/// 截止时间 ⇒ 还能等多久（`None` = 无限等；`Some(ZERO)` = 已经到期）。
fn remaining(deadline: Option<Instant>) -> Option<Duration> {
    deadline.map(|dl| {
        let now = Instant::now();
        if now >= dl {
            Duration::ZERO
        } else {
            dl - now
        }
    })
}

/// 等一批字节的结果。
enum Wait {
    /// 拿到了字节（缓冲区里可能还多出**下一条**请求的字节 —— 流水线）。
    Got,
    /// 对端关闭且缓冲已空。
    Eof,
    /// 预算用尽（**不是** EOF）。
    TimedOut,
    /// 读失败（连接坏了）。
    Failed(String),
}

/// 等一批字节并追加到 `buf`。
///
/// ⚠ `buf` 里已经有字节 ⇒ **立刻返回**（那些字节可能是上一条请求的尾巴）。
fn wait_for_bytes(cid: i64, buf: &mut Vec<u8>, deadline: Option<Instant>) -> Wait {
    if !buf.is_empty() {
        return Wait::Got;
    }
    match recv_blocking(cid, 64 * 1024, remaining(deadline)) {
        Ok(ReadOutcome::Bytes(b)) => {
            buf.extend_from_slice(&b);
            Wait::Got
        }
        Ok(ReadOutcome::Eof) => Wait::Eof,
        Ok(ReadOutcome::TimedOut) => Wait::TimedOut,
        Err(e) => Wait::Failed(e),
    }
}

/// 读头块时的失败。
enum HeadErr {
    /// 超上限 ⇒ **431**。
    TooLarge(String),
    /// 预算用尽 ⇒ **408**。
    TimedOut,
    /// 连接坏了 / 报文不完整。
    Failed(String),
}

/// 读满头块（到 `\r\n\r\n` 为止）⇒ 头块**长度**（不含那个分隔符）。
///
/// ⚠ **上限在读取过程中就判** ⇒ 超限的头块**不会**被读进内存。
fn read_head_block(
    cid: i64,
    buf: &mut Vec<u8>,
    max_bytes: usize,
    deadline: Option<Instant>,
) -> Result<usize, HeadErr> {
    loop {
        if let Some(i) = find_head_end(buf) {
            // ⚠ 头块**已完整** ⇒ 这时才能算它的真实长度（含分隔符的 4 字节）。
            if i + 4 > max_bytes {
                return Err(HeadErr::TooLarge(format!(
                    "the request header block of {} bytes exceeds the {} byte cap",
                    i + 4,
                    max_bytes
                )));
            }
            return Ok(i);
        }
        // ⚠ 头块还没结束 ⇒ 它的长度**至少**是 `buf.len()` ⇒ 超过上限就是超限。
        if buf.len() > max_bytes {
            return Err(HeadErr::TooLarge(format!(
                "the request header block exceeds the {} byte cap",
                max_bytes
            )));
        }
        match recv_blocking(cid, 64 * 1024, remaining(deadline)) {
            Ok(ReadOutcome::Bytes(b)) => buf.extend_from_slice(&b),
            Ok(ReadOutcome::Eof) => {
                return Err(HeadErr::Failed(
                    "the peer closed the connection before the request was complete".into(),
                ))
            }
            Ok(ReadOutcome::TimedOut) => return Err(HeadErr::TimedOut),
            Err(e) => return Err(HeadErr::Failed(e)),
        }
    }
}

/// 头块 ⇒ 请求行 + 头字段 + `Content-Length`。
struct HeadInfo {
    method: String,
    path: String,
    version: String,
    headers: Vec<(String, String)>,
    content_length: Option<u64>,
    /// 请求里说了 `Connection: close`（可与别的 token 并列）。
    wants_close: bool,
}

/// 头块 ⇒ 请求行 + 头字段。
///
/// ⚠ **判据与 `TCP_SERVE` 的读路径逐字相同**（同一个头出现两次 ⇒ 拒；`TE` ⇒ 拒；
///   obs-fold ⇒ 拒；头名不是 token ⇒ 拒；值里含 CR/LF ⇒ 拒）。
///   ⚠ 本函数是**新写的一份**，不是对既有读路径的改写 —— 既有成员**一字未动**
///   ⇒ 两份判据必须**人工保持一致**（改一处就要改另一处）。
fn parse_request_head(head: &str) -> Result<HeadInfo, String> {
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let version = parts.next().unwrap_or("").to_string();
    if method.is_empty() || path.is_empty() || version.is_empty() || parts.next().is_some() {
        return Err(format!("malformed request line {request_line:?}"));
    }
    if !method.bytes().all(is_tchar) {
        return Err(format!(
            "the request method {method:?} is not a valid token"
        ));
    }
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(format!("unsupported request version {version:?}"));
    }
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut content_length: Option<u64> = None;
    let mut wants_close = false;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        // obs-fold（行首空白）是**已知的拆分/走私向量** ⇒ 拒绝，不合并。
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(format!("obs-fold in the request header: {line:?}"));
        }
        let Some((k, v)) = line.split_once(':') else {
            return Err(format!("malformed request header line {line:?}"));
        };
        let v = v.trim();
        if v.contains('\r') || v.contains('\n') {
            return Err(format!("the request header {k:?} value contains CR or LF"));
        }
        let lk = k.to_ascii_lowercase();
        match lk.as_str() {
            "content-length" => {
                // 同一个头出现两次 —— 即便值相同也拒（走私信号）。
                if content_length.is_some() {
                    return Err("duplicate Content-Length header".into());
                }
                let Ok(n) = v.parse::<u64>() else {
                    return Err(format!("Content-Length {v:?} is not a number"));
                };
                content_length = Some(n);
            }
            // ⚠ chunked 是走私向量 ⇒ 拒绝，不解码（与 HTTP 客户端同口径）。
            "transfer-encoding" => {
                return Err(
                    "Transfer-Encoding is not supported — chunked is a smuggling vector, \
                     refused rather than decoded"
                        .into(),
                )
            }
            // ⚠ `Connection` 是一张**逗号分隔的 token 表** ⇒ 逐项看。
            "connection" if v.split(',').any(|t| t.trim().eq_ignore_ascii_case("close")) => {
                wants_close = true;
            }
            _ => {}
        }
        headers.push((lk, v.to_string()));
    }
    Ok(HeadInfo {
        method,
        path,
        version,
        headers,
        content_length,
        wants_close,
    })
}

/// 读体时的失败。
enum BodyErr {
    /// 超上限 ⇒ **413**。
    TooLarge(String),
    /// 预算用尽 ⇒ **408**。
    TimedOut,
    /// 连接坏了 / 正文不完整。
    Failed(String),
}

/// 按 `Content-Length` 读满请求体 ⇒ 正文（并把**下一条**请求的余留字节留在 `buf`）。
///
/// ⚠ **上限在读取之前就判**：`Content-Length` 超上限 ⇒ **一个字节都不读**。
fn read_body(
    cid: i64,
    buf: &mut Vec<u8>,
    body_start: usize,
    want: u64,
    max_bytes: usize,
    deadline: Option<Instant>,
) -> Result<Vec<u8>, BodyErr> {
    if want > max_bytes as u64 {
        return Err(BodyErr::TooLarge(format!(
            "the request body of {want} bytes exceeds the {} byte cap",
            max_bytes
        )));
    }
    while ((buf.len() - body_start) as u64) < want {
        // ⚠ 读取过程中也守着上限（声明与实到不一致时的兜底）。
        if buf.len() - body_start > max_bytes {
            return Err(BodyErr::TooLarge(format!(
                "the request body exceeds the {} byte cap",
                max_bytes
            )));
        }
        match recv_blocking(cid, 64 * 1024, remaining(deadline)) {
            Ok(ReadOutcome::Bytes(b)) => buf.extend_from_slice(&b),
            Ok(ReadOutcome::Eof) => {
                return Err(BodyErr::Failed(
                    "the peer closed the connection before the request was complete".into(),
                ))
            }
            Ok(ReadOutcome::TimedOut) => return Err(BodyErr::TimedOut),
            Err(e) => return Err(BodyErr::Failed(e)),
        }
    }
    let end = body_start + want as usize;
    let body = buf[body_start..end].to_vec();
    // ⚠ **余下的字节属于下一条请求**（流水线）⇒ 留在缓冲里，不丢。
    buf.drain(..end);
    Ok(body)
}

/// handler 的响应里可选的 `keep_alive` 键（`BOOLEAN`）⇒ 「读完这条就关」。
///
/// ⚠ **可选**：没有这个键 ⇒ 沿用默认（HTTP/1.1 ⇒ 继续读下一条）。
fn keep_alive_flag(v: &Value) -> Option<bool> {
    let entries = dict_entries_of(v)?;
    match entry_get(&entries, "keep_alive")? {
        Value::Boolean(b) => Some(*b),
        _ => None,
    }
}

/// 分块写出（**每块之间**查预算）。
///
/// ⚠ **已知缺口**：传输层的 `write_all_bytes` **没有**超时（`tcp.rs` 不在本项的
///   改动面内）⇒ 卡在**一块之内**的写**不可打断** ⇒ 预算是「块与块之间」兑现的。
fn write_with_budget(cid: i64, wire: &[u8], deadline: Option<Instant>) -> Result<(), String> {
    const CHUNK: usize = 64 * 1024;
    let mut off = 0usize;
    while off < wire.len() {
        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                return Err(format!(
                    "the response write exceeded its time budget — only {off} of {} bytes \
                     were written",
                    wire.len()
                ));
            }
        }
        let end = (off + CHUNK).min(wire.len());
        write_all_bytes(cid, &wire[off..end])?;
        off = end;
    }
    Ok(())
}

/// 状态码 → 理由短语（**只覆盖本成员自己发的那些码**）。
///
/// ⚠ **刻意不碰 `reason_phrase`**（那是既有成员 `TCP_SERVE` 用的）⇒ 既有成员
///   一字未改；本表是**新写的一份**，只管 400 / 408 / 413 / 431 / 500。
fn stream_reason_phrase(status: i64) -> Option<&'static str> {
    Some(match status {
        400 => "Bad Request",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        _ => return None,
    })
}

/// 回一个**只有状态行**的错误响应（400 / 408 / 413 / 431）。
///
/// ⚠ 必定带 `Connection: close` ⇒ 这些响应之后连接就该关。
fn write_status_response(cid: i64, status: i64, reason: &str) -> Result<(), String> {
    let body = reason.as_bytes();
    let mut wire = Vec::with_capacity(128 + body.len());
    let line = match stream_reason_phrase(status) {
        Some(phrase) => format!("HTTP/1.1 {status} {phrase}\r\n"),
        None => format!("HTTP/1.1 {status}\r\n"),
    };
    wire.extend_from_slice(line.as_bytes());
    wire.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    wire.extend_from_slice(b"Connection: close\r\n\r\n");
    wire.extend_from_slice(body);
    write_all_bytes(cid, &wire).map(|_| ())
}

/// `TCP_SERVE_STREAM` 的运行期失败 ⇒ `ERR([kind:"SocketError", op, reason])`。
fn stream_error(reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("SocketError".into()),
        ),
        (
            Value::String("op".into()),
            Value::String("SERVE_STREAM".into()),
        ),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

/// 记一笔失败 ⇒ **关掉这条连接、结束循环**（本成员只管一条连接 ⇒ 没有「下一条」）。
fn record_stream_failure(host: &mut dyn StdHost, reason: &str) {
    let err = stream_error(reason);
    let payload = match &err {
        Value::Err(inner) => inner.display(),
        _ => reason.to_string(),
    };
    host.ctx().warn(ErrorCode::E0063, payload);
}

/// `TCP_SERVE_STREAM(conn, opts, handler) -> INTEGER`
///
/// 在**一条**连接上**循环**处理请求，直到：对端关闭 / 空闲超时 / 客户端发了
/// `Connection: close` / handler 选择关。
///
/// 返回**成功服务**的**请求**数（不是连接数 —— 一条连接上可以服务多条请求）。
///
/// ⚠ **元数 / 类型错 ⇒ `E0022` / `E0030`**（诊断）；**运行期失败 ⇒ `ERR` 值**。
/// ⚠ **本成员不 accept、不关监听器** —— 它只管给定的**一条**连接。
/// ⚠ **handler 跑在原生帧里** ⇒ 与 `TCP_SERVE` **同一契约**（handler 里调宿主
///   挂起点成员 ⇒ 既有 `E0014`）。
pub(crate) fn tcp_serve_stream(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "SERVE_STREAM";
    if args.len() != 3 {
        return Err(arity(host, NAME, args.len(), 3));
    }
    let cid = int_arg(host, NAME, &args[0])?;
    let opts = stream_opts(host, &args[1])?;
    // ⚠ handler 必须**可调用** ⇒ 类型错 ⇒ `E0030`（不是运行期失败）。
    match &args[2] {
        Value::Closure { .. } | Value::NativeFn { .. } => {}
        other => {
            return Err(serve_type_err(host, NAME, "a function handler", other));
        }
    }
    let handler = args[2].clone();

    // ⚠ **跨请求保留**的缓冲：上一条请求的余留字节（流水线）留给下一条。
    let mut buf: Vec<u8> = Vec::new();
    let mut served: i64 = 0;
    loop {
        let req_start = Instant::now();
        let total_deadline = deadline_of(opts.total_ms, req_start);

        // ① 等这条请求的**首字节**（空闲窗口与首字节预算**取先到者**）。
        let first_deadline = earlier(
            deadline_of(opts.first_byte_ms, req_start),
            deadline_of(opts.idle_timeout_ms, req_start),
        );
        match wait_for_bytes(cid, &mut buf, first_deadline) {
            Wait::Got => {}
            // ⚠ 对端关闭 ⇒ 这条连接收工（**不是**失败）。
            Wait::Eof => {
                close_conn_raw(cid);
                break;
            }
            Wait::TimedOut => {
                if served > 0 {
                    // ⚠ **空闲**超时 ⇒ **静默**关连接（不 unsolicited 回 408）。
                } else {
                    let _ = write_status_response(
                        cid,
                        408,
                        "the server did not receive a complete request within the time it \
                         was prepared to wait",
                    );
                }
                close_conn_raw(cid);
                break;
            }
            Wait::Failed(why) => {
                record_stream_failure(host, &why);
                close_conn_raw(cid);
                break;
            }
        }
        let t_first = Instant::now();

        // ② 头块（预算：**从首字节**起算）。
        let headers_deadline = earlier(deadline_of(opts.headers_ms, t_first), total_deadline);
        let head_end = match read_head_block(cid, &mut buf, opts.max_header_bytes, headers_deadline)
        {
            Ok(i) => i,
            Err(HeadErr::TooLarge(why)) => {
                let _ = write_status_response(cid, 431, &why);
                close_conn_raw(cid);
                break;
            }
            Err(HeadErr::TimedOut) => {
                let _ = write_status_response(
                    cid,
                    408,
                    "the request header block did not arrive within the configured budget",
                );
                close_conn_raw(cid);
                break;
            }
            Err(HeadErr::Failed(why)) => {
                record_stream_failure(host, &why);
                close_conn_raw(cid);
                break;
            }
        };
        let head = match std::str::from_utf8(&buf[..head_end]) {
            Ok(s) => s,
            Err(e) => {
                let why = format!("the request header is not valid UTF-8: {e}");
                let _ = write_status_response(cid, 400, &why);
                close_conn_raw(cid);
                break;
            }
        };

        // ③ 请求行 + 头字段。
        let info = match parse_request_head(head) {
            Ok(i) => i,
            Err(why) => {
                let _ = write_status_response(cid, 400, &why);
                close_conn_raw(cid);
                break;
            }
        };
        let t_head = Instant::now();

        // ④ 请求体（预算：**从头块结束**起算）。
        let body_deadline = earlier(deadline_of(opts.body_ms, t_head), total_deadline);
        let want = info.content_length.unwrap_or(0);
        let body = match read_body(
            cid,
            &mut buf,
            head_end + 4,
            want,
            opts.max_body_bytes,
            body_deadline,
        ) {
            Ok(b) => b,
            Err(BodyErr::TooLarge(why)) => {
                let _ = write_status_response(cid, 413, &why);
                close_conn_raw(cid);
                break;
            }
            Err(BodyErr::TimedOut) => {
                let _ = write_status_response(
                    cid,
                    408,
                    "the request body did not arrive within the configured budget",
                );
                close_conn_raw(cid);
                break;
            }
            Err(BodyErr::Failed(why)) => {
                record_stream_failure(host, &why);
                close_conn_raw(cid);
                break;
            }
        };

        // ⑤ 调 handler（**原生帧内** ⇒ 与 `TCP_SERVE` 同一契约）。
        // ⚠ **响应阶段**的预算从**请求读完**起算（**含 handler 的执行时间**）⇒
        //   预算用尽 ⇒ **放弃这次响应**并关连接（客户端读到 EOF，不是半截响应）。
        let respond_deadline = earlier(
            deadline_of(opts.response_ms, Instant::now()),
            total_deadline,
        );
        let header_dict = Value::dict(
            info.headers
                .iter()
                .map(|(k, v)| (Value::String(k.clone()), Value::String(v.clone())))
                .collect(),
        );
        let request = Value::dict(vec![
            (
                Value::String("method".into()),
                Value::String(info.method.clone()),
            ),
            (
                Value::String("path".into()),
                Value::String(info.path.clone()),
            ),
            (
                Value::String("version".into()),
                Value::String(info.version.clone()),
            ),
            (Value::String("headers".into()), header_dict),
            (
                Value::String("body".into()),
                Value::String(String::from_utf8_lossy(&body).to_string()),
            ),
        ]);
        let outcome = match host.call(&handler, vec![request], "SERVE_STREAM handler") {
            Ok(o) => o,
            Err(e) => {
                let why = format!("the handler failed: {}", e.diagnostic().message);
                let _ = write_failure_response(cid, &why);
                close_conn_raw(cid);
                break;
            }
        };
        // ⚠ **挂起信号**：handler 跑在原生帧里 ⇒ 没有调度器可挂 ⇒ 「不得再挂」
        //   只能落成「这一条连接作废」（W-38 同款）。
        if outcome.signal != Signal::None {
            let _ = write_failure_response(
                cid,
                "the handler suspended — a handler must not call a host-suspending member \
                 (TCP_RECV / TCP_ACCEPT / SLEEP_ASYNC): the handler runs in a native frame, \
                 not in a task body",
            );
            close_conn_raw(cid);
            break;
        }
        // ⑥ 响应形状（缺键 / 类型不对 ⇒ 这一条作废）。
        let response = match parse_response(&outcome.value) {
            Ok(r) => r,
            Err(why) => {
                let _ = write_failure_response(cid, &why);
                close_conn_raw(cid);
                break;
            }
        };
        // ⚠ handler 可以选择「读完这条就关」（`keep_alive: FALSE`）。
        let handler_closes = keep_alive_flag(&outcome.value) == Some(false);

        // ⑦ 写出响应（预算：**响应阶段** = 从请求读完到写完；总预算也管着它）。
        if let Some(dl) = respond_deadline {
            if Instant::now() >= dl {
                let why = "the response phase exceeded its time budget — the response was \
                           not written and the connection was closed";
                record_stream_failure(host, why);
                close_conn_raw(cid);
                break;
            }
        }
        let wire = render_response(&response, &info.version);
        if let Err(why) = write_with_budget(cid, &wire, respond_deadline) {
            record_stream_failure(host, &why);
            close_conn_raw(cid);
            break;
        }
        served += 1;

        // ⑧ 收尾：客户端说 close / HTTP/1.0 / handler 选择关 ⇒ 关；否则**读下一条**。
        if handler_closes || info.wants_close || info.version == "HTTP/1.0" {
            close_conn_raw(cid);
            break;
        }
    }
    Ok(Outcome::normal(Value::Integer(served)))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// 仓内本地 HTTP 夹具:绑 `127.0.0.1:0`、回一句**原样**的字节。
    ///
    /// ⚠ **绝不访问外网** —— CI 不稳定,且违反本仓「可复现」卖点。
    /// 夹具给的是**原始响应字节**,所以 `chunked` / 无 `Content-Length` /
    /// 超大 `Content-Length` 这些形状都能精确造出来。
    fn fixture(raw: &'static str) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf); // 读到头即可,本夹具不校验请求
                let _ = s.write_all(raw.as_bytes());
                let _ = s.flush();
                // `Connection: close` 的形状靠直接关连接来表达。
            }
        });
        port
    }

    fn err_reason(v: &Value) -> Option<&str> {
        let Value::Err(p) = v else { return None };
        let Value::Dict(entries) = &**p else {
            return None;
        };
        entries
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "reason"))
            .and_then(|(_, v)| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
    }

    fn dict_get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
        let Value::Dict(entries) = v else { return None };
        entries
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == key))
            .map(|(_, v)| v)
    }

    #[test]
    fn get_returns_status_headers_and_body() {
        let p = fixture("HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-T: v\r\n\r\nhello");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/x"),
            None,
            vec![],
            5000,
        );
        assert_eq!(dict_get(&v, "status"), Some(&Value::Integer(200)));
        assert_eq!(dict_get(&v, "body"), Some(&Value::String("hello".into())));
        assert_eq!(
            dict_get(dict_get(&v, "headers").unwrap(), "X-T"),
            Some(&Value::String("v".into()))
        );
    }

    #[test]
    fn post_sends_body_and_content_length() {
        // 夹具把请求原样回显,好让测试看到我们**发出去了**什么。
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let sink = seen.clone();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                *sink.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).into_owned();
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });
        let v = request(
            "HTTP_POST",
            "POST",
            &format!("http://127.0.0.1:{port}/p"),
            Some("hi"),
            vec![("X-A".into(), "b".into())],
            5000,
        );
        assert_eq!(dict_get(&v, "status"), Some(&Value::Integer(200)));
        let got = seen.lock().unwrap().clone();
        assert!(
            got.starts_with("POST /p HTTP/1.1\r\n"),
            "request line: {got:?}"
        );
        assert!(
            got.contains("Content-Length: 2\r\n"),
            "body length: {got:?}"
        );
        assert!(got.contains("X-A: b\r\n"), "caller header: {got:?}");
        assert!(
            got.ends_with("\r\n\r\nhi"),
            "body follows the blank line: {got:?}"
        );
    }

    #[test]
    fn chunked_is_refused_not_decoded() {
        let p =
            fixture("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/"),
            None,
            vec![],
            5000,
        );
        let r = err_reason(&v).expect("must be an ERR value");
        assert!(r.contains("chunked"), "reason should name the vector: {r}");
    }

    #[test]
    fn no_length_and_no_close_is_refused() {
        let p = fixture("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nhello");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/"),
            None,
            vec![],
            5000,
        );
        let r = err_reason(&v).expect("must be an ERR value");
        assert!(r.contains("neither Content-Length"), "reason: {r}");
    }

    #[test]
    fn redirect_is_returned_not_followed() {
        let p =
            fixture("HTTP/1.1 302 Found\r\nLocation: /elsewhere\r\nContent-Length: 4\r\n\r\ngoin");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/a"),
            None,
            vec![],
            5000,
        );
        assert_eq!(
            dict_get(&v, "status"),
            Some(&Value::Integer(302)),
            "3xx 必须原样返回(ADR-0028 D4),不能跟过去"
        );
        assert_eq!(
            dict_get(dict_get(&v, "headers").unwrap(), "Location"),
            Some(&Value::String("/elsewhere".into()))
        );
    }

    #[test]
    fn https_and_connection_refused_are_err_values() {
        let v = request(
            "HTTP_GET",
            "GET",
            "https://example.com/",
            None,
            vec![],
            1000,
        );
        assert!(err_reason(&v)
            .expect("https must be an ERR value")
            .contains("plaintext"));

        // 绑一个端口拿到号随即关闭 —— 连接必被拒绝。
        let dead = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{dead}/"),
            None,
            vec![],
            2000,
        );
        assert!(
            err_reason(&v).is_some(),
            "连接拒绝必须是 ERR 值而不是 panic"
        );
    }

    #[test]
    fn body_over_the_cap_is_err_not_oom() {
        // 声明一个超过 8 MiB 上限的 Content-Length,但一个字节都不发。
        let p = fixture("HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/"),
            None,
            vec![],
            5000,
        );
        let r = err_reason(&v).expect("must be an ERR value");
        assert!(r.contains("cap"), "reason should name the cap: {r}");
    }

    fn p(s: &str) -> Parts {
        parse("t", s).expect("parses")
    }

    /// 机制钉子。**期望值取自 RFC 3986 原文**,不是从本实现输出抄的 ——
    /// 全量证据在 `net_contract.rs`(§5.4 全部 40 条逐字冻结)。
    #[test]
    fn appendix_b_regex_splits_five_components() {
        // RFC 3986 Appendix B 正文给出的那一个例子。
        let u = p("http://www.ics.uci.edu/pub/ietf/uri/#Related");
        assert_eq!(u.scheme.as_deref(), Some("http"));
        assert_eq!(u.host.as_deref(), Some("www.ics.uci.edu"));
        assert_eq!(u.path, "/pub/ietf/uri/");
        assert_eq!(u.query, None, "RFC 明写 query 是 <undefined>");
        assert_eq!(u.fragment.as_deref(), Some("Related"));
    }

    /// §5.3 末段的 undefined / empty 之分 —— 混同就分不开 query 与 path。
    #[test]
    fn undefined_and_empty_components_differ() {
        assert_eq!(p("http://a/").query, None, "no `?` ⇒ undefined");
        assert_eq!(
            p("http://a/?").query.as_deref(),
            Some(""),
            "`?` 后面立刻结束 ⇒ empty"
        );
        assert_eq!(p("http://a/#").fragment.as_deref(), Some(""));
    }

    #[test]
    fn remove_dot_segments_matches_rfc() {
        // §5.2.4 正文那两张缓冲表的第一行。
        assert_eq!(remove_dot_segments("/a/b/c/./../../g"), "/a/g");
        assert_eq!(remove_dot_segments("mid/content=5/../6"), "mid/6");
        // `..` 不能改 authority ⇒ 回退到根后不再往上删。
        assert_eq!(remove_dot_segments("/../g"), "/g");
    }

    #[test]
    fn strict_mode_keeps_same_scheme_prefix() {
        let base = p("http://a/b/c/d;p?q");
        assert_eq!(recompose(&resolve(&base, &p("http:g"))), "http:g");
    }
}
