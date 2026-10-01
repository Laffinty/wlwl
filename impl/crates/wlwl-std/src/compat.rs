//! [v0.11 M2 / ADR-0022 §4] 内部表示兼容层。
//!
//! io/fs/json/format/ai/agent 六个模块的函数体历史上以 serde_json 为
//! 中间表示;边界直通后 serde_json 只是这些模块的**内部**表示(跨界
//! 类型只有 `Value`,由 lib.rs 的 `wrap` 统一转换)。本模块提供它们
//! 原有的助手与别名,函数体与测试零改动。

use wlwl_error::ErrorCode;

/// serde_json 在 std 模块内部表示语境下的别名。
pub type StdValue = serde_json::Value;

/// 旧 std 错误形态(code + message);`wrap` 出口统一转 `WlwlError`。
#[derive(Debug, Clone)]
pub struct StdError {
    pub code: ErrorCode,
    pub message: String,
}

/// E0022(带函数名,P3-012 口径,与原 lib::arity_error 逐字一致)。
pub fn arity_error(fn_name: &str, got: usize, want: usize) -> StdError {
    StdError {
        code: ErrorCode::E0022,
        message: format!("{fn_name}: function expects {want} argument(s), got {got}"),
    }
}

/// E0030。
pub fn type_error(fn_name: &str, expected: &str, got: &StdValue) -> StdError {
    StdError {
        code: ErrorCode::E0030,
        message: format!(
            "{}: expected {}, got {}",
            fn_name,
            expected,
            json_type_name(got)
        ),
    }
}

pub fn json_type_name(v: &StdValue) -> &'static str {
    match v {
        StdValue::Null => "null",
        StdValue::Bool(_) => "boolean",
        StdValue::Number(_) => "number",
        StdValue::String(_) => "string",
        StdValue::Array(_) => "array",
        StdValue::Object(_) => "dict",
    }
}

/// 提取字符串实参(旧 lib::expect_string 签名与措辞)。
pub fn expect_string<'a>(
    fn_name: &str,
    args: &'a [StdValue],
    i: usize,
    want_arity: usize,
) -> Result<&'a str, StdError> {
    if args.len() != want_arity {
        return Err(arity_error(fn_name, args.len(), want_arity));
    }
    match &args[i] {
        StdValue::String(s) => Ok(s.as_str()),
        other => Err(type_error(fn_name, "string", other)),
    }
}
