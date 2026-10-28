//! `wlwl interface` / `wlwl schema` —— 面向工具的 JSON 导出(v0.10 Step 10 ·
//! 计划书 §5.3 P1-3「interface / schema JSON」)。
//!
//! 两个形状,两种消费者:
//!
//! | 子命令 | 回答什么 | 消费者 |
//! |---|---|---|
//! | `interface <file>` | 这个模块**对外承诺**什么 | AI 工具 / 构建脚本(改名前先问一遍承诺) |
//! | `schema` | WLWL 的类型系统**有哪几类** | 工具的静态分析层(别把这份清单硬编码在自己代码里) |
//!
//! 两者都**只读**,都不改盘上的任何东西。`interface` 的内容直接来自
//! Step 6 / 7 的签名模型([`wlwl_types::sig`]),所以它跟 `wlwl sig` /
//! `sig-gen` / `check` 说的必然是同一件事 —— 不另建一套推导。

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use wlwl_ast::Expr;
use wlwl_eval::runtime::{ScopeSnapshotEntry, TaskSnapshot, TaskSnapshotEntry};

use crate::OutputFormat;

/// JSON 产物的 schema 版本。字段增删时**必须**动它 —— 工具靠它判断
/// 自己的解析器还能不能用。
const SCHEMA_VERSION: &str = "1.0.0";

/// [项目扩展 / G-07] `wlwl namespace [path]` —— **命名空间**的成员清单。
///
/// ## 它补的缺口
///
/// `wlwl interface` 只能对**文件**求公共面，`wlwl schema` 给的是类型系统与诊断码
/// ⇒ 想知道「`wlwl:std.net` 到底有哪些成员」，使用者只能**读文档**或者
/// **反推二进制字符串表**（字符串表里成员名是连续拼接的裸串：
/// `"URL_JOINURL_PARSE"`、`"HTTP_GETHTTP_POST"` —— 本项目就是这么发现的）。
/// 这类「靠猜」的 API 发现成本，正是标准库迟迟长不起来的隐形原因。
///
/// ## 形态
///
/// 与 `interface` / `schema` 一样：**只读**、输出 JSON、带 `*_schema_version`。
/// 成员名单**直接取自实现**（`wlwl_std::namespace_paths()` + `resolve()`）——
/// 与附录 A 镜像同源（`wlwl_eval::stdlib_mirror` 也走这一条），不另建一套推导。
///
/// ## ⚠ 使用边界（**规范侧的要求，不是偏好**）
///
/// 这份输出是**实现的成员面**，**不是**规范的公共面。两条纪律（规范仓
/// 注册镜像册 §0.7 规则 1 与一致性册 §5.5 规则 5）：
///
/// 1. **不得**用它生成或改写规范的**附录 A**（冻结快照）——
///    §0.7 规则 1 明写「表体**不得**由实现清单自动生成，**不得**为追平实现的
///    当次改动而改写」。⇒ 本命令**只读**，它**没有**任何写回路径。
/// 2. **不得**把规范外扩展（一致性册 §5.5 的「实现侧扩展台账」里的成员）
///    说成「规范已支持」，也**不得**把它们列进「公共面」。
///    ⇒ 台账成员**不是**规范公共面的一部分（§5.5 规则 1）。
///
/// ⇒ 输出里带 `"surface": "implementation"` 字段，让**消费这个 JSON 的工具**
///   自己就能判断它拿到的不是公共面清单。
pub fn namespace_command(path: Option<&str>) -> ExitCode {
    let mut out: Vec<Value> = Vec::new();
    for p in wlwl_std::namespace_paths() {
        if let Some(want) = path {
            if p != want {
                continue;
            }
        }
        let members: Vec<String> = match wlwl_std::resolve(p) {
            Some(wlwl_std::StdBackend::Native(spec)) => spec
                .functions
                .iter()
                .map(|(n, _)| (*n).to_string())
                .collect(),
            Some(wlwl_std::StdBackend::Lang(src)) => wlwl_std::lang_exports(src.source),
            None => Vec::new(),
        };
        out.push(json!({
            "path": p,
            "member_count": members.len(),
            "members": members,
        }));
    }
    if let Some(want) = path {
        if out.is_empty() {
            eprintln!("error: unknown namespace: {want}");
            eprintln!(
                "hint: run `wlwl namespace` with no argument to list every resolvable namespace"
            );
            return ExitCode::from(1);
        }
    }
    let doc = json!({
        "interface_schema_version": SCHEMA_VERSION,
        // ⚠ [W-17] **这是实现的成员面，不是规范的公共面** ——
        //   消费方据此判断「不得拿它去生成/改写附录 A」（注册镜像册 §0.7 规则 1），
        //   也据此不得把规范外扩展说成「规范已支持」（一致性册 §5.5 规则 5）。
        "surface": "implementation",
        "namespaces": out,
    });
    match serde_json::to_string_pretty(&doc) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: namespace serialize failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// `wlwl interface <file>` —— 模块的公开面。
pub fn interface_file(file: &std::path::Path) -> ExitCode {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read '{}': {}", file.display(), e);
            return ExitCode::from(1);
        }
    };
    let file_name = file.to_string_lossy().to_string();
    let ast = match wlwl_parser::parse(&source, &file_name) {
        Ok(a) => a,
        Err(e) => {
            // 逐条:一个错误可能带**多条**诊断(§8.6)⇒ 全部打印,别只打
            // 第一条。
            eprint!(
                "{}",
                crate::diag_output::render_error_text(&e, OutputFormat::Human)
            );
            return ExitCode::from(1);
        }
    };
    match serde_json::to_string_pretty(&interface_of(&file_name, &ast)) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: interface serialize failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// 组装一个模块的 interface 载荷。
