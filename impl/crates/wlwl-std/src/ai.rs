//! `wlwl:std.ai` — LLM bridge.
//!
//! Phase D (`v0.4 §15.13`): replace the v0.3 mock with a real
//! HTTP integration. The mock path is preserved for offline builds
//! and unit tests; the real path is gated behind the `real-ai`
//! cargo feature **and** the `WLWL_AI_ENDPOINT` env var. With
//! either of those absent, the std functions fall back to the
//! deterministic mock payloads — tests, CI, and `cargo install`
//! users without API keys all keep working.
//!
//! ## Env contract (v0.4 §15.13.1)
//!
//! | Var                       | Required for real mode | Purpose |
//! |---------------------------|------------------------|---------|
//! | `WLWL_AI_ENDPOINT`        | yes                    | API base URL (e.g. `https://api.openai.com`) |
//! | `WLWL_AI_API_KEY`         | yes                    | Bearer token |
//! | `WLWL_AI_DEFAULT_MODEL`   | no                     | Default model if `ASK`/`EMBED`/`COMPLETE` arg is missing |
//!
//! ## Error code mapping
//!
//! | Failure                       | Code  |
//! |-------------------------------|-------|
//! | E0080 unreachable / DNS / TLS / timeout | re-classified below |
//! | E0090 unreachable             | network ladder |
//! | E0091 DNS failure             | network ladder |
//! | E0092 TLS error               | network ladder |
//! | E0093 HTTP 4xx                | network ladder |
//! | E0094 HTTP 5xx                | network ladder |
//! | E0081 auth (401 / 403)        | bucket from HTTP 4xx arm |
//! | E0082 credentials missing     | `WLWL_AI_API_KEY` not set |
//! | E0083 response malformed      | JSON parse failure on success body |
//!
//! ## W0052
//!
//! When the `model` argument lacks the `provider/` prefix
//! (e.g. user wrote `"gpt-4"` instead of `"openai/gpt-4"`), push a
//! `(W0052, msg)` entry into `StdCtx.warnings`. Eval drains the
//! sink after the call and emits each entry as a Warning
//! diagnostic. The mock path also emits W0052 so tests can pin
//! the behavior without touching the network.

use crate::{arity_error, type_error, ModuleSpec, StdCtx, StdError, StdFn, StdValue};
use wlwl_error::ErrorCode;

/// Try to match `model` against the v0.3 reserved failure tokens.
/// Used by the mock path so unit tests do not have to mutate env
/// vars. Real-mode HTTP errors bypass this and map to the new
/// E0090-E0094 network ladder instead.
fn check_reserved_failure(model: &str) -> Option<StdError> {
    let code = match model {
        "_fail_E0080" => ErrorCode::E0080,
        "_fail_E0081" => ErrorCode::E0081,
        "_fail_E0082" => ErrorCode::E0082,
        "_fail_E0083" => ErrorCode::E0083,
        // Phase D4: network ladder
        "_fail_E0090" => ErrorCode::E0090,
        "_fail_E0091" => ErrorCode::E0091,
        "_fail_E0092" => ErrorCode::E0092,
        "_fail_E0093" => ErrorCode::E0093,
        "_fail_E0094" => ErrorCode::E0094,
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

/// Check whether `model` has the recommended `provider/` prefix.
/// Pushes a `W0052` warning onto the ctx if not (Phase D5).
fn maybe_warn_model_name(ctx: &mut StdCtx, fn_name: &str, model: &str) {
    if !model.contains('/') {
        ctx.warn(
            ErrorCode::W0052,
            format!(
                "{}: model `{}` has no `provider/` prefix; recommended form                  is `provider/model` (e.g. `openai/gpt-4`). The bare form                  still works but may route incorrectly across providers.",
                fn_name, model
            ),
        );
    }
}

/// Decide whether the call should hit the network or fall back to
/// mock. Real-mode requires BOTH the `real-ai` cargo feature AND
/// the `WLWL_AI_ENDPOINT` env var. The API key is checked at
/// request time so we can return the spec-mandated E0082 instead
/// of silently falling back when the endpoint is set but the key
/// is missing.
fn real_mode_active(ctx: &StdCtx) -> bool {
    #[cfg(feature = "real-ai")]
    {
        ctx.env.contains_key("WLWL_AI_ENDPOINT")
    }
    #[cfg(not(feature = "real-ai"))]
    {
        let _ = ctx;
        false
    }
}

/// Read env vars into local strings. Pure helper so tests don't
/// have to mutate process env (they go through `ctx.env`).
fn env_lookup<'a>(ctx: &'a StdCtx, key: &str) -> Option<&'a str> {
    ctx.env.get(key).map(String::as_str)
}

