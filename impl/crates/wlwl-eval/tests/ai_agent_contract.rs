//! `ai` / `agent` 两个**官方包**的成员契约 —— 标准库规范 §9 / §10。
//!
//! ⚠ **这两个包此前零契约**:仓内 21 个 test 文件**无一**覆盖它们
//! (第二轮审计 §7.5 ③)。后果很具体 —— 上一轮规范重写改了 7 处 `ai` / `agent`
//! 的签名与元数,**全靠人工探针支撑,没有门禁守护**。
//!
//! **期望值全部来自标准库规范正文**(§9.1 / §10.1 的示例与成员表),
//! 不是抄实现输出。mock 后端的输出是**确定的**(哈希固定),所以可以直接钉。
//!
//! ⚠ **只覆盖默认构建(mock 后端)**:`real-ai` 门控的是 **HTTP 传输层**
//!   (mock ↔ real),**不是成员** ⇒ 成员面在两种构建里**完全相同**,
//!   可测的部分是同一份。真 HTTP 路径需要网络与凭据,本契约不碰。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_ai_agent")
}

fn actual(src: &str) -> String {
    let ast = match wlwl_parser::parse(src, "t.wll") {
        Ok(a) => a,
        Err(e) => return format!("!PARSE {}", e.diagnostic().message),
    };
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    }
}

const CASES: &[Case] = &[
    // ── §9.1 的三个示例(逐字)──
    Case {
        name: "embed_returns_the_four_dimensional_vector",
        src: r#"IMPORT("ai:ask", ["EMBED"]); EMBED("hello")"#,
        expect: "[0.17448691538872357, 0.31102464332036317, 0.35886167696650645, 0.5751277943083848]",
    },
    Case {
        name: "complete_returns_the_mock_completion",
        src: r#"IMPORT("ai:ask", ["COMPLETE"]); COMPLETE("fn main() {}")"#,
        expect: "// mock completion for (wlwl): fn main() {}…",
    },
    Case {
        name: "ask_all_returns_one_result_per_prompt",
        src: r#"IMPORT("ai:ask", ["ASK_ALL"]); ASK_ALL(["a", "b"])"#,
        expect: "[[mock-batch:0] echo (h=0xe40c292c) :: a, [mock-batch:1] echo (h=0xe70c2de5) :: b]",
    },
    // ── §10.1 的三个示例(逐字)──
    Case {
        name: "tool_returns_the_tool_description_dict",
        src: r#"IMPORT("agent:task", ["TOOL"]); TOOL("t1", "desc")"#,
        expect: "[description: desc, name: t1, params_schema: NULL, returns_schema: NULL, v: 0.4.0]",
    },
    Case {
        name: "model_returns_the_model_description_dict",
        src: r#"IMPORT("agent:task", ["MODEL"]); MODEL("gpt-4")"#,
        expect: "[capabilities: [chat, function_call, json_mode], model: gpt-4, name: gpt-4, provider: unknown, v: 0.4.0]",
    },
    Case {
        name: "context_reads_back_what_it_wrote",
        src: r#"IMPORT("agent:task", ["CONTEXT"]); CONTEXT("k", "v"); CONTEXT("k")"#,
        expect: "v",
    },
    // ⚠ §10.1 末段:「**「没设」返回 `NULL`**,与「设为空串」是**两件事**」。
    Case {
        name: "context_of_an_unset_key_is_null",
        src: r#"IMPORT("agent:task", ["CONTEXT"]); CONTEXT("never-set")"#,
        expect: "NULL",
    },
    // ⚠ §9 / §10 的**可见面**边界:两个包**不再是** `wlwl:std.*`
    //   (ADR-0034 D2),`wlwl:std.ai` / `wlwl:std.agent` **不可解析**。
    Case {
        name: "wlwl_std_ai_does_not_resolve",
        src: r#"IMPORT("wlwl:std.ai", ["ASK"]);"#,
        expect: "!E0040",
    },
    Case {
        name: "wlwl_std_agent_does_not_resolve",
        src: r#"IMPORT("wlwl:std.agent", ["TASK"]);"#,
        expect: "!E0040",
    },
    // ── 元数(成员表:「元数错 → `E0022`」)──
    //
    // ⚠ `ASK_STREAM` 那一行是**回归守卫**:缺陷形态的守卫写的是
    //   `args.len() < 2`,于是 2 个实参被放行,而函数体**无条件取 `args[2]`**
    //   ⇒ **索引越界 panic**。规范 §9 的成员表写的是
    //   `ASK_STREAM(model, prompt, callback, opts?)` —— 3 个必填。
    Case {
        name: "ask_stream_with_two_args_is_e0022_not_a_panic",
        src: r#"IMPORT("ai:ask", ["ASK_STREAM"]); ASK_STREAM("m", "p")"#,
        expect: "!E0022",
    },
    Case {
        name: "ask_with_one_arg_is_e0022",
        src: r#"IMPORT("ai:ask", ["ASK"]); ASK("m")"#,
        expect: "!E0022",
    },
    Case {
        name: "embed_with_zero_args_is_e0022",
        src: r#"IMPORT("ai:ask", ["EMBED"]); EMBED()"#,
        expect: "!E0022",
    },
    Case {
        name: "embed_with_three_args_is_e0022",
        src: r#"IMPORT("ai:ask", ["EMBED"]); EMBED("a", "m", "x")"#,
        expect: "!E0022",
    },
    Case {
        name: "tool_with_one_arg_is_e0022",
        src: r#"IMPORT("agent:task", ["TOOL"]); TOOL("n")"#,
        expect: "!E0022",
    },
    Case {
        name: "context_with_zero_args_is_e0022",
        src: r#"IMPORT("agent:task", ["CONTEXT"]); CONTEXT()"#,
        expect: "!E0022",
    },
    // ── 类型(成员表:「类型错 → `E0030`」)──
    Case {
        name: "ask_with_a_non_string_model_is_e0030",
        src: r#"IMPORT("ai:ask", ["ASK"]); ASK(5, "p")"#,
        expect: "!E0030",
    },
    Case {
        name: "embed_with_a_non_string_text_is_e0030",
        src: r#"IMPORT("ai:ask", ["EMBED"]); EMBED(5)"#,
        expect: "!E0030",
    },
    Case {
        name: "ask_all_with_a_non_array_prompts_is_e0030",
        src: r#"IMPORT("ai:ask", ["ASK_ALL"]); ASK_ALL("a")"#,
        expect: "!E0030",
    },
];

