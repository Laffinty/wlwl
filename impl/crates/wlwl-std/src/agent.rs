//! `wlwl:std.agent` \u2014 agent-style LLM helpers (v0.4 §15.14).
//!
//! Phase D3 (§15.14.1 \u2013 §15.14.5): thin wrappers over `wlwl:std.ai`
//! for the high-level "agent" shape that AI tools prefer. The v0.4
//! surface is intentionally minimal: each function delegates to
//! `wlwl:std.ai.ASK` (for TASK) or returns a structured DICT (for
//! TOOL / MODEL / CONTEXT). The richer orchestration \u2014 tool
//! calling loops, multi-turn memory, sub-agent dispatch \u2014 lands
//! in v0.5 alongside the interpreter-callback work in D1b / D2.
//!
//! ## Functions
//!
//! | Name        | Arity     | What it does                              |
//! |-------------|-----------|-------------------------------------------|
//! | `TASK`      | 2..3      | ASK with a pre-set system prompt for `name` |
//! | `TOOL`      | 2..4      | Returns a TOOL spec DICT (name/desc/schema) |
//! | `CALL_TOOL` | 2         | Executes a TOOL spec; v0.4 mock (E0093 if real wire) |
//! | `MODEL`     | 1         | Returns MODEL info {name, provider, caps} |
//! | `CONTEXT`   | 1..2      | Sets/gets a process-level context key     |
//!
//! ## Task registry (v0.4 §15.14.1)
//!
//! TASK(name, prompt, opts?) looks up `name` in a small built-in
//! registry and prepends the matching system prompt before
//! dispatching to `ASK`. Unrecognized names fall back to a
//! generic helper system prompt + a `W0052` name-bucket warning.
//!
//! ## 与类型名 `TASK` 的同名问题 (v0.8 §2.8 / G-04)
//!
//! 本模块导出 `TASK(name, prompt, opts?) -> TASK_RESULT`(agent 形态)与
//! `wlwl:std.ai.TASK(...)`(ai 形态);而 §2.1 类型表里的类型名 `TASK`
//! 也叫 `TASK`。同名不冲突,理由:
//!
//! - `TYPE(handle)` 返回字符串 `"TASK"`,**`TYPE` 形参位置不做名字解析** —
//!   它取运行时值,返回字符串,过程中不查任何名字表。
//! - 用户作用域内的 `TASK`(由 `IMPORT` 注入)与类型名空间独立;两者在各自
//!   作用域中独立解析。`IMPORT("wlwl:std.agent", TASK)` 注入的是本模块的
//!   函子,不是类型名。
//! - 任何上下文歧义由文法消解:类型名出现在 `:` 类型注解后(`LET(x: TASK, ...)`),
//!   标识符出现在表达式位置(`IMPORT(...) TASK(...)`)。两者在不同语法槽位,
//!   parser 不需要符号表预查。
//!
//! **文档约定**:本模块内及 `wlwl-spec-v0.7.md` §10.11 一律写作
//! `wlwl:std.agent.TASK` / `wlwl:std.ai.TASK` 以避免阅读混淆。
//! 用户代码内直接 `TASK(...)` 也合法(取决于 `IMPORT` 是否引入同名函子)。
//! 函数签名表格里的"显示形式"沿用裸名 `TASK` — 不动 §10.11 实际签名表。

use crate::{arity_error, type_error, ModuleSpec, StdCtx, StdError, StdFn, StdValue};
use serde_json;
use wlwl_error::ErrorCode;
// use std::collections::BTreeMap; // StdValue = serde_json::Value uses serde_json::Map

/// Built-in TASK system prompts.
fn lookup_task_prompt(name: &str) -> Option<&'static str> {
    match name {
        "summarize" => Some(
            "You are a summarizer. Reply in plain prose. Keep the reply to 3 \
             sentences or fewer. Preserve all named entities, dates, and numeric \
             values from the input.",
        ),
        "translate" => Some(
            "You are a translator. Detect the source language and translate to \
             the language named in the user's prompt. Preserve tone and \
             register; do not paraphrase.",
        ),
        "extract" => Some(
            "You are a structured extractor. Reply with JSON that matches the \
             schema in the user's prompt. Do not include prose, explanations, \
             or markdown fences.",
        ),
        "classify" => Some(
            "You are a classifier. Reply with exactly one label from the \
             allowed set named in the user's prompt. If unsure, reply with \
             the 'unknown' label.",
        ),
        "rewrite" => Some(
            "You are an editor. Rewrite the user's text for clarity and \
             concision while preserving the original meaning, voice, and \
             named entities.",
        ),
        _ => None,
    }
}

// \u2500\u2500 TASK \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

