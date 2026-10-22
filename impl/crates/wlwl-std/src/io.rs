//! `wlwl:std.io` — `PRINT`, `PRINT_ERR`, `INPUT` (v0.3 §15.1).
//!
//! `PRINT` here is the std-module form. It co-exists with the
//! `resolve_builtin("PRINT")` fallback in `wlwl-eval` so that
//! programs that have NOT imported the std module still work; once
//! the user does `IMPORT("wlwl:std.io", ["PRINT"])`, the env-bound
//! `NativeFn` takes priority (per the dispatch rules in
//! `eval_call`).
//!
//! ## [F-01] 直通边界:渲染与全局内建**逐字同源**
//!
//! 本模块整体移出 `crate::compat` 的 serde_json 中间表示(ADR-0022 方向):
//! 三个成员直接收发真 [`Value`],渲染走 [`Value::display`] —— 与全局内建
//! `PRINT` / `PRINT_ERR`(`wlwl-eval` 的 `builtin_print` / `builtin_print_err`)
//! **用的是同一个函数**。
//!
//! 改之前本模块走 `crate::wrap`:`Value → serde_json::Value →` serde_json 的
//! `Display`。那一步带来**三个**可观察差异(全部违反语言规范 §2.5):
//!
//! 1. 紧凑分隔符 ⇒ 数组渲染成 `[1,2,3]` 而不是 `[1, 2, 3]`;
//! 2. `serde_json::Map` 是 `BTreeMap` ⇒ **字典键序被改写为字典序**
//!    (`["b": 1, "a": 2]` 渲染成 `{"a":2,"b":1}`),而 §2.5 的口径是插入序;
//! 3. JSON **没有 `RESULT` 变体** ⇒ `OK(1)` 被摊平成 `1`。
//!
//! ⇒ 同一份程序只要加一行 `IMPORT("wlwl:std.io", ["PRINT"])`,输出形态就变。
//! 现在两侧**逐字节相同**;进程级的守卫在
//! `crates/wlwl-cli/tests/io_print_parity.rs`。

use crate::{ModuleSpec, StdFn};
use std::io::BufRead;
use std::io::Write;
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{Outcome, StdHost, Value};

/// 渲染一行:逐个实参走 [`Value::display`],以空格分隔(语言规范 §2.5)。
///
/// 这是 `std.io` 的**唯一**渲染口径 —— `PRINT` 与 `PRINT_ERR` 共用它,
/// 而它与全局内建 `PRINT` 逐字同源(同一个 `Value::display`)。
fn render_line(args: &[Value]) -> String {
    args.iter()
        .map(|v| v.display())
        .collect::<Vec<String>>()
        .join(" ")
}

/// `PRINT(...)` —— 依序渲染实参,以空格分隔输出到标准输出;返回 `NULL`。
pub fn std_print(_host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    println!("{}", render_line(&args));
    Ok(Outcome::normal(Value::Null))
}

/// `PRINT_ERR(...)` —— `PRINT` 的 stderr 同款:同样的渲染、同样的 `NULL`
/// 返回,但写到 stderr(`eprintln!`),好让脚本把诊断与输出分开重定向。
///
/// 我们**刻意**用新鲜的 `eprintln!` 而不是 `StdCtx` 的上下文(`StdCtx`
/// 还不带 stderr 字段)—— 跨进程 I/O 不在 std 边界的契约里。给 `StdCtx`
/// 加一个 `stderr: Box<dyn Write>` 字段是 Phase D / Phase E 的事
/// (跨边界输出捕获的可测性),按计划 §5.10 推迟。
pub fn std_print_err(_host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    eprintln!("{}", render_line(&args));
    Ok(Outcome::normal(Value::Null))
}

/// `INPUT` 的实参校验 —— 与读 stdin **分开**,这样元数与类型两条判据
/// 都能在单元测试里验(读 stdin 那条路会挂住测试进程)。
///
/// 0 个实参 → 无 prompt;1 个 → `Some(prompt)`(非 `STRING` → `E0030`);
/// 超过 1 个 → `E0022`。
fn prompt_arg<'a>(host: &mut dyn StdHost, args: &'a [Value]) -> Result<Option<&'a str>, WlwlError> {
    match args {
        [] => Ok(None),
        [Value::String(s)] => Ok(Some(s.as_str())),
        [other] => Err(host.diag(
            ErrorCode::E0030,
            format!("INPUT: expected string, got {}", crate::value_kind(other)),
        )),
        _ => Err(arity(host, "INPUT", args.len(), 1)),
    }
}

