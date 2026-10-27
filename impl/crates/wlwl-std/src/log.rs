//! [W-26 / v0.12.x] `wlwl:std.log` —— **结构化日志**(JSON 行)+ **运行时指标**。
//!
//! ## 这个模块是什么
//!
//! 两个子系统，**一个文件、一个命名空间**：
//!
//! 1. **结构化日志**：`LOG(level, message, fields?)` 写**一行一个 JSON 对象**
//!    （不是自由文本）⇒ 日志能被机器解析（`grep` / `jq` / 采集器都吃它）。
//! 2. **运行时指标**：进程级**注册表** + **计数器** + **延迟直方图** +
//!    **快照**（`LOG_METRICS()`）。
//!
//! ## 形态约定（与 `wlwl:std.net` / `wlwl:std.fs` 同款）
//!
//! - 运行期可预期的失败 ⇒ **`ERR([kind:"LogError", op, reason])` 值**，不是原生诊断。
//! - 程序员错误（元数 / 类型）⇒ `E0022` / `E0030` 诊断。
//! - 句柄是**不透明 `INTEGER`**（进程级表的下标）。表里**只放整数**
//!   （计数器的值、直方图的桶计数），不放任何 `Value`。
//!
//! ## 级别过滤
//!
//! `TRACE < DEBUG < INFO < WARN < ERROR`。阈值来自：
//!
//! 1. `LOG_SET_LEVEL(level)` **显式**设定（进程级）；
//! 2. 缺省 ⇒ 宿主环境里的 **`WLWL_LOG_LEVEL`**；
//! 3. 再缺省 ⇒ **`INFO`**。
//!
//! ⚠ **`WLWL_LOG_LEVEL` 读的是 `ctx.env`**（`StdCtx` 启动时从操作系统环境拍的
//!   那份快照），**不是**直接读 `std::env` —— 与 `wlwl:std.env` 的立场一致
//!   （`std.env` 的成员也只读 `ctx.env`）。⇒ 真实程序里
//!   `WLWL_LOG_LEVEL=debug wlwl app.wll` 照样生效（`StdCtx::from_process`
//!   把它拍进了快照），而契约测试可以**注入**它，不必改进程环境。
//!
//! ⚠ **低于阈值 ⇒ 不输出，且返回 `NULL`** —— 日志级别**不得**改变程序语义：
//!   同一段脚本在 `INFO` 与 `TRACE` 下必须**行为一致**，只是话多话少。
//!
//! ## JSON 行的形状（**键序稳定**）
//!
//! ```text
//! {"ts":1750000000000,"level":"INFO","msg":"hello","fields":{"k":"v","n":1}}
//! ```
//!
//! - 顶层键序**固定**为 `ts` → `level` → `msg` → `fields` ⇒ 便于 `grep` 与对拍。
//! - `fields` 的键**按字典序排**（不是插入序）⇒ 同一个字典**逐次**产出同一行。
//! - `fields` **总是**存在（没给就是 `{}`）⇒ 形状稳定，解析端不用分支。
//! - `ts` = Unix 纪元起的**毫秒**（`INTEGER`），取自宿主的墙钟
//!   （`host.clock_wall_ms()`）⇒ 与 `std.time::NOW` 同源，测试气泡里是虚拟钟。
//!
//! ## 输出目标
//!
//! - 缺省 ⇒ **stderr**（运行日志不污染 stdout 的数据流）。
//! - `LOG_FILE(path)` ⇒ 切到**文件**，**追加写**（`create` + `append`）。
//!
//! ⚠ **审计日志不在本模块**：需求方要求「审计日志 + 运行日志分离」——
//!   审计是 **W-25** 的事，本模块**不碰**审计的任何语义。
//!   ⇒ 两者的分离靠**各自切各自的目标文件**实现（审计侧自己切），
//!   本模块只保证「运行日志切到哪儿，审计不会跟着跑」。
//!
//! ## 线程安全
//!
//! 仓内样板（`tcp.rs`）：`static X: LazyLock<Mutex<…>>` + 锁中毒
//! `unwrap_or_else(|e| e.into_inner())`（**中毒不 panic** —— 一个线程 panic
//! 不该把整个进程的日志系统带走）。
//!
//! - 计数器用 `AtomicI64::fetch_add` ⇒ **自增不丢更新**（并发下先读后写必错，
//!   这条由契约测试钉住）。
//! - 直方图用 `Mutex`（观测比自增少，且要改多个桶）。
//!
//! ## 指标的形状
//!
//! `LOG_METRICS()` 返回：
//!
//! ```text
//! {"counters": {"<名字>": <整数>},
//!  "histograms": {"<名字>": {"count": <n>, "sum": <ms>,
//!                            "buckets": {"1": <n>, …, "+Inf": <n>}}}}
//! ```
//!
//! - `counters` = **至少被自增过一次**的计数器（Prometheus 同款：没有样本
//!   就不导出）。名字**按字典序**排。
//! - `histograms` = **至少被观测过一次**的直方图。桶是**累积**的
//!   （`"1"` = 延迟 ≤ 1 ms 的观测数），`"+Inf"` = 观测总数。
//! - 名字与键**都按字典序**排 ⇒ 快照**逐次**一致（可对拍）。
//!
//! ## 余项：当前连接数 / 任务数
//!
//! 本模块**不读**求值器内部状态（那要跨 crate 改 `wlwl-eval`）⇒
//! 「当前连接数」「当前任务数」**不在**本项内。
//!
//! ⇒ **交接给 W-12 / W-20**：它们在**建连 / 关连**时上报即可，
//! 机制已经齐了（一个计数器 + 一个自增）：
//!
//! ```text
//! LET(h, METRIC_COUNTER("http.connections.current"))
//! METRIC_INC(h, 1)   // 建连
//! METRIC_INC(h, -1)  // 关连
//! ```
//!
//! ⚠ `METRIC_COUNTER(name)` 是**幂等**的：同名再调一次**拿回同一个句柄**
//!   ⇒ HTTP 层可以在初始化时取一次句柄、之后只 `METRIC_INC`。

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_error::WlwlResult;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.log",
    functions: &[
        // ── 结构化日志 ────────────────────────────────────────────────
        ("LOG", log as StdFn),
        ("LOG_SET_LEVEL", log_set_level as StdFn),
        ("LOG_FILE", log_file as StdFn),
        // ── 运行时指标 ────────────────────────────────────────────────
        ("LOG_METRICS", log_metrics as StdFn),
        ("METRIC_COUNTER", metric_counter as StdFn),
        ("METRIC_INC", metric_inc as StdFn),
        ("METRIC_TIMER", metric_timer as StdFn),
    ],
};

