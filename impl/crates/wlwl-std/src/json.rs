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
                // [D11-019] 键只接受 `STRING`。整数键是被 M2 的边界转换
                // 顺手放开的(M2 的目标是「搬移」,不是「放宽」),会让
                // `FORMAT("{0}", ["a":1]` 之后 INDEX_SET 出来的整数键字典
                // 悄悄变成字符串键 `{"2": …}`;旧边界一律 `E0030`。
                let key = match k {
                    Value::String(s) => s.clone(),
                    other => return Err(bad("string dict key", type_name(other))),
                };
                obj.insert(key, value_to_json(val)?);
            }
            serde_json::Value::Object(obj)
        }
        // [D11-019] `OK(v)` 传**内层**,不是 `{"Ok": v}`。§12 的 `OK` 只是
        // 值的一层包装,跨界时拆掉(旧边界 `value_to_std_value` 就是这么写的,
        // M2 的「纯搬移」纪律要求逐字保持)。包成对象会让
        // `FORMAT("{0}", OK(1))` 从 `1` 变成 `Ok: 1`。
        Value::Ok(inner) => value_to_json(inner)?,
        // [D11-019] `ERR` 仍然拒绝(旧边界同)。语言层面它**到不了这里**:
        // §12.6 的 ERR 透明传播在调用边界就短路了(实测 `FORMAT("{0}",
        // ERR("e"))` 直接返回那个 ERR,成员体根本不执行),所以这一臂是
        // 给「本函数被直接调用」兜底的 —— 留着是为了不让它变成一个
        // 永远走不到、却把 ERR 变成 `{"Err": …}` 的地雷。
        Value::Err(_) => {
            return Err(bad("OK/primitives at std boundary", "ERR(...)"));
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

    // ── [D11-019] 边界转换的语义锁 ──
    //
    // M2 把边界从 eval 侧的 `value_to_std_value` 搬进本文件时,三处
    // 可观察行为被顺手改了(报「纯搬移」,实际是搬移 + 放宽)。这三条
    // 逐条钉回 M2 之前的口径;`wrap` 由 io / fs / json / format / ai /
    // agent 六个模块共用,所以这里是唯一的收口点。

    #[test]
    fn boundary_unwraps_ok_to_its_inner_value() {
        let got = values_to_json(&[Value::Ok(Box::new(Value::Integer(1)))]).unwrap();
        assert_eq!(got, vec![serde_json::json!(1)]);
        // 嵌套也要拆(旧边界递归下去)。
        let nested = values_to_json(&[Value::Ok(Box::new(Value::Ok(Box::new(Value::String(
            "x".into(),
        )))))])
        .unwrap();
        assert_eq!(nested, vec![serde_json::json!("x")]);
    }

    #[test]
    fn boundary_rejects_err() {
        let err = values_to_json(&[Value::Err(Box::new(Value::String("e".into())))]).unwrap_err();
        assert_eq!(err.0, wlwl_error::ErrorCode::E0030);
    }

    #[test]
    fn boundary_rejects_integer_dict_keys() {
        let d = Value::Dict(vec![(Value::Integer(2), Value::String("v".into()))]);
        let err = values_to_json(&[d]).unwrap_err();
        assert_eq!(err.0, wlwl_error::ErrorCode::E0030);
        assert!(
            err.1.contains("string dict key"),
            "message should name the old expectation, got: {}",
            err.1
        );
    }

    #[test]
    fn boundary_accepts_string_dict_keys() {
        let d = Value::Dict(vec![(Value::String("a".into()), Value::Integer(1))]);
        assert_eq!(
            values_to_json(&[d]).unwrap(),
            vec![serde_json::json!({"a": 1})]
        );
    }

    /// ADR-0022 §4 明确保留的唯一一条跨模块契约:闭包跨界一律 `E0030`。
    /// 它与上面三条同属「边界不许悄悄放宽」这一族,故放在一起钉。
    #[test]
    fn boundary_still_rejects_closures() {
        let f = Value::Closure {
            params: vec![],
            body: Box::new(wlwl_ast::Expr::Literal(
                wlwl_ast::Literal::Null,
                wlwl_ast::Span::new("t.wll", 0, 0),
            )),
            env: wlwl_value::Env::new(),
        };
        let err = values_to_json(&[f]).unwrap_err();
        assert_eq!(err.0, wlwl_error::ErrorCode::E0030);
    }
}