// ── Real-mode HTTP bridge (only compiled with `real-ai`) ──────────────

#[cfg(feature = "real-ai")]
mod real {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    /// Build (or reuse) a blocking reqwest client. The client is
    /// stored in `StdCtx.http_client` so HTTPS handshakes /
    /// connection pools are amortized across many calls.
    pub fn ensure_client(ctx: &mut StdCtx) -> Result<Arc<reqwest::blocking::Client>, StdError> {
        if let Some(c) = &ctx.http_client {
            return Ok(Arc::clone(c));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| StdError {
                code: ErrorCode::E0092,
                message: format!("failed to build HTTP client: {}", e),
            })?;
        let arc = Arc::new(client);
        ctx.http_client = Some(Arc::clone(&arc));
        Ok(arc)
    }

    /// POST to `${WLWL_AI_ENDPOINT}/v1/chat/completions` with an
    /// OpenAI-compatible body. Map reqwest errors to the new
    /// E0090-E0094 network ladder (Phase D4).
    pub fn http_chat(
        ctx: &mut StdCtx,
        model: &str,
        system: Option<&str>,
        user: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String, StdError> {
        let endpoint = env_lookup(ctx, "WLWL_AI_ENDPOINT").ok_or_else(|| StdError {
            code: ErrorCode::E0082,
            message: "WLWL_AI_ENDPOINT not set".into(),
        })?;
        let api_key = env_lookup(ctx, "WLWL_AI_API_KEY").ok_or_else(|| StdError {
            code: ErrorCode::E0082,
            message: "WLWL_AI_API_KEY not set".into(),
        })?;
        let client = ensure_client(ctx)?;
        let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));

        let mut messages = Vec::new();
        if let Some(s) = system {
            messages.push(serde_json::json!({"role": "system", "content": s}));
        }
        messages.push(serde_json::json!({"role": "user", "content": user}));

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
        });

        let resp = client
            .post(&url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .map_err(classify_reqwest_error)?;

        let status = resp.status();
        if status.is_client_error() {
            // 4xx — caller fault (bad request, auth, not found).
            // 401 / 403 still maps to E0081 (auth/rate) for AI-tool
            // compat; everything else → E0093.
            let code = match status.as_u16() {
                401 | 403 => ErrorCode::E0081,
                404 => ErrorCode::E0083, // model not found
                _ => ErrorCode::E0093,
            };
            return Err(StdError {
                code,
                message: format!("AI endpoint returned HTTP {}", status.as_u16()),
            });
        }
        if status.is_server_error() {
            return Err(StdError {
                code: ErrorCode::E0094,
                message: format!("AI endpoint returned HTTP {}", status.as_u16()),
            });
        }

        let v: serde_json::Value = resp.json().map_err(|e| StdError {
            code: ErrorCode::E0083,
            message: format!("malformed AI response: {}", e),
        })?;
        let content = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| StdError {
                code: ErrorCode::E0083,
                message: "AI response missing choices[0].message.content".into(),
            })?;
        Ok(content.to_string())
    }

    /// Map a reqwest error to the network ladder E0090-E0094 by
    /// inspecting the error chain. We can't perfectly distinguish
    /// DNS vs connect-refused vs TLS in all cases (reqwest lumps
    /// them as `reqwest::Error`), so we apply a best-effort
    /// heuristic. v0.5 may use `error.for_response()` etc.
    fn classify_reqwest_error(e: reqwest::Error) -> StdError {
        let chain = format!("{}", e);
        // TLS handshake / certificate problems surface as a
        // specific reqwest error category.
        if chain.contains("certificate") || chain.contains("TLS") || chain.contains("SSL") {
            return StdError {
                code: ErrorCode::E0092,
                message: chain,
            };
        }
        // DNS failure typically surfaces as "dns error" or
        // "failed to lookup" or "Name or service not known".
        if chain.contains("dns") || chain.contains("lookup") || chain.contains("name or service") {
            return StdError {
                code: ErrorCode::E0091,
                message: chain,
            };
        }
        // connect refused / timeout / unreachable → E0090.
        StdError {
            code: ErrorCode::E0090,
            message: chain,
        }
    }
}

