//! [v0.10.1 / R10-064] spec 附录 G ↔ `BUILTIN_REGISTRY` 的**签名锁**。
//!
//! 背景:`docs/appendix_G.md` 由 [`wlwl_eval::registry::generate_appendix_g_md`]
//! 从 `BUILTIN_REGISTRY` 生成,生成器与注册表之间**有**锁
//! (`b11_generated_md_matches_registry`)。但 spec 自己的附录 G 是**手维护**
//! 的第三份副本,而那个锁只查**条目数**(`b11_registry_count_matches_spec_table`),
//! 不查**签名**。
//!
//! 结果是签名能悄悄分叉:实测 110 条里有 **37 条** spec 与注册表写法不同。
//!
//! ## 方向:注册表也曾是漂移的一方
//!
//! 「三份副本、两份锁」这个结构容易让人默认 spec 是漂移的那一方。**实测不是。**
//! 逐条对着 `wlwl-eval` 的 dispatch 实现核实之后,`BUILTIN_REGISTRY` 在 **12 处**
//! 说的与实现相反:`SUB` 第三参是 `len` 不是 `end`、`INT` / `FLOAT` 返回裸值、
//! `INDEX_SET` 返回容器不是 `NULL`、`WHILE` 恒返回 `NULL`、`AT` 强制 3 参、
//! `IMPORT` 只接 2 参、`THIS` 必须 `THIS()`、`==` / `!=` 各掉一个字符、
//! `MODULE_REF` 返回 `DICT`,以及 `ARRAY(items...)` / `DICT(pairs...)` 这两个
//! **根本不存在**的带参形式。
//!
//! 所以本文件锁的是**一致性**,判据是「两边必须逐条相同」,而不是「谁对谁错」。
//! 修的方向由实现决定,不由文件位置决定。
//!
//! ## 解析器必须认 `\|` 转义
//!
//! 表格切分若不认反斜杠转义,`||` 这一行会被切成三段,于是 spec 侧与注册表侧
//! **同样被切坏、比出来「相同」** —— 这正是最初那份 37 条清单**漏掉 `||`** 的原因。
//! `cells()` 与 `normalize()` 都按转义处理,否则这条锁在最容易漂的一行上恰好放行。

use std::collections::BTreeMap;

use wlwl_eval::registry::BUILTIN_REGISTRY;

fn spec_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("standard")
        .join("wlwl-spec-v0.11.md")
}

/// 切一行表格,尊重 `"..."` / `'...'` 里的竖线**和** `\|` 转义竖线。
fn cells(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                cur.push(c);
            }
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            // 转义竖线:吃掉反斜杠与竖线两个字符,当成普通内容
            None if c == '\\' && it.peek() == Some(&'|') => {
                it.next();
                cur.push('|');
            }
            None if c == '|' => {
                out.push(cur.trim().to_string());
                cur = String::new();
            }
            None => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// 归一化一个单元格,只剥掉 **markdown 脚手架**,不碰签名内容本身。
///
/// 剥掉的:反引号、`\|` 转义竖线还原成 `|`、首尾与重复空白。
/// **不**剥的:参数名、参数个数、`?` / `...` 标记、返回类型、错误码、
/// 括号内的注解 —— 这些正是要锁的东西。
fn normalize(s: &str) -> String {
    let joined: Vec<String> = s
        .split_whitespace()
        .map(|w| w.replace('`', "").replace("\\|", "|"))
        .collect();
    joined.join(" ")
}

fn read_spec_rows() -> (BTreeMap<String, String>, BTreeMap<String, ()>) {
    let p = spec_path();
    // 读不到 spec 时 panic 而不是 skip —— 一个「文件不在就悄悄通过」的锁
    // 正是本文件要消灭的那种失败模式。
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "cannot read the spec at {}: {e}\n\
             A sync test that silently skips when the file is missing is exactly \
             the failure mode this test exists to remove.",
            p.display()
        )
    });
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("## ") && l.contains("附录 G"))
        .unwrap_or_else(|| panic!("appendix G heading not found in {}", p.display()));
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| l.starts_with("## "))
        .map(|(i, _)| i)
        .unwrap_or(lines.len());

    let mut sigs = BTreeMap::new();
    let mut names = BTreeMap::new();
    for l in &lines[start..end] {
        if !l.starts_with('|') || l.contains("-----") {
            continue;
        }
        let c = cells(l);
        if c.len() < 3 || !c[1].starts_with('`') {
            continue;
        }
        let name = normalize(&c[1]);
        if name.is_empty() || name == "名称" {
            continue;
        }
        names.insert(name.clone(), ());
        sigs.insert(name, normalize(&c[2]));
    }
    (sigs, names)
}