// ── 级别 ────────────────────────────────────────────────────────────────────

/// 日志级别。**声明顺序 = 严重度顺序**（`derive(Ord)` 给出 `TRACE < ERROR`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    /// 级别名 → 级别。**大小写不敏感**、允许首尾空白。
    ///
    /// ⚠ 认不得 ⇒ `None`（**不**猜一个默认级别）：猜了就等于把「写错了级别名」
    ///   这件事藏起来，而那恰恰是最该被看见的错。
    fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_uppercase().as_str() {
            "TRACE" => Some(Level::Trace),
            "DEBUG" => Some(Level::Debug),
            "INFO" => Some(Level::Info),
            "WARN" => Some(Level::Warn),
            "ERROR" => Some(Level::Error),
            _ => None,
        }
    }

    /// 级别 → 写进日志行的**大写**名字。
    fn name(self) -> &'static str {
        match self {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

/// 缺省级别的环境变量名（读 `ctx.env`）。
const ENV_LEVEL: &str = "WLWL_LOG_LEVEL";

// ── 进程级状态：级别阈值 + 输出目标 ─────────────────────────────────────────

/// **进程级**的日志状态。
struct LogState {
    /// `Some` = 被 `LOG_SET_LEVEL` **显式**设定过；`None` = 走缺省
    /// （`WLWL_LOG_LEVEL` → `INFO`）。
    level: Option<Level>,
    /// `Some` = 输出目标已切到文件；`None` = 输出到 stderr。
    file: Option<File>,
}

static STATE: LazyLock<Mutex<LogState>> = LazyLock::new(|| {
    Mutex::new(LogState {
        level: None,
        file: None,
    })
});

/// **宿主 / 测试用**：把本模块的**进程级**状态复位（级别回到缺省、输出目标
/// 回到 stderr、指标表清空）。
///
/// ⚠ **它不是 `std.log` 的成员** —— 不在 `SPEC` 里，脚本调不到它。
///
/// ⚠ **为什么需要它**：级别阈值 / 输出目标 / 指标表都是**进程级**的，而契约
///   测试在**同一个进程**里并行跑 ⇒ 没有复位手段，「环境变量缺省级别」那条
///   就钉不成确定性断言（上一个用例留下的显式级别会把它盖住）。
pub fn reset_state() {
    {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        st.level = None;
        st.file = None;
    }
    let mut reg = METRICS.lock().unwrap_or_else(|e| e.into_inner());
    reg.by_name.clear();
    reg.by_handle.clear();
    reg.next_handle = 1;
}

// ── 进程级状态：指标注册表 ─────────────────────────────────────────────────

/// 延迟直方图的**桶上界**（毫秒）。`+Inf` 桶是溢出桶（不在表里，单列）。
const BUCKET_BOUNDS_MS: [i64; 9] = [1, 5, 10, 25, 50, 100, 250, 500, 1000];

/// 一个指标的直方图部分。
///
/// ⚠ 桶计数存的是**每桶**的观测数（不是累积）；**累积**在快照时算出来 ⇒
///   观测时只改一个桶，锁内工作量最小。
struct Histogram {
    count: i64,
    sum: i64,
    /// `buckets[i]` = 落在第 `i` 个桶里的观测数（`i == BUCKET_BOUNDS_MS.len()`
    /// 那一格是 `+Inf` 溢出桶）。
    buckets: [i64; BUCKET_BOUNDS_MS.len() + 1],
}

impl Histogram {
    fn new() -> Histogram {
        Histogram {
            count: 0,
            sum: 0,
            buckets: [0; BUCKET_BOUNDS_MS.len() + 1],
        }
    }

    /// 记一次观测（毫秒）。返回累计观测数。
    fn observe(&mut self, ms: i64) -> i64 {
        let slot = BUCKET_BOUNDS_MS
            .iter()
            .position(|&bound| ms <= bound)
            .unwrap_or(BUCKET_BOUNDS_MS.len());
        self.buckets[slot] += 1;
        self.count += 1;
        self.sum += ms;
        self.count
    }

    /// 累积桶计数（`"1"` = 延迟 ≤ 1 ms 的观测数）。
    fn cumulative_buckets(&self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.buckets.len());
        let mut running = 0;
        for c in self.buckets {
            running += c;
            out.push(running);
        }
        out
    }
}

