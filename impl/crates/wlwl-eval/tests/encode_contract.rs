//! `wlwl:std.encode` 成员契约 —— 标准库规范 §11(v0.11.2 M1)。
//!
//! 与 `format_contract.rs` 同款结构(都是「一个 R2 模块一份」),这里多两件事:
//!
//! 1. **期望值取自 RFC,不是实现输出。** §11.4 的 base64 表是 RFC 4648 §10
//!    的官方向量,hex / percent-encoding 的期望也来自 RFC 3986 §2.5 的
//!    `unreserved` 集。抄实现输出等于把实现自己的 bug 钉成规范 —— 本册
//!    的正例全部手写,Rust 侧的 `encode::tests` 再拿同一批向量对一次。
//! 2. **失败格锁「形状」不锁「措辞」。** 解码失败断言的是
//!    `IS_ERR(...)` 为 `TRUE` 且载荷 `kind` 为 `DecodeError`;`reason`
//!    的文本会随措辞调整,不进断言。
//!
//! 「应然」侧(规范 §11 表格)与实现不同源是这套锁的全部意义:镜像生成器
//! 与同步测试两侧都读实现,规范表格被改坏而实现没动时它们照样全绿。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 每条自带 `IMPORT` —— 成员不是全局内建(附录 G 是全局名单的唯一真相)。
const IMP: &str = r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE","BASE64_DECODE","HEX_ENCODE","HEX_DECODE","URL_ENCODE","URL_DECODE"]); "#;