///
/// 形状与 `wlwl_types::sig` 的签名模型一一对应:
/// `exports[]` 就是 `.wll.sig` 的条目(名字 + 类型 + 形参 / 返回),
/// `sealed` 是 `SEALED([...])` 声明面,`signature_file` 是旁路签名文件
/// 在磁盘上的位置(写没写由工具自己看文件在不在)。
pub fn interface_of(file: &str, ast: &Expr) -> Value {
    // 静态层交出两样东西:根作用域绑定类型(Step 6)与推导出的签名(Step 7)。
    let builtins = crate::builtin_sig_table();
    let checked = wlwl_types::check_program_detailed(ast, &builtins);
    let contract = wlwl_types::ModuleContract::from_module(
        ast,
        file,
        Some(wlwl_types::sig_from_module(ast, &checked.declared)),
    );

    let exports: Vec<Value> = contract
        .sig
        .as_ref()
        .map(|sig| {
            sig.entries
                .values()
                .map(|entry| match &entry.ty {
                    wlwl_types::Ty::Fun { params, ret } => json!({
                        "name": entry.name,
                        "kind": "function",
                        "params": params.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                        "returns": ret.to_string()
                    }),
                    other => json!({
                        "name": entry.name,
                        "kind": "value",
                        "type": other.to_string()
                    }),
                })
                .collect()
        })
        .unwrap_or_default();

    json!({
        "interface_schema_version": SCHEMA_VERSION,
        "module": file,
        "signature_file": signature_file_path(file),
        // 声明面:None = 模块没写 SEALED(不是「空面」,见 `collect_sealed`)。
        "sealed": contract.seal,
        "exports": exports
    })
}

fn signature_file_path(file: &str) -> String {
    format!("{file}.sig")
}

/// `wlwl schema` —— 类型系统 + 静态契约诊断码的机器可读清单。
pub fn schema_command() -> ExitCode {
    match serde_json::to_string_pretty(&type_schema()) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: schema serialize failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// 类型系统的 schema。
///
/// 清单来自运行期真实类型(spec §2.1)+ 静态层的类型 IR + 显式约束
/// (Step 9)。**刻意不写成「从代码里反射出来」** —— 那需要一个
/// 「注册表 → 类型名」的单向依赖,为了省几行 JSON 去动分层不划算。
/// 代价是:静态层将来加一个类型头,这里要跟着改;锁测试
/// `schema_type_names_match_the_static_layer` 就是盯这件事的。
pub fn type_schema() -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "types": [
            { "name": "INTEGER", "kind": "scalar", "runtime_type": "INTEGER" },
            { "name": "FLOAT",   "kind": "scalar", "runtime_type": "FLOAT" },
            { "name": "STRING",  "kind": "scalar", "runtime_type": "STRING" },
            { "name": "BOOLEAN", "kind": "scalar", "runtime_type": "BOOLEAN" },
            { "name": "NULL",    "kind": "scalar", "runtime_type": "NULL" },
            { "name": "ARRAY",   "kind": "container", "arity": 1,
              "syntax": "ARRAY[T]",
              "note": "parser 强制要求方括号;裸写 `ARRAY` 报 E0010" },
            { "name": "DICT",    "kind": "container", "arity": 2, "syntax": "DICT[K, V]" },
            { "name": "OPTION",  "kind": "wrapper", "arity": 1, "syntax": "OPTION[T]",
              "note": "纯注解糖(spec §2.1 没有这个运行时类型);见证是 NULL 或 RESULT[T, ·];裸 T 不通过" },
            { "name": "RESULT",  "kind": "wrapper", "arity": 2, "syntax": "RESULT[T, E]",
              "note": "两个运行期构造子:OK(_) / ERR(_)" },
            { "name": "FUN",     "kind": "function", "syntax": "FUN(...) -> U",
              "note": "箭头形式本版不可从源码到达(计划书 §3.3);静态层可达性由 `->` 终结符缺席所致" },
            { "name": "DYNAMIC", "kind": "top_bottom",
              "note": "未标注处的回退类型;与任何类型双向可赋值,所以它从不产生诊断" }
        ],
        "named_types_not_modelled": [
            "TASK", "CHANNEL", "CLASS", "INSTANCE"
        ],
        "bounds": [
            { "name": "Comparable", "syntax": "T: Comparable",
              "members": ["INTEGER", "FLOAT", "STRING"],
              "note": "成员集合对着运行期 cmp_op 量出来:比较算子只放行数值与字符串,其余 E0030" }
        ],
        "type_variables": {
            "syntax": "T: Bound(方括号内与顶层注解都认)",
            "scope": "调用点实例化;无 monomorphization,运行期擦除",
            "note": "裸的未知类型名是不透明类型(拼错即报),不是变量"
        },
        "diagnostics": static_contract_codes()
    })
}