/// 一个**已注册**的指标：一个计数器 + 一个直方图（共享同一个句柄）。
struct Metric {
    name: String,
    /// 计数器值。**原子**自增 ⇒ 并发下不丢更新。
    counter: AtomicI64,
    /// 计数器**是否被自增过**（哪怕又减回 0）⇒ 快照只导出**有样本**的项。
    touched: AtomicBool,
    /// 延迟直方图。
    histogram: Mutex<Histogram>,
}

/// 进程级指标注册表：名字 ⇄ 句柄。
struct Registry {
    /// 名字 → 句柄（`BTreeMap` ⇒ 名字按字典序，**快照逐次一致**）。
    by_name: BTreeMap<String, i64>,
    /// 句柄 → 指标。
    by_handle: BTreeMap<i64, Arc<Metric>>,
    /// 下一个句柄（从 `1` 起 ⇒ `0` 永远不是合法句柄）。
    next_handle: i64,
}

static METRICS: LazyLock<Mutex<Registry>> = LazyLock::new(|| {
    Mutex::new(Registry {
        by_name: BTreeMap::new(),
        by_handle: BTreeMap::new(),
        next_handle: 1,
    })
});

/// 注册（或取回）一个**同名**指标的句柄。
///
/// ⚨ **幂等**：同名再调一次**拿回同一个句柄**（不重置计数）⇒ 上层可以
///   在初始化时取一次句柄、之后只 `METRIC_INC`。
fn register_metric(name: &str) -> i64 {
    let mut reg = METRICS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&h) = reg.by_name.get(name) {
        return h;
    }
    let handle = reg.next_handle;
    reg.next_handle += 1;
    let metric = Arc::new(Metric {
        name: name.to_string(),
        counter: AtomicI64::new(0),
        touched: AtomicBool::new(false),
        histogram: Mutex::new(Histogram::new()),
    });
    reg.by_name.insert(name.to_string(), handle);
    reg.by_handle.insert(handle, metric);
    handle
}

/// 按句柄取指标（**克隆 Arc** 后立刻放锁 ⇒ 计数/观测**不**在注册表的锁里做）。
fn metric_by_handle(handle: i64) -> Option<Arc<Metric>> {
    let reg = METRICS.lock().unwrap_or_else(|e| e.into_inner());
    reg.by_handle.get(&handle).cloned()
}

// ── 诊断 / 值级错误助手 ────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: &str) -> WlwlError {
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

fn str_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<String, WlwlError> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::CellStr(c) => Ok(c.as_str()),
        other => Err(type_err(host, name, "a string", other)),
    }
}

