//! `wlwl lsp` —— 语言服务器薄壳(v0.10 Step 10 · 计划书 §5.3 P1-3)。
//!
//! # 为什么是「薄壳」
//!
//! 不引 LSP 框架(不碰 `Cargo.toml` / `Cargo.lock`,不拉新依赖),直接把
//! 已有能力包一层 LSP 线协议:[`wlwl_parser`] 出 AST 与语法诊断,
//! [`crate::collect_static_diagnostics`] 出静态契约诊断(与 `wlwl check`
//! **同一份口径**),[`wlwl_eval`] 的注册表出补全与签名。
//!
//! 能力只做计划书 §5.3 点名的三项(diagnostics / definition / hover),
//! 外加「补全由注册表驱动」那半句 —— 决策点 D-6 的 A 档。B 档(rename /
//! format)与 C 档(不做 lsp)都还开着,没被本文件关掉。
//!
//! # 线协议
//!
//! stdio + `Content-Length` 分帧(LSP base protocol)。收发都是
//! `Content-Length: N\r\n\r\n<JSON>`。
//!
//! # 位置约定(薄壳的诚实边界)
//!
//! - `initialize` 里声明 `"positionEncoding": "utf-8"`,并按 **1 起算**的列
//!   与 WLWL 自己的 span 对齐(诊断的行/列直接透传,编辑器能对上)。
//! - **定义跳转**定位到「节点 span 内的第一个同名 token」:AST 的 span 覆盖
//!   整个节点(`LET(x, …)` 而不是光标下的 `x`),而薄壳不建完整的
//!   offset 映射表。这对顶层 `LET` / `FUN` / 形参是准的;对嵌套表达式里
//!   反复出现的同名标识符可能落在第一个 —— 宁可粗一点,不要一套半吊子的
//!   token 表。

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};
use wlwl_ast::{Expr, Pattern, Span};
use wlwl_error::{Location, Severity, WlwlDiagnostic};

/// 一个打开中的文档:文本 + 解析结果 + 根作用域绑定类型。
///
/// **不缓存诊断** —— 诊断是在同步那一刻算出来、直接装进
/// `publishDiagnostics` 帧的;存一份在文档里只会诱使后来者去读一个可能
/// 已经过期的副本(要重查,重算就是了)。
struct Doc {
    text: String,
    /// 解析成功时的 AST;语法错时为 `None`(错已进诊断)
    ast: Option<Expr>,
    /// 根作用域绑定类型(Step 6 的 `DeclaredBinding`,供 hover 用)
    declared: Vec<(String, String)>,
}

/// LSP 服务器状态。
pub struct Server {
    docs: HashMap<String, Doc>,
    /// 根作用域符号表:`名字 -> (行, 列)`。定位用,不含类型。
    symbols: HashMap<String, Vec<(u32, u32)>>,
    shutdown_requested: bool,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    pub fn new() -> Server {
        Server {
            docs: HashMap::new(),
            symbols: HashMap::new(),
            shutdown_requested: false,
        }
    }

    /// stdio 主循环。`initialize` 之后的 `exit` 结束。
    pub fn run() -> ExitCode {
        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        let stdout = std::io::stdout();
        let mut writer = stdout.lock();
        let mut server = Server::new();
        while let Some(payload) = read_frame(&mut reader) {
            for out in server.handle(&payload) {
                if write_frame(&mut writer, &out).is_err() {
                    // 管道断了(客户端退出)—— 不是错误,静默收工。
                    return ExitCode::SUCCESS;
                }
            }
            if server.shutdown_requested {
                return ExitCode::SUCCESS;
            }
        }
        ExitCode::SUCCESS
    }