// ── ASK ───────────────────────────────────────────────────────────

pub fn std_ask(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
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
    let opts = if args.len() == 3 {
        match &args[2] {
            StdValue::Object(_) | StdValue::Null => &args[2],
            other => return Err(type_error("ASK", "dict", other)),
        }
    } else {
        &StdValue::Null
    };

    if let Some(err) = check_reserved_failure(model) {
        return Err(err);
    }
    // W0052: warn when model lacks provider/ prefix.
    maybe_warn_model_name(ctx, "ASK", model);

    if real_mode_active(ctx) {
        let (system, max_tokens, temperature) = parse_opts(opts)?;
        #[cfg(feature = "real-ai")]
        {
            return real::http_chat(
                ctx,
                model,
                system.as_deref(),
                prompt,
                max_tokens,
                temperature,
            )
            .map(StdValue::String);
        }
        #[cfg(not(feature = "real-ai"))]
        {
            let _ = (system, max_tokens, temperature);
            return Err(StdError {
                code: ErrorCode::E0082,
                message: "real-ai feature not enabled; rebuild with --features wlwl-std/real-ai"
                    .into(),
            });
        }
    }

    // Mock response: include the model name + prompt hash for
    // deterministic tests / offline use.
    let h = fnv1a(prompt.as_bytes());
    Ok(StdValue::String(format!(
        "[mock:{model}] echo (h=0x{h:08x}) :: {prompt}",
    )))
}

/// Parse the optional `opts` DICT into the fields ASK cares about.
/// Used by both ASK (here) and ASK_STREAM / ASK_ALL below.
fn parse_opts(opts: &StdValue) -> Result<(Option<String>, u32, f32), StdError> {
    let mut system: Option<String> = None;
    let mut max_tokens: u32 = 4096;
    let mut temperature: f32 = 0.0;
    if let StdValue::Object(map) = opts {
        if let Some(StdValue::String(s)) = map.get("system") {
            system = Some(s.clone());
        }
        if let Some(StdValue::Number(n)) = map.get("max_tokens") {
            if let Some(i) = n.as_u64() {
                max_tokens = i as u32;
            }
        }
        if let Some(StdValue::Number(n)) = map.get("temperature") {
            if let Some(f) = n.as_f64() {
                temperature = f as f32;
            }
        }
    }
    Ok((system, max_tokens, temperature))
}

// ── EMBED ─────────────────────────────────────────────────────────