pub fn std_task(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_error("TASK", args.len(), 3));
    }
    let name = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("TASK", "string", other)),
    };
    let prompt = match &args[1] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("TASK", "string", other)),
    };
    let _opts = if args.len() == 3 {
        &args[2]
    } else {
        &StdValue::Null
    };

    let system_prompt = match lookup_task_prompt(name) {
        Some(s) => s,
        None => {
            ctx.warn(
                ErrorCode::W0052,
                format!(
                    "TASK: unknown task name `{}`; using a generic helper prompt. \
                     Known tasks: summarize, translate, extract, classify, rewrite.",
                    name
                ),
            );
            "You are a helpful assistant."
        }
    };

    Ok(StdValue::String(format!(
        "[task:{name}] sys=\"{}\" user=\"{prompt}\"",
        system_prompt.replace('\n', " ")
    )))
}

// \u2500\u2500 TOOL \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

pub fn std_tool(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() < 2 || args.len() > 4 {
        return Err(arity_error("TOOL", args.len(), 4));
    }
    let _ = ctx;
    let name = match &args[0] {
        StdValue::String(s) => s.clone(),
        other => return Err(type_error("TOOL", "string", other)),
    };
    let description = match &args[1] {
        StdValue::String(s) => s.clone(),
        other => return Err(type_error("TOOL", "string", other)),
    };
    let params_schema = if args.len() >= 3 {
        match &args[2] {
            StdValue::Object(_) | StdValue::Null => Some(args[2].clone()),
            other => return Err(type_error("TOOL", "dict", other)),
        }
    } else {
        Some(StdValue::Null)
    };
    let returns_schema = if args.len() == 4 {
        match &args[3] {
            StdValue::Object(_) | StdValue::Null => Some(args[3].clone()),
            other => return Err(type_error("TOOL", "dict", other)),
        }
    } else {
        Some(StdValue::Null)
    };

    let mut map: serde_json::Map<String, StdValue> = serde_json::Map::new();
    map.insert("name".into(), StdValue::String(name));
    map.insert("description".into(), StdValue::String(description));
    map.insert(
        "params_schema".into(),
        params_schema.unwrap_or(StdValue::Null),
    );
    map.insert(
        "returns_schema".into(),
        returns_schema.unwrap_or(StdValue::Null),
    );
    map.insert("v".into(), StdValue::String("0.4.0".into()));
    Ok(StdValue::Object(map))
}

// \u2500\u2500 CALL_TOOL \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

pub fn std_call_tool(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() != 2 {
        return Err(arity_error("CALL_TOOL", args.len(), 2));
    }
    let _ = ctx;
    let tool = match &args[0] {
        StdValue::Object(_) => &args[0],
        other => return Err(type_error("CALL_TOOL", "dict", other)),
    };
    let params = match &args[1] {
        StdValue::Object(_) | StdValue::Null => &args[1],
        other => return Err(type_error("CALL_TOOL", "dict", other)),
    };

    let tool_name = match tool {
        StdValue::Object(m) => match m.get("name") {
            Some(StdValue::String(s)) => s.clone(),
            _ => "<unnamed>".to_string(),
        },
        _ => "<unnamed>".to_string(),
    };

    let mut out: serde_json::Map<String, StdValue> = serde_json::Map::new();
    out.insert("ok".into(), StdValue::Bool(false));
    out.insert("error_code".into(), StdValue::String("E0093".into()));
    out.insert(
        "message".into(),
        StdValue::String(format!(
            "CALL_TOOL({}) is mocked in v0.4. Process-level tool executor \
             lands in v0.5 (plan §5.4). Pass `params={:?}`.",
            tool_name, params
        )),
    );
    out.insert("params".into(), params.clone());
    Ok(StdValue::Object(out))
}

// \u2500\u2500 MODEL \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

pub fn std_model(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() != 1 {
        return Err(arity_error("MODEL", args.len(), 1));
    }
    let _ = ctx;
    let name = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("MODEL", "string", other)),
    };

    let (provider, model) = match name.find('/') {
        Some(i) => (name[..i].to_string(), name[i + 1..].to_string()),
        None => {
            ctx.warn(
                ErrorCode::W0052,
                format!(
                    "MODEL: name `{}` has no `provider/` prefix; \
                     `provider` defaults to `unknown`.",
                    name
                ),
            );
            ("unknown".to_string(), name.to_string())
        }
    };

    let mut caps: Vec<StdValue> = Vec::new();
    if model.starts_with("gpt-4") || model.starts_with("gpt-3.5") {
        caps.push(StdValue::String("chat".into()));
        caps.push(StdValue::String("function_call".into()));
        caps.push(StdValue::String("json_mode".into()));
    } else if model.starts_with("claude") || model.starts_with("claude-") {
        caps.push(StdValue::String("chat".into()));
        caps.push(StdValue::String("vision".into()));
        caps.push(StdValue::String("tool_use".into()));
    } else if model.starts_with("text-embed") {
        caps.push(StdValue::String("embed".into()));
    } else {
        caps.push(StdValue::String("chat".into()));
    }

    let mut map: serde_json::Map<String, StdValue> = serde_json::Map::new();
    map.insert("name".into(), StdValue::String(name.to_string()));
    map.insert("provider".into(), StdValue::String(provider));
    map.insert("model".into(), StdValue::String(model));
    map.insert("capabilities".into(), StdValue::Array(caps));
    map.insert("v".into(), StdValue::String("0.4.0".into()));
    Ok(StdValue::Object(map))
}

