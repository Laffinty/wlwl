// [v0.10.1 / R10-050] 附录 A 文法的**机器**校验。
//!
//! 为什么需要它:附录 A 是规范面,但它**从来没有被机器读过** —— 一份
//! 写着「规范性」的文法可以自相矛盾、可以引用根本不存在的非终结符,照样
//! 通过 review。R10-051 修的两处漂移就是活例子:
//!
//! - `NameItem` 在 §9.2 与附录 A 给了**两个不同定义**(§9.2 漏了
//!   `| identifier` 分支)。
//! - `ModuleDecl` 在附录 A 里定义完整,却**没有任何产生式引用它** ——
//!   悬空,等于没进文法。
//!
//! 这两条都是人眼看出来的。本文件把「人眼看」变成「CI 拦」。
//!
//! 读的是 `docs/spec/wlwl-spec-v0.11.md` 本体,不是它的副本 —— 规范
//! 只有一份,校验必须对着它做。
//!
//! **故意不做**的事:不把 EBNF 展开成真正的文法分析器。这里只抓三类漂移
//! (悬空引用 / 重复定义 / 跨章节不一致),它们是实际发生过的那些;完整的
//! 文法等价性检查是另一件工程。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// 规范文件(相对 `impl/crates/wlwl-parser`)。
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
             A grammar validator that silently skips when the file is missing is \
             exactly the failure mode R10-050 exists to remove.",
            p.display()
        )
    })
}

/// 一个 EBNF 产生式:`Name  = body .`
struct Production {
    name: String,
    body: String,
}

/// 从一段文本里抽出所有 `Name = body .` 产生式。
///
/// 判据是行尾的 ` .` —— 附录 A 与 §9.2 的产生式都遵守这个约定。`|` 换行
/// 续写的(E0112 那类)也算一条,因为续行仍以 ` .` 收尾。
fn productions_in(block: &str) -> Vec<Production> {
    let mut out = Vec::new();
    let mut cur: Option<(String, Vec<String>)> = None;

    for raw in block.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match &mut cur {
            // 续行:还没遇到结尾的 ` .`
            None => {
                if let Some((lhs, rest)) = line.split_once("=") {
                    let name = lhs.trim().to_string();
                    let rest = rest.trim().to_string();
                    if let Some(body) = rest.strip_suffix('.') {
                        out.push(Production {
                            name,
                            body: normalize(body),
                        });
                    } else {
                        cur = Some((name, vec![normalize(&rest)]));
                    }
                }
            }
            Some((name, parts)) => {
                if let Some(body) = line.strip_suffix('.') {
                    parts.push(normalize(body));
                    out.push(Production {
                        name: name.clone(),
                        body: normalize(&parts.join(" ")),
                    });
                    cur = None;
                } else {
                    parts.push(normalize(line));
                }
            }
        }
    }
    out
}

/// 归一化:压掉多余空白,让两条写法不同的同义产生式能比出「一样」。
fn normalize(s: &str) -> String {
    s.replace('\'', "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 抽出附录 A 的文法块(含 `Program = ...` 赋值的那一个)。
fn appendix_a_block(spec: &str) -> String {
    for block in fenced_blocks(spec) {
        if block.lines().any(|l| l.trim_start().starts_with("Program")) && block.contains('=') {
            return block;
        }
    }
    panic!("appendix A's grammar block (the one defining `Program`) was not found")
}

/// 抽出 §9.2 那个小块(定义 `Import` / `name_array` / `NameItem`)。
fn section_9_2_block(spec: &str) -> Option<String> {
    fenced_blocks(spec)
        .into_iter()
        .find(|b| b.contains("Import") && b.contains("NameItem"))
}

/// 所有围栏代码块(``` ... ```),内容去掉了围栏行。
fn fenced_blocks(spec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<Vec<&str>> = None;
    for line in spec.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            match cur.take() {
                None => cur = Some(Vec::new()),
                Some(b) => out.push(b.join("\n")),
            }
        } else if let Some(b) = cur.as_mut() {
            b.push(line);
        }
    }
    out
}

