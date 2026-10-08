//! [v0.11.3 M6 / addendum-06] `wlwl:std.env` —— 进程环境与命令行参数。
//!
//! **R2 全员**。数据来源是 `StdCtx`(`argv` / `env`),而 `StdCtx` 在
//! [`wlwl_value`] 里 —— 所以本模块**纯读宿主一个字段**,不碰操作系统 API。
//! 这与 `std.time` 直读系统时钟是**不同**的立场,写在这里免得下一个人「顺手统一」:
//! 时间的**源**在操作系统里(本语言就是时间源),而环境的**源**是启动时由宿主
//! 交给程序的 —— 它已经是「传进来的值」。
//!
//! ## 确定性(ADR-0026 G5 / 业主 2026-10-08 裁决)
//!
//! `ENV_KEYS()` **排序后返回**:`std::env::vars()` 的迭代序**逐次可能不同**,
//! 而**可复现是本语言卖点第一条** ⇒ 排序是**规范性要求**,不是实现偏好。
//!
//! `ENV_SET` **仅本进程可见**,**不写磁盘** —— 它改的是宿主给本进程的那份副本,
//! 子进程会继承,重启后消失。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.env",
    functions: &[
        ("ENV_GET", env_get as StdFn),
        ("ENV_SET", env_set as StdFn),
        ("ENV_KEYS", env_keys as StdFn),
        ("ARGS", args as StdFn),
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
}
