//! **失败口径契约表** —— 一致性册 §6.2 的 **V-05**。
//!
//! ## 这册钉的是什么
//!
//! 各标准库册的**成员表**有一列「失败」,写的是这一成员在什么形态下报什么
//! **诊断码**。规范仓 `script/check_spec.py` 的第 8 项(失败口径对拍)把
//! 「规范成员表的失败列」与「实现侧的断言位」对拍,两者之差就是**缺口清单**。
//! 本册是实现侧那一半:每条用例**真的跑一次**触发该失败形态的程序,并断言
//! 它报的码与规范写的一致。
//!
//! ## 期望值**手写自规范**,不是抄实现输出
//!
//! 每条用例的注释都写明「期望值出处 = 哪册哪一节的哪句话」。
//!
//! ⚠ **本表只比「码」,不比整条消息。** 规范成员表的「失败」列写的是**码**
//!   级口径(「元数错 → `E0022`」),措辞**不构成规范** —— 标准库·安全册
//!   §11.1 明写「`reason` 是人类可读的说明文本,其**具体措辞不构成规范**」。
//!   冻结措辞会把**规范没要求的东西**也冻进去:`LCM` 的 `E0034` 消息里点名的
//!   是内部的 `NEG`(不是 `LCM`),`ai` 的 `E0080` 消息里根本没有成员名 ⇒
//!   那些是实现的内部形状,不是口径。措辞漂了会红,红的是措辞,不是口径。
//!
//! ## 归属怎么算(与规范仓检查器同口径)
//!
//! 规范仓检查器认两种「断言位」:① 契约表里成对的 `src` / `expect` 字段;
//! ② `#[test] fn` 名里含成员名的行内单测。本册走 ① —— 它的归属**按用例**
//! 算:一条用例只 `IMPORT` 一个命名空间的**一个**成员,所以「谁被调用」唯一,
//! 归属推得出来。`expect` 里带上成员名是给检查器的**兜底**归属(万一
//! 「谁被调用」推不出,退到消息里的成员名)。
//!
//! ⚠ ① 要求 `src` 与 `expect` 的**字符串字面量在同一行** —— 多行 `concat!`
//!   形态的用例检查器**看不见**。本册全部写成单行。
//!
//! ## 「章级 blanket」是什么
//!
//! 标准库各册 §0.5 记法表的「失败」行统一写着:
//! 「实参层诊断(元数错 `E0022`、类型错 `E0030`)适用于**全部**成员,由语言
//! 核心册 §8 与 §11.2 统一规定;各章**可以**重复列出以便查阅」。
//! ⇒ `E0022` / `E0030` 是**全部成员**的通用口径,**每章 1–2 条代表性用例
//! 即可**,不必逐成员铺开(规范仓检查器也按章级 blanket 判定)。
//!
//! ## 已知**不可达**的缺口(登记,不作断言)
//!
//! 下面几条在**参考实现上无法触发** —— 不是漏写用例,是形态在 mock 后端下
//! 不可达。**本表不给它们造恒真的用例**;它们在规范仓第 8 项里仍报缺口,
//! 属**如实登记**的未收敛项:
//!
//! | 缺口 | 为什么不可达 |
//! |---|---|
//! | `ai::ASK_ALL` `E0080` | `ASK_ALL(prompts, opts?)` 的签名里**没有模型名**;mock 路径不发推理请求,也就没有「推理侧失败」可报(`wlwl-std` 的 `std_ask_all_inner` 不走 `check_reserved_failure`)。 |
//! | `agent::TASK` `E0080`–`E0083` / `E0090`–`E0094` | `TASK(name, prompt, opts?)` **不做推理**:它取内置系统提示拼一段描述文本就返回(`std_task_inner`),全程不触网 ⇒ 推理侧与传输侧的失败**都产生不了**。 |
//! | `std.encode::RANDOM_BYTES` / `RANDOM_HEX` `E0060` | `E0060` 只在**操作系统熵源读失败**时产生(`fill_entropy`);熵源正常 ⇒ 不可触发。 |
//! | `std.json::STRINGIFY` `E0071` | `E0071` 是 `serde_json::to_string` 失败的兜底;而 `serde_json::Value` 的键必是字符串、数必有限 ⇒ `to_string` **不可能失败**。不可表示的值(非有限浮点)在**边界转换**那一步就被 `E0030` 挡掉了。 |
//!
//! ## `std.io::INPUT` 的 `E0060` 不在本册
//!
//! `INPUT` 的读失败是**宿主资源**上的 I/O 错误(标准库·系统册 §1.1),语言层
//! 造不出来 —— `std_input` 直接读 `std::io::stdin()`,`StdHost` **没有** stdin
//! 接缝。那条断言在 `wlwl-std` 的 `io.rs` 行内单测里(合成一个总是失败的
//! `Read`),本册不重复。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    /// `!码 成员名` —— 规范成员表的「失败」列规定的**码**,后跟成员名
    /// (给规范仓检查器做兜底归属)。本表的断言只比**码**。
    expect: &'static str,
}

