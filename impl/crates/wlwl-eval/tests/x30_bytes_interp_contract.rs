//! [X-30] 字节串里的 `${` **按字面处理** —— 语言规范 §1.8。
//!
//! ## 它补的是什么
//!
//! 语言规范 §1.8 写:字节串字面量 `b"..."` 与字符串字面量的差别有三条 ——
//! ① 不做 UTF-8 校验;② **没有插值** —— `${` 在字节串里**不是**插值
//! (字节串是常量);③ 多一个 `\xNN` 转义。文法
//! `bytes_element = any_char_except_quote_backslash_newline | bytes_escape`
//! 也允许 `$` 与 `{` 作为**普通字节**。
//!
//! 缺陷形态:词法阶段把 `${` 当成**错误**(`E0001` "byte string literals do
//! not support ${...} interpolation")⇒ 规范说的是「**按字面处理**」,实现给
//! 的是「**不许出现**」⇒ 二者只有一处可改。
//!
//! ## 负向验证
//!
//! 把那条拒绝分支放回去 ⇒ **本表至少 4 条变红**
//! (`bytes.interp.*` / `bytes.interp.parses`)。
//!
//! ⚠ **普通字符串的插值不许被带坏** —— 那是同一段词法代码的另一半
//!   (`read_string` 走 `StrStart`/`StrText`/`StrEnd`)⇒ 表里有一条**回归守卫**。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 跑一段源码,返回「诊断形态」(`!CODE message`)或「值形态」(`Value::display`)。
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

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("wlwl_x30_bytes_interp")
}

const CASES: &[Case] = &[
    // ── `${` 在字节串里是**普通字节**(规范 §1.8) ─────────────────────
    Case {
        name: "bytes.interp.is_literal",
        src: r#"b"a${b}c""#,
        expect: "[97, 36, 123, 98, 125, 99]",
    },
    Case {
        name: "bytes.interp.len",
        src: r#"LEN(b"a${b}c")"#,
        expect: "6",
    },
    Case {
        name: "bytes.interp.opens_only",
        src: r#"b"${""#,
        expect: "[36, 123]",
    },
    Case {
        name: "bytes.interp.closes_only",
        src: r#"b"}$""#,
        expect: "[125, 36]",
    },
    // ⚠ 转义路径(`\$`)本来就给字面 `$` ⇒ **逐字不变**(回归守卫)。
    Case {
        name: "bytes.interp.escaped_dollar_is_unchanged",
        src: r#"b"a\${b}c""#,
        expect: "[97, 36, 123, 98, 125, 99]",
    },
    // ⚠ **普通字符串的插值不许被带坏**(回归守卫)。
    // ⚠ `+` 在本语言里是**调用式**拼写(`+(a, b)`)—— 写成中缀 `1 + 1` 会被
    //   解析成两个表达式(`"x${1 + 1}y"` ⇒ `"x11y"`)。
    Case {
        name: "string.interp.is_unchanged",
        src: r#""a${+(1, 1)}c""#,
        expect: "a2c",
    },
    // ⚠ 字节串仍**不是**新类型(规范 §1.8:脱糖成 `ARRAY[INTEGER]`)。
    Case {
        name: "bytes.type_is_array",
        src: r#"TYPE(b"ab")"#,
        expect: "ARRAY",
    },
];

#[test]
fn a_dollar_brace_in_a_byte_string_is_two_plain_bytes() {
    for c in CASES {
        let got = actual(c.src);
        assert_eq!(got, c.expect, "case {}: {}", c.name, c.src);
    }
}