// ───────────────────────────────────────────────────────────────────
// [W-30 / F-3] 任务检视 —— 只读 JSON 视图 + 只读检视端点。
//
// ⚠⚠ **安全边界:三条,逐条都有代码位置**
//
// 1. **只绑回环** —— [`start_inspect_endpoint`] 在 `bind` **之前**检查
//    `addr.ip().is_loopback()` ⇒ 传 `0.0.0.0:1234` **拒绝启动**。
//    为什么:暴露一个「能读进程状态」的端口就是**信息泄露面** —— 任务树、
//    挂起点、已运行时长这些东西在生产环境里就是业务情报(谁在等哪个 socket、
//    哪个请求处理了多久)。⇒ 这不是「顺手加个调试口」。
// 2. **只读** —— 端点只认**一个** op(`tasks`),别的 op 一律**回绝**
//    ([`is_read_only_op`])。⚠ 这道判定是**承重**的:去掉它 ⇒ 任何 op
//    (包括 `eval`)都会拿到快照 ⇒ 守卫用例
//    `inspect_endpoint_rejects_non_read_ops` **变红**。
// 3. **无执行能力** —— 端点线程**只**拿到 `Arc<Mutex<Option<TaskSnapshot>>>`:
//    纯数据(`String` / `i64` / `bool`),**没有求值器句柄** ⇒ 它**结构上**
//    就没法让解释器跑东西(不是「约定不跑」,是「没有能跑的东西」)。
//
// ⚠ **零新增依赖**:手写 `std::net::TcpListener`(照 `wlwl-std/src/tcp.rs` 的
//   既有手写 socket 做法);协议是「JSON 行」—— 一行一个 JSON 对象。
//
// ⚠ **端口占用**:bind 失败**立刻**报可诊断的一行 + 非零退出码(不静默重试
//   到超时 —— 那样 CI 上只会看到「卡住」,看不到「为什么」)。
// ───────────────────────────────────────────────────────────────────

/// 检视端点**唯一**放行的 op。
///
/// ⚠ 只有一个读 op ⇒ 端点**没有**任何能改状态 / 能触发求值的入口。
const READ_ONLY_OPS: [&str; 1] = ["tasks"];

/// 只读判定 —— **承重**的那道闸门。
///
/// ⚠ 去掉这个函数(或者把调用它的 `if` 删掉)⇒ 端点会对**任何** op 都回
///   快照 ⇒ 守卫用例 `inspect_endpoint_rejects_non_read_ops` 变红。
fn is_read_only_op(op: &str) -> bool {
    READ_ONLY_OPS.contains(&op)
}

/// `wlwl tasks` 的 JSON 载荷 —— **只读**的任务视图。
///
/// 形状与 `namespace` / `interface` / `schema` 同款:带 `*_schema_version`、
/// 纯 JSON、字段增删要动版本号。
///
/// ⚠ `"surface": "implementation"` 与 `namespace` 同一口径:这是**实现的**
///   运行期视图,**不是**规范的公共面(规范里没有「任务检视」这一节)。
pub fn tasks_json(snapshot: &TaskSnapshot) -> Value {
    json!({
        "tasks_schema_version": SCHEMA_VERSION,
        // ⚠ 与 `namespace` 同一口径:实现面,不是规范公共面。
        "surface": "implementation",
        // ⚠ **只读**这件事本身写进载荷 ⇒ 消费方不必再问「它会不会改东西」。
        "readonly": true,
        // ⚠ 时钟域:绝对**单调**毫秒(进程启动起)⇒ 与 `deadline_ms` /
        //   睡眠到点同域 ⇒ 三个读数可以互相比较。
        "clock": "monotonic_ms_since_process_start",
        "now_ms": snapshot.now_ms,
        "task_count": snapshot.tasks.len(),
        "scope_count": snapshot.scopes.len(),
        "tasks": snapshot.tasks.iter().map(task_entry_value).collect::<Vec<_>>(),
        // ⚠ 任务树 = **作用域树** + 每个作用域上挂着的任务(仓里没有
        //   「谁 spawn 了谁」的记录 ⇒ 不另建一套父子表)。
        "task_tree": scope_tree_value(&snapshot.scopes, &snapshot.tasks, None),
    })
}

