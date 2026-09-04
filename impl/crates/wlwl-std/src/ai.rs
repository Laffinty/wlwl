//! `wlwl:std.ai` — mock LLM bridge (v0.3 §15.11).
//!
//! This is the **mock** implementation agreed for Phase 4: no real
//! HTTP, no real LLM provider. The three functions (ASK / EMBED /
//! COMPLETE) return deterministic content derived from the inputs
//! and the optional `WLWL_AI_*` environment variables. Production
//! code that wants real provider access wires a different
//! `StdFn` into the same module spec slot; the function signature
//! (`fn(&mut StdCtx, Vec<StdValue>) -> Result<StdValue, StdError>`)
//! is the only stable contract.
//!
//! ## Error code triggers
//!
//! The mock exposes the four v0.3 AI error codes by matching
//! reserved `model` names (so unit tests do not have to mutate
//! environment variables):
//!
//! | `model`            | Result             | Code  |
//! |--------------------|--------------------|-------|
//! | `"_fail_E0080"`    | `Err(E0080)`       | unreachable |
//! | `"_fail_E0081"`    | `Err(E0081)`       | auth / rate-limit |
//! | `"_fail_E0082"`    | `Err(E0082)`       | response malformed |
//! | `"_fail_E0083"`    | `Err(E0083)`       | timeout |
//!
//! A non-reserved model returns a deterministic mock payload derived
//! from the input.

use crate::{arity_error, type_error, StdCtx, StdError, StdFn, StdValue, ModuleSpec};
use wlwl_error::ErrorCode;

/// Match `model` against the reserved failure tokens. Returns
/// `Some(StdError)` if the model signals a synthetic error, `None`
/// otherwise. Reserved tokens are case-sensitive so an end-user
/// can freely use a model literally named "gpt-4".
fn check_reserved_failure(model: &str) -> Option<StdError> {
    let code = match model {
        "_fail_E0080" => ErrorCode::E0080,
        "_fail_E0081" => ErrorCode::E0081,
        "_fail_E0082" => ErrorCode::E0082,
        "_fail_E0083" => ErrorCode::E0083,
        _ => return None,
    };
    Some(StdError {
        code,
        message: format!(
            "std.ai mock: model `{}` reserved for triggering {}",
            model,
            code.as_str()
        ),
    })
}

// ── ASK ───────────────────────────────────────────────────────

pub fn std_ask(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_error("ASK", args.len(), 3));
    }
    let model = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("ASK", "string", other)),
    };
    let prompt = match &args[1] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("ASK", "string", other)),
    };
    if let Some(err) = check_reserved_failure(model) {
        return Err(err);
    }
    // Mock response: include the model name so callers can see
    // their model was honoured, and embed a 32-bit hash of the
    // prompt for determinism.
    let h = fnv1a(prompt.as_bytes());
    Ok(StdValue::String(format!(
        "[mock:{model}] echo (h=0x{h:08x}) :: {prompt}",
    )))
}

// ── EMBED ─────────────────────────────────────────────────────

pub fn std_embed(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() < 1 || args.len() > 2 {
        return Err(arity_error("EMBED", args.len(), 2));
    }
    let text = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("EMBED", "string", other)),
    };
    let model = if args.len() == 2 {
        match &args[1] {
            StdValue::String(s) => s.as_str(),
            other => return Err(type_error("EMBED", "string", other)),
        }
    } else {
        "default"
    };
    if let Some(err) = check_reserved_failure(model) {
        return Err(err);
    }
    // Fake 4-dim vector derived from FNV-1a hashes of the
    // (text, model) pair. Deterministic + bounded.
    let h1 = fnv1a(text.as_bytes());
    let h2 = fnv1a(model.as_bytes());
    let v = vec![
        ((h1 & 0xFFFF) as f64) / 65535.0,
        (((h1 >> 16) & 0xFFFF) as f64) / 65535.0,
        ((h2 & 0xFFFF) as f64) / 65535.0,
        (((h2 >> 16) & 0xFFFF) as f64) / 65535.0,
    ];
    let arr: Vec<StdValue> = v
        .into_iter()
        .map(|x| serde_json::Number::from_f64(x).map(StdValue::Number).unwrap_or(StdValue::Null))
        .collect();
    Ok(StdValue::Array(arr))
}

// ── COMPLETE ──────────────────────────────────────────────────

