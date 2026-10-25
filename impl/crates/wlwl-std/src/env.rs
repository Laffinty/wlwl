//! [v0.11.3 M6 / addendum-06] `wlwl:std.env` —— 进程环境与命令行参数。
//!
//! **R2 全员**。数据来源是 `StdCtx`(`argv` / `env`),而 `StdCtx` 在
//! [`wlwl_value`] 里 —— 所以本模块**纯读宿主一个字段**,不碰操作系统 API。
//! 这与 `std.time` 直读系统时钟是**不同**的立场,写在这里免得下一个人「顺手统一」:
//! 时间的**源**在操作系统里(本语言就是时间源),而环境的**源**是启动时由宿主
//! 交给程序的 —— 它已经是「传进来的值」。
//!
//! ⚠ **一处显式例外**：`SCRIPT_PATH` 要把相对路径变成绝对路径,必须读一次
//! 进程 CWD ⇒ 它是本模块**唯一**读操作系统 API 的成员。其余四个成员
//! (`ENV_GET` / `ENV_SET` / `ENV_KEYS` / `ARGS`)仍然纯读 `StdCtx`。
//!
//! ## 确定性(ADR-0026 G5 / 业主 2026-10-08 裁决)
//!
//! `ENV_KEYS()` **排序后返回**:`std::env::vars()` 的迭代序**逐次可能不同**,
//! 而**可复现是本语言卖点第一条** ⇒ 排序是**规范性要求**,不是实现偏好。
//!
//! `ENV_SET` **仅本进程可见**,**不写磁盘** —— 它改的是**启动时从宿主环境拍的
//! 那份快照**(`StdCtx::from_process` 里的 `std::env::vars().collect()`),
//! 而不是操作系统那份真的环境,所以进程重启后自然消失。
//!
//! ⚠🔴 **[N-029]** 旧注释在这里写的是「**子进程会继承**」—— **那半句是错的,
//! 已改正。** ⚠ 要分清两份东西:
//!
//! | | 来源 | `PROCESS_RUN` 的子进程看得到吗 |
//! |---|---|---|
//! | 操作系统环境 | `std::env::vars()` | **看得到**(`Command` 默认继承真实环境) |
//! | wlwl 级环境表 `ctx.env` | 上述快照 + `ENV_SET` 的写入 | **看不到** |
//!
//! `PROCESS_RUN` 只把 `opts.env` 里显式给的键加进去(`process.rs` 里
//! `for (k, v) in &extra_env { cmd.env(k, v); }`),**从不合并 `ctx.env`** ⇒
//! `ENV_SET("FOO", "bar")` 之后 `PROCESS_RUN(["…", "printenv", "FOO"])`,
//! 子进程**读不到** `FOO`。
//!
//! ⚠ **为什么是「改文档」而不是「改成继承」**(`D13-067`):让子进程继承
//! `ctx.env` 是**行为变更**,会让**既有程序**的子进程环境突然多出变量;而且
//! 子进程的环境将取决于**语句执行顺序**(程序跑到哪儿 `ENV_SET` 了多少),
//! 这与本模块开头「**可复现是本语言卖点第一条**」那条立场直接冲突。⇒ 先把
//! 文档改对,把「要不要继承」留成一个**独立的产品裁决**(见台账),不在缺陷
//! 修复批次里夹带。⚠ 注意规范 §19 说的「追加到继承来的环境」指的是**操作系统
//! 那份**,那句本身是对的,不必改。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_error::WlwlResult;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.env",
    functions: &[
        ("ENV_GET", env_get as StdFn),
        ("ENV_SET", env_set as StdFn),
        ("ENV_KEYS", env_keys as StdFn),
        ("ARGS", args as StdFn),
        // [项目扩展 / G-22] 「当前脚本路径」。`ARGS()[0]` 是**解释器**自己的
        // 路径（实测 `…/target/release/wlwl.exe`），拿不到被运行的脚本路径。
        // 宿主的 `StdCtx.source_file` 里**已经有**入口文件名（诊断定位用），
        // 只是没暴露给 WLWL 代码 ⇒ 这里把它暴露出去。
        ("SCRIPT_PATH", script_path as StdFn),
    ],
};

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

fn str_of(host: &mut dyn StdHost, name: &str, v: &Value) -> Result<String, WlwlError> {
    match v {
        Value::String(s) => Ok(s.clone()),
        other => Err(type_err(host, name, "a string", other)),
    }
}

fn ok(v: Value) -> Result<Outcome, WlwlError> {
    Ok(Outcome::normal(v))
}

/// `ENV_GET(name, default?) -> STRING | NULL`
///
/// 取环境变量。**不存在且无缺省 → `NULL`**(不是 `ERR`、也不是 `""` ——
/// 「没设」和「设为空串」是两件事,分不开就没法读 `.env` 覆盖后的真实状态)。
pub fn env_get(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "ENV_GET";
    if args.is_empty() || args.len() > 2 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let name = str_of(host, NAME, &args[0])?;
    if let Some(found) = host.ctx().env.get(&name) {
        return ok(Value::String(found.clone()));
    }
    match args.get(1) {
        None | Some(Value::Null) => ok(Value::Null),
        Some(v) => ok(v.clone()),
    }
}