/// 一个任务的 JSON 条目。
fn task_entry_value(t: &TaskSnapshotEntry) -> Value {
    json!({
        "id": t.id,
        "generation": t.generation,
        "state": t.state,
        // ⚠ **挂在哪个挂起点** —— 只有 `suspended` 才有值(其余是 `null`)。
        "suspend_point": match &t.suspend_point {
            Some(sp) => json!({ "kind": sp.kind, "detail": sp.detail }),
            None => json!(null),
        },
        "parent_scope": t.parent_scope,
        // ⚠ **已运行时长** = 快照时刻 − 启动时刻(同一个单调钟域)。
        "started_at_ms": t.started_at_ms,
        "elapsed_ms": t.elapsed_ms,
        "deadline_ms": t.deadline_ms,
        "awaited": t.awaited,
        "cancel_requested": t.cancel_requested,
    })
}

/// 作用域树(递归)—— `task_tree` 的节点。
fn scope_tree_value(
    scopes: &[ScopeSnapshotEntry],
    tasks: &[TaskSnapshotEntry],
    parent: Option<usize>,
) -> Vec<Value> {
    let mut out = Vec::new();
    for sc in scopes.iter().filter(|s| s.parent == parent) {
        let mut node = json!({
            "scope_id": sc.id,
            "scope_parent": sc.parent,
            "cancelled": sc.cancelled,
            "tasks": sc.task_ids
                .iter()
                .filter_map(|id| tasks.iter().find(|t| t.id == *id))
                .map(task_entry_value)
                .collect::<Vec<_>>(),
        });
        node["children"] = Value::Array(scope_tree_value(scopes, tasks, Some(sc.id)));
        out.push(node);
    }
    out
}

/// 打印一份任务快照(JSON)—— 与 `namespace` / `interface` / `schema` 同款出口。
pub fn print_tasks(snapshot: &TaskSnapshot) -> ExitCode {
    match serde_json::to_string_pretty(&tasks_json(snapshot)) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: tasks serialize failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// 只读检视端点 —— 跑在一个**后台线程**里。
pub struct InspectEndpoint {
    /// 实际绑上的地址(端口可能是 `0` = 让系统挑)。
    pub addr: SocketAddr,
    /// accept 线程(结束时 drop 掉即可;进程退出时它自然结束)。
    _handle: std::thread::JoinHandle<()>,
}

/// 起一个**只读**检视端点。
///
/// ⚠ `addr` **必须**是回环地址(`127.0.0.1` / `::1`)—— 非回环**拒绝启动**
///   (见本节开头的安全边界第 1 条)。
/// ⚠ bind 失败 ⇒ 返回 `Err`(一行可诊断的说明)⇒ 调用方据此非零退出。
pub fn start_inspect_endpoint(
    addr: &str,
    slot: Arc<Mutex<Option<TaskSnapshot>>>,
) -> Result<InspectEndpoint, String> {
    let target: SocketAddr = addr
        .parse()
        .map_err(|e| format!("cannot parse endpoint address '{addr}': {e}"))?;
    // ⚠⚠ **安全边界第 1 条**:只绑回环。
    //   `0.0.0.0` ⇒ 所有网卡可见 ⇒ 一个「能读进程状态」的端口就是信息泄露面。
    if !target.ip().is_loopback() {
        return Err(format!(
            "refusing to bind {target}: the inspection endpoint is loopback-only \
             (use 127.0.0.1:PORT; 0.0.0.0 would expose the process state to the network)"
        ));
    }
    // ⚠⚠ **端口占用探测**（在 bind **之前**）。
    //
    //   为什么不能只看 bind 的错误：**Windows 上 `SO_REUSEADDR` 的语义是
    //   「后绑的人**抢走**那个端口」** ⇒ 第二次 bind **不会**失败（实测：
    //   两个实例都报「listening on 127.0.0.1:9732」）⇒ 端口被占时**静默**
    //   抢端口 ⇒ 第一个端点突然变聋 ⇒ 比「明确报错」坏得多。
    //   ⇒ 这里先**探**一次：连得上 ⇒ 有人在听 ⇒ 明确报错、不启动。
    //
    //   ⚠ 探测与 bind 之间有一个**竞态窗口**（两个实例同时起）⇒ 那种情况
    //   仍可能后绑者抢端口 —— 但那要求两个实例**同一瞬间**启动，CI 上撞端口
    //   的常见形态（一个还在跑）已被这条探测挡住。
    if port_is_taken(target) {
        return Err(format!(
            "cannot bind {target}: the port is already in use (another inspection \
             endpoint is probably running there)"
        ));
    }
    let listener = TcpListener::bind(target)
        .map_err(|e| format!("cannot bind {target}: {e} (is the port already in use?)"))?;
    let bound = listener
        .local_addr()
        .map_err(|e| format!("cannot read the local address of {target}: {e}"))?;
    let handle = std::thread::Builder::new()
        .name("wlwl-inspect".to_string())
        .spawn(move || serve_inspect(listener, slot))
        .map_err(|e| format!("cannot spawn the inspection thread: {e}"))?;
    Ok(InspectEndpoint {
        addr: bound,
        _handle: handle,
    })
}

/// 这个端口上**是不是已经有人在听**?
///
/// ⚠ 只用来在 bind **之前**判「端口被占」(见 [`start_inspect_endpoint`]
///   里那段注释:Windows 的 `SO_REUSEADDR` 会让第二次 bind **静默成功**)。
/// ⚠ 连不上 ⇒ 没人听(本地回环上是**立即** ECONNREFUSED,不是等超时)。
fn port_is_taken(addr: SocketAddr) -> bool {
    if addr.port() == 0 {
        // 端口 0 = 让系统挑 ⇒ 不可能有「占用」。
        return false;
    }
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

/// accept 循环 —— 每个连接一条独立线程(免得一个慢客户端卡住整个端点)。
fn serve_inspect(listener: TcpListener, slot: Arc<Mutex<Option<TaskSnapshot>>>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            // 单个连接失败(例如对端半路消失)⇒ 不该拖垮端点。
            continue;
        };
        let slot = Arc::clone(&slot);
        let _ = std::thread::Builder::new()
            .name("wlwl-inspect-conn".to_string())
            .spawn(move || {
                let _ = handle_inspect_conn(stream, slot);
            });
    }
}

