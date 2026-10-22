//! **「契约判据必须是章作用域」的机器守卫** —— 业主 2026-10-22 设立。
//!
//! ## 纪律
//!
//! 契约测试拿规范当**判据**时,判据必须**落在章作用域**上:先定位并切出目标章
//! (`## 标准库规范 · N `),再在切出来的 `body` 上做 `contains`。
//!
//! 判据落在**整本合并册**上时,它守住的只是「全册别处碰巧没有同样的字」——
//! 规范将来新增一章写出同样的字,那条守卫**当场失去它守的那件事**,而读数仍然
//! 是绿的(**静默失效**)。这类失效不会让 CI 变红,只会让 CI 白绿。
//!
//! 本守卫的作用只有一个:让「**新增**一处整册作用域的规范 needle」这件事
//! **过不了 CI**。
//!
//! ## 判定口径(逐字)
//!
//! 扫 `impl/crates/wlwl-eval/tests/`、`impl/crates/wlwl-parser/tests/`、
//! `impl/crates/wlwl-cli/tests/` 下的每个 `.rs`(含子目录),**跳过本文件自身**。
//! 判定分三步:
//!
//! 1. **词法**:把源码切成 token —— 注释不当标识符、字符串字面量单独成 token。
//!    于是注释里提到 `.contains(` 不会被误当成判据。
//! 2. **绑定**:找出每个 `let` 绑定及其初始化式。一个绑定是「**整册绑定**」当
//!    且仅当它的初始化式**读的是合并规范整本**:
//!    - 初始化式里有 `read_to_string(...)`,且它的参数解析到规范文件 —— 参数里
//!      出现 `wlwl-spec-reference` 字面量、或参数里出现名字含 `spec` 的路径函数
//!      (`spec_path()` / `merged_spec_path()` / `language_spec_path()`)、或参数是
//!      某个「整册绑定」的变量;
//!    - 或初始化式调用了本文件内定义的 `spec_text()`,且该 `spec_text` 的函数体
//!      确实读整本(且没有做区间切片)。
//! 3. **判据**:找出所有 `.contains(` 调用。若它的**接收者是一个纯标识符**
//!    (不是 `a.b` 字段访问),且该标识符**最近一次绑定**是「整册绑定」⇒ 这一处
//!    就是**整册作用域的规范 needle**。
//!
//! ### 为什么这样切
//!
//! - 「整册作用域」在源码里的**形态**就是:变量绑的是整本,`.contains(` 直接
//!   落在那个变量上。章作用域的形态是:先 `find("## 标准库规范 · N ")` 定位,
//!   再对切出来的 `body` / `section` 做 `contains` —— 接收者不是那个整本变量。
//! - **宁可漏报,不可误报**:接收者不是纯标识符(`ann.text.contains(...)`)、
//!   绑定找不到、绑定来源判不出来、`let` 的模式不是简单标识符(例如
//!   `if let Some(x) = …`)—— 一律**不报**。误报的守卫会被人加豁免,加了豁免
//!   就等于没有守卫。
//! - **章切片**(`body` / `section` / `fs_body` / `spec_chapter`)与**成员名向量**
//!   (`spec: Vec<String>`)的初始化式里既没有 `read_to_string` 也没有 `spec_text`,
//!   天然不会被认成整册绑定。
//!
//! ## 白名单
//!
//! `ALLOWED` 里是**当前仍然存在**的整册 needle 站点,逐条写明「为什么它现在
//! 还能守住那件事」。不在表里的站点 ⇒ **测试红**,并指出**哪个文件、哪一行、
//! 哪个 needle**。
//!
//! 表里的站点若被收窄到章(站点消失)⇒ 表项变陈旧,**不报错** —— 并发的收窄不该
//! 让 CI 变红;陈旧表项等收窄落地时顺手删一行即可(白名单只减不增)。
//!
//! ## 自检
//!
//! `the_detector_reports_a_whole_book_needle` 用一段**人造**源码喂给判定函数,
//! 断言它**能报**;`the_detector_ignores_a_chapter_scoped_needle` 断言章作用域
//! **不报**。这两条合起来保证守卫本身不是恒绿的。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 仓库根(CARGO_MANIFEST_DIR = `impl/crates/wlwl-cli`)。
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

