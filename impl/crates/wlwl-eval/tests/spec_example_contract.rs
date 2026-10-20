//! 规范里的**示例**必须真的能跑 —— 语言标准 §17.1「可用的规律」第 1 条。
//!
//! 背景:第二轮审计(review-2)§9 登记的「规范侧内部张力」之一 ——
//! §17.1 规律 1 的示例 `YIELD(); YIELD(); SET(k, 5); k` **按原文不可执行**:
//! §3.4 要求 `SET` 的名字**当前已绑定**且单元格**可变**(须由 `LET MUT` 建立),
//! 而示例里的 `k` 从未绑定 ⇒ 实测 `E0020`(未定义名 `k`)。
//! ⇒ 缺陷形态已修(示例补上 `LET MUT(k, 0)`),本册把「示例可执行」钉住。
//!
//! ## 两条纪律
//!
//! 1. **示例从规范正文里抽出来跑**,不是在测试里抄一份。抄一份就变成「测试
//!    自己的副本」—— 规范改了它不会红,等于没钉。本册读的是规范本体。
//! 2. **期望值不抄实现**:期望 `5` 出自规范原文那句「得 `5`」;期望的错误码
//!    `E0020` 出自 §3.4「名字不存在 → `E0020`」。
//!
//! ## 负向验证
//!
//! 把规范退回缺陷形态(抽掉 `LET MUT(k, 0)`)⇒ `the_spec_example_actually_runs`
//! **转红**(实际得到 `E0020` 而非 `5`),还原后全绿。

use std::path::PathBuf;

use wlwl_eval::Evaluator;

/// 语言标准本体(相对 `impl/crates/wlwl-eval`)。
fn spec_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("spec")
        .join("wlwl-spec-v0.11.md")
}

fn spec_text() -> String {
    let p = spec_path();
    std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "cannot read the spec at {}: {e}\n\
             a contract that silently skips when the file is missing is exactly \
             the failure mode this file exists to remove",
            p.display()
        )
    })
}

/// 从 §17.1「可用的规律」那段里,抽出**含 `SET(` 的那个**反引号代码片段。
///
/// 为什么按「含 `SET(`」挑而不是按行号挑:行号会随规范增删漂移,而「这一条
/// 规律里演示 `SET` 的那个例子」是稳定的语义锚点。
fn rule_1_set_example(spec: &str) -> String {
    let anchor = "**可用的规律,两条:**";
    let start = spec
        .find(anchor)
        .unwrap_or_else(|| panic!("§17.1's \"可用的规律,两条:\" anchor is gone from the spec"));
    let section = &spec[start..];

    // 只看紧邻的一段(规律 1 + 规律 2),不把整章都扫进来。
    let section: String = section.chars().take(1200).collect();

    let mut fragments = Vec::new();
    let mut rest = section.as_str();
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        fragments.push(after[..close].to_string());
        rest = &after[close + 1..];
    }

    let mut hits: Vec<String> = fragments
        .into_iter()
        .filter(|f| f.contains("SET("))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "§17.1's rule 1 must contain exactly one `SET(...)` example, found {}: {hits:?}",
        hits.len()
    );
    hits.pop().unwrap()
}

/// 解析 + 求值,返回可比较的文本(与 `parser_syntax_contract.rs` 同一形状)。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "contract.wll")
        .unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(std::env::temp_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let d = e.diagnostic();
            if d.message.is_empty() {
                format!("!{}", d.code.as_str())
            } else {
                format!("!{} {}", d.code.as_str(), d.message)
            }
        }
    }
}

/// §17.1 规律 1 的示例**必须真能跑**,且跑出规范写明的 `5`。
///
/// 示例被放进一个任务体里(§17.1:`YIELD` 只能在调度驱动的上下文里用),再
/// `AWAIT` 它的终值 —— 与规范那句「得 `5`」说的是同一件事。
#[test]
fn the_spec_example_actually_runs() {
    let spec = spec_text();
    let example = rule_1_set_example(&spec);

    let program = format!(
        "SCOPE(FUN(() ,\
         \n    LET(t, SPAWN(FUN(() , {example})));\
         \n    AWAIT(t)\
         \n));"
    );
    let out = actual(&program);
    assert_eq!(
        out, "5",
        "§17.1's rule-1 example must run and yield 5 (the spec says 得 `5`).\n\
         extracted example: {example}\n\
         program: {program}\n\
         got: {out}"
    );
}

/// 被 `SET` 改写的那个名字,**必须先绑定且可变**(`LET MUT`)—— §3.4。
///
/// 这一条把「示例为什么需要 `LET MUT`」钉成条文级:不是「示例里恰好写了」,
/// 而是「`SET` 的前置条件要求它」。
#[test]
fn the_spec_example_binds_the_mutated_name_before_set() {
    let spec = spec_text();
    let example = rule_1_set_example(&spec);

    // 被改的名字:`SET(<name>, …)`
    let set_at = example
        .find("SET(")
        .unwrap_or_else(|| panic!("no SET( in the extracted example: {example}"));
    let after_set = &example[set_at + "SET(".len()..];
    let name: String = after_set
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    assert!(!name.is_empty(), "cannot read the mutated name: {example}");

    let mutable_binding = format!("LET MUT({name},");
    assert!(
        example.contains(&mutable_binding),
        "§17.1's rule-1 example mutates `{name}` but never binds it as mutable.\n\
         §3.4: `SET` requires the name to be currently bound and the cell to be \
         mutable (established by `LET MUT`), otherwise it is `E0020` / `E0024`.\n\
         expected to find `{mutable_binding}` in: {example}"
    );
}