fn int_arg(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<i64, WlwlError> {
    match v {
        Value::Integer(i) => Ok(*i),
        other => Err(type_err(host, name, "an integer", other)),
    }
}

/// `ERR([kind: "LogError", op, reason])` —— **值**，不是原生诊断。
///
/// ⚠ 与 `std.fs` / `std.process` / `std.compress` **同构**，只是 `kind` 不同：
///   「打不开日志文件」「级别名不认识」「句柄不存在」都是**可预期的运行期
///   结果** ⇒ 调用方多半要 `IS_ERR` 分支，而不是让整个程序终止。
fn log_err(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("LogError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

fn ok(v: Value) -> Result<Outcome, WlwlError> {
    Ok(Outcome::normal(v))
}

// ── JSON 行 ─────────────────────────────────────────────────────────────────

/// 把字符串写成 JSON 字符串字面量（含引号）。
///
/// ⚠ 转义集 = JSON 规范要求的那一套（`"` / `\` / 控制字符）；
///   **非 ASCII 字符原样透出**（UTF-8 本身就是合法 JSON 源码）。
fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// 把一个 `Value` 写成 JSON 值。
///
/// ⚠ **只接受 JSON 能表示的形状**（字符串 / 整数 / 有限浮点 / 布尔 / null /
///   数组 / 字典）。其余（闭包、原生函数、`ERR`、非有限浮点……）⇒ `Err`，
///   由调用方包成 `LogError` **值**（数据问题，不是程序员类型错）。
fn json_value(out: &mut String, v: &Value) -> Result<(), String> {
    match v {
        Value::Null => out.push_str("null"),
        Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Integer(i) => out.push_str(&i.to_string()),
        Value::Float(f) => {
            if !f.is_finite() {
                return Err(format!("non-finite float {f} is not JSON-serializable"));
            }
            out.push_str(&f.to_string());
        }
        Value::String(s) => json_string(out, s),
        Value::CellStr(c) => json_string(out, &c.as_str()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                json_value(out, item)?;
            }
            out.push(']');
        }
        // 累加器变体：可观察面与 `Array` / `Dict` **逐字相同**（`ADR-0033`）⇒
        // 这里**照样**按数组 / 对象写，别把它们判成「不可序列化」。
        Value::Cell(c) => {
            let items = c.borrow();
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                json_value(out, item)?;
            }
            out.push(']');
        }
        Value::Dict(d) => json_object(out, d.entries())?,
        Value::DictCell(c) => {
            let inner = c.inner_rc();
            json_object(out, inner.entries())?;
        }
        // `OK(v)` 只是一层包装 ⇒ 拆掉（与 `std.json` 的边界转换同款）。
        Value::Ok(inner) => json_value(out, inner)?,
        Value::Err(_) => return Err("an ERR value is not JSON-serializable".to_string()),
        other => {
            return Err(format!(
                "{} is not JSON-serializable",
                crate::value_kind(other)
            ));
        }
    }
    Ok(())
}

/// 把字典条目写成 JSON 对象。**键按字典序排** ⇒ 逐次产出同一段文本。
fn json_object(out: &mut String, entries: &[(Value, Value)]) -> Result<(), String> {
    let mut pairs: Vec<(String, &Value)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let key = match k {
            Value::String(s) => s.clone(),
            Value::CellStr(c) => c.as_str(),
            other => {
                return Err(format!(
                    "a dict key must be a string, got {}",
                    crate::value_kind(other)
                ));
            }
        };
        pairs.push((key, v));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    out.push('{');
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_string(out, k);
        out.push(':');
        json_value(out, v)?;
    }
    out.push('}');
    Ok(())
}

/// 拼出一整行 JSON（**不含**换行）。
///
/// ⚠ 顶层键序**固定**：`ts` → `level` → `msg` → `fields`。
fn format_line(
    ts: i64,
    level: Level,
    msg: &str,
    fields: &[(Value, Value)],
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("{\"ts\":");
    out.push_str(&ts.to_string());
    out.push_str(",\"level\":");
    json_string(&mut out, level.name());
    out.push_str(",\"msg\":");
    json_string(&mut out, msg);
    out.push_str(",\"fields\":");
    json_object(&mut out, fields)?;
    out.push('}');
    Ok(out)
}

// ── 成员：结构化日志 ────────────────────────────────────────────────────────

/// `LOG(level, message, fields?) -> NULL`
///
/// 写一条**结构化日志**（一行一个 JSON 对象）。
///
/// ⚠ **返回值恒为 `NULL`**（写没写、级别够不够，都一样）—— 日志**不得**改变
///   程序语义：同一段脚本在 `INFO` 与 `TRACE` 下行为必须一致。
///
/// ⚠ 低于阈值 ⇒ **不输出**（也**不**算失败）。
pub fn log(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "LOG";
    if args.len() < 2 || args.len() > 3 {
        return Err(arity(host, NAME, args.len(), "2 or 3"));
    }
    let level_name = str_arg(host, NAME, &args[0])?;
    let msg = str_arg(host, NAME, &args[1])?;
    let level = match Level::parse(&level_name) {
        Some(l) => l,
        None => {
            return ok(log_err(
                NAME,
                format!(
                    "unknown level {level_name:?}; expected one of TRACE/DEBUG/INFO/WARN/ERROR"
                ),
            ))
        }
    };
    let fields: Vec<(Value, Value)> = match args.get(2) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Dict(d)) => d.entries().to_vec(),
        Some(Value::DictCell(c)) => {
            let inner = c.inner_rc();
            inner.entries().to_vec()
        }
        Some(other) => {
            return Err(type_err(host, NAME, "a dict for fields", other));
        }
    };

    // 低于阈值 ⇒ 不输出，但**不改变语义** ⇒ 照样返回 `NULL`。
    if level < effective_level(host) {
        return ok(Value::Null);
    }

    let ts = host.clock_wall_ms();
    let line = match format_line(ts, level, &msg, &fields) {
        Ok(l) => l,
        Err(reason) => return ok(log_err(NAME, reason)),
    };
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    match write_line(&mut st, &line) {
        Ok(()) => ok(Value::Null),
        Err(e) => ok(log_err(NAME, format!("writing the log line failed: {e}"))),
    }
}