/// 处理一条连接:读**一行** JSON 请求 ⇒ 回**一行** JSON 响应。
fn handle_inspect_conn(
    stream: TcpStream,
    slot: Arc<Mutex<Option<TaskSnapshot>>>,
) -> std::io::Result<()> {
    // ⚠ 读超时:免得一个连上就不说话的客户端把连接线程挂死。
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    let read = reader.read_line(&mut line)?;
    if read == 0 {
        // 空连接(只是来探端口的)⇒ 不回话。
        return Ok(());
    }
    let response = inspect_response(&line, &slot);
    let mut out = stream;
    out.write_all(response.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

/// 一行请求 ⇒ 一行响应。
///
/// ⚠⚠ **安全边界第 2 条(只读)**在这里落地:`is_read_only_op` 之外的一切 op
///   一律**回绝** ⇒ 端点**没有**任何能改状态 / 能触发求值的入口。
/// ⚠ **安全边界第 3 条(无执行能力)**:这个函数能碰到的只有
///   `slot`(纯数据)⇒ 它**拿不到**求值器 ⇒ 就算 op 判定被改坏,也没有东西
///   可执行(守卫仍然会红,因为响应形状变了)。
fn inspect_response(line: &str, slot: &Arc<Mutex<Option<TaskSnapshot>>>) -> String {
    let request: Value = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "error": format!("request is not valid JSON: {e}"),
                "readonly": true,
            })
            .to_string()
        }
    };
    let op = request.get("op").and_then(|v| v.as_str()).unwrap_or("");
    if !is_read_only_op(op) {
        // ⚠ **回绝**,不是「换个 op 照样办」。
        return json!({
            "error": "read-only inspection endpoint: this op is not allowed",
            "readonly": true,
            "rejected_op": op,
        })
        .to_string();
    }
    let snapshot = match slot.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    let snapshot = snapshot.unwrap_or_else(TaskSnapshot::empty);
    tasks_json(&snapshot).to_string()
}

/// `wlwl tasks --addr <ADDR>` —— **只读客户端**:取回端点的快照并打印。
///
/// ⚠ 客户端**只发一个读 op** ⇒ 它自己也没有执行能力。
pub fn tasks_query_command(addr: &str) -> ExitCode {
    let target: SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: cannot parse endpoint address '{addr}': {e}");
            return ExitCode::from(1);
        }
    };
    let mut stream = match TcpStream::connect(target) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot connect to {target}: {e}");
            eprintln!(
                "hint: start one with `wlwl repl --serve {target}` (the endpoint is \
                 loopback-only and read-only)"
            );
            return ExitCode::from(1);
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    if let Err(e) = stream.write_all(b"{\"op\":\"tasks\"}\n") {
        eprintln!("error: cannot send the read request to {target}: {e}");
        return ExitCode::from(1);
    }
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => {
            eprintln!("error: {target} closed the connection without answering");
            return ExitCode::from(1);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("error: cannot read the answer from {target}: {e}");
            return ExitCode::from(1);
        }
    }
    // ⚠ **必须能解析** —— 「JSON 可解析」是端点的契约,不是愿望。
    let value: Value = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: the answer from {target} is not valid JSON: {e}");
            return ExitCode::from(1);
        }
    };
    match serde_json::to_string_pretty(&value) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: tasks serialize failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// 环回自检 —— 「起服务 ⇒ 查 tasks ⇒ JSON 可解析」+「写请求必须被拒」。