    /// 处理一条 JSON-RPC 消息,返回**要发出的**帧(响应 + 通知)。
    ///
    /// 纯函数式地对着 `payload` 工作、不碰 stdio,所以整条协议可以
    /// 在单测里用脚本驱动,不用起进程。
    pub fn handle(&mut self, payload: &str) -> Vec<String> {
        let Ok(msg) = serde_json::from_str::<Value>(payload) else {
            // 解析不出来的消息按 LSP 规范忽略(服务端不该被一条坏消息打死)。
            return Vec::new();
        };
        let method = msg.get("method").and_then(|m| m.as_str());
        let Some(method) = method else {
            // 客户端发来的响应(本薄壳不主动发请求),忽略。
            return Vec::new();
        };
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => response(
                id,
                json!({
                    "capabilities": {
                        // 1 = Full(整篇重同步;薄壳不做增量)。
                        "textDocumentSync": 1,
                        "hoverProvider": true,
                        "definitionProvider": true,
                        "completionProvider": { "resolveProvider": false },
                        "positionEncoding": "utf-8"
                    },
                    "serverInfo": { "name": "wlwl-lsp", "version": env!("CARGO_PKG_VERSION") }
                }),
            ),
            "shutdown" => {
                self.shutdown_requested = true;
                response(id, Value::Null)
            }
            "exit" => {
                self.shutdown_requested = true;
                Vec::new()
            }
            "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didSave" => {
                let (uri, text) = doc_text(&params);
                self.open_or_update(&uri, text)
            }
            "textDocument/didClose" => match doc_uri(&params) {
                Some(uri) => {
                    self.docs.remove(&uri);
                    // 关掉文件要把诊断清空,否则编辑器里会留着旧划线。
                    notification(
                        "textDocument/publishDiagnostics",
                        json!({ "uri": uri, "diagnostics": [] }),
                    )
                }
                None => Vec::new(),
            },
            "textDocument/hover" => response(id, self.hover(&params)),
            "textDocument/definition" => response(id, self.definition(&params)),
            "textDocument/completion" => response(id, self.completion(&params)),
            // 其余方法(`$/cancelRequest`、`textDocument/publishDiagnostics` 的
            // 通知方向等)一律忽略:薄壳只承诺上面那几个能力,不假装支持。
            _ => Vec::new(),
        }
    }

    /// 打开 / 同步一篇文档,回一条 `publishDiagnostics`。
    fn open_or_update(&mut self, uri: &str, text: String) -> Vec<String> {
        let path = uri_to_path(uri);
        let base_dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // 静态诊断与 `wlwl check` 走同一个函数(见模块文档)。
        let mut diags: Vec<WlwlDiagnostic> = Vec::new();
        let (ast, declared) = match wlwl_parser::parse_with_warnings(&text, &path.to_string_lossy())
        {
            Ok((ast, warns)) => {
                for w in &warns {
                    diags.push(warning_to_diagnostic(w, &path.to_string_lossy()));
                }
                // **lint 这一路也要走**:`run_file` 报的 `W0010` / `W0011` /
                // `W0012` / `W0001` 来自 `wlwl_parser::lint`,不是
                // `parse_with_warnings`。漏掉它,LSP 的划线就会比
                // `wlwl check` 少 —— 那正是「同一份口径」这句话最不能
                // 破的地方。
                for w in wlwl_parser::lint(&ast) {
                    diags.push(warning_to_diagnostic(&w, &path.to_string_lossy()));
                }
                diags.extend(crate::collect_static_diagnostics(&ast, &path, &base_dir));
                let declared = wlwl_types::check_program_detailed(&ast, &HashMap::new())
                    .declared
                    .into_iter()
                    .map(|b| (b.name, b.ty.to_string()))
                    .collect();
                (Some(ast), declared)
            }
            Err(e) => {
                diags.push(e.diagnostic().clone());
                (None, Vec::new())
            }
        };

        self.symbols = ast.as_ref().map(index_symbols).unwrap_or_default();
        self.docs.insert(
            uri.to_string(),
            Doc {
                text,
                ast,
                declared,
            },
        );
        notification(
            "textDocument/publishDiagnostics",
            json!({
                "uri": uri,
                "diagnostics": diags.iter().map(to_lsp_diagnostic).collect::<Vec<_>>()
            }),
        )
    }

    /// 悬停:光标下的标识符 → 静态类型 / 注册表签名 / 声明类型。
    fn hover(&self, params: &Value) -> Value {
        let (Some((uri, line, col)), Some(doc)) = (position_of(params), self.current_doc(params))
        else {
            return Value::Null;
        };
        let _ = (uri, line);
        let Some(word) = word_at(&doc.text, line, col) else {
            return Value::Null;
        };
        let Some(body) = self.describe(doc, &word) else {
            return Value::Null;
        };
        json!({
            "contents": { "kind": "markdown", "value": body }
        })
    }

    /// 一个名字能给出的所有信息,按「有才说」的顺序拼。
    fn describe(&self, doc: &Doc, word: &str) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        // 1. 本文件的静态类型(Step 6 的绑定表)。
        if let Some((_, ty)) = doc.declared.iter().find(|(n, _)| n == word) {
            parts.push(format!("```wlwl\n{word}: {ty}\n```"));
        }
        // 2. 内建注册表签名(110 条,含文档串)。补全与 hover 的公共来源。
        if let Some(spec) = wlwl_eval::registry::BUILTIN_REGISTRY
            .iter()
            .find(|s| s.name == word)
        {
            parts.push(format!("```wlwl\n{}\n```", spec.signature));
            if let Some(sig) = &spec.sig {
                parts.push(format!("_builtin · static signature: {sig:?}_"));
            }
        }
        // 3. 从模块签名(Step 6/7)里查:导入名在别的模块里是什么类型。
        if parts.is_empty() {
            if let Some(ast) = &doc.ast {
                if let Some(decl) = declared_type_in_imports(ast, word) {
                    parts.push(format!("```wlwl\n{word}: {decl}\n```"));
                }
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }

    /// 定义跳转:同文档的绑定,或导入名 → 那个模块文件里的顶层绑定。
    fn definition(&self, params: &Value) -> Value {
        let (Some((uri, line, col)), Some(doc)) = (position_of(params), self.current_doc(params))
        else {
            return Value::Null;
        };
        let Some(word) = word_at(&doc.text, line, col) else {
            return Value::Null;
        };
        // 同文档优先:顶层 `LET` / `FUN` / 形参都在符号表里。
        if let Some(positions) = self.symbols.get(&word) {
            let (l, c) = positions[0];
            return json!({ "uri": uri, "range": range_of(l, c) });
        }
        // 跨文件:导入名 → 解析到模块文件,找它导出面里的顶层绑定。
        let Some(ast) = &doc.ast else {
            return Value::Null;
        };
        let path = uri_to_path(&uri);
        let base_dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        for (spec_path, names) in import_sites(ast) {
            if !names.iter().any(|n| n == &word) {
                continue;
            }
            // std 模块没有磁盘源 → 无处可跳(签名在 hover 里给)。
            let Ok(Some(dep)) = wlwl_eval::resolve_module_file(&spec_path, &base_dir) else {
                continue;
            };
            let Ok(src) = std::fs::read_to_string(&dep) else {
                continue;
            };
            let Ok(dep_ast) = wlwl_parser::parse(&src, &dep.to_string_lossy()) else {
                continue;
            };
            if let Some((l, c)) = find_binding(&dep_ast, &src, &word) {
                return json!({
                    "uri": path_to_uri(&dep),
                    "range": range_of(l, c)
                });
            }
        }
        Value::Null
    }

    /// 补全:注册表 110 条 + 本文件顶层绑定 + 导入名。注册表那一大坨是主力
    /// ——「补全由注册表驱动」就是指它。
    fn completion(&self, params: &Value) -> Value {
        let Some(doc) = self.current_doc(params) else {
            return json!({ "isIncomplete": false, "items": [] });
        };
        let mut items: Vec<Value> = Vec::new();
        for spec in wlwl_eval::registry::BUILTIN_REGISTRY {
            items.push(json!({
                "label": spec.name,
                "kind": 3,                      // LSP CompletionItemKind::Function
                "detail": spec.signature,
                "documentation": {
                    "kind": "markdown",
                    "value": format!("*{:?}* · spec §{}", spec.group, spec.section)
                }
            }));
        }
        for (name, ty) in &doc.declared {
            items.push(json!({
                "label": name,
                "kind": 6,                      // CompletionItemKind::Variable
                "detail": ty
            }));
        }
        if let Some(ast) = &doc.ast {
            for (_, names) in import_sites(ast) {
                for n in names {
                    items.push(json!({ "label": n, "kind": 6, "detail": "imported" }));
                }
            }
        }
        json!({ "isIncomplete": false, "items": items })
    }

    fn current_doc(&self, params: &Value) -> Option<&Doc> {
        let uri = params
            .get("textDocument")
            .and_then(|t| t.get("uri"))
            .and_then(|u| u.as_str())?;
        self.docs.get(uri)
    }
}