fn registry_table() -> BTreeMap<String, String> {
    BUILTIN_REGISTRY
        .iter()
        .map(|s| (normalize(s.name), normalize(s.signature)))
        .collect()
}

/// [R10-064] spec 附录 G 与注册表的**签名**必须逐条相同。
///
/// 失败信息里逐条列出两边写法,便于判断该改哪一侧 —— 通常是**注册表**:
/// 它驱动运行期行为,但它自己也会写错(v0.10.1 在这里改了 12 处)。
#[test]
fn spec_appendix_g_signatures_match_the_registry() {
    let (spec, _) = read_spec_rows();
    let reg = registry_table();

    assert!(
        !spec.is_empty(),
        "spec 附录 G table came out empty — the parser is broken, not the spec"
    );

    let mut problems: Vec<String> = Vec::new();

    for name in reg.keys() {
        match spec.get(name) {
            None => problems.push(format!("{name}: in the registry, missing from spec 附录 G")),
            Some(s) if *s != reg[name] => problems.push(format!(
                "{name}:\n      spec     : {s}\n      registry : {}",
                reg[name]
            )),
            Some(_) => {}
        }
    }
    for name in spec.keys() {
        if !reg.contains_key(name) {
            problems.push(format!("{name}: in spec 附录 G, missing from the registry"));
        }
    }

    assert!(
        problems.is_empty(),
        "spec 附录 G and BUILTIN_REGISTRY disagree on {} entr(y/ies).\n\
         Both describe the same runtime contract, so when they differ one of them is \
         simply wrong — check it against the wlwl-eval dispatch impl and fix that side \
         (plus this file's doc comment if the registry moved):\n    {}",
        problems.len(),
        problems.join("\n    ")
    );
}

/// [R10-064] 条目数对得上,且**没有重名**。
///
/// 原来这条只断言注册表有 110 条,不检查 spec 表里到底有几条 —— 有人把 spec
/// 的一行删了也照样绿。重名同样要拦:`BTreeMap::insert` 对重名是**覆盖**,
/// 上一行会被静默吞掉,行数不变、签名却来自后一行。
#[test]
fn spec_appendix_g_has_one_row_per_registry_entry() {
    let (spec, names) = read_spec_rows();
    let reg = registry_table();

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for k in names.keys() {
        *counts.entry(k.as_str()).or_insert(0) += 1;
    }
    let dupes: Vec<&str> = counts
        .iter()
        .filter(|(_, c)| **c > 1)
        .map(|(k, _)| *k)
        .collect();
    assert!(
        dupes.is_empty(),
        "spec 附录 G has duplicate rows: {dupes:?} — the BTreeMap above silently \
         overwrote them, so a count check alone would not notice"
    );

    assert_eq!(
        spec.len(),
        reg.len(),
        "spec 附录 G has {} rows for {} registry entries",
        spec.len(),
        reg.len()
    );
}

/// [R10-064] `||` 这一行必须能被切出**恰好 7 列**。
///
/// 这一条是**解析器自身的回归锁**,存在的理由很具体:`||` 的名字与签名里各含
/// 两个竖线。不认 `\|` 转义时,`cells()` 会把这一行切成 13 段,而 `normalize()`
/// 再把碎片拼回去 —— spec 侧与注册表侧**同样被切坏**,比出来"相同",锁恰好在
/// 最容易漂的一行上放行。签名相等那条测试看不见这个洞,只有数列能看见。
#[test]
fn the_or_operator_row_parses_as_seven_columns() {
    let p = spec_path();
    let text = std::fs::read_to_string(&p).expect("spec readable");
    let line = text
        .lines()
        .find(|l| l.starts_with('|') && cells(l).len() == 9 && normalize(&cells(l)[1]) == "||")
        .unwrap_or_else(|| {
            panic!(
                "no `||` row in spec 附录 G with 9 cells (leading+trailing empty included). \
                 A row that needs \\| escaping parses to 13 cells without it."
            )
        });
    let c = cells(line);
    assert_eq!(
        normalize(&c[2]),
        "||(a, b) -> BOOLEAN (short-circuit)",
        "`||` signature cell must survive escaping intact, got {:?}",
        c[2]
    );
    assert!(
        line.contains(r"\|\|"),
        "the `||` row must escape its pipes as `\\|\\|`, otherwise every markdown \
         table parser sees {n} columns instead of 7:\n  {line}",
        n = c.len() - 2,
    );
}