///
/// 返回 `(tasks_json_ok, readonly_rejected_ok)`。
///
/// ⚠ 这是端点**自己**证明自己没坏:连自己、问一次、解析一次、再发一个
///   **写**请求确认它被回绝。⇒ 端点起不来时,使用者第一眼看到的是
///   「端点自检没过」,而不是「端点起来了但行为不对」。
pub fn inspect_endpoint_selftest(addr: SocketAddr) -> (bool, bool) {
    let tasks_ok = match inspect_query(addr, "{\"op\":\"tasks\"}") {
        Ok(answer) => serde_json::from_str::<Value>(&answer).is_ok(),
        Err(_) => false,
    };
    // ⚠ 写请求:`op` 不在只读白名单里 ⇒ 必须被回绝(响应里带
    //   `"readonly": true`,且**不含**快照字段)。
    let rejected = match inspect_query(addr, "{\"op\":\"eval\",\"expr\":\"PRINT(1)\"}") {
        Ok(answer) => match serde_json::from_str::<Value>(&answer) {
            Ok(v) => {
                v.get("readonly").and_then(|b| b.as_bool()) == Some(true)
                    && v.get("tasks_schema_version").is_none()
            }
            Err(_) => false,
        },
        Err(_) => false,
    };
    (tasks_ok, rejected)
}

/// [W-30 / F-3] 检视端的守卫用例。
///
/// ⚠ 这几条是**安全边界**的锁:边界被放宽 ⇒ 这里**变红**。
#[cfg(test)]
mod inspect_tests {
    use super::*;
    use wlwl_eval::runtime::SuspendPoint;

    /// **反向守卫**:端点只放行**读** op ⇒ 写请求必须被**回绝**。
    ///
    /// ⚠ 把 `is_read_only_op` 的判定去掉(或把调用它的那道 `if` 删掉)⇒
    ///   写请求也会拿到快照 ⇒ 这条**变红**。
    #[test]
    fn inspect_endpoint_rejects_non_read_ops() {
        let slot = Arc::new(Mutex::new(None));
        let ep = start_inspect_endpoint("127.0.0.1:0", Arc::clone(&slot)).expect("binds");
        // 读 op ⇒ 放行,且答案**必须**是可解析的 JSON。
        let answer = inspect_query(ep.addr, "{\"op\":\"tasks\"}").expect("answered");
        let value: Value = serde_json::from_str(answer.trim()).expect("valid JSON");
        assert_eq!(value["tasks_schema_version"], SCHEMA_VERSION);
        assert_eq!(value["readonly"], json!(true));
        // 写 op ⇒ **回绝**(不是「换个 op 照样办」)。
        let answer =
            inspect_query(ep.addr, "{\"op\":\"eval\",\"expr\":\"PRINT(1)\"}").expect("answered");
        let value: Value = serde_json::from_str(answer.trim()).expect("valid JSON");
        assert_eq!(value["readonly"], json!(true));
        assert_eq!(value["rejected_op"], json!("eval"));
        assert!(
            value.get("tasks_schema_version").is_none(),
            "a refused op must NOT get the snapshot: {value}"
        );
    }

    /// **安全边界第 1 条**:非回环地址**拒绝启动**。
    ///
    /// ⚠ `0.0.0.0` = 所有网卡可见 ⇒ 一个「能读进程状态」的端口就是信息泄露面。
    #[test]
    fn inspect_endpoint_refuses_non_loopback() {
        let slot = Arc::new(Mutex::new(None));
        let err = match start_inspect_endpoint("0.0.0.0:0", slot) {
            Ok(_) => panic!("the inspection endpoint must refuse a non-loopback address"),
            Err(e) => e,
        };
        assert!(err.contains("loopback-only"), "{err}");
    }