/// 词法终结符 —— 附录 A 里**不带引号**写的那批。
///
/// 规范对终结符用了两套写法:关键字写成 `"LET"`,而 §1.4–1.5 的词法记号
/// (`identifier` / `int_lit` / …)直接裸写。裸写的那批如果不登记,悬空检查
/// 会把 `LetExpr -> identifier` 报成「引用了未定义的非终结符」——
/// 满屏假阳性,这条检查就废了。
///
/// 列表必须与规范 §1.4 的记号表一致;新增词法记号时要同步加进来。
const LEXICAL_TERMINALS: &[&str] = &[
    "identifier",
    "int_lit",
    "float_lit",
    "string_lit",
    "char_lit",
    "any_char_except_quote_backslash_newline_dollar",
    "0",
    "b",
    "f",
    "n",
    "r",
    "t",
    "u",
    "0bf",
];

/// 右部里出现的所有**非终结符**候选。
///
/// 先把 `"..."` / `'...'` 整段删掉(里面是终结符的**内容**,不是标识符),
/// 再按 token 切。顺序反了就会把 `any_char_except_quote_backslash_newline_dollar`
/// 这种字符串内容当成引用 —— 满屏都是「引用了不存在的非终结符」。
fn referenced_names(body: &str) -> BTreeSet<String> {
    let mut stripped = String::with_capacity(body.len());
    let mut quote: Option<char> = None;
    for c in body.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None => stripped.push(c),
        }
    }
    stripped
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
        // EBNF 自己的记号不是非终结符
        .filter(|s| !matches!(*s, "|" | "[" | "]" | "{" | "}" | "?" | ":" | "="))
        .map(|s| s.to_string())
        .collect()
}