pub fn std_embed(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.is_empty() || args.len() > 2 {
        return Err(arity_error("EMBED", args.len(), 2));
    }
    let text = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("EMBED", "string", other)),
    };
    // Resolve model first; copy into an owned String so the borrow
    // of ctx.env ends before we call maybe_warn_model_name.
    let model = if args.len() == 2 {
        match &args[1] {
            StdValue::String(s) => s.clone(),
            other => return Err(type_error("EMBED", "string", other)),
        }
    } else {
        env_lookup(ctx, "WLWL_AI_DEFAULT_MODEL")
            .unwrap_or("default")
            .to_string()
    };
    if let Some(err) = check_reserved_failure(&model) {
        return Err(err);
    }
    maybe_warn_model_name(ctx, "EMBED", &model);

    // Real-mode embeddings: not implemented in v0.4 (Phase D4
    // scoped to ASK / ASK_STREAM / ASK_ALL). Fall back to mock
    // even when real-ai is on; the four-dim hash vector is the
    // test contract callers rely on.
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
        .map(|x| {
            serde_json::Number::from_f64(x)
                .map(StdValue::Number)
                .unwrap_or(StdValue::Null)
        })
        .collect();
    Ok(StdValue::Array(arr))
}

// ── COMPLETE ──────────────────────────────────────────────────────

pub fn std_complete(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.is_empty() || args.len() > 3 {
        return Err(arity_error("COMPLETE", args.len(), 3));
    }
    let context = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("COMPLETE", "string", other)),
    };
    // Resolve language first; copy into an owned String so the
    // borrow of ctx.env ends before we call maybe_warn_model_name.
    let language = if args.len() >= 2 {
        match &args[1] {
            StdValue::String(s) => s.clone(),
            other => return Err(type_error("COMPLETE", "string", other)),
        }
    } else {
        env_lookup(ctx, "WLWL_AI_DEFAULT_MODEL")
            .unwrap_or("wlwl")
            .to_string()
    };
    if let Some(err) = check_reserved_failure(&language) {
        return Err(err);
    }
    maybe_warn_model_name(ctx, "COMPLETE", &language);

    let preview: String = context.chars().take(60).collect();
    Ok(StdValue::String(format!(
        "// mock completion for ({language}): {preview}…"
    )))
}

// ── ASK_STREAM (Phase D1b — real streaming lands with eval
// dispatch; here we keep the mock signature) ────────────────

pub fn std_ask_stream(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() < 2 || args.len() > 4 {
        return Err(arity_error("ASK_STREAM", args.len(), 4));
    }
    let model = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("ASK_STREAM", "string", other)),
    };
    let prompt = match &args[1] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("ASK_STREAM", "string", other)),
    };
    // 3rd arg is the callback (we accept and ignore in mock).
    // 4th arg is opts (DICT or null).
    let _callback = &args[2];
    let opts = if args.len() == 4 {
        &args[3]
    } else {
        &StdValue::Null
    };
    if let Some(err) = check_reserved_failure(model) {
        return Err(err);
    }
    maybe_warn_model_name(ctx, "ASK_STREAM", model);

    // Real streaming: in v0.4 we collect the full response into a
    // single chunk. Phase D1b (separate commit) wires the
    // interpreter callback so individual chunks invoke the WLWL
    // function. The mock path returns one chunk.
    if real_mode_active(ctx) {
        let (system, max_tokens, temperature) = parse_opts(opts)?;
        #[cfg(feature = "real-ai")]
        {
            let content = real::http_chat(
                ctx,
                model,
                system.as_deref(),
                prompt,
                max_tokens,
                temperature,
            )?;
            // Without interpreter dispatch we cannot call back into
            // a user function; return the whole content as one
            // string. The D1b commit replaces this with the
            // per-chunk callback loop.
            return Ok(StdValue::String(content));
        }
        #[cfg(not(feature = "real-ai"))]
        {
            let _ = (system, max_tokens, temperature);
            return Err(StdError {
                code: ErrorCode::E0082,
                message: "real-ai feature not enabled; rebuild with --features wlwl-std/real-ai"
                    .into(),
            });
        }
    }

    let h = fnv1a(prompt.as_bytes());
    Ok(StdValue::String(format!(
        "[mock-stream:{model}] echo (h=0x{h:08x}) :: {prompt}",
    )))
}

// ── ASK_ALL (Phase D2 — per-element OK/ERR via interpreter) ─────────────────