/// `LOG_SET_LEVEL(level) -> NULL`
///
/// 设定**级别阈值**（进程级）。级别名大小写不敏感。
///
/// ⚠ **空字符串是唯一的例外**：它**复位**阈值（回到缺省：`WLWL_LOG_LEVEL`
///   → `INFO`）。没有复位手段的话，「环境变量决定缺省级别」这条就钉不成
///   确定性断言（上一个用例留下的显式级别会把它盖住）。
///
/// ⚠ 认不得的级别名 ⇒ **`ERR([kind:"LogError", …])` 值**，且**当前阈值不变**
///   （失败不能顺手把日志级别改掉）。
pub fn log_set_level(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "LOG_SET_LEVEL";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), "1"));
    }
    let name = str_arg(host, NAME, &args[0])?;
    if name.is_empty() {
        // 复位：回到缺省（`WLWL_LOG_LEVEL` → `INFO`）。
        STATE.lock().unwrap_or_else(|e| e.into_inner()).level = None;
        return ok(Value::Null);
    }
    let level = match Level::parse(&name) {
        Some(l) => l,
        None => {
            return ok(log_err(
                NAME,
                format!("unknown level {name:?}; expected one of TRACE/DEBUG/INFO/WARN/ERROR"),
            ))
        }
    };
    STATE.lock().unwrap_or_else(|e| e.into_inner()).level = Some(level);
    ok(Value::Null)
}

/// `LOG_FILE(path) -> NULL`
///
/// 把**运行日志**的输出目标切到文件（**追加写**：文件不存在则创建）。
///
/// ⚠ 打不开 ⇒ **`ERR([kind:"LogError", …])` 值**，且**目标不变**
///   （失败不能顺手把日志扔进 stderr 或半开的文件）。
///
/// ⚠ **只管运行日志**：审计日志（W-25）是另一条线、另一个文件。
pub fn log_file(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "LOG_FILE";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), "1"));
    }
    let path = str_arg(host, NAME, &args[0])?;
    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            return ok(log_err(
                NAME,
                format!("cannot open the log file {path:?}: {e}"),
            ))
        }
    };
    STATE.lock().unwrap_or_else(|e| e.into_inner()).file = Some(file);
    ok(Value::Null)
}

// ── 成员：运行时指标 ───────────────────────────────────────────────────────

/// `METRIC_COUNTER(name) -> INTEGER`
///
/// 注册一个**同名**指标并返回它的句柄（不透明 `INTEGER`）。
///
/// ⚠ **幂等**：同名再调一次**拿回同一个句柄**，**不**清零已有计数。
pub fn metric_counter(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "METRIC_COUNTER";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), "1"));
    }
    let name = str_arg(host, NAME, &args[0])?;
    if name.is_empty() {
        return ok(log_err(NAME, "the metric name must not be empty"));
    }
    ok(Value::Integer(register_metric(&name)))
}

/// `METRIC_INC(handle, delta) -> INTEGER`
///
/// **原子**自增计数器，返回**新值**。
///
/// ⚠ 原子性是**可观测**的：N 个线程各 `+1` ⇒ 结果**恰好**是 N
///   （先读后写必丢更新 ⇒ 这条由契约测试钉住）。
///
/// ⚠ `delta` 可以是负数（那是减法）；句柄不存在 ⇒ **`ERR` 值**。
pub fn metric_inc(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "METRIC_INC";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), "2"));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let delta = int_arg(host, NAME, &args[1])?;
    let metric = match metric_by_handle(handle) {
        Some(m) => m,
        None => return ok(log_err(NAME, format!("unknown metric handle {handle}"))),
    };
    // ⚨ `fetch_add` 返回**旧值** ⇒ 新值 = 旧值 + delta。
    // ⚨ 这一步**必须**是原子的：先读后写在并发下必丢更新。
    let new = metric.counter.fetch_add(delta, Ordering::SeqCst) + delta;
    metric.touched.store(true, Ordering::SeqCst);
    ok(Value::Integer(new))
}