const CASES: &[Case] = &[
    // ── base64: RFC 4648 §10 表(§11.4)────────────────────────────────
    Case {
        name: "b64_empty",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("")"#
        ),
        expect: "",
    },
    Case {
        name: "b64_f",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("f")"#
        ),
        expect: "Zg==",
    },
    Case {
        name: "b64_fo",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("fo")"#
        ),
        expect: "Zm8=",
    },
    Case {
        name: "b64_foo",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("foo")"#
        ),
        expect: "Zm9v",
    },
    Case {
        name: "b64_foob",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("foob")"#
        ),
        expect: "Zm9vYg==",
    },
    Case {
        name: "b64_fooba",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("fooba")"#
        ),
        expect: "Zm9vYmE=",
    },
    Case {
        name: "b64_foobar",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("foobar")"#
        ),
        expect: "Zm9vYmFy",
    },
    // ── base64: URL-safe 分歧在语言层**可观察**(§11.4 末段)──────────
    // U+083F 的 UTF-8 字节是 E0 A0 BF ⇒ 标准表 `4KC/`、URL 表 `4KC_`。
    // 用 FROM_CODEPOINTS 而不是写字面量:终端与编辑器对这类字符的处理不一致,
    // 而这里要比的是**字节**。2 字节 UTF-8 永远产生不出 `+/`,所以码点必须
    // 从 3 字节起挑 —— 怎么挑见 §11.4 的注(不要手算)。
    Case {
        name: "b64_url_safe_flag_diverges",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE(FROM_CODEPOINTS([2111]), TRUE)"#
        ),
        expect: "4KC_",
    },
    Case {
        name: "b64_standard_alphabet_has_slash",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE(FROM_CODEPOINTS([2111]))"#
        ),
        expect: "4KC/",
    },
    Case {
        name: "b64_decode_url_safe_alphabet",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"BASE64_DECODE("4KC_")"#
        ),
        expect: "\u{083f}",
    },
    Case {
        name: "b64_decode_foobar",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"BASE64_DECODE("Zm9vYmFy")"#
        ),
        expect: "foobar",
    },
    // §11.3-1:解码两张字母表都收,且忽略折行
    Case {
        name: "b64_decode_ignores_line_breaks",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"BASE64_DECODE("Zm9v\r\nYmFy")"#
        ),
        expect: "foobar",
    },
    // 补位数量必须与末组长度匹配(§11.2)。少了这条,`"Zg="` 会被当成 `"Zg"`
    // 收下,解出比输入**短**一个字节的 `"f"` —— 截断的 base64 静默给错值。
    Case {
        name: "b64_decode_half_padded_is_decode_error",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"AT_K(ERR_PAYLOAD(BASE64_DECODE("Zg=")), "kind", "?")"#
        ),
        expect: "DecodeError",
    },
    Case {
        name: "b64_decode_one_pad_on_three_chars_is_valid",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"BASE64_DECODE("Zm8=")"#
        ),
        expect: "fo",
    },
    // 省略补位是允许的(RFC 4648 §3.2 的可选项),与「补了一半」是两种形态
    Case {
        name: "b64_decode_unpadded_is_accepted",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"[BASE64_DECODE("Zg"), BASE64_DECODE("Zm9vYmFy")]"#
        ),
        expect: "[f, foobar]",
    },
    // ── base64 失败格:锁形状不锁 reason 措辞(§11.1)──────────────────
    // 注意必须先过 `ERR_PAYLOAD`:`AT_K` **不是** §12.2 的 ERR 消费者,
    // 直接喂 ERR 会按透明传播把它顶到顶层变成 E0102,断言就变成在测别的东西。
    Case {
        name: "b64_decode_illegal_char_is_decode_error",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"AT_K(ERR_PAYLOAD(BASE64_DECODE("!!!")), "kind", "?")"#
        ),
        expect: "DecodeError",
    },
    Case {
        name: "b64_decode_data_after_padding_rejected",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"IS_ERR(BASE64_DECODE("Zm9v=Yg=="))"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "b64_decode_bad_length_rejected",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"IS_ERR(BASE64_DECODE("Zh"))"#
        ),
        expect: "TRUE",
    },
    Case {
        name: "b64_decode_non_utf8_rejected",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_DECODE"]); "#,
            r#"IS_ERR(BASE64_DECODE("/w=="))"#
        ),
        expect: "TRUE",
    },
    // ── hex ──────────────────────────────────────────────────────────
    Case {
        name: "hex_encode_lowercase_no_separator",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_ENCODE"]); "#,
            r#"HEX_ENCODE("Hi")"#
        ),
        expect: "4869",
    },
    Case {
        name: "hex_encode_empty",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_ENCODE"]); "#,
            r#"HEX_ENCODE("")"#
        ),
        expect: "",
    },
    Case {
        name: "hex_decode_round_trip",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_DECODE"]); "#,
            r#"HEX_DECODE("4869")"#
        ),
        expect: "Hi",
    },
    Case {
        name: "hex_decode_accepts_uppercase_digits",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_DECODE"]); "#,
            r#"HEX_DECODE("4a4F")"#
        ),
        expect: "JO",
    },
    Case {
        name: "hex_decode_odd_length_is_decode_error",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_DECODE"]); "#,
            r#"AT_K(ERR_PAYLOAD(HEX_DECODE("abc")), "kind", "?")"#
        ),
        expect: "DecodeError",
    },
    Case {
        name: "hex_decode_illegal_digit_is_decode_error",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_DECODE"]); "#,
            r#"AT_K(ERR_PAYLOAD(HEX_DECODE("zz")), "kind", "?")"#
        ),
        expect: "DecodeError",
    },
    // ── percent-encoding: RFC 3986 §2.1 的 unreserved 集(§11.3-2)────
    Case {
        name: "url_encode_leaves_unreserved_alone",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_ENCODE"]); "#,
            r#"URL_ENCODE("aZ09-._~")"#
        ),
        expect: "aZ09-._~",
    },
    Case {
        name: "url_encode_space_is_pct20",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_ENCODE"]); "#,
            r#"URL_ENCODE(" ")"#
        ),
        expect: "%20",
    },
    Case {
        name: "url_encode_percent_is_double_encoded",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_ENCODE"]); "#,
            r#"URL_ENCODE("%")"#
        ),
        expect: "%25",
    },
    Case {
        name: "url_encode_plus_is_pct2B",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_ENCODE"]); "#,
            r#"URL_ENCODE("+")"#
        ),
        expect: "%2B",
    },
    Case {
        name: "url_decode_round_trip",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_DECODE"]); "#,
            r#"URL_DECODE("a%20b%26c%3Dd")"#
        ),
        expect: "a b&c=d",
    },
    // §11.3-2:`+` 不折成空格,这是与 form 编码的关键分歧
    Case {
        name: "url_decode_plus_stays_literal",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_DECODE"]); "#,
            r#"URL_DECODE("a+b")"#
        ),
        expect: "a+b",
    },
    Case {
        name: "url_decode_truncated_escape_is_decode_error",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_DECODE"]); "#,
            r#"AT_K(ERR_PAYLOAD(URL_DECODE("a%2")), "kind", "?")"#
        ),
        expect: "DecodeError",
    },
    Case {
        name: "url_decode_illegal_escape_is_decode_error",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_DECODE"]); "#,
            r#"AT_K(ERR_PAYLOAD(URL_DECODE("a%zz")), "kind", "?")"#
        ),
        expect: "DecodeError",
    },
    // ── 跨成员往返:§11.3-3 的实际用法 ─────────────────────────────────
    Case {
        name: "b64_round_trip_non_ascii",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE","BASE64_DECODE"]); "#,
            r#"BASE64_DECODE(BASE64_ENCODE("Hello, 世界"))"#
        ),
        expect: "Hello, 世界",
    },
    Case {
        name: "hex_round_trip_non_ascii",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_ENCODE","HEX_DECODE"]); "#,
            r#"HEX_DECODE(HEX_ENCODE("héllo"))"#
        ),
        expect: "héllo",
    },
    // ── 程序员错误走原生诊断(§11.1)───────────────────────────────────
    Case {
        name: "encode_non_string_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_ENCODE"]); "#,
            r#"HEX_ENCODE(5)"#
        ),
        expect: "!E0030 HEX_ENCODE: expected string, got integer",
    },
    Case {
        name: "decode_arity_is_e0022",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["HEX_DECODE"]); "#,
            r#"HEX_DECODE("48", "69")"#
        ),
        expect: "!E0022 HEX_DECODE: function expects 1 argument(s), got 2",
    },
    Case {
        name: "encode_arity_zero_is_e0022",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["URL_ENCODE"]); "#,
            r#"URL_ENCODE()"#
        ),
        expect: "!E0022 URL_ENCODE: function expects 1 argument(s), got 0",
    },
    Case {
        name: "url_safe_flag_must_be_boolean",
        src: concat!(
            r#"IMPORT("wlwl:std.encode", ["BASE64_ENCODE"]); "#,
            r#"BASE64_ENCODE("x", 1)"#
        ),
        expect: "!E0030 BASE64_ENCODE: expected boolean, got integer",
    },
];