/// 章级 blanket 用例:元数错 `E0022` / 类型错 `E0030`(**每章 1–2 条即可**)。
const BLANKET_CASES: &[Case] = &[
    // ── std.compress ───────────────────────────────────────────────────
    //
    // 期望值出处:标准库·系统册 §20.1 成员表「`ZSTD_COMPRESS(data, level?)` |
    // 元数错 `E0022`;非字节数组 / `level` 越界 `E0030`」;章级依据另见
    // 标准库·系统册 §0.5 记法表「失败」行(「实参层诊断……适用于**全部**成员」)。
    Case {
        name: "compress_zstd_compress_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS"]); ZSTD_COMPRESS()"#,
        expect: "!E0022 ZSTD_COMPRESS",
    },
    Case {
        name: "compress_zstd_compress_non_array_payload_is_e0030",
        src: r#"IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS"]); ZSTD_COMPRESS("not bytes")"#,
        expect: "!E0030 ZSTD_COMPRESS",
    },
    // ── std.env ─────────────────────────────────────────────────────────
    //
    // 期望值出处:标准库·系统册 §18.1 成员表「`ENV_GET(name, default?)` |
    // 元数 > 2 / 类型错 → `E0022` / `E0030`」。
    Case {
        name: "env_get_too_many_arguments_is_e0022",
        src: r#"IMPORT("wlwl:std.env", ["ENV_GET"]); ENV_GET("a", "b", "c")"#,
        expect: "!E0022 ENV_GET",
    },
    Case {
        name: "env_get_non_string_name_is_e0030",
        src: r#"IMPORT("wlwl:std.env", ["ENV_GET"]); ENV_GET(5)"#,
        expect: "!E0030 ENV_GET",
    },
    // ── std.fs ──────────────────────────────────────────────────────────
    //
    // 期望值出处:标准库·系统册 §2.1 成员表「路径族」:`PATH_JOIN(parts...)` |
    // 「元数 < 1 或类型错 → `E0022` / `E0030`」。
    Case {
        name: "fs_path_join_without_parts_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_JOIN"]); PATH_JOIN()"#,
        expect: "!E0022 PATH_JOIN",
    },
    Case {
        name: "fs_path_join_non_string_part_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_JOIN"]); PATH_JOIN(5)"#,
        expect: "!E0030 PATH_JOIN",
    },
    // ── std.json ────────────────────────────────────────────────────────
    //
    // 期望值出处:标准库·核心册 §3.1 成员表「`JSON_TOKENIZE(s)` | 元数 /
    // 类型错 → `E0022` / `E0030`」;§3.3 失败口径「元数 / 类型错仍是诊断
    // (`E0022` / `E0030`),不是 `ERR` 值」。
    Case {
        name: "json_tokenize_without_argument_is_e0022",
        src: r#"IMPORT("wlwl:std.json", ["JSON_TOKENIZE"]); JSON_TOKENIZE()"#,
        expect: "!E0022 JSON_TOKENIZE",
    },
    // ── std.math ────────────────────────────────────────────────────────
    //
    // 期望值出处:标准库·核心册 §7.1 成员表(`GCD` / `LCM` 行的「非整数实参
    // → `E0030`」);`E0022` 走章级 blanket —— 标准库·核心册 §0.5 记法表
    // 「失败」行:「实参层诊断(元数错 `E0022`、类型错 `E0030`)适用于**全部**
    // 成员,由语言核心册 §8 与 §11.2 统一规定」。
    //
    // ⚠ `GCD` 是 R1 门面(纯 wlwl),所以它的元数错由**语言层**报 `E0022`
    //   (消息形如「function expects 2..2 argument(s), got 1」),而 `E0030` 由
    //   门面的类型检查报(消息点名 `GCD`)。两者都是原生诊断。
    Case {
        name: "math_gcd_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.math", ["GCD"]); GCD(12)"#,
        expect: "!E0022 GCD",
    },
    // ── std.process ─────────────────────────────────────────────────────
    //
    // 期望值出处:标准库·系统册 §19.1 成员表「`PROCESS_ID() -> INTEGER` |
    // 元数 ≠ 0 → `E0022`」与「`PROCESS_RUN(argv, opts?)` | 元数 / 类型错 →
    // `E0022` / `E0030`」。
    Case {
        name: "process_id_with_argument_is_e0022",
        src: r#"IMPORT("wlwl:std.process", ["PROCESS_ID"]); PROCESS_ID(1)"#,
        expect: "!E0022 PROCESS_ID",
    },
    Case {
        name: "process_run_with_string_argv_is_e0030",
        src: r#"IMPORT("wlwl:std.process", ["PROCESS_RUN"]); PROCESS_RUN("not an argv array")"#,
        expect: "!E0030 PROCESS_RUN",
    },
    // ── std.rand ────────────────────────────────────────────────────────
    //
    // 期望值出处:标准库·系统册 §17.1 成员表「`SEED(entropy) -> DICT` |
    // 元数错 `E0022`;熵类型错 / 字节超 `0..=255` → `E0030`」。
    Case {
        name: "rand_seed_with_integer_entropy_is_e0030",
        src: r#"IMPORT("wlwl:std.rand", ["SEED"]); SEED(5)"#,
        expect: "!E0030 SEED",
    },
    // ── std.regex ───────────────────────────────────────────────────────
    //
    // 期望值出处:标准库·文本册 §16.1 成员「`RE(pattern) -> DICT` | 模式
    // 语法错 → `E0030`;实参类型错 → `E0030`;元数错 → `E0022`」。
    Case {
        name: "regex_re_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.regex", ["RE"]); RE()"#,
        expect: "!E0022 RE",
    },
    Case {
        name: "regex_re_non_string_pattern_is_e0030",
        src: r#"IMPORT("wlwl:std.regex", ["RE"]); RE(5)"#,
        expect: "!E0030 RE",
    },
];