pub fn std_complete(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() < 1 || args.len() > 3 {
        return Err(arity_error("COMPLETE", args.len(), 3));
    }
    let context = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("COMPLETE", "string", other)),
    };
    let language = if args.len() >= 2 {
        match &args[1] {
            StdValue::String(s) => s.as_str(),
            other => return Err(type_error("COMPLETE", "string", other)),
        }
    } else {
        "wlwl"
    };
    if let Some(err) = check_reserved_failure(language) {
        return Err(err);
    }
    // Trim the context to 60 chars and wrap in a comment-like
    // suggestion. The mock never reads language from the model
    // path; the trigger uses language to keep symmetry with the
    // table above (so `_fail_E0080` in the language slot works).
    let preview: String = context.chars().take(60).collect();
    Ok(StdValue::String(format!(
        "// mock completion for ({language}): {preview}…"
    )))
}

// ── ASK_STREAM (spec §15.11.4 stub) ───────────────────────────
//
// P3-012 stub. v0.3 §15.11.4: "v0.3 同步调用; v0.4 议程:
// ASK_STREAM(model, prompt, callback)". The mock implementation
// accepts the same `(model, prompt, callback)` shape and returns
// the same OK(string) payload as `ASK`. A future real HTTP client
// will chunk the response and invoke the callback per chunk; for
// now the callback is a no-op (we validate the arity and types
// but do not invoke the WLWL function from a std_fn context —
// the interpreter is the one place that can dispatch into user
// functions).
pub fn std_ask_stream(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_error("ASK_STREAM", args.len(), 3));
    }
    // Reuse ASK's model/prompt validation + mock content. The
    // third slot (callback) is accepted for arity symmetry but
    // not invoked from this std_fn context.
    let model = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("ASK_STREAM", "string", other)),
    };
    let prompt = match &args[1] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("ASK_STREAM", "string", other)),
    };
    if let Some(err) = check_reserved_failure(model) {
        return Err(err);
    }
    let h = fnv1a(prompt.as_bytes());
    Ok(StdValue::String(format!(
        "[mock-stream:{model}] echo (h=0x{h:08x}) :: {prompt}",
    )))
}

// ── ASK_ALL (spec §15.11.4 stub) ──────────────────────────────
//
// P3-012 stub. Batch ASK over an array of prompts, returning
// ARRAY of OK/ERR. v0.3 runs the calls serially in interpreter
// order; v0.4 will replace this with a real concurrent fan-out.
pub fn std_ask_all(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() != 1 {
        return Err(arity_error("ASK_ALL", args.len(), 1));
    }
    let prompts = match &args[0] {
        StdValue::Array(a) => a,
        other => return Err(type_error("ASK_ALL", "array", other)),
    };
    // Validate every prompt is a string up front (so we can
    // produce a single coherent error if not). Real fan-out
    // would dispatch each element back through the interpreter.
    for (_i, p) in prompts.iter().enumerate() {
        if !matches!(p, StdValue::String(_)) {
            // v0.3 std_fn has no per-element error path, so we
            // report a single ASK_ALL-level E0030 pointing at
            // the offending element. v0.4 should route this
            // through the interpreter to produce per-element
            // OK / ERR (spec §15.11.4).
            return Err(type_error("ASK_ALL", "string", p));
        }
    }
    // Mock per-prompt response: same shape as ASK, with index
    // included so callers can correlate result <-> input.
    let mut out: Vec<StdValue> = Vec::with_capacity(prompts.len());
    for (i, p) in prompts.iter().enumerate() {
        let StdValue::String(prompt) = p else { unreachable!() };
        let h = fnv1a(prompt.as_bytes());
        out.push(StdValue::String(format!(
            "[mock-batch:{i}] echo (h=0x{h:08x}) :: {prompt}"
        )));
    }
    Ok(StdValue::Array(out))
}

// ── FNV-1a (32-bit) for deterministic hash bits ───────────────