// `IMP` 保留给将来批量生成用例时引用;逐条手写 `IMPORT` 是刻意的 ——
// 它让每条用例单独复制出去仍然能跑(这正是 W-06 立的规矩)。
#[allow(dead_code)]
const _UNUSED: &str = IMP;

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_encode_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn actual(src: &str) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

#[test]
fn encode_contract() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if got == c.expect {
                None
            } else {
                Some(format!(
                    "{name}:\n      expected: {want}\n      actual:   {got}",
                    name = c.name,
                    want = c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl:std.encode contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// 规范 §11 的成员表 ↔ `SPEC.functions`。
///
/// 反向守卫:表格被改成别的形状(列数变了、签名写法变了)时提取器可能一个
/// 成员都抽不出来,那时 `[] == []` 会绿。所以正面锁住「至少 6 个」。
#[test]
fn encode_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 11 ").expect("§11 header exists");
    let end = text[start..]
        .find("\n## 12 ")
        .map(|i| start + i)
        .expect("§12 header follows §11");
    let body = &text[start..end];

    let mut spec: Vec<String> = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        // 成员格是 `` `NAME(sig)` ``,散文格里也可能出现 `` `NAME` ``。
        // 名字在**反引号或左括号**中取较早的那个 —— 只看到反引号会把
        // `BASE64_ENCODE(s, url_safe?)` 整个当成名字,然后被下面的
        // `contains('(')` 过滤掉,静默抽成 0 个。
        let name: String = rest
            .chars()
            .take_while(|c| *c != '`' && *c != '(')
            .collect();
        if name.is_empty() {
            continue;
        }
        // 只收全大写标识符 —— §11.3 / §11.4 的散文表格里还有 hex 字节之类的行。
        if name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            spec.push(name);
        }
    }

    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.encode")
        .expect("wlwl:std.encode resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();

    assert_eq!(
        spec.len(),
        6,
        "§11 table extractor found {} member(s): {spec:?} — the table's shape changed \
         and the extractor needs updating",
        spec.len()
    );
    for n in [
        "BASE64_ENCODE",
        "BASE64_DECODE",
        "HEX_ENCODE",
        "HEX_DECODE",
        "URL_ENCODE",
        "URL_DECODE",
    ] {
        assert!(
            spec.iter().any(|m| m == n),
            "§11 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.iter().any(|m| m == n),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    assert_eq!(impls.len(), 6, "wlwl:std.encode exports 6 members");
}

fn spec_path() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md")
}

/// §11.4 的 RFC 向量表与 §11.3 的三条取舍必须留在规范里。
///
/// 与 `format_contract::the_divergence_is_documented` 同理:代码侧的锁与
/// 文档侧的锁必须成对。只锁代码,下一个人看到「测试说行为是这样」却找不到
/// 依据;只锁文档,下一个人会「修实现」去迁就文档。
#[test]
fn the_rfc_vectors_and_tradeoffs_are_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in [
        "RFC 4648",
        "RFC 3986",
        "DecodeError",
        "unreserved",
        "U+083F",
    ] {
        assert!(
            text.contains(needle),
            "stdlib spec §11 lost `{needle}` — either the design changed on purpose \
             (update this test and §11 together) or the section was trimmed by accident"
        );
    }
}
