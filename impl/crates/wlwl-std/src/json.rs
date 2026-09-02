//! `wlwl:std.json` — `PARSE`, `STRINGIFY` (v0.3 §15.3 + §14.4 E0070/E0071).

use crate::{expect_string, arity_error, type_error, StdCtx, StdError, StdFn, StdValue, ModuleSpec};
use wlwl_error::ErrorCode;

pub fn std_parse(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    let s = expect_string("PARSE", &args, 0, 1)?;
    match serde_json::from_str::<StdValue>(s) {
        Ok(v) => Ok(v),
        Err(e) => Err(StdError {
            code: ErrorCode::E0070,
            message: format!("PARSE: {}", e),
        }),
    }
}

pub fn std_stringify(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_object() {
        let mut ctx = StdCtx::default();
        let v = std_parse(
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
        let v = std_parse(&mut ctx, vec![StdValue::String("[1, 2, 3]".into())]).unwrap();
        assert_eq!(v, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn parse_invalid_is_e0070() {
        let mut ctx = StdCtx::default();
        let err = std_parse(
            &mut ctx,
            vec![StdValue::String("{not json}".into())],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0070);
    }

    #[test]
    fn stringify_object() {
        let mut ctx = StdCtx::default();
        let v = std_stringify(
            &mut ctx,
            vec![serde_json::json!({"x": 1, "y": "z"})],
        )
        .unwrap();
        // serde_json::to_string produces compact form (no spaces).
        assert_eq!(v, StdValue::String(r#"{"x":1,"y":"z"}"#.into()));
    }

    #[test]
    fn stringify_array() {
        let mut ctx = StdCtx::default();
        let v = std_stringify(&mut ctx, vec![serde_json::json!([1, 2, 3])]).unwrap();
        assert_eq!(v, StdValue::String("[1,2,3]".into()));
    }

    #[test]
    fn arity_mismatch_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_parse(&mut ctx, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn spec_lists_parse_and_stringify() {
        assert_eq!(SPEC.path, "wlwl:std.json");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["PARSE", "STRINGIFY"]);
    }
}