/// 成员特有码的用例(不走章级 blanket)。
const MEMBER_CASES: &[Case] = &[
    // ── std.math::LCM 的 E0034 / E0035 ──────────────────────────────────
    //
    // 期望值出处:标准库·核心册 §7.1 成员表「`GCD(a, b)` / `LCM(a, b) ->
    // INTEGER` | ……取绝对值时实参为 `INTEGER` 下界 → `E0034`(中止整个
    // 运行);`LCM` 的中间乘积溢出 i64 → `E0035`」。
    //
    // ⚠ **`INTEGER` 下界写不成字面量**:`-9223372036854775808` 的**幅值**超出
    //   i64,词法层先报 `E0001`(实测)。⇒ 用 `-( -(0, 9223372036854775807), 1)`
    //   算出来(本语言没有前缀负号,减法是严格二元的 `-(a, b)` 形态)。
    Case {
        name: "math_lcm_integer_min_is_e0034",
        src: r#"IMPORT("wlwl:std.math", ["LCM"]); LCM(-( -(0, 9223372036854775807), 1), 6)"#,
        expect: "!E0034 LCM",
    },
    //
    // ⚠ `E0035` 的形态:**先除后乘**仍会溢出 —— `LCM` 取绝对值、辗转相除,
    //   乘积 `a/gcd * b` 仍可能超出 i64。`2^62` 与 `2^62-1` 互素 ⇒ 最小公倍数
    //   超出 i64 ⇒ 中止整个运行。
    Case {
        name: "math_lcm_product_overflow_is_e0035",
        src: r#"IMPORT("wlwl:std.math", ["LCM"]); LCM(4611686018427387904, 4611686018427387903)"#,
        expect: "!E0035 LCM",
    },
    // ── ai 官方包的 E0080 ───────────────────────────────────────────────
    //
    // 期望值出处:标准库·官方包册 §9.1 成员表「`EMBED(text, model?)` |
    // ……推理侧失败 → `E0080`」;§9.2 共享失败口径「推理请求失败 →
    // `E0080`」。
    //
    // ⚠ 触发手段是**参考实现的 mock 后端保留 token**:模型名取
    //   `_fail_E0080` ⇒ 该成员直接产 `E0080` 诊断(实现见 `wlwl-std` 的
    //   `ai.rs` 里 `check_reserved_failure`)。这条是**实现侧的测试接缝**,
    //   不是网络路径(真后端要凭据与网络)。
    Case {
        name: "ai_embed_reserved_failure_token_is_e0080",
        src: r#"IMPORT("ai:ask", ["EMBED"]); EMBED("hello", "_fail_E0080")"#,
        expect: "!E0080 EMBED",
    },
    //
    // 期望值出处:标准库·官方包册 §9.1 成员表「`COMPLETE(context,
    // language?, opts?)` | ……推理侧失败 → `E0080`」。
    //
    // ⚠ `COMPLETE` 的第二个形参是**语言名**,但缺省时取环境变量
    //   `WLWL_AI_DEFAULT_MODEL` ⇒ 实现侧拿它当模型名跑保留 token 检查 ⇒
    //   把它写成 `_fail_E0080` 即可触发。
    Case {
        name: "ai_complete_reserved_failure_token_is_e0080",
        src: r#"IMPORT("ai:ask", ["COMPLETE"]); COMPLETE("fn main() {}", "_fail_E0080")"#,
        expect: "!E0080 COMPLETE",
    },
    //
    // 期望值出处:标准库·官方包册 §9.1 成员表「`ASK_STREAM(model, prompt,
    // callback, opts?)` | ……推理侧失败 → `E0080`」。
    //
    // ⚠ 第三个形参 `callback` 当前**必须**是 `NULL`(§9.5 第 2 条)⇒ 传
    //   `NULL`,不能传函数值。
    Case {
        name: "ai_ask_stream_reserved_failure_token_is_e0080",
        src: r#"IMPORT("ai:ask", ["ASK_STREAM"]); ASK_STREAM("_fail_E0080", "p", NULL)"#,
        expect: "!E0080 ASK_STREAM",
    },
];