/// `INPUT(prompt?)` —— 读一行标准输入,返回 `STRING`。
///
/// **0 或 1 个实参**:给了 1 个时它**必须**是 `STRING`(否则 `E0030`);
/// 超过 1 个 → `E0022`。给了 prompt 时**必须**先把 prompt 写到标准输出再读
/// (标准库·系统册 §1.1:「给了 `prompt` 时**必须**先把 `prompt` 写到标准
/// 输出再读」)。
///
/// ⚠ **EOF 与读失败是两件事**(标准库·系统册 §1.2):一个字节都没读到 ⇒
/// 返回 `""`;只有 I/O 错误才报 `E0060`。
///
/// [X-05] 此前本成员**只收 0 个实参**(给 1 个即 `E0022`),而全局内建
/// `INPUT` 收 `prompt?` —— 两者分叉。规范(标准库·系统册 §1.1)写的是
/// `INPUT(prompt?) -> STRING`,且 §1.2 要求「两条路径的可观察行为**必须**
/// 逐字相同」⇒ 改的是**实现**,不是规范。
pub fn std_input(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if let Some(prompt) = prompt_arg(host, &args)? {
        // prompt **不**带换行:换行由 prompt 自己带(与全局内建 `INPUT` 同款)。
        print!("{prompt}");
        std::io::stdout().flush().ok();
    }
    let stdin = std::io::stdin();
    let mut locked = stdin.lock();
    let line = read_input_line(&mut locked).map_err(|e| host.diag(e.code, e.message))?;
    Ok(Outcome::normal(Value::String(line)))
}

/// E0022(带函数名;措辞与 `compat::arity_error` 逐字一致)。
fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

/// `INPUT` 的失败载荷。
///
/// ⚠ 本模块整体移出了 `crate::compat` 的 serde_json 中间表示(ADR-0022
/// 方向),所以**不**复用 `compat::StdError` —— 那是 serde_json 内部表示层
/// 的配套类型。本地这一个只带 `(code, message)`,由 `std_input` 在有宿主的
/// 地方转成诊断。
#[derive(Debug)]
pub(crate) struct InputError {
    code: ErrorCode,
    message: String,
}