/// `METRIC_TIMER(handle, ms) -> INTEGER`
///
/// 记一次**延迟观测**（毫秒），返回累计观测数。
///
/// ⚠ 负延迟 ⇒ **`ERR` 值**（那不是观测，是数据坏了）。
pub fn metric_timer(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "METRIC_TIMER";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), "2"));
    }
    let handle = int_arg(host, NAME, &args[0])?;
    let ms = int_arg(host, NAME, &args[1])?;
    if ms < 0 {
        return ok(log_err(
            NAME,
            format!("the observed latency must be >= 0, got {ms}"),
        ));
    }
    let metric = match metric_by_handle(handle) {
        Some(m) => m,
        None => return ok(log_err(NAME, format!("unknown metric handle {handle}"))),
    };
    let count = {
        let mut h = metric.histogram.lock().unwrap_or_else(|e| e.into_inner());
        h.observe(ms)
    };
    ok(Value::Integer(count))
}

/// `LOG_METRICS() -> DICT`
///
/// 返回**指标快照**（JSON 可序列化）。
///
/// ⚠ 形状：`{"counters": {…}, "histograms": {…}}` —— 两段的键**都按字典序**
///   ⇒ 快照**逐次**一致（可对拍）。
pub fn log_metrics(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "LOG_METRICS";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), "0"));
    }
    ok(snapshot())
}

// ── 内部实现 ───────────────────────────────────────────────────────────────

/// 当前生效的级别阈值：显式设定 > `WLWL_LOG_LEVEL` > `INFO`。
fn effective_level(host: &mut dyn StdHost) -> Level {
    let explicit = STATE.lock().unwrap_or_else(|e| e.into_inner()).level;
    if let Some(l) = explicit {
        return l;
    }
    let from_env = host
        .ctx()
        .env
        .get(ENV_LEVEL)
        .and_then(|v| Level::parse(v.as_str()));
    from_env.unwrap_or(Level::Info)
}

/// 写一行（**带**换行）到当前目标：文件或 stderr。
fn write_line(st: &mut LogState, line: &str) -> std::io::Result<()> {
    match st.file.as_mut() {
        Some(f) => {
            f.write_all(line.as_bytes())?;
            f.write_all(b"\n")?;
            f.flush()
        }
        None => {
            let mut err = std::io::stderr().lock();
            err.write_all(line.as_bytes())?;
            err.write_all(b"\n")?;
            err.flush()
        }
    }
}

/// 指标快照。
fn snapshot() -> Value {
    let reg = METRICS.lock().unwrap_or_else(|e| e.into_inner());
    let mut counters: Vec<(Value, Value)> = Vec::new();
    let mut histograms: Vec<(Value, Value)> = Vec::new();
    for metric in reg.by_handle.values() {
        if metric.touched.load(Ordering::SeqCst) {
            counters.push((
                Value::String(metric.name.clone()),
                Value::Integer(metric.counter.load(Ordering::SeqCst)),
            ));
        }
        let buckets = {
            let h = metric.histogram.lock().unwrap_or_else(|e| e.into_inner());
            if h.count == 0 {
                None
            } else {
                Some((h.count, h.sum, h.cumulative_buckets()))
            }
        };
        if let Some((count, sum, cumulative)) = buckets {
            let mut bucket_entries: Vec<(Value, Value)> = Vec::with_capacity(cumulative.len());
            for (i, c) in cumulative.iter().enumerate() {
                let key = match BUCKET_BOUNDS_MS.get(i) {
                    Some(&bound) => bound.to_string(),
                    None => "+Inf".to_string(),
                };
                bucket_entries.push((Value::String(key), Value::Integer(*c)));
            }
            histograms.push((
                Value::String(metric.name.clone()),
                Value::dict(vec![
                    (Value::String("count".into()), Value::Integer(count)),
                    (Value::String("sum".into()), Value::Integer(sum)),
                    (Value::String("buckets".into()), Value::dict(bucket_entries)),
                ]),
            ));
        }
    }
    Value::dict(vec![
        (Value::String("counters".into()), Value::dict(counters)),
        (Value::String("histograms".into()), Value::dict(histograms)),
    ])
}