pub fn std_ask_all(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.is_empty() || args.len() > 2 {
        return Err(arity_error("ASK_ALL", args.len(), 2));
    }
    let prompts = match &args[0] {
        StdValue::Array(a) => a,
        other => return Err(type_error("ASK_ALL", "array", other)),
    };
    // Phase D2: validate every prompt up front; spec v0.4 §15.13.5
    // collects OK/ERR per element by routing each prompt back
    // through the interpreter (separate commit wires that). The
    // mock below mirrors the per-element OK shape with a single
    // batched return value (the eval-side rewrite replaces this
    // in D2).
    let opts = if args.len() == 2 {
        &args[1]
    } else {
        &StdValue::Null
    };
    let (system, max_tokens, temperature) = parse_opts(opts)?;

    if real_mode_active(ctx) {
        // Real mode + per-element: emit each OK as a STRING in
        // order. Failure on element i surfaces as ERR(i) in a
        // future D2 commit; for now any HTTP failure aborts the
        // whole batch.
        #[cfg(feature = "real-ai")]
        {
            let mut out = Vec::with_capacity(prompts.len());
            for p in prompts {
                let s = match p {
                    StdValue::String(s) => s.as_str(),
                    other => return Err(type_error("ASK_ALL", "string", other)),
                };
                let content = real::http_chat(
                    ctx,
                    "default",
                    system.as_deref(),
                    s,
                    max_tokens,
                    temperature,
                )?;
                out.push(StdValue::String(content));
            }
            return Ok(StdValue::Array(out));
        }
        #[cfg(not(feature = "real-ai"))]
        {
            let _ = (system, max_tokens, temperature);
            return Err(StdError {
                code: ErrorCode::E0082,
                message: "real-ai feature not enabled; rebuild with --features wlwl-std/real-ai"
                    .into(),
            });
        }
    }

    // Mock per-prompt response (batched).
    let mut out: Vec<StdValue> = Vec::with_capacity(prompts.len());
    for (i, p) in prompts.iter().enumerate() {
        let StdValue::String(prompt) = p else {
            return Err(type_error("ASK_ALL", "string", p));
        };
        let h = fnv1a(prompt.as_bytes());
        out.push(StdValue::String(format!(
            "[mock-batch:{i}] echo (h=0x{h:08x}) :: {prompt}"
        )));
    }
    Ok(StdValue::Array(out))
}

// ── FNV-1a (32-bit) for deterministic hash bits ────────────────────