    /// [W-30 / F-3] **端到端**:调度器把一个任务挂到通道上 ⇒ 检视槽里必须看得到
    /// 「**挂在哪个挂起点**」+「已运行时长」。
    ///
    /// ⚠ 「suspended」只有**运行期**看得到:程序跑完之后,作用域退出会把没结束的
    ///   任务**硬取消**(并发册 §17.1)⇒ 这条用例守着「发布 → 快照 → JSON」整条路。
    #[test]
    fn a_parked_task_shows_its_suspend_point_in_the_snapshot() {
        use wlwl_eval::channel::Channel;
        use wlwl_eval::runtime::{Direction, Scheduler, ScopeId, TaskId};
        use wlwl_eval::task::Task;
        use wlwl_eval::{ChannelId, Env, Value};

        let slot = Arc::new(Mutex::new(None));
        let mut sched = Scheduler::new();
        sched.channels.push(Channel::new(ChannelId(0), 1, 0));
        sched.inspect_slot = Some(Arc::clone(&slot));
        let gen = sched.bump_generation();
        sched.push_task(Task::new_pending(
            TaskId(0),
            gen,
            Value::Null,
            Env::new(),
            ScopeId(0),
        ));
        // 挂到通道上（发送方向）⇒ 状态转 Suspended。
        sched
            .park_for_channel_op(
                TaskId(0),
                ChannelId(0),
                Direction::Send,
                Some(Value::Integer(7)),
            )
            .expect("park succeeds");
        let snapshot = slot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .expect("the slot was published");
        let v = tasks_json(&snapshot);
        assert_eq!(v["tasks"][0]["state"], json!("suspended"));
        assert_eq!(
            v["tasks"][0]["suspend_point"]["kind"],
            json!("channel_send")
        );
        assert_eq!(v["tasks"][0]["suspend_point"]["detail"], json!("channel=0"));
        assert!(
            v["tasks"][0]["elapsed_ms"].as_i64().unwrap_or(-1) >= 0,
            "elapsed_ms must be a non-negative duration"
        );
    }

    /// 快照的**形状**:任务树 / 挂起点 / 已运行时长 / 截止时间。
    #[test]
    fn tasks_json_carries_tree_suspend_point_and_elapsed() {
        let snapshot = TaskSnapshot {
            now_ms: 1_000,
            tasks: vec![TaskSnapshotEntry {
                id: 0,
                generation: 1,
                state: "suspended",
                suspend_point: Some(SuspendPoint {
                    kind: "channel_recv",
                    detail: Some("channel=0".to_string()),
                }),
                parent_scope: 0,
                started_at_ms: 900,
                elapsed_ms: 100,
                deadline_ms: None,
                awaited: false,
                cancel_requested: false,
            }],
            scopes: vec![ScopeSnapshotEntry {
                id: 0,
                parent: None,
                task_ids: vec![0],
                cancelled: false,
            }],
        };
        let v = tasks_json(&snapshot);
        assert_eq!(v["tasks_schema_version"], SCHEMA_VERSION);
        assert_eq!(v["readonly"], json!(true));
        assert_eq!(v["clock"], "monotonic_ms_since_process_start");
        assert_eq!(v["task_count"], json!(1));
        assert_eq!(v["tasks"][0]["state"], json!("suspended"));
        assert_eq!(
            v["tasks"][0]["suspend_point"]["kind"],
            json!("channel_recv")
        );
        assert_eq!(v["tasks"][0]["suspend_point"]["detail"], json!("channel=0"));
        assert_eq!(v["tasks"][0]["elapsed_ms"], json!(100));
        // 任务树 = 作用域树 + 挂在上面的任务。
        assert_eq!(v["task_tree"][0]["scope_id"], json!(0));
        assert_eq!(v["task_tree"][0]["tasks"][0]["id"], json!(0));
        assert!(v["task_tree"][0]["children"]
            .as_array()
            .expect("array")
            .is_empty());
        // 整份文档**必须**能被 JSON 解析器读回来(消费方的第一道门槛)。
        let text = serde_json::to_string(&v).unwrap();
        let back: Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(back["tasks"][0]["id"], json!(0));
    }
}