/// `ENV_SET(name, value) -> NULL` —— **仅本进程可见,不写磁盘**。
///
/// ⚠ **写进的是 `ctx.env` 这份 wlwl 级快照,不是操作系统环境** ⇒
/// **`PROCESS_RUN` 起的子进程看不到它**(该模块只加 `opts.env` 显式给的键)。
/// 详见本文件头部 `N-029` 的对照表。
pub fn env_set(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "ENV_SET";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let name = str_of(host, NAME, &args[0])?;
    let value = str_of(host, NAME, &args[1])?;
    host.ctx().env.insert(name, value);
    ok(Value::Null)
}

/// `ENV_KEYS() -> ARRAY[STRING]` —— 全部键名,**排序后返回**(确定性)。
pub fn env_keys(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "ENV_KEYS";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), 0));
    }
    let mut keys: Vec<String> = host.ctx().env.keys().cloned().collect();
    keys.sort();
    ok(Value::Array(keys.into_iter().map(Value::String).collect()))
}

/// `ARGS() -> ARRAY[STRING]` —— 命令行参数。
///
/// ⚠️ **含 `argv[0]`**(程序自身的路径)—— 与 Go `os.Args` / Python `sys.argv`
/// 同款。调用方要「用户传进来的参数」就 `SLICE(ARGS(), 1)`。
pub fn args(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    const NAME: &str = "ARGS";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), 0));
    }
    let v: Vec<Value> = host
        .ctx()
        .argv
        .iter()
        .map(|s| Value::String(s.clone()))
        .collect();
    ok(Value::Array(v))
}

/// [项目扩展 / G-22] `SCRIPT_PATH() -> STRING | NULL` —— **入口脚本**的路径。
///
/// ⚠ 为什么需要：`ARGS()[0]` 是**解释器**自己的路径（实测 `…/target/release/wlwl.exe`）
///   ⇒ 自包含应用定位不到自己的资源目录（静态文件、配置、模板）。
/// ⚠ 数据来源：宿主的 `StdCtx.source_file`（诊断定位用的「入口源文件名」）
///   —— 它**本来就存在**，只是没暴露给 WLWL 代码。
///
/// **契约（四条）**
/// 1. 返回**入口脚本**的路径 —— 不是解释器路径（那是 `ARGS()[0]`），也不是
///    「当前正在执行的模块」（导入的模块不换这个值：它是**入口**，整轮运行不变）。
/// 2. 返回**绝对路径**。宿主给的是「命令行原样」的那份（可能是相对路径），
///    而相对路径的基准是进程 CWD —— 应用改一次 CWD 就会失效。
///    ⇒ 绝对化在本成员里做（见 `absolutize`）。
/// 3. **没有入口脚本 ⇒ `NULL`**（嵌入式求值 / REPL / 库里直接 eval）。
///    ⚠ 宿主那时的哨兵是 `wlwl_value::NO_SOURCE_FILE`（`<runtime>`）——
///    那是**诊断定位**用的占位名，**不是路径** ⇒ 直接返回它就是把内部约定
///    泄漏到公开成员面（应用会拿它去拼路径）。⇒ 这里翻译成 `NULL`。
/// 4. **不是**解释器路径。
///
/// ⚠ **本成员是 `std.env` 里唯一一个读操作系统 API 的成员**（`current_dir`，
///   只为第 2 条的绝对化）。本文件头部「纯读宿主一个字段」那条立场对
///   `ENV_GET` / `ENV_SET` / `ENV_KEYS` / `ARGS` 成立，对本成员是**显式例外**：
///   没有 CWD 就无法把相对路径变成绝对路径，而「相对路径的基准」这件事
///   **必须**由本成员说清楚（台账 G-22 的验收判据第 2 条）。
pub fn script_path(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "SCRIPT_PATH";
    if !args.is_empty() {
        return Err(arity(host, NAME, args.len(), 0));
    }
    let p = host.ctx().source_file.clone();
    if p == wlwl_value::NO_SOURCE_FILE {
        // 没有入口脚本 ⇒ 没有「当前脚本路径」⇒ `NULL`（「暂时没有」是正常值）。
        return ok(Value::Null);
    }
    ok(Value::String(absolutize(&p)))
}