/// 跑一条用例,返回**诊断码**。
///
/// 程序**跑成功**返回 `OK`;解析失败返回 `PARSE:<码>` —— 两者都**不等于**
/// 任何 `E` 码 ⇒ 一条「本该失败却不失败」的用例必然变红,不会假绿。
fn actual_code(src: &str) -> String {
    let ast = match wlwl_parser::parse(src, "t.wll") {
        Ok(a) => a,
        Err(e) => return format!("PARSE:{}", e.diagnostic().code.as_str()),
    };
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(_) => "OK".to_string(),
        Err(e) => e.diagnostic().code.as_str().to_string(),
    }
}

/// 本表的期望值里只有**码**(后跟成员名)⇒ 取第一段作为码。
fn expected_code(expect: &str) -> &str {
    expect
        .trim_start_matches('!')
        .split(' ')
        .next()
        .unwrap_or("")
}

/// 全部用例的公共执行器:跑一遍,收集**全部**不符的条目再一起报。
fn check(cases: &[Case], what: &str) {
    let bad: Vec<String> = cases
        .iter()
        .filter_map(|c| {
            let got = actual_code(c.src);
            let want = expected_code(c.expect);
            if got == want {
                None
            } else {
                Some(format!(
                    "{name}:\n      expected: {want}\n      actual:   {got}",
                    name = c.name
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl {what} failure-surface drift:\n    {}",
        bad.join("\n    ")
    );
}

/// **章级 blanket**:`E0022` / `E0030` 适用于全部成员(标准库各册 §0.5),
/// 每章 1–2 条代表性用例即可。
#[test]
fn chapter_wide_arity_and_type_codes_are_pinned() {
    check(BLANKET_CASES, "standard library");
}

/// **成员特有码**:规范成员表的「失败」列里**逐成员**写明的那几条。
#[test]
fn member_specific_failure_codes_are_pinned() {
    check(MEMBER_CASES, "standard library");
}

/// **表里没有恒真行** —— 每条用例都必须**真的失败**。
///
/// 这条守的是「假绿」这个头号风险:一条跑成功的用例如果期望值写成
/// 「跑成功」,它就什么都没钉住。这里把「真的失败」单独钉一道:程序跑成功
/// 或根本没解析出来,都算不合格 —— 那种用例的期望值多半是抄来的。
#[test]
fn every_case_in_the_table_really_fails() {
    for c in BLANKET_CASES.iter().chain(MEMBER_CASES.iter()) {
        let got = actual_code(c.src);
        assert_ne!(got, "OK", "case {} does not fail at all", c.name);
        assert!(
            !got.starts_with("PARSE:"),
            "case {} does not even parse: {got}",
            c.name
        );
        assert!(
            got.starts_with('E'),
            "case {} produced a non-diagnostic outcome: {got}",
            c.name
        );
    }
}

/// **负向对照**:同样的比较逻辑,期望值写错时**必须**被判为不符。
///
/// 这条守的是**比较器本身** —— 一个「永远返回通过」的比较器会让整张表恒绿。
/// 它用的是一条**真的**会失败的用例(`ZSTD_COMPRESS()` 少一个实参 ⇒
/// `E0022`),把期望值改成 `E0030` ⇒ 比较器必须拒绝。
#[test]
fn the_comparator_rejects_a_wrong_expected_code() {
    let wrong = Case {
        name: "negative_control",
        src: r#"IMPORT("wlwl:std.compress", ["ZSTD_COMPRESS"]); ZSTD_COMPRESS()"#,
        expect: "!E0030 ZSTD_COMPRESS",
    };
    let got = actual_code(wrong.src);
    assert_eq!(
        got, "E0022",
        "the case under test must really be an arity error"
    );
    assert_ne!(
        got,
        expected_code(wrong.expect),
        "the comparator must reject a wrong expected code"
    );
}

/// 用例的临时目录 —— 一次编译,全量跑(本册的用例都不碰文件系统)。
fn scratch_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| common::scratch("wlwl_failure_surface"))
        .clone()
}