// \u2500\u2500 CONTEXT \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

pub fn std_context(ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.is_empty() || args.len() > 2 {
        return Err(arity_error("CONTEXT", args.len(), 2));
    }
    let key = match &args[0] {
        StdValue::String(s) => s.as_str(),
        other => return Err(type_error("CONTEXT", "string", other)),
    };
    let env_key = format!("WLWL_AGENT_CTX_{}", key.to_uppercase());

    if args.len() == 2 {
        let value = &args[1];
        let raw = match value {
            StdValue::String(s) => s.clone(),
            StdValue::Null => "".to_string(),
            other => format!("{:?}", other),
        };
        ctx.env.insert(env_key, raw);
        Ok(StdValue::Null)
    } else {
        Ok(match ctx.env.get(&env_key) {
            Some(v) => StdValue::String(v.clone()),
            None => StdValue::Null,
        })
    }
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.agent",
    functions: &[
        ("TASK", std_task as StdFn),
        ("TOOL", std_tool as StdFn),
        ("CALL_TOOL", std_call_tool as StdFn),
        ("MODEL", std_model as StdFn),
        ("CONTEXT", std_context as StdFn),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> StdCtx {
        StdCtx::default()
    }

    #[test]
    fn task_known_name_returns_mock_payload() {
        let mut c = ctx();
        let v = std_task(
            &mut c,
            vec![
                StdValue::String("summarize".into()),
                StdValue::String("long article".into()),
            ],
        )
        .unwrap();
        match v {
            StdValue::String(s) => {
                assert!(s.contains("[task:summarize]"), "{}", s);
                assert!(s.contains("long article"));
            }
            other => panic!("got {:?}", other),
        }
    }

    #[test]
    fn task_unknown_name_emits_w0052_and_uses_generic() {
        let mut c = ctx();
        let v = std_task(
            &mut c,
            vec![
                StdValue::String("not-a-real-task".into()),
                StdValue::String("hi".into()),
            ],
        )
        .unwrap();
        match v {
            StdValue::String(s) => {
                assert!(s.contains("not-a-real-task"));
                assert!(s.contains("helpful assistant"));
            }
            other => panic!("got {:?}", other),
        }
        assert_eq!(c.warnings.len(), 1);
        assert_eq!(c.warnings[0].0, ErrorCode::W0052);
    }

    #[test]
    fn task_arity_wrong_is_e0022() {
        let mut c = ctx();
        let err = std_task(&mut c, vec![StdValue::String("summarize".into())]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn task_non_string_name_is_e0030() {
        let mut c = ctx();
        let err = std_task(
            &mut c,
            vec![
                StdValue::Number(serde_json::Number::from(1)),
                StdValue::String("p".into()),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn tool_returns_dict_with_all_fields() {
        let mut c = ctx();
        let schema = serde_json::json!({"type": "object"});
        let v = std_tool(
            &mut c,
            vec![
                StdValue::String("weather_lookup".into()),
                StdValue::String("Look up weather for a city".into()),
                schema,
            ],
        )
        .unwrap();
        let m = match v {
            StdValue::Object(m) => m,
            other => panic!("expected dict, got {:?}", other),
        };
        assert_eq!(
            m.get("name"),
            Some(&StdValue::String("weather_lookup".into()))
        );
        assert_eq!(m.get("v"), Some(&StdValue::String("0.4.0".into())));
        assert!(m.contains_key("description"));
        assert!(m.contains_key("params_schema"));
    }

    #[test]
    fn tool_no_schema_defaults_null() {
        let mut c = ctx();
        let v = std_tool(
            &mut c,
            vec![
                StdValue::String("noop".into()),
                StdValue::String("does nothing".into()),
            ],
        )
        .unwrap();
        let m = match v {
            StdValue::Object(m) => m,
            other => panic!("got {:?}", other),
        };
        assert_eq!(m.get("params_schema"), Some(&StdValue::Null));
        assert_eq!(m.get("returns_schema"), Some(&StdValue::Null));
    }

    #[test]
    fn tool_arity_wrong_is_e0022() {
        let mut c = ctx();
        let err = std_tool(&mut c, vec![StdValue::String("x".into())]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn tool_non_string_name_is_e0030() {
        let mut c = ctx();
        let err = std_tool(
            &mut c,
            vec![
                StdValue::Number(serde_json::Number::from(1)),
                StdValue::String("d".into()),
            ],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn call_tool_returns_v04_mock_envelope() {
        let mut c = ctx();
        let tool = serde_json::json!({
            "name": "weather_lookup",
            "description": "Look up weather",
            "params_schema": null,
            "returns_schema": null,
            "v": "0.4.0",
        });
        let params = serde_json::json!({"city": "Beijing"});
        let v = std_call_tool(&mut c, vec![tool, params]).unwrap();
        let m = match v {
            StdValue::Object(m) => m,
            other => panic!("expected dict, got {:?}", other),
        };
        assert_eq!(m.get("ok"), Some(&StdValue::Bool(false)));
        assert_eq!(m.get("error_code"), Some(&StdValue::String("E0093".into())));
        assert!(m.contains_key("message"));
    }

    #[test]
    fn call_tool_non_dict_first_arg_is_e0030() {
        let mut c = ctx();
        let err = std_call_tool(
            &mut c,
            vec![StdValue::String("not-a-dict".into()), StdValue::Null],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn call_tool_arity_wrong_is_e0022() {
        let mut c = ctx();
        let err = std_call_tool(&mut c, vec![StdValue::Null]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn model_namespaced_returns_provider_and_caps() {
        let mut c = ctx();
        let v = std_model(&mut c, vec![StdValue::String("openai/gpt-4".into())]).unwrap();
        let m = match v {
            StdValue::Object(m) => m,
            other => panic!("expected dict, got {:?}", other),
        };
        assert_eq!(m.get("provider"), Some(&StdValue::String("openai".into())));
        assert_eq!(m.get("model"), Some(&StdValue::String("gpt-4".into())));
        let caps = match m.get("capabilities") {
            Some(StdValue::Array(a)) => a,
            other => panic!("caps not array: {:?}", other),
        };
        let has_chat = caps
            .iter()
            .any(|x| matches!(x, StdValue::String(s) if s == "chat"));
        assert!(has_chat, "expected chat cap, got {:?}", caps);
        assert!(c.warnings.is_empty(), "got {:?}", c.warnings);
    }

    #[test]
    fn model_bare_emits_w0052() {
        let mut c = ctx();
        let v = std_model(&mut c, vec![StdValue::String("gpt-4".into())]).unwrap();
        let m = match v {
            StdValue::Object(m) => m,
            other => panic!("got {:?}", other),
        };
        assert_eq!(m.get("provider"), Some(&StdValue::String("unknown".into())));
        assert_eq!(c.warnings.len(), 1);
        assert_eq!(c.warnings[0].0, ErrorCode::W0052);
    }

    #[test]
    fn model_anthropic_includes_tool_use_cap() {
        let mut c = ctx();
        let v = std_model(
            &mut c,
            vec![StdValue::String("anthropic/claude-3-opus".into())],
        )
        .unwrap();
        let m = match v {
            StdValue::Object(m) => m,
            other => panic!("got {:?}", other),
        };
        let caps = match m.get("capabilities") {
            Some(StdValue::Array(a)) => a,
            other => panic!("caps not array: {:?}", other),
        };
        let has_tool_use = caps
            .iter()
            .any(|x| matches!(x, StdValue::String(s) if s == "tool_use"));
        assert!(has_tool_use, "got {:?}", caps);
    }

    #[test]
    fn model_arity_wrong_is_e0022() {
        let mut c = ctx();
        let err = std_model(&mut c, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn model_non_string_name_is_e0030() {
        let mut c = ctx();
        let err =
            std_model(&mut c, vec![StdValue::Number(serde_json::Number::from(1))]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn context_set_then_get_roundtrips_via_env() {
        let mut c = ctx();
        std_context(
            &mut c,
            vec![
                StdValue::String("language".into()),
                StdValue::String("zh".into()),
            ],
        )
        .unwrap();
        let got = std_context(&mut c, vec![StdValue::String("language".into())]).unwrap();
        assert_eq!(got, StdValue::String("zh".into()));
    }

    #[test]
    fn context_missing_returns_null() {
        let mut c = ctx();
        let got = std_context(&mut c, vec![StdValue::String("not-set".into())]).unwrap();
        assert_eq!(got, StdValue::Null);
    }

    #[test]
    fn context_arity_wrong_is_e0022() {
        let mut c = ctx();
        let err = std_context(&mut c, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn context_non_string_key_is_e0030() {
        let mut c = ctx();
        let err =
            std_context(&mut c, vec![StdValue::Number(serde_json::Number::from(1))]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }

    #[test]
    fn spec_contains_all_five() {
        assert_eq!(SPEC.path, "wlwl:std.agent");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["TASK", "TOOL", "CALL_TOOL", "MODEL", "CONTEXT"]);
    }
}