/// Internal helper: read one line from any `BufRead` source. Split
/// out from `std_input` (P3-009c) so the EOF / IO-error / CRLF paths
/// can be tested without spawning a subprocess.
///
/// Contract:
///   * EOF before any byte → returns `Ok("")`.
///   * Reads to `\n` or EOF, then strips trailing `\n` / `\r`.
///   * Any other I/O error → `Err` with `E0060`.
pub(crate) fn read_input_line<R: BufRead>(r: &mut R) -> Result<String, InputError> {
    let mut line = String::new();
    let n = r.read_line(&mut line).map_err(|e| InputError {
        code: ErrorCode::E0060,
        message: format!("INPUT: failed to read stdin: {}", e),
    })?;
    if n == 0 {
        // EOF before any byte
        return Ok(String::new());
    }
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(line)
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.io",
    functions: &[
        ("PRINT", std_print as StdFn),
        ("INPUT", std_input as StdFn),
        // Phase B10 (spec §15.1): PRINT_ERR writes to stderr.
        // Same value formatting as PRINT, but eprintln!.
        ("PRINT_ERR", std_print_err as StdFn),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Signal;
    use std::io::{Cursor, Error as IoError, Read};

    /// 一个够用的宿主:`std.io` 的三个成员只需要 `diag`。
    ///
    /// 形状照 `time.rs` 里的 `NullHost` —— 顺带证明 `StdHost` 的面小到
    /// 可以手写。
    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut crate::StdCtx {
            // `std.io` 的任何成员都不读 ctx;这个 panic 是**故意的** ——
            // 它把「本模块不经宿主取上下文」变成可执行的断言。
            panic!("std.io must not touch StdCtx")
        }

        fn call(
            &mut self,
            _f: &Value,
            _args: Vec<Value>,
            _name: &str,
        ) -> wlwl_error::WlwlResult<Outcome> {
            panic!("std.io has no member that calls back into the host")
        }

        fn diag(&mut self, code: ErrorCode, message: String) -> wlwl_error::WlwlError {
            wlwl_error::WlwlDiagnostic::new(
                code,
                message,
                wlwl_error::Location::point("<test>", 0, 0),
            )
            .into()
        }
    }

    #[test]
    fn print_emits_space_joined() {
        let out = std_print(
            &mut NullHost,
            vec![Value::String("hello".into()), Value::Integer(42)],
        )
        .expect("PRINT succeeds");
        assert_eq!(out.value, Value::Null);
        assert_eq!(out.signal, Signal::None);
    }

    #[test]
    fn print_err_returns_null_like_print() {
        let out =
            std_print_err(&mut NullHost, vec![Value::Integer(1)]).expect("PRINT_ERR succeeds");
        assert_eq!(out.value, Value::Null);
        assert_eq!(out.signal, Signal::None);
    }

    #[test]
    fn print_formatting_matches_builtin() {
        // The std PRINT and the builtin PRINT must agree on
        // null/boolean/array/dict formatting so the user does not see
        // a behaviour shift after IMPORT.
        //
        // [F-01] 改之前这条注释与断言**互相矛盾**:断言写的是 serde_json 的
        // 紧凑风格 `"[1,2]"`,把偏离锁成了契约(这正是缺陷能长期存在的
        // 直接原因)。现在断言的是语言规范 §2.5 的口径,注释与断言终于说
        // 同一件事。
        assert_eq!(render_line(&[Value::Null]), "NULL");
        assert_eq!(render_line(&[Value::Boolean(true)]), "TRUE");
        assert_eq!(render_line(&[Value::Boolean(false)]), "FALSE");
        let arr = Value::Array(vec![Value::Integer(1), Value::Integer(2)]);
        assert_eq!(render_line(&[arr]), "[1, 2]");
    }

    #[test]
    fn print_formatting_numbers_and_dicts() {
        // Numbers render bare; dicts render as `[k: v, ...]` in **insertion
        // order** (language spec §2.5). serde_json's `BTreeMap` order —
        // which used to rewrite the keys into dictionary order — is gone.
        assert_eq!(render_line(&[Value::Integer(42)]), "42");
        assert_eq!(render_line(&[Value::Float(1.0)]), "1.0");
        let d = Value::dict(vec![(Value::String("k".into()), Value::Integer(1))]);
        assert_eq!(render_line(&[d]), "[k: 1]");
        // Insertion order, not dictionary order.
        let d = Value::dict(vec![
            (Value::String("b".into()), Value::Integer(1)),
            (Value::String("a".into()), Value::Integer(2)),
        ]);
        assert_eq!(render_line(&[d]), "[b: 1, a: 2]");
    }

    /// [F-01] **契约守卫(a)**:同一批值,`std.io` 的渲染与
    /// `Value::display()`(全局内建 `PRINT` 用的就是它)**逐字相同**。
    ///
    /// 两道断言各有分工:
    ///   * 头一条把 std.io **绑在** `Value::display()` 上 —— 两侧不许分叉;
    ///   * 末一条把字面口径**钉死** —— `Value::display()` 自己哪天变了,
    ///     这里也会红(否则「两侧一致」但一起跑偏,没人看得见)。
    #[test]
    fn std_io_rendering_is_byte_identical_to_value_display() {
        let cases: Vec<(Value, &str)> = vec![
            (Value::Integer(42), "42"),
            (Value::Float(1.0), "1.0"),
            (Value::Float(2.5), "2.5"),
            (Value::String("hello".into()), "hello"),
            (Value::Boolean(true), "TRUE"),
            (Value::Boolean(false), "FALSE"),
            (Value::Null, "NULL"),
            (
                Value::Array(vec![
                    Value::Integer(1),
                    Value::Integer(2),
                    Value::Integer(3),
                ]),
                "[1, 2, 3]",
            ),
            (
                Value::dict(vec![
                    (Value::String("a".into()), Value::Integer(1)),
                    (Value::String("b".into()), Value::String("x".into())),
                ]),
                "[a: 1, b: x]",
            ),
            // Insertion order, not dictionary order.
            (
                Value::dict(vec![
                    (Value::String("b".into()), Value::Integer(1)),
                    (Value::String("a".into()), Value::Integer(2)),
                ]),
                "[b: 1, a: 2]",
            ),
            (Value::Ok(Box::new(Value::Integer(1))), "OK(1)"),
            (Value::Err(Box::new(Value::String("e".into()))), "ERR(e)"),
            // Nested containers render recursively, same shape.
            (
                Value::Array(vec![
                    Value::Array(vec![Value::Integer(1), Value::Integer(2)]),
                    Value::dict(vec![(
                        Value::String("k".into()),
                        Value::Array(vec![Value::Integer(3)]),
                    )]),
                ]),
                "[[1, 2], [k: [3]]]",
            ),
        ];
        for (v, expected) in &cases {
            assert_eq!(
                render_line(std::slice::from_ref(v)),
                v.display(),
                "std.io rendering must be byte-identical to Value::display() ({expected})"
            );
            assert_eq!(render_line(std::slice::from_ref(v)), *expected);
        }
    }

    #[test]
    fn input_arity_mismatch_is_e0022() {
        // [X-05] `INPUT(prompt?)` 收 **0 或 1** 个实参;>1 才报元数错。
        //
        // ⚠ 1 个实参是**合法**的(会读 stdin)—— 单元测试里**不**走那条路
        // (它会真的去读测试进程的 stdin);正向路径由 CLI 层的
        // `io_input_parity.rs` 守(那里能喂 stdin)。
        let err = std_input(
            &mut NullHost,
            vec![Value::String("p".into()), Value::String("q".into())],
        )
        .unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        assert_eq!(
            err.diagnostic().message,
            "INPUT: function expects 1 argument(s), got 2"
        );
    }

    #[test]
    fn input_prompt_must_be_string() {
        // [X-05] 给了 prompt 就必须是 `STRING` —— 类型错走 `E0030`
        // (标准库·系统册 §1.1 的失败列:「`prompt` 非 `STRING` → `E0030`」)。
        let err = std_input(&mut NullHost, vec![Value::Integer(42)]).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        assert_eq!(
            err.diagnostic().message,
            "INPUT: expected string, got integer"
        );
    }

    #[test]
    fn input_accepts_zero_or_one_argument() {
        // 元数下界是 **0** —— 0 个实参**合法**(无 prompt,直接读)。
        //
        // 这条只验**校验阶段**:验的是 `prompt_arg` 的判定,不去读 stdin
        // (读的那一段由 `read_input_line` 的单测与 CLI 层的 parity 测试覆盖)。
        assert_eq!(prompt_arg(&mut NullHost, &[]).unwrap(), None);
        assert_eq!(
            prompt_arg(&mut NullHost, &[Value::String("name: ".into())]).unwrap(),
            Some("name: ")
        );
    }

    #[test]
    fn spec_contains_print_and_input() {
        assert_eq!(SPEC.path, "wlwl:std.io");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"PRINT"));
        assert!(names.contains(&"INPUT"));
        assert!(names.contains(&"PRINT_ERR"));
    }

    // ---- read_input_line: P3-009c ----

    #[test]
    fn read_line_strips_lf() {
        let mut cur = Cursor::new(b"hello\n".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "hello");
    }

    #[test]
    fn read_line_strips_crlf() {
        // Windows line ending — should strip both \r and \n.
        let mut cur = Cursor::new(b"hello\r\n".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "hello");
    }

    #[test]
    fn read_line_strips_trailing_cr_only() {
        // Old Mac line ending — should strip the lone \r.
        let mut cur = Cursor::new(b"hello\r".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "hello");
    }

    #[test]
    fn read_line_eof_returns_empty_string() {
        // EOF before any byte — return empty string per the
        // documented contract (do NOT raise E0060).
        let mut cur = Cursor::new(Vec::<u8>::new());
        assert_eq!(read_input_line(&mut cur).unwrap(), "");
    }

    #[test]
    fn read_line_eof_after_partial_line_returns_partial() {
        // EOF in the middle of a line (no terminator) is fine — return
        // what we got.
        let mut cur = Cursor::new(b"no-newline".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "no-newline");
    }

    #[test]
    fn read_line_preserves_empty_line() {
        // A blank line (`\n` immediately) is a valid line of length
        // zero — should return `""`, not signal EOF.
        let mut cur = Cursor::new(b"\n".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "");
    }

    // Helper to convert a Read into a BufRead for the failing-read test.
    trait ReadExt: Read + Sized {
        fn bufreader(self) -> std::io::BufReader<Self> {
            std::io::BufReader::new(self)
        }
    }
    impl<T: Read> ReadExt for T {}

    // A `Read` impl that always fails — the synthetic source of E0060.
    //
    // ⚠ **它必须住在模块作用域,不能住在用例体里**:规范仓的失败口径对拍
    //   （`script/check_spec.py` 第 8 项）读行内单测时,把函数体截到
    //   「第一个整行只有 `}`」为止 —— 嵌套的 `impl` 块会在那里**截断**,
    //   截断点之后的断言宏**看不见**。断言写在用例体里就等于没写。
    struct FailingRead;
    impl Read for FailingRead {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(IoError::other("synthetic"))
        }
    }

    #[test]
    fn read_line_input_io_error_is_e0060() {
        // A `Read` impl that always fails should produce E0060, not
        // E0001 / E0002.
        //
        // ⚠ **用例名里必须带成员名 `INPUT`**:规范仓检查器的归属口径是
        //   「`#[test] fn` 名里含成员名的蛇形形式」⇒ 名字不含 `input` 就
        //   推不出这条断言属于 `std.io::INPUT`,那条 `E0060` 口径就永远
        //   处于「未钉住」状态。
        let r = FailingRead;
        let err = read_input_line(&mut r.bufreader()).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0060);
        assert!(err.message.contains("INPUT: failed to read stdin"));
    }
}