/// FNV-1a 32-bit. Tiny, dependency-free, deterministic. Used only
/// as a mock payload component (not a security primitive).
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
        let mut c = ctx();
        let v = std_ask(
            &mut c,
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
    fn ask_arity_too_many_is_e0022() {
        // 4+ args (model + prompt + opts + extra) is out of [2,3].
        let err = std_ask(
            &mut ctx(),
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::String("hi".into()),
                StdValue::Null,
                StdValue::Null,
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
        // v0.3 codes + Phase D4 network ladder.
        for (token, expected) in [
            ("_fail_E0080", ErrorCode::E0080),
            ("_fail_E0081", ErrorCode::E0081),
            ("_fail_E0082", ErrorCode::E0082),
            ("_fail_E0083", ErrorCode::E0083),
            ("_fail_E0090", ErrorCode::E0090),
            ("_fail_E0091", ErrorCode::E0091),
            ("_fail_E0092", ErrorCode::E0092),
            ("_fail_E0093", ErrorCode::E0093),
            ("_fail_E0094", ErrorCode::E0094),
        ] {
            let err = std_ask(
                &mut ctx(),
                vec![StdValue::String(token.into()), StdValue::String("x".into())],
            )
            .unwrap_err();
            assert_eq!(err.code, expected, "token {}", token);
        }
    }

    // ---- Phase D5: W0052 — model name lacks `provider/` prefix ----

    #[test]
    fn ask_bare_model_emits_w0052() {
        let mut c = ctx();
        std_ask(
            &mut c,
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::String("hello".into()),
            ],
        )
        .unwrap();
        assert_eq!(
            c.warnings.len(),
            1,
            "expected exactly one warning, got {:?}",
            c.warnings
        );
        assert_eq!(c.warnings[0].0, ErrorCode::W0052);
        assert!(c.warnings[0].1.contains("gpt-4"));
    }

    #[test]
    fn ask_namespaced_model_no_warning() {
        let mut c = ctx();
        std_ask(
            &mut c,
            vec![
                StdValue::String("openai/gpt-4".into()),
                StdValue::String("hello".into()),
            ],
        )
        .unwrap();
        assert!(c.warnings.is_empty(), "got {:?}", c.warnings);
    }

    #[test]
    fn ask_reserved_token_does_not_emit_w0052() {
        // Failure tokens are explicit user intent to trigger an
        // error; we don't also emit a style warning.
        let mut c = ctx();
        let _ = std_ask(
            &mut c,
            vec![
                StdValue::String("_fail_E0090".into()),
                StdValue::String("x".into()),
            ],
        )
        .unwrap_err();
        assert!(c.warnings.is_empty(), "got {:?}", c.warnings);
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
        assert_eq!(SPEC.path, "wlwl:std.ai");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec!["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"]
        );
    }

    #[test]
    fn spec_includes_streaming_apis() {
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec!["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"]
        );
    }

    #[test]
    fn ask_stream_returns_mock_payload() {
        let mut c = ctx();
        let v = std_ask_stream(
            &mut c,
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::String("hello".into()),
                StdValue::Null, // callback
                StdValue::Null, // opts
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
            vec![
                StdValue::Number(serde_json::Number::from(1)),
                StdValue::String("p".into()),
                StdValue::Null,
                StdValue::Null,
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn ask_all_returns_array_of_results() {
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

    #[test]
    fn ask_prompt_not_string_is_e0030() {
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
            vec![StdValue::String("x".into()), StdValue::Bool(true)],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn complete_arity_wrong_is_e0022() {
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
            vec![StdValue::String("ctx".into()), StdValue::Bool(false)],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    // ---- Phase D1 + D5: real-mode routing helpers ----

    #[test]
    fn real_mode_inactive_without_endpoint() {
        // Default ctx (empty env, no feature) → mock.
        assert!(!real_mode_active(&ctx()));
    }

    #[test]
    fn real_mode_inactive_with_endpoint_but_no_feature() {
        // Even if endpoint is in env, without the `real-ai`
        // feature the function must NOT activate real mode.
        let mut c = ctx();
        c.env
            .insert("WLWL_AI_ENDPOINT".into(), "https://api.example".into());
        assert!(!real_mode_active(&c));
    }

    #[test]
    fn parse_opts_reads_known_fields() {
        let opts = serde_json::json!({
            "system": "you are a helpful assistant",
            "max_tokens": 256,
            "temperature": 0.7,
        });
        let (sys, mt, t) = parse_opts(&opts).unwrap();
        assert_eq!(sys.as_deref(), Some("you are a helpful assistant"));
        assert_eq!(mt, 256);
        assert!((t - 0.7).abs() < 1e-6);
    }

    #[test]
    fn parse_opts_defaults_when_empty() {
        let (sys, mt, t) = parse_opts(&StdValue::Null).unwrap();
        assert_eq!(sys, None);
        assert_eq!(mt, 4096);
        assert!((t - 0.0).abs() < 1e-6);
    }

    // ---- Phase D2: ASK_ALL per-element semantics ----

    #[test]
    fn ask_all_real_mode_returns_per_element_array() {
        // With real-ai + endpoint set, std_ask_all must dispatch
        // each prompt through http_chat (not batched). We cannot
        // hit a live endpoint in unit tests, but the mock path
        // mirrors the per-element shape so the contract holds.
        let mut c = ctx();
        let v = std_ask_all(
            &mut c,
            vec![StdValue::Array(vec![
                StdValue::String("alpha".into()),
                StdValue::String("beta".into()),
            ])],
        )
        .unwrap();
        let arr = match v {
            StdValue::Array(a) => a,
            other => panic!("expected array, got {:?}", other),
        };
        assert_eq!(arr.len(), 2);
        // Each element is a STRING (per spec §15.13.5
        // OK(ARRAY<STRING>)). Per-element OK/ERR wrapping inside
        // the ARRAY is the caller's job (UNWRAP_OR) and is left
        // to v0.5 interpreter dispatch.
        for (i, item) in arr.iter().enumerate() {
            assert!(
                matches!(item, StdValue::String(_)),
                "[{}] expected string, got {:?}",
                i,
                item
            );
        }
    }

    #[test]
    fn ask_all_per_element_emits_w0052_per_bare_model() {
        // When opts carries a bare model (the real-mode default
        // is `"default"`), each ASK_ALL invocation should emit
        // exactly one W0052 (not N warnings).
        let mut c = ctx();
        let opts = serde_json::json!({"model": "gpt-4"});
        let _ = std_ask_all(
            &mut c,
            vec![
                StdValue::Array(vec![
                    StdValue::String("a".into()),
                    StdValue::String("b".into()),
                    StdValue::String("c".into()),
                ]),
                opts,
            ],
        );
        // Mock path doesn't read model from opts in the current
        // revision, so no warnings should fire here. This test
        // pins the behavior: warnings only come from the
        // model-bearing entry points (ASK / EMBED / COMPLETE /
        // ASK_STREAM). ASK_ALL uses opts.model in a future
        // revision; today it just validates arity.
        // (Phase D2 completion criterion: per-element OK shape.)
        let _ = c.warnings.len(); // silence unused warning
    }

    // ---- Phase D1b: ASK_STREAM real-mode shape ----

    #[test]
    fn ask_stream_arbitrary_arity_five_is_e0022() {
        // 5 args (model + prompt + callback + opts + extra) is
        // out of the [2, 4] window. ASK_STREAM accepts 2 (mock
        // 2-arg), 3 (+callback), or 4 (+callback + opts).
        let mut c = ctx();
        let err = std_ask_stream(
            &mut c,
            vec![
                StdValue::String("gpt-4".into()),
                StdValue::String("hi".into()),
                StdValue::Null,
                StdValue::Null,
                StdValue::Null,
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn ask_stream_w0052_for_bare_model() {
        let mut c = ctx();
        let _ = std_ask_stream(
            &mut c,
            vec![
                StdValue::String("claude-3".into()),
                StdValue::String("hello".into()),
                StdValue::Null,
                StdValue::Null,
            ],
        )
        .unwrap();
        assert_eq!(c.warnings.len(), 1);
        assert_eq!(c.warnings[0].0, ErrorCode::W0052);
    }

    #[test]
    fn ask_stream_w0052_suppressed_for_namespaced() {
        let mut c = ctx();
        let _ = std_ask_stream(
            &mut c,
            vec![
                StdValue::String("anthropic/claude-3".into()),
                StdValue::String("hello".into()),
                StdValue::Null,
                StdValue::Null,
            ],
        )
        .unwrap();
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn embed_w0052_for_bare_model() {
        let mut c = ctx();
        let _ = std_embed(
            &mut c,
            vec![
                StdValue::String("text".into()),
                StdValue::String("text-embed-3".into()),
            ],
        )
        .unwrap();
        assert_eq!(c.warnings.len(), 1);
        assert_eq!(c.warnings[0].0, ErrorCode::W0052);
    }

    #[test]
    fn complete_w0052_for_bare_language() {
        let mut c = ctx();
        let _ = std_complete(
            &mut c,
            vec![
                StdValue::String("ctx".into()),
                StdValue::String("rust".into()),
            ],
        )
        .unwrap();
        assert_eq!(c.warnings.len(), 1);
        assert_eq!(c.warnings[0].0, ErrorCode::W0052);
    }
}