// ── 文档 / 位置工具 ───────────────────────────────────────────

fn doc_uri(params: &Value) -> Option<String> {
    params
        .get("textDocument")
        .and_then(|t| t.get("uri"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string())
}

/// 取文档全文:三个同步方法共用一套读法。
///
/// `didChange` 走 **Full** 同步(能力里声明的就是 `textDocumentSync: 1`),
/// 所以只认 `contentChanges[last].text`;缺文本时退化成空串(不猜)。
fn doc_text(params: &Value) -> (String, String) {
    let uri = doc_uri(params).unwrap_or_default();
    let text = params
        .get("textDocument")
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .or_else(|| {
            params
                .get("contentChanges")
                .and_then(|c| c.as_array())
                .and_then(|a| a.last())
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
        })
        .unwrap_or("")
        .to_string();
    (uri, text)
}

fn position_of(params: &Value) -> Option<(String, u32, u32)> {
    let uri = doc_uri(params)?;
    let pos = params.get("position")?;
    // LSP 的行/列是 0 起算;WLWL 的 span 是 1 起算。转一次,别让编辑器
    // 整体偏一格。
    let line = pos.get("line")?.as_u64()? as u32 + 1;
    let col = pos.get("character")?.as_u64()? as u32 + 1;
    Some((uri, line, col))
}

fn range_of(line: u32, col: u32) -> Value {
    json!({
        "start": { "line": line.saturating_sub(1), "character": col.saturating_sub(1) },
        "end": { "line": line.saturating_sub(1), "character": col.saturating_sub(1) + 1 }
    })
}

/// 取光标所在的标识符(向两侧扩展到词边界)。
fn word_at(text: &str, line: u32, col: u32) -> Option<String> {
    let src = text.lines().nth(line.checked_sub(1)? as usize)?;
    let chars: Vec<char> = src.chars().collect();
    let mut i = col.saturating_sub(1) as usize;
    if i >= chars.len() {
        // 光标在行尾:向左看一眼。
        if i == 0 {
            return None;
        }
        i -= 1;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    if !is_word(chars[i]) {
        return None;
    }
    let start = {
        let mut s = i;
        while s > 0 && is_word(chars[s - 1]) {
            s -= 1;
        }
        s
    };
    let mut end = i;
    while end + 1 < chars.len() && is_word(chars[end + 1]) {
        end += 1;
    }
    Some(chars[start..=end].iter().collect())
}

/// 顶层符号表:`LET` / `FUN` 名字 / 形参 / 模式绑定。函数体的局部变量
/// **不进** —— 跟 Step 6 的绑定表同一条纪律(契约只对公开面说话),
/// 定位同理:顶层与形参才是「跳过去值得看」的地方。
fn index_symbols(root: &Expr) -> HashMap<String, Vec<(u32, u32)>> {
    let mut out: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut add = |name: &str, span: &Span| {
        out.entry(name.to_string())
            .or_default()
            .push((span.line_start, span.col_start));
    };
    fn walk(e: &Expr, add: &mut dyn FnMut(&str, &Span)) {
        match e {
            Expr::Block { exprs, .. } => {
                for x in exprs {
                    walk(x, add);
                }
            }
            Expr::Let {
                name,
                type_annotation,
                value,
                span,
                ..
            } => {
                // 有类型注解时,注解的 span 正好落在 `名: 类型` 上,更贴近名字。
                match type_annotation {
                    Some(a) => add(name, &a.span),
                    None => add(name, span),
                }
                walk(value, add);
            }
            Expr::LetPattern { value, .. } => walk(value, add),
            Expr::Fun {
                name, params, body, ..
            } => {
                if let Some(n) = name {
                    add(n, e.span());
                }
                for p in params {
                    add(&p.name, &p.span);
                }
                walk(body, add);
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk(cond, add);
                walk(then_branch, add);
                if let Some(b) = else_branch {
                    walk(b, add);
                }
            }
            Expr::While { cond, body, .. } => {
                walk(cond, add);
                walk(body, add);
            }
            Expr::For { iter, body, .. } => {
                walk(iter, add);
                walk(body, add);
            }
            Expr::Array { items, .. } => {
                for i in items {
                    walk(i, add);
                }
            }
            Expr::Dict { entries, .. } => {
                for (k, v) in entries {
                    walk(k, add);
                    walk(v, add);
                }
            }
            Expr::Match {
                value,
                clauses,
                default,
                ..
            } => {
                walk(value, add);
                for c in clauses {
                    for n in pattern_names(&c.pattern) {
                        add(&n, &pattern_span(&c.pattern));
                    }
                    walk(&c.body, add);
                }
                walk(default, add);
            }
            Expr::Ok { value, .. }
            | Expr::Err { value, .. }
            | Expr::Panic { value, .. }
            | Expr::Try { value, .. }
            | Expr::IsOk { value, .. }
            | Expr::IsErr { value, .. } => walk(value, add),
            _ => {}
        }
    }
    walk(root, &mut add);
    out
}

/// 模式节点的 span。`Pattern` 六个变体各带自己的 span,这里收一个口。
fn pattern_span(p: &Pattern) -> Span {
    match p {
        Pattern::Ident(_, s)
        | Pattern::Wildcard(s)
        | Pattern::Literal(_, s)
        | Pattern::Constructor { span: s, .. } => s.clone(),
        Pattern::Array(_, _, s) | Pattern::Dict(_, s) => s.clone(),
    }
}

/// 模式里绑定的名字。
fn pattern_names(p: &Pattern) -> Vec<String> {
    let mut out = Vec::new();
    fn go(p: &Pattern, out: &mut Vec<String>) {
        match p {
            Pattern::Ident(n, _) => out.push(n.clone()),
            Pattern::Array(items, rest, _) => {
                for i in items {
                    go(i, out);
                }
                if let Some(r) = rest {
                    go(r, out);
                }
            }
            Pattern::Dict(entries, _) => {
                for (_, s) in entries {
                    go(s, out);
                }
            }
            Pattern::Constructor { inner, .. } => go(inner, out),
            _ => {}
        }
    }
    go(p, &mut out);
    out
}

/// 导入点:`(IMPORT 的路径, 它引入的名字)`。std 模块也在列 —— 只是
/// 「跳过去」时没有文件可跳。
fn import_sites(root: &Expr) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    fn walk(e: &Expr, out: &mut Vec<(String, Vec<String>)>) {
        match e {
            Expr::Block { exprs, .. } => {
                for x in exprs {
                    walk(x, out);
                }
            }
            Expr::Import { path, names, .. } => {
                out.push((
                    path.clone(),
                    names.iter().map(|n| n.local_name().to_string()).collect(),
                ));
            }
            Expr::If {
                then_branch,
                else_branch,
                ..
            } => {
                walk(then_branch, out);
                if let Some(b) = else_branch {
                    walk(b, out);
                }
            }
            _ => {}
        }
    }
    walk(root, &mut out);
    out
}

/// 在模块 AST 里找某个导出名的**顶层绑定**位置。
///
/// 拿不到名字的精确列(AST 的 span 覆盖整个节点),所以在节点 span 内
/// 找**第一个同名 token** —— 顶层 `LET` / `FUN` 上这是准的。
fn find_binding(root: &Expr, source: &str, name: &str) -> Option<(u32, u32)> {
    fn find(root: &Expr, name: &str) -> Option<Span> {
        match root {
            Expr::Block { exprs, .. } => exprs.iter().find_map(|e| find(e, name)),
            Expr::Let { name: n, span, .. } if n == name => Some(span.clone()),
            Expr::Fun {
                name: Some(n),
                span,
                ..
            } if n == name => Some(span.clone()),
            _ => None,
        }
    }
    let span = find(root, name)?;
    let col = token_col_in_span(source, &span, name).unwrap_or(span.col_start);
    Some((span.line_start, col))
}

/// 在 span 覆盖的那一行里找 `name` 的第一个完整 token 列(1 起算)。
fn token_col_in_span(source: &str, span: &Span, name: &str) -> Option<u32> {
    let line = source
        .lines()
        .nth(span.line_start.checked_sub(1)? as usize)?;
    let chars: Vec<char> = line.chars().collect();
    let from = span.col_start.checked_sub(1)? as usize;
    let mut i = from;
    while i < chars.len() {
        if chars[i..].starts_with(&name.chars().collect::<Vec<_>>()[..]) {
            // 词边界:前后都不能还是标识符字符。
            let before_ok = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
            let after = i + name.chars().count();
            let after_ok =
                after >= chars.len() || !(chars[after].is_alphanumeric() || chars[after] == '_');
            if before_ok && after_ok {
                return Some(i as u32 + 1);
            }
        }
        i += 1;
    }
    None
}

/// 导入名在**本程序顶层**声明为什么类型。
///
/// 只做「本程序里能不能看出来」:顶层有同名绑定就用它的注解,否则回
/// `None` 让 hover 保持安静 —— 跨模块的类型归 definition 那一侧的活。
fn declared_type_in_imports(root: &Expr, word: &str) -> Option<String> {
    fn declared(root: &Expr, word: &str) -> Option<String> {
        match root {
            Expr::Block { exprs, .. } => exprs.iter().find_map(|e| declared(e, word)),
            Expr::Let {
                name: n,
                type_annotation: Some(a),
                ..
            } if n == word => Some(wlwl_types::Ty::from_type_expr(&a.expr).to_string()),
            _ => None,
        }
    }
    declared(root, word)
}

// ── LSP ↔ 内部类型 ───────────────────────────────────────────

/// parser 的 `Warning` → `WlwlDiagnostic`(两者形状不同:parser 的是
/// `(line, col, line_end, col_end)` 四元组)。
fn warning_to_diagnostic(w: &wlwl_parser::Warning, file: &str) -> WlwlDiagnostic {
    let (line, col, line_end, col_end) = w.span;
    let mut d = WlwlDiagnostic::new(
        w.code,
        w.message.clone(),
        Location::range(file.to_string(), line, col, line_end, col_end),
    );
    d.severity = Severity::Warning;
    d
}

fn to_lsp_diagnostic(d: &WlwlDiagnostic) -> Value {
    // WLWL 的 span 是 1 起算,LSP 是 0 起算。
    let severity = match d.severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        _ => 3,
    };
    // 退化保护:空 span(行 0 / 列 0)的诊断不能映射成 `-1`,编辑器会
    // 直接拒收整份 publish。
    let line = d.location.line.saturating_sub(1);
    let col = d.location.col.saturating_sub(1);
    let line_end = d.location.line_end.saturating_sub(1).max(line);
    let col_end = d.location.col_end.saturating_sub(1).max(col);
    json!({
        "range": {
            "start": { "line": line, "character": col },
            "end": { "line": line_end, "character": col_end }
        },
        "severity": severity,
        "code": d.code.as_str(),
        "source": "wlwl",
        "message": d.message
    })
}