/// [R10-050] 附录 A 里**没有悬空的非终结符**。
///
/// 这一条直接锁住 R10-051 的 `ModuleDecl`:修之前它定义完整却没有任何
/// 产生式引用它 —— 定义了一个东西,却没接进文法。
#[test]
fn appendix_a_has_no_dangling_nonterminals() {
    let spec = spec_text();
    let prods = productions_in(&appendix_a_block(&spec));
    assert!(
        prods.len() > 20,
        "appendix A looks empty: {} productions",
        prods.len()
    );

    let defined: BTreeSet<String> = prods.iter().map(|p| p.name.clone()).collect();

    let mut dangling: Vec<String> = Vec::new();
    for p in &prods {
        for r in referenced_names(&p.body) {
            // 终结符在规范里要么带引号(上面已剥掉),要么是 §1.4 的裸记号。
            // 剩下的标识符若都不是非终结符,那就是拼错或引用了不存在的东西。
            if !defined.contains(&r) && !LEXICAL_TERMINALS.contains(&r.as_str()) {
                dangling.push(format!("{} -> {}", p.name, r));
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "appendix A references nonterminals it never defines (they are dead weight \
         or typos — a production nothing can reach defines nothing):\n  {}",
        dangling.join("\n  ")
    );
}

/// [R10-050] 附录 A 里每个非终结符**只被定义一次**。
///
/// 一个非终结符有两个定义意味着「哪个为准」没有答案;读者与工具会各挑一个。
#[test]
fn appendix_a_defines_each_nonterminal_once() {
    let spec = spec_text();
    let prods = productions_in(&appendix_a_block(&spec));
    let mut seen: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for p in &prods {
        seen.entry(p.name.as_str())
            .or_default()
            .push(p.body.as_str());
    }
    let dupes: Vec<String> = seen
        .iter()
        .filter(|(_, bodies)| bodies.len() > 1)
        .map(|(n, bodies)| format!("{n} defined {} times: {bodies:?}", bodies.len()))
        .collect();
    assert!(
        dupes.is_empty(),
        "appendix A defines a nonterminal more than once — there is no rule for \
         which one wins:\n  {}",
        dupes.join("\n  ")
    );
}

/// [R10-050] §9.2 与附录 A 对**同一个非终结符**的说法必须一致。
///
/// 这一条锁住 R10-051 的另一半:`NameItem` 在 §9.2 漏了 `| identifier`
/// 分支,而附录 A 有。两处对同一个符号给了两个文法,读者无从判断哪个对。
///
/// 附录 A 是**规范面**,所以不一致时以它为准 —— 本测试只报差异,不替人
/// 决定该改哪一边。
#[test]
fn section_9_2_and_appendix_a_agree_on_shared_nonterminals() {
    let spec = spec_text();
    let block_9_2 = section_9_2_block(&spec)
        .expect("section 9.2's grammar block (defining NameItem) was not found");
    let block_a = appendix_a_block(&spec);

    let p_9_2: BTreeMap<String, String> = productions_in(&block_9_2)
        .into_iter()
        .map(|p| (p.name, p.body))
        .collect();
    let p_a: BTreeMap<String, String> = productions_in(&block_a)
        .into_iter()
        .map(|p| (p.name, p.body))
        .collect();

    let mut diffs: Vec<String> = Vec::new();
    for (name, body_9_2) in &p_9_2 {
        let Some(body_appendix_a) = p_a.get(name) else {
            diffs.push(format!("{name}: defined in 9.2, missing from appendix A"));
            continue;
        };
        if body_9_2 != body_appendix_a {
            diffs.push(format!(
                "{name}:\n    9.2:   {body_9_2}\n    app A: {body_appendix_a}"
            ));
        }
    }
    assert!(
        diffs.is_empty(),
        "section 9.2 and appendix A give different grammars for the same \
         nonterminal. Appendix A is the normative surface, so 9.2 is the side that \
         should move:\n  {}",
        diffs.join("\n  ")
    );
}

/// [R10-050] 附录 A 的文法必须覆盖全部 14 个关键字入口。
///
/// 这是防「文法块被整段删掉 / 改写」的最后一道:上面的检查都建立在「能
/// 解析出产生式」之上,如果有人把块换成散文,产生式数会掉到 0,前几条会红。
/// 这一条正面点名关键入口。
#[test]
fn appendix_a_covers_the_keyword_entry_points() {
    let spec = spec_text();
    let prods = productions_in(&appendix_a_block(&spec));
    let names: BTreeSet<String> = prods.into_iter().map(|p| p.name).collect();
    for required in [
        "Program",
        "Block",
        "ExprStmt",
        "Expression",
        "ModuleDecl",
        "Import",
        "Export",
        "SealedDecl",
        "NameItem",
        "LetExpr",
        "FunExpr",
        "IfExpr",
        "WhileExpr",
        "ForExpr",
        "MatchExpr",
        "ReturnExpr",
        "BreakExpr",
        "ContinueExpr",
    ] {
        assert!(
            names.contains(required),
            "appendix A no longer defines `{required}`"
        );
    }
}

/// [R10-050] `ModuleDecl` 真的接进了 `Program`(R10-051 的回归网)。
///
/// 「定义了」不等于「可达」。这一条专门验可达性:从 `Program` 出发做一次
/// 引用闭包,`ModuleDecl` 必须在闭包里。
#[test]
fn module_decl_is_reachable_from_program() {
    let spec = spec_text();
    let prods = productions_in(&appendix_a_block(&spec));
    let map: BTreeMap<String, BTreeSet<String>> = prods
        .iter()
        .map(|p| (p.name.clone(), referenced_names(&p.body)))
        .collect();

    // 从 Program 做引用闭包。
    let mut reach: BTreeSet<String> = BTreeSet::new();
    let mut stack = vec!["Program".to_string()];
    while let Some(n) = stack.pop() {
        if !reach.insert(n.clone()) {
            continue;
        }
        if let Some(kids) = map.get(&n) {
            for k in kids {
                if map.contains_key(k) && !reach.contains(k) {
                    stack.push(k.clone());
                }
            }
        }
    }

    assert!(
        reach.contains("ModuleDecl"),
        "ModuleDecl is defined but unreachable from Program — the grammar does not \
         actually contain it. Reachable so far: {reach:?}"
    );
    assert!(
        reach.contains("Import") && reach.contains("Export") && reach.contains("SealedDecl"),
        "the three declaration forms must all be reachable, got {reach:?}"
    );
}