/// 向端点发**一行**请求、读回**一行**响应。
fn inspect_query(addr: SocketAddr, request: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(addr).map_err(|e| format!("connect failed: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    stream
        .write_all(format!("{request}\n").as_bytes())
        .map_err(|e| format!("write failed: {e}"))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("read failed: {e}"))?;
    if line.trim().is_empty() {
        return Err("empty answer".to_string());
    }
    Ok(line)
}

/// 静态契约段的诊断码:条件与「是否恒警告」。
///
/// 这份清单服务工具(按码分流渲染),不是给人读的表 —— 人读的表在
/// spec §11.2(Step 12 派生)。
fn static_contract_codes() -> Value {
    json!([
        { "code": "E0110", "warn": "W0110", "always_warning": false,
          "condition": "annotation mismatch at a boundary (param / LET annotation vs value)" },
        { "code": "E0111", "warn": "W0111", "always_warning": false,
          "condition": "call mismatch (argument count or argument type)" },
        { "code": "E0112", "warn": "W0112", "always_warning": false,
          "condition": "return type mismatch" },
        { "code": "E0113", "warn": "W0113", "always_warning": false,
          "condition": "exported / imported name is not declared in the module contract" },
        { "code": "E0114", "warn": "W0114", "always_warning": false,
          "condition": "the contract declares a name the module does not export" },
        { "code": "E0115", "warn": "W0115", "always_warning": false,
          "condition": "module signature type conflicts with the implementation annotation" },
        { "code": "E0116", "warn": "W0116", "always_warning": false,
          "condition": "non-exhaustive match (a constructor is missing and the default arm was omitted)" },
        { "code": "W0117", "warn": null, "always_warning": true,
          "condition": "unreachable match clause or a dead default arm; never escalates in `error` mode, and has no E counterpart" }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_lists_exports_with_their_declared_types() {
        let ast = wlwl_parser::parse(
            "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
             LET(PI, 3);\n\
             EXPORT([\"add\", \"PI\"]);\n",
            "math.wll",
        )
        .expect("parses");
        let v = interface_of("math.wll", &ast);
        assert_eq!(v["interface_schema_version"], SCHEMA_VERSION);
        assert_eq!(v["module"], "math.wll");
        assert_eq!(v["signature_file"], "math.wll.sig");
        // 没写 SEALED → `sealed` 是 null(「没声明」≠「声明了空面」)。
        assert!(v["sealed"].is_null());
        let exports = v["exports"].as_array().expect("array");
        assert_eq!(exports.len(), 2);
        let add = exports
            .iter()
            .find(|e| e["name"] == "add")
            .expect("add is exported");
        assert_eq!(add["kind"], "function");
        assert_eq!(add["params"][0], "INTEGER");
        assert_eq!(add["returns"], "INTEGER");
        let pi = exports
            .iter()
            .find(|e| e["name"] == "PI")
            .expect("PI is exported");
        assert_eq!(pi["kind"], "value");
        assert_eq!(pi["type"], "INTEGER");
    }

    #[test]
    fn interface_reports_the_sealed_surface() {
        let ast = wlwl_parser::parse(
            "SEALED([\"open\"]);\nLET(open, 1);\nEXPORT([\"open\"]);\n",
            "m.wll",
        )
        .expect("parses");
        let v = interface_of("m.wll", &ast);
        assert_eq!(v["sealed"], json!(["open"]));
    }

    #[test]
    fn interface_is_valid_json_for_a_bounded_generic() {
        // Step 9 的带约束变量要能原样出现在 interface 里(工具会照着它
        // 理解模块的承诺)。
        let ast = wlwl_parser::parse(
            "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, a));\n\
             EXPORT([\"max\"]);\n",
            "g.wll",
        )
        .expect("parses");
        let v = interface_of("g.wll", &ast);
        let text = serde_json::to_string(&v).unwrap();
        assert!(text.contains("T: Comparable"), "{text}");
        // 真的能被 JSON 解析器读回来(工具的第一道门槛)。
        let back: Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(back["exports"][0]["params"][0], "T: Comparable");
    }

    /// schema 里的类型名必须与静态层真的认识的那批**逐个对得上**。
    /// 静态层加了新类型头而这里忘了改,工具就会按一份过期清单说话。
    #[test]
    fn schema_type_names_match_the_static_layer() {
        let v = type_schema();
        let listed: Vec<&str> = v["types"]
            .as_array()
            .expect("types array")
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        for name in [
            "INTEGER", "FLOAT", "STRING", "BOOLEAN", "NULL", "ARRAY", "DICT", "OPTION", "RESULT",
            "FUN", "DYNAMIC",
        ] {
            assert!(listed.contains(&name), "{} missing from schema", name);
        }
        // 约束清单:`Comparable` 只有一个,且成员集合是量出来的那三个。
        let bounds = v["bounds"].as_array().expect("bounds array");
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0]["name"], "Comparable");
        assert_eq!(bounds[0]["members"], json!(["INTEGER", "FLOAT", "STRING"]));
    }

    #[test]
    fn schema_diagnostic_codes_carry_their_warn_pairing() {
        let v = type_schema();
        let codes = v["diagnostics"].as_array().expect("diagnostics array");
        let by_code = |c: &str| {
            codes
                .iter()
                .find(|d| d["code"] == c)
                .unwrap_or_else(|| panic!("{} missing", c))
        };
        for (e, w) in [("E0110", "W0110"), ("E0113", "W0113"), ("E0116", "W0116")] {
            assert_eq!(by_code(e)["warn"], w, "{} must pair with {}", e, w);
            assert_eq!(by_code(e)["always_warning"], false);
        }
        // W0117 的不对称是**故意的**,schema 必须如实说(工具按它决定
        // 拦不拦构建)。
        let orphan = by_code("W0117");
        assert!(orphan["warn"].is_null());
        assert_eq!(orphan["always_warning"], true);
    }
}