// ── URL ↔ 路径 ────────────────────────────────────────────────

/// `file:///C:/x/y.wll` → `C:\x\y.wll`。非 `file:` 的 URI 原样当路径用
/// (Linux 上的 `file:///home/...` 正好落在这条上)。
pub fn uri_to_path(uri: &str) -> PathBuf {
    let raw = uri.strip_prefix("file://").unwrap_or(uri);
    // Windows 上把 `/C:/…` 的前导斜杠去掉;其余平台原样。
    let trimmed = if cfg!(windows) {
        raw.strip_prefix('/').unwrap_or(raw)
    } else {
        raw
    };
    PathBuf::from(trimmed)
}

pub fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

// ── 线协议收发 ────────────────────────────────────────────────

/// 读一帧:`Content-Length` 头 + 空行 + 载荷。头的大小写不敏感。
fn read_frame(reader: &mut impl BufRead) -> Option<String> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // 空行 = 头结束
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().ok();
            }
        }
    }
    let len = len?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

fn write_frame(writer: &mut impl Write, payload: &str) -> std::io::Result<()> {
    write!(
        writer,
        "Content-Length: {}\r\n\r\n{}",
        payload.len(),
        payload
    )?;
    writer.flush()
}

fn response(id: Option<Value>, result: Value) -> Vec<String> {
    let Some(id) = id else {
        // 没有 id 的请求按 LSP 规范就是通知,不必回。
        return Vec::new();
    };
    vec![json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()]
}