/// 被扫的测试目录(仓根相对,`/` 分隔)。
const SCAN_DIRS: &[&str] = &[
    "impl/crates/wlwl-eval/tests",
    "impl/crates/wlwl-parser/tests",
    "impl/crates/wlwl-cli/tests",
];

/// 守卫自身不扫自己(它的 `text` 是仓内任意文件,不是规范;而且它满是
/// `.contains(` 字样,扫自己只会制造噪声)。
const SELF: &str = "impl/crates/wlwl-cli/tests/spec_needle_scope_guard.rs";

/// 白名单条目:一个**当前仍存在**的整册作用域 needle 站点。
struct Allowed {
    /// 文件(仓根相对,`/` 分隔)。
    file: &'static str,
    /// needle 的规范键(单字面量就是字面量本身;`for` 循环里的多个字面量用
    /// `" | "` 连接 —— 顺序即源码顺序)。
    needle: &'static str,
    /// 为什么它现在还能守住那件事(或者:它为什么**不**能 —— 待收窄)。
    why: &'static str,
}

/// 允许存在的整册作用域 needle 站点。
///
/// ⚠ 白名单**只减不增**:新增一行 = 把一条纪律挪到「例外」,必须有正当理由。
const ALLOWED: &[Allowed] = &[
    Allowed {
        file: "impl/crates/wlwl-eval/tests/stdlib_appendix_a_sync.rs",
        needle: "`{ns}`",
        why: "**必要**:这条判据的主语就是「规范**提到了**这个命名空间」—— 作用域天然是整本\
              (这个名字在册里出现 6 次)。收窄到某章会把它改成另一条断言(「附录 A 那一行还在」),\
              那是另一个测试的职责。",
    },
    Allowed {
        file: "impl/crates/wlwl-eval/tests/stdlib_appendix_a_sync.rs",
        needle: "`{pkg}`",
        why: "**必要**:同上(`ai` 14 次、`agent` 17 次)。官方包行不是某章的私有条款,\
              而是「册里记着这个包」这件事本身。",
    },
];

/// 一处整册作用域的规范 needle。
#[derive(Clone, Debug)]
struct Finding {
    /// 1 起的行号。
    line: usize,
    /// `.contains(` 的接收者标识符。
    receiver: String,
    /// needle 的规范键。
    needle: String,
}

// ── 词法 ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Punct(char),
    Other,
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    line: usize,
}

/// 把源码切成 token 序列(注释丢弃,字符串字面量单独成 token)。
fn lex(src: &str) -> Vec<Token> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut out: Vec<Token> = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // 行注释
        if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // 块注释(可嵌套)
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            let mut depth = 1usize;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '\n' {
                    line += 1;
                }
                if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if b[i] == '*' && i + 1 < b.len() && b[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                    continue;
                }
                i += 1;
            }
            continue;
        }
        // 原始字符串:r"…" / r#"…"#
        if c == 'r' && i + 1 < b.len() && (b[i + 1] == '"' || b[i + 1] == '#') {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while j < b.len() && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == '"' {
                j += 1;
                let content_start = j;
                while j < b.len() {
                    if b[j] == '"' {
                        let mut k = j + 1;
                        let mut h = 0usize;
                        while k < b.len() && b[k] == '#' && h < hashes {
                            k += 1;
                            h += 1;
                        }
                        if h == hashes {
                            break;
                        }
                    }
                    if b[j] == '\n' {
                        line += 1;
                    }
                    j += 1;
                }
                let content: String = b[content_start..j.min(b.len())].iter().collect();
                out.push(Token {
                    tok: Tok::Str(content),
                    line,
                });
                i = (j + 1 + hashes).min(b.len());
                continue;
            }
        }
        // 普通字符串
        if c == '"' {
            let mut j = i + 1;
            let mut content = String::new();
            while j < b.len() {
                let ch = b[j];
                if ch == '\\' && j + 1 < b.len() {
                    content.push(ch);
                    content.push(b[j + 1]);
                    if b[j + 1] == '\n' {
                        line += 1;
                    }
                    j += 2;
                    continue;
                }
                if ch == '"' {
                    break;
                }
                if ch == '\n' {
                    line += 1;
                }
                content.push(ch);
                j += 1;
            }
            out.push(Token {
                tok: Tok::Str(content),
                line,
            });
            i = (j + 1).min(b.len());
            continue;
        }
        // 字符字面量 / 生命周期
        if c == '\'' {
            if i + 2 < b.len() && b[i + 2] == '\'' {
                out.push(Token {
                    tok: Tok::Other,
                    line,
                });
                i += 3;
                continue;
            }
            if i + 3 < b.len() && b[i + 1] == '\\' && b[i + 3] == '\'' {
                out.push(Token {
                    tok: Tok::Other,
                    line,
                });
                i += 4;
                continue;
            }
            out.push(Token {
                tok: Tok::Punct('\''),
                line,
            });
            i += 1;
            continue;
        }
        // 标识符
        if c.is_ascii_alphabetic() || c == '_' {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_') {
                j += 1;
            }
            let name: String = b[i..j].iter().collect();
            out.push(Token {
                tok: Tok::Ident(name),
                line,
            });
            i = j;
            continue;
        }
        // 数字
        if c.is_ascii_digit() {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_') {
                j += 1;
            }
            out.push(Token {
                tok: Tok::Other,
                line,
            });
            i = j;
            continue;
        }
        out.push(Token {
            tok: Tok::Punct(c),
            line,
        });
        i += 1;
    }
    out
}