/// 把「命令行原样」的路径变成**绝对路径**。
///
/// ⚠ 相对路径以**进程 CWD**为基准补全 —— 因为宿主给的就是按 CWD 解释的那份路径。
/// ⚠ **不**做 `canonicalize`：那会解析符号链接（返回的就不再是「用户写的那个
///   路径」）且要求路径存在（不存在时行为突变）。这里只做「拼 CWD」这一件事。
/// ⚠ CWD 读不到时（极少见：目录被删）⇒ **原样返回**，不制造假路径。
fn absolutize(p: &str) -> String {
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        return p.to_string();
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path).to_string_lossy().to_string(),
        Err(_) => p.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_value::StdCtx;

    /// 本模块**要**读 `ctx`,所以这里的宿主与 `time.rs` / `rand.rs` 的
    /// `NullHost` 相反 —— 那两个的 `ctx()` 是故意 panic 的断言。
    struct Host(StdCtx);

    impl StdHost for Host {
        fn ctx(&mut self) -> &mut StdCtx {
            &mut self.0
        }
        fn call(&mut self, _f: &Value, _a: Vec<Value>, _n: &str) -> Result<Outcome, WlwlError> {
            panic!("std.env has no member that calls back into the host")
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
        ctx.argv = vec!["prog".into()];
        Host(ctx)
    }

    #[test]
    fn get_returns_value_null_or_default() {
        let mut h = host_with_env(&[("A", "1")]);
        let Value::String(v) = env_get(&mut h, vec![Value::String("A".into())])
            .unwrap()
            .value
        else {
            panic!()
        };
        assert_eq!(v, "1");
        let Value::Null = env_get(&mut h, vec![Value::String("MISSING".into())])
            .unwrap()
            .value
        else {
            panic!("missing env var must be NULL, not an error and not \"\"")
        };
        let Value::String(d) = env_get(
            &mut h,
            vec![
                Value::String("MISSING".into()),
                Value::String("fallback".into()),
            ],
        )
        .unwrap()
        .value
        else {
            panic!()
        };
        assert_eq!(d, "fallback");
    }

    /// ⚠️ 这条是**确定性契约**的守卫:同样的 env,两次调用必须**顺序相同**,
    /// 且**恰好是键名排序**。`std::env::vars()` 的迭代序逐次可能不同 —— 排序是
    /// 规范性要求,不是实现偏好。
    ///
    /// (先前这里还断言「fixture 的 HashMap 顺序 != sorted」,那是个**坏断言**:
    /// `HashMap` 的顺序是任意的,偶尔会碰巧等于排序,于是这条测试本身变成
    /// **偶发红**。判据应该是**输出**的形状,不是输入的形状。)
    #[test]
    fn env_keys_is_sorted_and_deterministic() {
        let mut h = host_with_env(&[("ZZZ", "1"), ("AAA", "2"), ("MMM", "3")]);
        let first = env_keys(&mut h, vec![]).unwrap().value.display();
        let second = env_keys(&mut h, vec![]).unwrap().value.display();
        assert_eq!(first, second, "ENV_KEYS must be deterministic across calls");
        assert_eq!(first, "[AAA, MMM, ZZZ]", "ENV_KEYS must be sorted");
    }

    #[test]
    fn set_is_visible_to_a_later_get() {
        let mut h = host_with_env(&[]);
        env_set(
            &mut h,
            vec![Value::String("K".into()), Value::String("v".into())],
        )
        .unwrap();
        let Value::String(v) = env_get(&mut h, vec![Value::String("K".into())])
            .unwrap()
            .value
        else {
            panic!()
        };
        assert_eq!(v, "v");
    }

    #[test]
    fn args_includes_argv0() {
        let mut h = host_with_env(&[]);
        h.0.argv = vec!["prog".into(), "one".into()];
        let Value::Array(v) = args(&mut h, vec![]).unwrap().value else {
            panic!()
        };
        assert_eq!(
            v.len(),
            2,
            "ARGS includes argv[0], like Go os.Args / Python sys.argv"
        );
    }

    #[test]
    fn arity_is_checked() {
        let mut h = host_with_env(&[]);
        assert_eq!(
            env_get(&mut h, vec![]).unwrap_err().diagnostic().code,
            ErrorCode::E0022
        );
        assert_eq!(
            env_keys(&mut h, vec![Value::Integer(1)])
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0022
        );
        assert_eq!(
            env_get(&mut h, vec![Value::Integer(1)])
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0030
        );
    }

    // ── SCRIPT_PATH ────────────────────────────────────────────────────────

    #[test]
    fn script_path_is_null_when_the_host_has_no_entry_file() {
        let mut h = host_with_env(&[]);
        // 缺省哨兵就是 `NO_SOURCE_FILE` ⇒ 成员必须翻译成 `NULL`。
        assert_eq!(h.0.source_file, wlwl_value::NO_SOURCE_FILE);
        let Value::Null = script_path(&mut h, vec![]).unwrap().value else {
            panic!("no entry script must be NULL, not the `<runtime>` sentinel")
        };
    }

    #[test]
    fn script_path_absolutizes_a_relative_entry_path() {
        let mut h = host_with_env(&[]);
        h.0.source_file = "pkg/main.wll".into();
        let Value::String(v) = script_path(&mut h, vec![]).unwrap().value else {
            panic!()
        };
        let cwd = std::env::current_dir().expect("cwd must be readable");
        assert_eq!(
            v,
            cwd.join("pkg/main.wll").to_string_lossy().to_string(),
            "a relative entry path must be resolved against the process CWD"
        );
    }

    #[test]
    fn script_path_rejects_arguments() {
        let mut h = host_with_env(&[]);
        assert_eq!(
            script_path(&mut h, vec![Value::Integer(1)])
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0022
        );
    }
}