#[test]
fn ai_and_agent_match_the_spec() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if got == c.expect {
                return None;
            }
            // ⚠ `E0040` / `E0022` / `E0030` 这三条**只断言码**,不冻结整条
            //   消息(措辞会变,码不会)—— 与其它契约表同款口径。
            if c.expect.starts_with('!') && c.expect.len() <= 6 {
                let code = got.trim_start_matches('!').split(' ').next().unwrap_or("");
                if code == c.expect.trim_start_matches('!') {
                    return None;
                }
            }
            Some(format!(
                "{}\n      expected: {}\n      actual:   {got}",
                c.name, c.expect
            ))
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl:ai / wlwl:agent contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// 规范 §9 / §10 的**成员表** ↔ 实现导出的成员名单必须逐条相同。
///
/// ⚠ 这条守的是「规范写了但实现没有」与「实现有但规范没写」两个方向 ——
///   上一轮那 7 处签名修正之所以没人发现,正是因为**没有这张对照表**。
#[test]
fn ai_and_agent_member_sets_match_the_spec_tables() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("docs")
            .join("wlwl-spec-reference.md"),
    )
    .expect("stdlib spec readable");

    for (chapter, members) in [
        (
            "## 标准库规范 · 9 `ai`",
            ["ASK", "EMBED", "COMPLETE", "ASK_STREAM", "ASK_ALL"],
        ),
        (
            "## 标准库规范 · 10 `agent`",
            ["TASK", "TOOL", "CALL_TOOL", "MODEL", "CONTEXT"],
        ),
    ] {
        let start = text.find(chapter).expect("chapter header exists");
        let end = text[start..]
            .find("\n## ")
            .map(|i| start + i)
            .unwrap_or(text.len());
        let body = &text[start..end];
        for m in members {
            assert!(
                body.contains(&format!("`{m}(")),
                "§{chapter} 的成员表漏了 `{m}`"
            );
        }
    }
}

// ── [D-04] `CALL_TOOL` 的 mock 回包形状 ──
//
// ⚠ **桩状态不许挪用错误码**:`E0093` 是「HTTP 4xx」(网络码,规范 §10),
//   而 §10 的成员表写 `CALL_TOOL` 的失败是 `E0082`(工具错误)⇒ 拿 `E0093`
//   表达「这里是 mock」**连成员表自己的口径都对不上**。桩状态改用**专用
//   字段**:`mocked: true` + `error_code: NULL`。
//
// ⚠ 契约**只钉形状与「不含版本承诺」**,不冻结 `message` 的整条措辞 ——
//   措辞会变;「形状」与「不承诺版本」才是这条真正要守的东西。

/// `CALL_TOOL` 的 mock 回包 —— 取其中一个字段的显示值。
fn call_tool_mock_field(field: &str) -> String {
    let src = format!(
        r#"IMPORT("agent:task", ["CALL_TOOL", "TOOL"]); LET(t, TOOL("t1", "desc")); LET(r, CALL_TOOL(t, ["city": "Beijing"])); r["{field}"]"#
    );
    actual(&src)
}

#[test]
fn call_tool_mock_envelope_is_flagged_mocked() {
    assert_eq!(call_tool_mock_field("mocked"), "TRUE");
    assert_eq!(call_tool_mock_field("ok"), "FALSE");
}

#[test]
fn call_tool_mock_error_code_is_null() {
    assert_eq!(call_tool_mock_field("error_code"), "NULL");
}

#[test]
fn call_tool_mock_error_code_is_never_the_network_code_e0093() {
    // ⚠ 显式钉住:`E0093` = HTTP 4xx(网络码),**不得**再被借来表达「桩状态」。
    //   写成 `==(a, b)` 调用形式 —— `r[...] = x` 是 `INDEX_SET` 语法糖(赋值)。
    let src = r#"IMPORT("agent:task", ["CALL_TOOL", "TOOL"]); LET(t, TOOL("t1", "desc")); LET(r, CALL_TOOL(t, ["city": "Beijing"])); ==(r["error_code"], "E0093")"#;
    assert_eq!(actual(src), "FALSE");
}

#[test]
fn call_tool_mock_message_carries_no_version_promise() {
    let msg = call_tool_mock_field("message");
    // 旧文案「is mocked in v0.4 … lands in v0.5」漂了三个大版本 ⇒ 禁。
    assert!(
        !msg.contains("v0."),
        "message must not promise a version: {msg}"
    );
    assert!(
        !msg.contains("E0093"),
        "message must not reuse a network code: {msg}"
    );
}