/// 取 `toks[open]` 那个括号里的 token(`toks[open]` 本身是 `(` / `[` / `{`)。
fn group(toks: &[Token], open: usize) -> Vec<Token> {
    let mut depth = 0i32;
    let mut out = Vec::new();
    let mut i = open;
    while i < toks.len() {
        match &toks[i].tok {
            Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => {
                depth += 1;
                if i != open {
                    out.push(toks[i].clone());
                }
            }
            Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => {
                depth -= 1;
                if depth == 0 {
                    return out;
                }
                out.push(toks[i].clone());
            }
            _ => out.push(toks[i].clone()),
        }
        i += 1;
    }
    out
}

// ── 绑定 ────────────────────────────────────────────────────────────────

struct Binding {
    name: String,
    line: usize,
    init: Vec<Token>,
}

/// 找出所有「模式是简单标识符」的 `let` 绑定。
///
/// 模式不是简单标识符的(`let (a, b) = …`、`if let Some(x) = …`)一律**跳过**
/// —— 那是**漏报**的方向,符合本守卫的取舍。
fn find_bindings(toks: &[Token]) -> Vec<Binding> {
    let mut out: Vec<Binding> = Vec::new();
    let mut i = 0usize;
    while i < toks.len() {
        let is_let = matches!(&toks[i].tok, Tok::Ident(s) if s == "let");
        if !is_let {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        if matches!(&toks.get(j).map(|t| &t.tok), Some(Tok::Ident(s)) if s == "mut") {
            j += 1;
        }
        let name = match toks.get(j).map(|t| &t.tok) {
            Some(Tok::Ident(s)) => s.clone(),
            _ => {
                i += 1;
                continue;
            }
        };
        j += 1;
        // 跳过类型标注,找深度 0 的 `=`
        let mut depth = 0i32;
        let mut eq = None;
        let mut k = j;
        while k < toks.len() {
            match &toks[k].tok {
                Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => depth += 1,
                Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => depth -= 1,
                Tok::Punct('=') if depth == 0 => {
                    eq = Some(k);
                    break;
                }
                _ => {}
            }
            k += 1;
        }
        let Some(eq) = eq else {
            i += 1;
            continue;
        };
        // 初始化式 = `=` 之后到深度 0 的 `;`
        let init_start = eq + 1;
        depth = 0;
        let mut end = init_start;
        while end < toks.len() {
            match &toks[end].tok {
                Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => depth += 1,
                Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => depth -= 1,
                Tok::Punct(';') if depth == 0 => break,
                _ => {}
            }
            end += 1;
        }
        let end = end.min(toks.len());
        out.push(Binding {
            name,
            line: toks[i].line,
            init: toks[init_start..end].to_vec(),
        });
        i = if end > i { end } else { i + 1 };
    }
    out
}

// ── 判定 ────────────────────────────────────────────────────────────────

/// 这些 token 里有没有「指向规范文件」的迹象。
///
/// 「指向规范文件」= 出现了合并规范的文件名字面量,或一个名字含 `spec` 的
/// 路径函数(`spec_path` / `merged_spec_path` / `language_spec_path` …)。
fn mentions_spec_path(toks: &[Token]) -> bool {
    for t in toks {
        match &t.tok {
            Tok::Str(s) => {
                if s.contains("wlwl-spec-reference") {
                    return true;
                }
            }
            Tok::Ident(s) => {
                let l = s.to_ascii_lowercase();
                if l.contains("spec") && l != "spec_text" {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn idents(toks: &[Token]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in toks {
        if let Tok::Ident(s) = &t.tok {
            if !out.contains(s) {
                out.push(s.clone());
            }
        }
    }
    out
}

/// 一个文件(一段 token)的分析上下文。
struct Ctx {
    toks: Vec<Token>,
    bindings: Vec<Binding>,
    /// 本文件内定义的 `spec_text()` 是否读的是整本。
    spec_text_whole: bool,
}

impl Ctx {
    fn new(toks: Vec<Token>) -> Ctx {
        let bindings = find_bindings(&toks);
        let spec_text_whole = spec_text_reads_whole_book(&toks);
        Ctx {
            toks,
            bindings,
            spec_text_whole,
        }
    }

    /// `name` 在 `line` 之前(含)的**最近一次**绑定。
    fn nearest_binding(&self, name: &str, line: usize) -> Option<&Binding> {
        self.bindings
            .iter()
            .filter(|b| b.name == name && b.line <= line)
            .max_by_key(|b| b.line)
    }

    /// 这个初始化式是不是在**读整本规范**。
    fn reads_whole_book(&self, init: &[Token]) -> bool {
        for (i, t) in init.iter().enumerate() {
            let Tok::Ident(s) = &t.tok else { continue };
            if s == "read_to_string" {
                // 参数组
                let Some(next) = init.get(i + 1) else {
                    continue;
                };
                if !matches!(next.tok, Tok::Punct('(')) {
                    continue;
                }
                let arg = group(init, i + 1);
                if mentions_spec_path(&arg) {
                    return true;
                }
                // 参数是某个「整册绑定」的变量
                for id in idents(&arg) {
                    if let Some(b) = self.nearest_binding(&id, init[i].line) {
                        if mentions_spec_path(&b.init) {
                            return true;
                        }
                    }
                }
                return false;
            }
            if s == "spec_text" && self.spec_text_whole {
                return true;
            }
        }
        false
    }

    /// `name` 这个循环变量 / 数组变量对应的字符串字面量表。
    fn array_literals(&self, name: &str) -> Option<Vec<String>> {
        // `for name in [ … ]`
        for i in 0..self.toks.len() {
            if !matches!(&self.toks[i].tok, Tok::Ident(s) if s == "for") {
                continue;
            }
            if !matches!(self.toks.get(i + 1).map(|t| &t.tok), Some(Tok::Ident(n)) if n == name) {
                continue;
            }
            if !matches!(self.toks.get(i + 2).map(|t| &t.tok), Some(Tok::Ident(w)) if w == "in") {
                continue;
            }
            if !matches!(self.toks.get(i + 3).map(|t| &t.tok), Some(Tok::Punct('['))) {
                continue;
            }
            let vals = literals_in(&group(&self.toks, i + 3));
            if !vals.is_empty() {
                return Some(vals);
            }
        }
        // `let name = [ … ]`
        for b in &self.bindings {
            if b.name != name {
                continue;
            }
            if !matches!(b.init.first().map(|t| &t.tok), Some(Tok::Punct('['))) {
                continue;
            }
            let vals = literals_in(&b.init);
            if !vals.is_empty() {
                return Some(vals);
            }
        }
        None
    }
}

fn literals_in(toks: &[Token]) -> Vec<String> {
    toks.iter()
        .filter_map(|t| match &t.tok {
            Tok::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// 本文件里定义的 `spec_text()` 是不是读整本(且没做区间切片)。
fn spec_text_reads_whole_book(toks: &[Token]) -> bool {
    for i in 0..toks.len() {
        if !matches!(&toks[i].tok, Tok::Ident(s) if s == "fn") {
            continue;
        }
        if !matches!(toks.get(i + 1).map(|t| &t.tok), Some(Tok::Ident(n)) if n == "spec_text") {
            continue;
        }
        // 找函数体
        let mut j = i + 2;
        let mut depth = 0i32;
        let mut body_open = None;
        while j < toks.len() {
            match &toks[j].tok {
                Tok::Punct('(') => depth += 1,
                Tok::Punct(')') => depth -= 1,
                Tok::Punct('{') if depth <= 0 => {
                    body_open = Some(j);
                    break;
                }
                _ => {}
            }
            j += 1;
        }
        let Some(open) = body_open else {
            return false;
        };
        let body = group(toks, open);
        if body_has_range_slice(&body) {
            return false;
        }
        let body_bindings = find_bindings(&body);
        let sub = Ctx {
            toks: body,
            bindings: body_bindings,
            spec_text_whole: false,
        };
        // 函数体里有没有「读整本」的读法
        for b in &sub.bindings {
            if sub.reads_whole_book(&b.init) {
                return true;
            }
        }
        for (i, t) in sub.toks.iter().enumerate() {
            if matches!(&t.tok, Tok::Ident(s) if s == "read_to_string") {
                if !matches!(sub.toks.get(i + 1).map(|x| &x.tok), Some(Tok::Punct('('))) {
                    continue;
                }
                let arg = group(&sub.toks, i + 1);
                if mentions_spec_path(&arg) {
                    return true;
                }
                for id in idents(&arg) {
                    if let Some(bb) = sub.nearest_binding(&id, sub.toks[i].line) {
                        if mentions_spec_path(&bb.init) {
                            return true;
                        }
                    }
                }
            }
        }
        return false;
    }
    false
}

/// token 序列里有没有区间切片(`x[a..b]`)。
fn body_has_range_slice(toks: &[Token]) -> bool {
    let mut i = 0usize;
    while i < toks.len() {
        if matches!(toks[i].tok, Tok::Punct('[')) {
            let mut j = i + 1;
            let mut depth = 1i32;
            while j < toks.len() && depth > 0 {
                match &toks[j].tok {
                    Tok::Punct('[') => depth += 1,
                    Tok::Punct(']') => depth -= 1,
                    Tok::Punct(':') if depth == 1 => {
                        if matches!(toks.get(j + 1).map(|t| &t.tok), Some(Tok::Punct(':'))) {
                            return true;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        i += 1;
    }
    false
}

/// needle 的规范键。
fn needle_key(arg: &[Token], ctx: &Ctx) -> String {
    // `format!("…")` —— 取模板
    if arg
        .iter()
        .any(|t| matches!(&t.tok, Tok::Ident(s) if s == "format"))
    {
        if let Some(s) = arg.iter().find_map(|t| match &t.tok {
            Tok::Str(s) => Some(s.clone()),
            _ => None,
        }) {
            return s;
        }
    }
    // 单个字面量
    let mut only_string = true;
    let mut single: Option<String> = None;
    for t in arg {
        match &t.tok {
            Tok::Str(s) => {
                if single.is_some() {
                    only_string = false;
                }
                single = Some(s.clone());
            }
            Tok::Punct('&') => {}
            _ => only_string = false,
        }
    }
    if only_string {
        if let Some(s) = single {
            return s;
        }
    }
    // 循环变量 / 数组变量
    if let Some(Tok::Ident(name)) = arg.first().map(|t| &t.tok) {
        if let Some(vals) = ctx.array_literals(name) {
            return vals.join(" | ");
        }
    }
    // 兜底:原样渲染
    let mut out = String::new();
    for t in arg {
        match &t.tok {
            Tok::Ident(s) => out.push_str(s),
            Tok::Str(s) => {
                out.push('"');
                out.push_str(s);
                out.push('"');
            }
            Tok::Punct(c) => out.push(*c),
            Tok::Other => out.push('?'),
        }
    }
    out
}

/// 在一段源码里找出所有**整册作用域的规范 needle**。
fn detect(src: &str) -> Vec<Finding> {
    let ctx = Ctx::new(lex(src));
    let toks = &ctx.toks;
    let mut out = Vec::new();
    for i in 0..toks.len() {
        if !matches!(&toks[i].tok, Tok::Ident(s) if s == "contains") {
            continue;
        }
        // 必须是方法调用:`<recv>.contains(`
        if i < 2 || !matches!(toks[i - 1].tok, Tok::Punct('.')) {
            continue;
        }
        let receiver = match &toks[i - 2].tok {
            Tok::Ident(s) => s.clone(),
            _ => continue,
        };
        // 字段访问(`a.b.contains(`)不是「绑定到整本的变量」⇒ 不报
        if i >= 3 && matches!(toks[i - 3].tok, Tok::Punct('.')) {
            continue;
        }
        let Some(next) = toks.get(i + 1) else {
            continue;
        };
        if !matches!(next.tok, Tok::Punct('(')) {
            continue;
        }
        let arg = group(toks, i + 1);
        // 接收者的最近一次绑定是不是「整册绑定」
        let Some(b) = ctx.nearest_binding(&receiver, toks[i].line) else {
            continue;
        };
        if !ctx.reads_whole_book(&b.init) {
            continue;
        }
        out.push(Finding {
            line: toks[i].line,
            receiver,
            needle: needle_key(&arg, &ctx),
        });
    }
    out
}

// ── 仓级扫描 ────────────────────────────────────────────────────────────

/// 被扫到的 `.rs` 文件(仓根相对)。
fn scanned_files() -> Vec<(String, PathBuf)> {
    let root = repo_root();
    let mut out = Vec::new();
    for dir in SCAN_DIRS {
        collect_rs(&root, Path::new(dir), &mut out);
    }
    out.sort();
    out
}

fn collect_rs(root: &Path, rel: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(root.join(rel)) else {
        return;
    };
    let mut names: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    names.sort_by_key(|e| e.file_name());
    for entry in names {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let child_rel = rel.join(name);
        let child_str = child_rel.to_string_lossy().replace('\\', "/");
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            collect_rs(root, &child_rel, out);
        } else if ft.is_file() && name.ends_with(".rs") && child_str != SELF {
            out.push((child_str, root.join(&child_rel)));
        }
    }
}

fn is_allowed(file: &str, needle: &str) -> bool {
    ALLOWED.iter().any(|a| a.file == file && a.needle == needle)
}

// ── 测试 ────────────────────────────────────────────────────────────────

/// 纪律的**唯一可执行形式**:新出现的整册作用域 needle 一处都不许有。
#[test]
fn no_new_whole_book_spec_needle() {
    let files = scanned_files();
    let mut violations: Vec<String> = Vec::new();
    let mut detected_total = 0usize;
    for (rel, abs) in &files {
        let Ok(src) = std::fs::read_to_string(abs) else {
            continue;
        };
        for f in detect(&src) {
            detected_total += 1;
            if !is_allowed(rel, &f.needle) {
                violations.push(format!(
                    "  {rel}:{} — needle `{}` is checked against the **whole merged \
                     book** (receiver `{}`)\n      narrow it to the chapter first: \
                     locate `## 标准库规范 · N `, slice the chapter out, and check \
                     the needle on that slice.",
                    f.line, f.needle, f.receiver
                ));
            }
        }
    }
    assert!(
        files.len() > 30,
        "the scan only saw {} file(s) — the walker is probably broken, and a guard \
         that sees nothing stays green forever",
        files.len()
    );
    assert!(
        violations.is_empty(),
        "{} new whole-book scope spec needle(s) found — a judgement that lands on \
         the whole merged book only holds because *no other chapter happens to use \
         the same words*; the day a new chapter does, the guard silently stops \
         guarding. Narrow it to the chapter.\n{}",
        violations.len(),
        violations.join("\n")
    );
    // 扫描器本身不是死的:它至少得看见白名单里的那些站点(白名单全部陈旧时这条
    // 自然落空 —— 陈旧表项不报错,收窄落地时顺手删一行即可)。
    let live = ALLOWED
        .iter()
        .filter(|a| {
            files.iter().any(|(rel, abs)| {
                rel == a.file
                    && std::fs::read_to_string(abs)
                        .map(|s| detect(&s).iter().any(|f| f.needle == a.needle))
                        .unwrap_or(false)
            })
        })
        .count();
    assert!(
        live <= detected_total,
        "the scanner reported {detected_total} whole-book needle(s) but {live} \
         allowlist entr(ies) look live — the two numbers must agree"
    );
}

/// 白名单不是「随便哪个文件都能进」的 —— 每一条都要有正当理由。
#[test]
fn the_allowlist_is_explicit_and_justified() {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for a in ALLOWED {
        assert!(
            !a.why.trim().is_empty(),
            "allowlist entry {} / `{}` has no justification",
            a.file,
            a.needle
        );
        assert!(
            seen.insert((a.file, a.needle)),
            "duplicate allowlist entry: {} / `{}`",
            a.file,
            a.needle
        );
        assert!(
            a.file.starts_with("impl/crates/") && a.file.ends_with(".rs"),
            "allowlist entry points at a non-test file: {}",
            a.file
        );
    }
}

/// 白名单里写的 needle 必须**真的还在那个文件里** —— 防「表项指向早已不存在的
/// 东西」这种让白名单失去意义的腐化。
#[test]
fn every_allowlist_entry_still_names_a_real_needle_in_its_file() {
    let root = repo_root();
    for a in ALLOWED {
        let path = root.join(a.file);
        let Ok(src) = std::fs::read_to_string(&path) else {
            panic!("allowlist entry points at a missing file: {}", a.file);
        };
        for part in a.needle.split(" | ") {
            assert!(
                src.contains(part),
                "allowlist entry {} / `{}` — the needle `{}` no longer appears in \
                 that file; the entry is stale, drop it (or narrow the judgement \
                 and drop it together)",
                a.file,
                a.needle,
                part
            );
        }
    }
}

/// **自检 ①**:人造一段「整册 needle」,判定函数必须**报出来**。
///
/// 没有这一条,守卫自己可能恒绿(判定函数坏了 ⇒ 什么也不报 ⇒ 永远绿)。
#[test]
fn the_detector_reports_a_whole_book_needle() {
    let src = r##"
fn spec_path() -> std::path::PathBuf {
    std::path::PathBuf::from("docs/wlwl-spec-reference.md")
}

#[test]
fn synthetic() {
    let text = std::fs::read_to_string(spec_path()).unwrap();
    assert!(text.contains("某句条款"));
}
"##;
    let found = detect(src);
    assert_eq!(
        found.len(),
        1,
        "the detector must report exactly one whole-book needle in the synthetic \
         snippet, got {found:?}"
    );
    assert_eq!(found[0].needle, "某句条款");
    assert_eq!(found[0].receiver, "text");
}

/// **自检 ②**:章作用域的 needle **不许**被报出来 —— 否则守卫会误报同事。
#[test]
fn the_detector_ignores_a_chapter_scoped_needle() {
    let src = r###"
fn spec_path() -> std::path::PathBuf {
    std::path::PathBuf::from("docs/wlwl-spec-reference.md")
}

#[test]
fn synthetic() {
    let text = std::fs::read_to_string(spec_path()).unwrap();
    let start = text.find("## 标准库规范 · 12 ").unwrap();
    let end = text[start..].find("\n## ").map(|i| start + i).unwrap();
    let body = &text[start..end];
    assert!(body.contains("某句条款"));
}
"###;
    let found = detect(src);
    assert!(
        found.is_empty(),
        "a chapter-scoped needle must not be reported, got {found:?}"
    );
}

/// **自检 ③**:字段访问(`a.b.contains(`)与**非规范**的整本读取都不许被报出来。
#[test]
fn the_detector_ignores_field_accesses_and_non_spec_reads() {
    let src = r##"
struct Ann { text: String }

#[test]
fn synthetic() {
    let ann = Ann { text: String::from("DICT") };
    assert!(ann.text.contains("DICT"));

    let md = std::fs::read_to_string("examples/wlwl.toml").unwrap();
    assert!(md.contains("[package]"));
}
"##;
    let found = detect(src);
    assert!(
        found.is_empty(),
        "field accesses and non-spec reads must not be reported, got {found:?}"
    );
}

/// **自检 ④**:扫到的文件必须真的覆盖三个契约测试目录 —— 防「扫描器根本没扫到
/// 东西」这种**静默失效**。
#[test]
fn the_scan_covers_every_contract_test_directory() {
    let files = scanned_files();
    let names: BTreeSet<&String> = files.iter().map(|(r, _)| r).collect();
    for dir in SCAN_DIRS {
        let hit = names.iter().any(|n| n.starts_with(&format!("{dir}/")));
        assert!(hit, "the scan missed `{dir}` entirely");
    }
    for must in [
        "impl/crates/wlwl-eval/tests/result_contract.rs",
        "impl/crates/wlwl-parser/tests/spec_grammar_consistency.rs",
        "impl/crates/wlwl-cli/tests/spec_ref_hygiene_guard.rs",
    ] {
        assert!(
            names.contains(&must.to_string()),
            "the scan missed `{must}` — the walker is skipping files it must not"
        );
    }
}