fn notification(method: &str, params: Value) -> Vec<String> {
    vec![json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_uri_str() -> String {
        path_to_uri(Path::new("/tmp/lsp-test/main.wll"))
    }

    fn open(server: &mut Server, text: &str) -> Value {
        let out = server.handle(
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": { "uri": doc_uri_str(), "text": text }
                }
            })
            .to_string(),
        );
        assert_eq!(out.len(), 1);
        serde_json::from_str(&out[0]).expect("frame must be JSON")
    }

    fn request(server: &mut Server, method: &str, line: u32, character: u32) -> Value {
        let out = server.handle(
            &json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": method,
                "params": {
                    "textDocument": { "uri": doc_uri_str() },
                    "position": { "line": line, "character": character }
                }
            })
            .to_string(),
        );
        assert_eq!(out.len(), 1, "{method} must answer exactly one frame");
        serde_json::from_str(&out[0]).expect("frame must be JSON")
    }

    #[test]
    fn initialize_advertises_exactly_the_three_thin_shell_capabilities() {
        let mut s = Server::new();
        let out = s.handle(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}).to_string(),
        );
        let v: Value = serde_json::from_str(&out[0]).unwrap();
        let caps = &v["result"]["capabilities"];
        assert_eq!(caps["hoverProvider"], true);
        assert_eq!(caps["definitionProvider"], true);
        assert!(caps["completionProvider"].is_object());
        // 全量同步 + 位置编码:薄壳不做增量,所以是 1(Full)。
        assert_eq!(caps["textDocumentSync"], 1);
        assert_eq!(caps["positionEncoding"], "utf-8");
        // D-6 的 B 档能力**没有**被偷偷加进来。
        assert!(caps.get("renameProvider").is_none());
        assert!(caps.get("documentFormattingProvider").is_none());
    }

    #[test]
    fn a_syntax_error_is_published_as_a_diagnostic() {
        let mut s = Server::new();
        // 缺分号 → E0013(与 `wlwl check` 报的是同一个码)。
        let frame = open(&mut s, "LET(x, 1) LET(y, 2);");
        let diags = frame["params"]["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0]["code"], "E0013");
        assert_eq!(diags[0]["severity"], 1);
        assert_eq!(diags[0]["source"], "wlwl");
    }

    /// 静态诊断口径与 `wlwl check` 一致:同一份源码,门禁开着就报。
    #[test]
    fn static_diagnostics_follow_the_manifest_switches() {
        // 逐个把 base_dir 换成一个自带清单的临时工程。
        let dir = std::env::temp_dir().join("wlwl-lsp-static");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("main.wll");
        let src = "LET(f, FUN((a: INTEGER) : INTEGER, \"wrong\"));\n";
        std::fs::write(&path, src).unwrap();

        let ast = wlwl_parser::parse(src, "main.wll").unwrap();
        for (gradual, expect_empty) in [("\"off\"", true), ("\"error\"", false)] {
            std::fs::write(
                dir.join("wlwl.toml"),
                format!("[package]\nname = \"p\"\nversion = \"0.0.1\"\nentry = \"main.wll\"\n\n[features]\ngradual_typing = {gradual}\n"),
            )
            .unwrap();
            let diags = crate::collect_static_diagnostics(&ast, &path, &dir);
            assert_eq!(
                diags.is_empty(),
                expect_empty,
                "gradual_typing = {gradual} should {}diagnostics",
                if expect_empty {
                    "produce no "
                } else {
                    "produce "
                }
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn definition_jumps_to_a_top_level_binding() {
        let mut s = Server::new();
        open(
            &mut s,
            "LET(total, 1);\nLET(other, FUN((x: INTEGER), x));\nPRINT(total);\n",
        );
        // `total` 在第 3 行的使用处 → 跳到第 1 行的定义。
        let r = request(&mut s, "textDocument/definition", 2, 6);
        assert_eq!(r["result"]["range"]["start"]["line"], 0);
        // 形参也能跳(`seed` 的**使用**处在第 38 列)。
        open(&mut s, "LET(f, FUN((seed: INTEGER) : INTEGER, seed));\n");
        let r = request(&mut s, "textDocument/definition", 0, 38);
        assert_eq!(r["result"]["range"]["start"]["line"], 0);
        // 不认识的名字 → null(不是空数组,也不是猜一个)。
        open(&mut s, "PRINT(nope);\n");
        let r = request(&mut s, "textDocument/definition", 0, 7);
        assert!(r["result"].is_null());
    }

    #[test]
    fn hover_shows_the_static_type_and_the_registry_signature() {
        let mut s = Server::new();
        // 局部绑定的静态类型(Step 6 的绑定表)。
        open(&mut s, "LET(total: INTEGER, 1);\nPRINT(total);\n");
        let h = request(&mut s, "textDocument/hover", 1, 6);
        let text = h["result"]["contents"]["value"].as_str().unwrap();
        assert!(text.contains("total: INTEGER"), "{text}");
        // 内建名 → 注册表签名。
        open(&mut s, "PRINT(\"hi\");\n");
        let h = request(&mut s, "textDocument/hover", 0, 2);
        let text = h["result"]["contents"]["value"].as_str().unwrap();
        assert!(text.contains("PRINT"), "{text}");
    }

    #[test]
    fn completion_is_driven_by_the_registry_plus_local_names() {
        let mut s = Server::new();
        open(
            &mut s,
            "LET(local_thing, 1);\nIMPORT(\"wlwl:std.io\", [\"PRINT\"]);\n",
        );
        let r = request(&mut s, "textDocument/completion", 0, 0);
        let items = r["result"]["items"].as_array().unwrap();
        // 注册表全量进补全(110 条)。
        let registry_len = wlwl_eval::registry::BUILTIN_REGISTRY.len();
        assert!(items.len() >= registry_len);
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"PRINT"));
        assert!(labels.contains(&"local_thing"));
    }

    #[test]
    fn closing_a_document_clears_its_diagnostics() {
        let mut s = Server::new();
        open(&mut s, "LET(x, 1) LET(y, 2);");
        let out = s.handle(
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didClose",
                "params": { "textDocument": { "uri": doc_uri_str() } }
            })
            .to_string(),
        );
        let v: Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(v["params"]["diagnostics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn a_malformed_message_does_not_kill_the_server() {
        let mut s = Server::new();
        assert!(s.handle("{not json").is_empty());
        assert!(s.handle(r#"{"jsonrpc":"2.0","id":1}"#).is_empty());
        // 服务器还活着。
        let out = s.handle(
            &json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {}}).to_string(),
        );
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn uri_and_path_round_trip() {
        let p = PathBuf::from(if cfg!(windows) {
            r"C:\a\b.wll"
        } else {
            "/a/b.wll"
        });
        let uri = path_to_uri(&p);
        assert!(uri.starts_with("file:///"));
        assert_eq!(uri_to_path(&uri), p);
    }

    #[test]
    fn frames_use_the_content_length_base_protocol() {
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, r#"{"a":1}"#).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("Content-Length: 7\r\n\r\n"));
        // 读回来:BufReader 需要一个 `Read`,`&[u8]` 自己就是 `Read`。
        let mut cursor = std::io::BufReader::new(text.as_bytes());
        assert_eq!(read_frame(&mut cursor).as_deref(), Some(r#"{"a":1}"#));
        assert!(read_frame(&mut cursor).is_none(), "EOF must end the loop");
    }
}