// ── 单元测试 ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    use wlwl_value::StdCtx;

    /// 进程内自增的取名序号（临时文件名用）。
    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// 用例之间的**全局闸**。
    ///
    /// ⚠ 本模块的状态（级别阈值 / 输出目标 / 指标表）是**进程级**的，而
    ///   libtest **并行**跑用例 ⇒ 没有闸，一个用例把级别设成 `ERROR`，
    ///   另一个用例的「环境变量缺省级别」断言就被盖住了（偶发红）。
    static GATE: Mutex<()> = Mutex::new(());

    /// 取闸 + **复位**模块状态（级别 / 目标 / 指标表）。
    fn gate() -> std::sync::MutexGuard<'static, ()> {
        let g = GATE.lock().unwrap_or_else(|e| e.into_inner());
        reset_state();
        g
    }

    /// 一个**构造上唯一**的临时文件路径（`$env:TEMP` 下）。
    ///
    /// ⚠ 名字**不**只由时钟决定（`common/mod.rs` 记着那个坑）：进程 id +
    ///   原子序号 + 纳秒，三段各管一段。
    fn temp_log_path(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, AtomicOrdering::Relaxed);
        std::env::temp_dir().join(format!(
            "wlwl_log_{tag}_{}_{seq}_{nanos}.jsonl",
            std::process::id()
        ))
    }

    /// 手写宿主：本模块只读 `ctx.env`，不回调宿主。
    struct Host(StdCtx);

    impl StdHost for Host {
        fn ctx(&mut self) -> &mut StdCtx {
            &mut self.0
        }
        fn call(&mut self, _f: &Value, _a: Vec<Value>, _n: &str) -> Result<Outcome, WlwlError> {
            panic!("std.log has no member that calls back into the host")
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

    fn host_with_env(pairs: &[(&str, &str)]) -> Host {
        let mut ctx = StdCtx::default();
        for (k, v) in pairs {
            ctx.env.insert((*k).to_string(), (*v).to_string());
        }
        Host(ctx)
    }

    fn s(v: &str) -> Value {
        Value::String(v.to_string())
    }

    // ── 级别 ────────────────────────────────────────────────────────────

    #[test]
    fn levels_are_ordered_and_parse_case_insensitively() {
        assert!(Level::Trace < Level::Debug);
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warn);
        assert!(Level::Warn < Level::Error);
        assert_eq!(Level::parse("info"), Some(Level::Info));
        assert_eq!(Level::parse(" WARN "), Some(Level::Warn));
        assert_eq!(Level::parse("verbose"), None);
        assert_eq!(Level::name(Level::Error), "ERROR");
    }

    // ── JSON 行 ──────────────────────────────────────────────────────────

    #[test]
    fn line_has_a_stable_key_order() {
        let fields = vec![
            (s("b"), Value::Integer(2)),
            (s("a"), Value::String("x".into())),
        ];
        let line = format_line(42, Level::Info, "hello", &fields).expect("formats");
        assert_eq!(
            line,
            r#"{"ts":42,"level":"INFO","msg":"hello","fields":{"a":"x","b":2}}"#
        );
    }

    #[test]
    fn line_escapes_the_json_specials() {
        let line = format_line(1, Level::Error, "a\"b\\c\nd\te", &[]).expect("formats");
        assert_eq!(
            line,
            r#"{"ts":1,"level":"ERROR","msg":"a\"b\\c\nd\te","fields":{}}"#
        );
        // 转义后仍是一条**合法** JSON。
        let parsed: serde_json::Value = serde_json::from_str(&line).expect("valid JSON");
        assert_eq!(parsed["msg"], "a\"b\\c\nd\te");
    }

    #[test]
    fn fields_may_hold_arrays_and_dicts() {
        let nested = Value::dict(vec![(s("k"), Value::Boolean(true))]);
        let fields = vec![(s("list"), Value::Array(vec![Value::Integer(1), nested]))];
        let line = format_line(7, Level::Warn, "m", &fields).expect("formats");
        assert_eq!(
            line,
            r#"{"ts":7,"level":"WARN","msg":"m","fields":{"list":[1,{"k":true}]}}"#
        );
    }

    #[test]
    fn a_non_serializable_field_is_an_err_value_not_a_diagnostic() {
        let _g = gate();
        let mut h = host_with_env(&[]);
        let out = log(
            &mut h,
            vec![
                s("INFO"),
                s("m"),
                Value::dict(vec![(
                    s("fn"),
                    Value::NativeFn {
                        name: "f".into(),
                        invoke: noop,
                    },
                )]),
            ],
        )
        .expect("returns a value");
        let Value::Err(payload) = out.value else {
            panic!("a non-serializable field must be an ERR value")
        };
        let Value::Dict(d) = *payload else { panic!() };
        let kind = d
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "kind"))
            .map(|(_, v)| v.clone());
        assert_eq!(kind, Some(Value::String("LogError".into())));
    }

    fn noop(_host: &mut dyn StdHost, _args: Vec<Value>) -> Result<Outcome, WlwlError> {
        ok(Value::Null)
    }

    // ── 级别过滤 / 输出目标 ──────────────────────────────────────────────

    #[test]
    fn a_level_below_the_threshold_is_not_written() {
        let _g = gate();
        let path = temp_log_path("filter");
        let mut h = host_with_env(&[]);
        log_set_level(&mut h, vec![s("ERROR")]).expect("sets the level");
        log_file(&mut h, vec![s(path.to_string_lossy().as_ref())]).expect("switches the target");
        let out = log(&mut h, vec![s("INFO"), s("suppressed")]).expect("returns a value");
        assert!(
            matches!(out.value, Value::Null),
            "a filtered log must still return NULL (the program semantics must not change)"
        );
        let body = std::fs::read_to_string(&path).expect("log file");
        assert!(
            body.is_empty(),
            "a line below the threshold must not be written, got {body:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_env_var_sets_the_default_level() {
        let _g = gate();
        let path = temp_log_path("env");
        let mut h = host_with_env(&[(ENV_LEVEL, "WARN")]);
        log_file(&mut h, vec![s(path.to_string_lossy().as_ref())]).expect("switches the target");
        log(&mut h, vec![s("INFO"), s("suppressed")]).expect("returns a value");
        log(&mut h, vec![s("WARN"), s("kept")]).expect("returns a value");
        let body = std::fs::read_to_string(&path).expect("log file");
        assert_eq!(body.lines().count(), 1, "body = {body:?}");
        assert!(body.contains("\"level\":\"WARN\""), "body = {body:?}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_unknown_level_is_an_err_value_and_keeps_the_threshold() {
        let _g = gate();
        let mut h = host_with_env(&[]);
        let out = log_set_level(&mut h, vec![s("VERBOSE")]).expect("returns a value");
        let Value::Err(_) = out.value else {
            panic!("an unknown level must be an ERR value")
        };
    }

    #[test]
    fn the_file_target_appends() {
        let _g = gate();
        let path = temp_log_path("append");
        let mut h = host_with_env(&[]);
        for msg in ["one", "two"] {
            log_file(&mut h, vec![s(path.to_string_lossy().as_ref())]).expect("switches");
            log(&mut h, vec![s("INFO"), s(msg)]).expect("returns a value");
        }
        let body = std::fs::read_to_string(&path).expect("log file");
        assert_eq!(body.lines().count(), 2, "body = {body:?}");
        let _ = std::fs::remove_file(&path);
    }

    // ── 指标 ─────────────────────────────────────────────────────────────

    #[test]
    fn the_counter_is_exact_under_concurrency() {
        let _g = gate();
        const THREADS: i64 = 8;
        const PER_THREAD: i64 = 500;
        let handle = register_metric("unit.counter.atomic");
        let metric = metric_by_handle(handle).expect("registered");
        let mut handles = Vec::new();
        for _ in 0..THREADS {
            let m = Arc::clone(&metric);
            handles.push(std::thread::spawn(move || {
                for _ in 0..PER_THREAD {
                    // ⚨ 这就是**成员里那一步**的等价物：`fetch_add` 返回旧值、
                    //   新值 = 旧值 + delta。改成「先读后写」这条必红。
                    m.counter.fetch_add(1, Ordering::SeqCst);
                    m.touched.store(true, Ordering::SeqCst);
                }
            }));
        }
        for h in handles {
            h.join().expect("thread");
        }
        assert_eq!(
            metric.counter.load(Ordering::SeqCst),
            THREADS * PER_THREAD,
            "a lost update means the increment was not atomic"
        );
    }

    #[test]
    fn the_snapshot_shape_is_stable() {
        let _g = gate();
        let mut h = host_with_env(&[]);
        let handle = metric_counter(&mut h, vec![s("unit.snapshot")]).expect("registers");
        let Value::Integer(handle) = handle.value else {
            panic!("METRIC_COUNTER must return an INTEGER handle")
        };
        metric_inc(&mut h, vec![Value::Integer(handle), Value::Integer(7)]).expect("increments");
        let snap = snapshot();
        let Value::Dict(top) = snap else {
            panic!("snapshot must be a dict")
        };
        let names: Vec<String> = top
            .iter()
            .map(|(k, _)| match k {
                Value::String(s) => s.clone(),
                other => panic!("top key must be a string, got {other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            vec!["counters".to_string(), "histograms".to_string()]
        );
        let _ = handle;
    }

    #[test]
    fn the_histogram_buckets_are_cumulative() {
        let mut h = Histogram::new();
        h.observe(1);
        h.observe(7);
        h.observe(2000);
        let cumulative = h.cumulative_buckets();
        assert_eq!(cumulative[0], 1, "the <=1ms bucket");
        assert_eq!(cumulative[1], 1, "the <=5ms bucket");
        // ⚠ 7 ms 落在「≤10 ms」这一档 ⇒ 累积到 2（不是 1）。
        assert_eq!(cumulative[2], 2, "the <=10ms bucket");
        assert_eq!(cumulative[3], 2, "the <=25ms bucket");
        assert_eq!(
            *cumulative.last().unwrap(),
            3,
            "the +Inf bucket is the total"
        );
        assert_eq!(h.count, 3);
        assert_eq!(h.sum, 2008);
    }
}