/// FNV-1a 32-bit. Tiny, dependency-free, deterministic. We use
/// the resulting hash bits only as a mock payload component; this
/// is not a security primitive.
fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in bytes {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.ai",
    functions: &[
        ("ASK", std_ask as StdFn),
        ("EMBED", std_embed as StdFn),
        ("COMPLETE", std_complete as StdFn),
        // P3-012: streaming / batch APIs (spec §15.11.4). The
        // mock implementations honor the v0.3 signature while
        // real HTTP streaming / fan-out lands in v0.4.
        ("ASK_STREAM", std_ask_stream as StdFn),
        ("ASK_ALL", std_ask_all as StdFn),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> StdCtx {
        StdCtx::default()
    }

    #[test]
    fn ask_mock_response() {
        let v = std_ask(
            &mut ctx(),
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::String("hello".into()),
            ],
        )
        .unwrap();
        match v {
            StdValue::String(s) => {
                assert!(s.contains("[mock:gpt-4]"), "{}", s);
                assert!(s.contains("hello"));
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn ask_arity_zero_is_e0022() {
        let err = std_ask(&mut ctx(), vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    #[test]
    fn ask_arity_too_many_is_e0022() {
        // ASK takes 2 (model + prompt) or 3 (+ opts). 4+ is E0022.
        let err = std_ask(
            &mut ctx(),
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::String("hi".into()),
                StdValue::String("opts".into()),
                StdValue::String("extra".into()),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn ask_non_string_model_is_e0030() {
        let err = std_ask(
            &mut ctx(),
            vec![StdValue::Number(42.into()), StdValue::String("hi".into())],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn ask_failure_tokens_trigger_each_error_code() {
        for (token, expected) in [
            ("_fail_E0080", ErrorCode::E0080),
            ("_fail_E0081", ErrorCode::E0081),
            ("_fail_E0082", ErrorCode::E0082),
            ("_fail_E0083", ErrorCode::E0083),
        ] {
            let err = std_ask(
                &mut ctx(),
                vec![
                    StdValue::String(token.into()),
                    StdValue::String("x".into()),
                ],
            )
            .unwrap_err();
            assert_eq!(err.code, expected, "token {}", token);
        }
    }

    #[test]
    fn embed_returns_deterministic_vector() {
        let v = std_embed(
            &mut ctx(),
            vec![
                StdValue::String("hello".into()),
                StdValue::String("text-embed-3".into()),
            ],
        )
        .unwrap();
        let v2 = std_embed(
            &mut ctx(),
            vec![
                StdValue::String("hello".into()),
                StdValue::String("text-embed-3".into()),
            ],
        )
        .unwrap();
        assert_eq!(v, v2);
        match v {
            StdValue::Array(items) => {
                assert_eq!(items.len(), 4);
                for it in &items {
                    assert!(matches!(it, StdValue::Number(_)));
                }
            }
            other => panic!("expected Array, got {:?}", other),
        }
    }

    #[test]
    fn embed_default_model_when_omitted() {
        let v = std_embed(&mut ctx(), vec![StdValue::String("x".into())]).unwrap();
        match v {
            StdValue::Array(items) => assert_eq!(items.len(), 4),
            _ => panic!(),
        }
    }

    #[test]
    fn embed_failure_token() {
        let err = std_embed(
            &mut ctx(),
            vec![
                StdValue::String("x".into()),
                StdValue::String("_fail_E0083".into()),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0083);
    }

    #[test]
    fn complete_mock_response_includes_language() {
        let v = std_complete(
            &mut ctx(),
            vec![
                StdValue::String("fun fib(n) {".into()),
                StdValue::String("rust".into()),
            ],
        )
        .unwrap();
        match v {
            StdValue::String(s) => {
                assert!(s.contains("(rust)"));
                assert!(s.contains("fun fib"));
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn complete_default_language_is_wlwl() {
        let v = std_complete(&mut ctx(), vec![StdValue::String("LET(x, 1);".into())]).unwrap();
        match v {
            StdValue::String(s) => assert!(s.contains("(wlwl)")),
            other => panic!("got {:?}", other),
        }
    }

    #[test]
    fn complete_failure_via_language() {
        let err = std_complete(
            &mut ctx(),
            vec![
                StdValue::String("ctx".into()),
                StdValue::String("_fail_E0082".into()),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0082);
    }

    #[test]
    fn spec_contains_all_three() {
        // P3-012 adds ASK_STREAM and ASK_ALL (spec §15.11.4).  The
        // legacy "only the three" test now asserts the original
        // three plus the two new entries are present.
        assert_eq!(SPEC.path, "wlwl:std.ai");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec!["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"]
        );
    }

    // ---- P3-012: ASK_STREAM + ASK_ALL stubs (spec §15.11.4) ----

    #[test]
    fn spec_includes_streaming_apis() {
        // After P3-012, ASK_STREAM / ASK_ALL are also exported
        // (alongside the v0.3 sync trio). v0.4 will replace the
        // mock bodies with real chunked / batch HTTP fan-out.
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec!["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"]
        );
    }

    #[test]
    fn ask_stream_returns_mock_payload() {
        // Mock returns the same OK(string) shape as ASK. The
        // third arg is the callback (we accept and ignore it in
        // the v0.3 stub; a real HTTP client would invoke it per
        // chunk).
        let mut c = ctx();
        let v = std_ask_stream(
            &mut c,
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::String("hello".into()),
                StdValue::Null, // callback slot (ignored in v0.3 mock)
            ],
        )
        .unwrap();
        match v {
            StdValue::String(s) => {
                assert!(s.contains("mock-stream"), "got {:?}", s);
                assert!(s.contains("hello"), "prompt should be echoed: {}", s);
            }
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn ask_stream_arity_error() {
        let mut c = ctx();
        let err = std_ask_stream(&mut c, vec![StdValue::String("m".into())]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
        assert!(err.message.contains("ASK_STREAM"));
    }

    #[test]
    fn ask_stream_type_error_on_non_string_model() {
        let mut c = ctx();
        let err = std_ask_stream(
            &mut c,
            vec![StdValue::Number(serde_json::Number::from(1)), StdValue::String("p".into())],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn ask_all_returns_array_of_results() {
        // Mock: ARRAY of mock payload strings, one per prompt.
        // v0.4 will route each prompt through a real ASK call
        // and collect OK/ERR per element.
        let mut c = ctx();
        let v = std_ask_all(
            &mut c,
            vec![StdValue::Array(vec![
                StdValue::String("a".into()),
                StdValue::String("b".into()),
                StdValue::String("c".into()),
            ])],
        )
        .unwrap();
        let arr = match v {
            StdValue::Array(a) => a,
            other => panic!("expected array, got {:?}", other),
        };
        assert_eq!(arr.len(), 3);
        for (i, item) in arr.iter().enumerate() {
            let s = match item {
                StdValue::String(s) => s,
                other => panic!("expected string at {}, got {:?}", i, other),
            };
            assert!(s.contains("mock-batch"), "[{}] got: {}", i, s);
            assert!(s.contains(&format!(":{}]", i)), "index missing: {}", s);
        }
    }

    #[test]
    fn ask_all_arity_error() {
        let mut c = ctx();
        let err = std_ask_all(&mut c, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
        assert!(err.message.contains("ASK_ALL"));
    }

    #[test]
    fn ask_all_type_error_on_non_string_prompt() {
        let mut c = ctx();
        let err = std_ask_all(
            &mut c,
            vec![StdValue::Array(vec![
                StdValue::String("ok".into()),
                StdValue::Number(serde_json::Number::from(42)),
            ])],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
        assert!(err.message.contains("ASK_ALL"));
    }
    // ---- P3-009d: type-error paths in ASK / EMBED / COMPLETE ----

    #[test]
    fn ask_prompt_not_string_is_e0030() {
        // Pass a number for the prompt (second arg). The prompt
        // type-error path on line 84 (other => return Err(type_error(...)))
        // should fire.
        let err = std_ask(
            &mut ctx(),
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::Number(serde_json::Number::from(1)),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
        assert!(err.message.contains("ASK: expected string"));
    }

    #[test]
    fn embed_arity_wrong_is_e0022() {
        // Three args is outside the [1, 2] window.
        let err = std_embed(
            &mut ctx(),
            vec![
                StdValue::String("x".into()),
                StdValue::String("m".into()),
                StdValue::Null,
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn embed_text_not_string_is_e0030() {
        let err = std_embed(
            &mut ctx(),
            vec![StdValue::Number(serde_json::Number::from(1))],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn embed_model_not_string_is_e0030() {
        let err = std_embed(
            &mut ctx(),
            vec![
                StdValue::String("x".into()),
                StdValue::Bool(true),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn complete_arity_wrong_is_e0022() {
        // Four args is outside the [1, 3] window.
        let err = std_complete(
            &mut ctx(),
            vec![
                StdValue::String("ctx".into()),
                StdValue::String("rust".into()),
                StdValue::Number(serde_json::Number::from(100)),
                StdValue::Null,
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn complete_context_not_string_is_e0030() {
        let err = std_complete(
            &mut ctx(),
            vec![StdValue::Number(serde_json::Number::from(1))],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn complete_language_not_string_is_e0030() {
        let err = std_complete(
            &mut ctx(),
            vec![
                StdValue::String("ctx".into()),
                StdValue::Bool(false),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }
}
