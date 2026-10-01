//! `wlwl:std.json` — `PARSE`, `STRINGIFY` (v0.3 §15.3 + §14.4 E0070/E0071).

use crate::compat::*;
use crate::{ModuleSpec, StdCtx, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{type_name, Outcome, StdHost, Value};

pub(super) fn std_parse_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    let s = expect_string("PARSE", &args, 0, 1)?;
    match serde_json::from_str::<StdValue>(s) {
        Ok(v) => Ok(v),
        Err(e) => Err(StdError {
            code: ErrorCode::E0070,
            message: format!("PARSE: {}", e),
        }),
    }
}

pub(super) fn std_stringify_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    if args.len() != 1 {
        return Err(arity_error("STRINGIFY", args.len(), 1));
    }
    // `serde_json::to_string` only fails on non-serializable types
    // (e.g. NaN, map with non-string keys). Our value type has only
    // serializable shapes, so this is a defensive E0071 — kept so the
    // spec surface is complete.
    match serde_json::to_string(&args[0]) {
        Ok(s) => Ok(StdValue::String(s)),
        Err(e) => Err(StdError {
            code: ErrorCode::E0071,
            message: format!("STRINGIFY: {}", e),
        }),
    }
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.json",
    functions: &[
        ("PARSE", std_parse as StdFn),
        ("STRINGIFY", std_stringify as StdFn),
    ],
};

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_parse(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, "std_parse", std_parse_inner, args)
}

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_stringify(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, "std_stringify", std_stringify_inner, args)
}

// ── Value ↔ serde_json(仅作内部表示转换;ADR-0022 §4)──

pub(crate) fn values_to_json(
    args: &[Value],
) -> Result<Vec<serde_json::Value>, (wlwl_error::ErrorCode, String)> {
    let mut out = Vec::with_capacity(args.len());
    for a in args {
        out.push(value_to_json(a)?);
    }
    Ok(out)
}

pub(crate) fn value_to_json(
    v: &Value,
) -> Result<serde_json::Value, (wlwl_error::ErrorCode, String)> {
    let bad = |expected: &str, got: &str| {
        (
            wlwl_error::ErrorCode::E0030,
            format!("std argument: expected {expected}, got {got}"),
        )
    };
    Ok(match v {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .ok_or_else(|| bad("finite number", "NaN/Inf float"))?,
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(value_to_json(item)?);
            }
            serde_json::Value::Array(out)
        }
        Value::Dict(entries) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in entries {
                let key = match k {
                    Value::String(s) => s.clone(),
                    Value::Integer(i) => i.to_string(),
                    other => return Err(bad("string/integer dict key", type_name(other))),
                };
                obj.insert(key, value_to_json(val)?);
            }
            serde_json::Value::Object(obj)
        }
        Value::Ok(inner) => {
            let mut obj = serde_json::Map::new();
            obj.insert("Ok".into(), value_to_json(inner)?);
            serde_json::Value::Object(obj)
        }
        Value::Err(inner) => {
            let mut obj = serde_json::Map::new();
            obj.insert("Err".into(), value_to_json(inner)?);
            serde_json::Value::Object(obj)
        }
        other => return Err(bad("json-representable value", type_name(other))),
    })
}

/// infallible:json 只可能来自 Value 的 round-trip。
pub(crate) fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(items) => {
            Value::Array(items.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => Value::Dict(
            obj.into_iter()
                .map(|(k, v)| (Value::String(k), json_to_value(v)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_object() {
        let mut ctx = StdCtx::default();
        let v = std_parse_inner(
            &mut ctx,
            vec![StdValue::String(r#"{"a": 1, "b": [2, 3]}"#.into())],
        )
        .unwrap();
        let expected: StdValue = serde_json::from_str(r#"{"a": 1, "b": [2, 3]}"#).unwrap();
        assert_eq!(v, expected);
    }

    #[test]
    fn parse_array() {
        let mut ctx = StdCtx::default();
        let v = std_parse_inner(&mut ctx, vec![StdValue::String("[1, 2, 3]".into())]).unwrap();
        assert_eq!(v, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn parse_invalid_is_e0070() {
        let mut ctx = StdCtx::default();
        let err =
            std_parse_inner(&mut ctx, vec![StdValue::String("{not json}".into())]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0070);
    }

    #[test]
    fn stringify_object() {
        let mut ctx = StdCtx::default();
        let v = std_stringify_inner(&mut ctx, vec![serde_json::json!({"x": 1, "y": "z"})]).unwrap();
        // serde_json::to_string produces compact form (no spaces).
        assert_eq!(v, StdValue::String(r#"{"x":1,"y":"z"}"#.into()));
    }

    #[test]
    fn stringify_array() {
        let mut ctx = StdCtx::default();
        let v = std_stringify_inner(&mut ctx, vec![serde_json::json!([1, 2, 3])]).unwrap();
        assert_eq!(v, StdValue::String("[1,2,3]".into()));
    }

    #[test]
    fn arity_mismatch_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_parse_inner(&mut ctx, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn spec_lists_parse_and_stringify() {
        assert_eq!(SPEC.path, "wlwl:std.json");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["PARSE", "STRINGIFY"]);
    }
}
