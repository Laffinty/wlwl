//! 模块契约:旁路签名文件 `<module>.wll.sig`(C1)+ 源内 `SEALED([...])`
//! 声明(C2)—— v0.10 Step 6,计划书 §4。
//!
//! # 两种载体,一个概念
//!
//! 「模块对外承诺了什么」这件事有两种写法,本层把它们统一成**同一个**
//! 「声明面」来比:
//!
//! | 载体 | 形态 | 决策 | 带类型 | 谁来查 |
//! |---|---|---|---|---|
//! | 签名文件 | `foo.wll.sig` 旁路文本 | D-2 | 是 | 模块自查 + **消费方**查 |
//! | 密封面 | 源内 `SEALED([...])` | D-3 | 否 | 模块自查 |
//!
//! 职责切分是刻意的:**签名是「给别人看的契约」**,所以消费方(`IMPORT`)
//! 用一个签名里没声明的名字也要报;**密封面是「模块自己划的私有线」**,
//! 它只与本模块的 `EXPORT` 比 —— 越界由模块自查那条报,不在每个调用点
//! 重复报一遍。这样一个根因只出一条诊断。
//!
//! # 签名文件文法
//!
//! 行导向,一条声明一行。`#` 起注释,空行忽略。
//!
//! ```text
//! # math.wll 的模块签名(决策 D-2:无签名 = v0.9 行为)
//! EXPORT add (INTEGER, INTEGER) : INTEGER
//! EXPORT PI : FLOAT
//! EXPORT names : ARRAY[STRING]
//! EXPORT maybe_count : OPTION[INTEGER]
//! ```
//!
//! - `EXPORT <名字>` —— 只声明名字,类型写 `DYNAMIC`(顶底元,与任何类型
//!   可赋值,等价于「这里我不表态」)。不需要「只声明名字」的简写:
//!   `DYNAMIC` 已经把这件事说清楚了,少一套语法少一处分叉。
//! - `EXPORT <名字> (<形参>, …) : <返回>` —— 函数。**这里不用箭头**:
//!   `FUN(INTEGER) -> STRING` 今天会被 parser 静默吸收成
//!   `Named("( INTEGER ) - > STRING")`(Step 3 实测,见计划书 §3.3),
//!   所以签名文法**自带**一套 `名字(形参) : 返回` 的写法,而不是复用
//!   那个到不了的箭头形式。
//! - 类型表达式**复用源码那一份文法**(`ARRAY[T]` / `DICT[K, V]` /
//!   `OPTION[T]` / `RESULT[T, E]`,一律方括号):实现见 `parse_type_text`
//   ——它把类型文本包进一个合成的 `LET(__t: <文本>, 0);` 交给真正的
//!   parser,所以签名里的类型和注解里的类型**不可能**跑偏。
//!
//! # 不误报优先的三条边界
//!
//! **无签名 = v0.9 行为。** 这是**结构性**保证而不是约定:签名是旁路
//! 文件,语言里没有任何内嵌的「这是签名」的标记,不存在「忘了写标记」
//! 这回事。
//!
//! **推不出确切类型的地方不判类型。** `LET(a, UNKNOWN())` 推断出的是
//! `Dynamic`,而 `Dynamic` 与任何类型可赋值,所以签名写了什么都不会报
//! —— 与 Step 5「未结构化内建一律落 `Dynamic`」同一条纪律。
//!
//! **只比公开面。** 契约只看根作用域绑定 + `EXPORT`,函数体里的局部
//! 变量、`MATCH` 子句绑定一律不参与。
//!
//! # 诊断码
//!
//! | 条件 | `error` 档 | `warn` 档 |
//! |---|---|---|
//! | 导出/导入的名字不在声明面内(多出) | `E0113` | `W0113` |
//! | 声明面声明了但实现没导出(缺失) | `E0114` | `W0114` |
//! | 签名类型与实现注解冲突 | `E0115` | `W0115` |
//!
//! `E0116+` 留给 Step 8 的 MATCH 穷尽性,本层不得占用。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use wlwl_ast::{Expr, Span};
use wlwl_error::{ErrorCode, Location, WlwlDiagnostic, WlwlError};

use crate::check::DeclaredBinding;
use crate::diag::{ContractCarrier, TypeDiag, TypeDiagKind};
use crate::ty::Ty;

// ── 签名模型 ──────────────────────────────────────────────────

/// 旁路签名文件里的一条导出声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigEntry {
    /// 导出名(大小写敏感,与 `EXPORT` / `IMPORT` 一致)
    pub name: String,
    /// 声明类型。`DYNAMIC` = 不表态。
    pub ty: Ty,
    /// 声明所在行(1 起),用于把诊断指回 `.sig` 文件
    pub line: u32,
}

/// 一份解析好的模块签名。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleSig {
    /// 声明的导出名 → 条目。用 `BTreeMap` 是为了诊断顺序确定。
    pub entries: BTreeMap<String, SigEntry>,
}

impl ModuleSig {
    /// 声明面里的名字集合。
    pub fn names(&self) -> BTreeSet<String> {
        self.entries.keys().cloned().collect()
    }
}

// ── 解析 ──────────────────────────────────────────────────────

/// 解析一份模块签名文件。
///
/// 失败一律是**语法错误**:签名文件是文本格式,报的是既有语法码
/// (`E0010` / `E0011` / `E0012`),不新增码号 —— 一个码号只对应一种条件,
/// 而「`.sig` 写错了」就是语法错误。
pub fn parse_module_sig(text: &str, file: &str) -> Result<ModuleSig, WlwlError> {
    let mut entries = BTreeMap::new();
    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx as u32 + 1;
        // `#` 起注释。类型文法里没有 `#`,所以无歧义。
        let line = match raw.split_once('#') {
            Some((head, _)) => head,
            None => raw,
        }
        .trim();
        if line.is_empty() {
            continue;
        }
        let entry = parse_entry(line, file, line_no)?;
        if entries
            .insert(
                entry.name.clone(),
                SigEntry {
                    name: entry.name.clone(),
                    ..entry
                },
            )
            .is_some()
        {
            // 同一个名字声明两次是**歧义**,不是「后者覆盖前者」——
            // 签名是契约,契约里的重名没有「取哪个」这种默认值可讲。
            return Err(sig_syntax_error(
                file,
                line_no,
                format!(
                    "`{}` is declared twice in this module signature",
                    entry.name
                ),
            ));
        }
    }
    Ok(ModuleSig { entries })
}

/// 解析一行 `EXPORT …`。
fn parse_entry(line: &str, file: &str, line_no: u32) -> Result<SigEntry, WlwlError> {
    let rest = line.strip_prefix("EXPORT").ok_or_else(|| {
        sig_syntax_error(
            file,
            line_no,
            format!("expected `EXPORT <name> (: <type>)?`, found `{line}`"),
        )
    })?;
    let rest = rest.trim_start();
    // 名字:标识符 / 关键字,后面跟 `(` / `:` / 空白 / 行尾。
    let name_end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let name = &rest[..name_end];
    if name.is_empty() {
        return Err(sig_syntax_error(
            file,
            line_no,
            format!("expected an export name after `EXPORT`, found `{line}`"),
        ));
    }
    let tail = rest[name_end..].trim_start();

    if let Some(after_paren) = tail.strip_prefix('(') {
        // 函数条目:`名字 (形参, …) : 返回`
        let close = after_paren.find(')').ok_or_else(|| {
            sig_syntax_error(
                file,
                line_no,
                "unclosed `(` in a function entry".to_string(),
            )
        })?;
        let params_text = &after_paren[..close];
        let after = after_paren[close + 1..].trim_start();
        let ret_text = after.strip_prefix(':').ok_or_else(|| {
            sig_syntax_error(
                file,
                line_no,
                format!(
                    "function entry `{}` needs a `: <return type>` after the parameter list",
                    name
                ),
            )
        })?;
        let params = split_type_list(params_text)
            .into_iter()
            .map(|t| parse_type_text(&t, file, line_no))
            .collect::<Result<Vec<_>, _>>()?;
        let ret = parse_type_text(ret_text.trim(), file, line_no)?;
        Ok(SigEntry {
            name: name.to_string(),
            ty: Ty::Fun {
                params,
                ret: Box::new(ret),
            },
            line: line_no,
        })
    } else {
        // 值条目:`名字 : 类型`
        let type_text = match tail.strip_prefix(':') {
            Some(t) => t,
            None if tail.is_empty() => "DYNAMIC",
            None => {
                return Err(sig_syntax_error(
                    file,
                    line_no,
                    format!("expected `:` or `(` after the name `{name}`, found `{tail}`"),
                ))
            }
        };
        let ty = parse_type_text(type_text.trim(), file, line_no)?;
        Ok(SigEntry {
            name: name.to_string(),
            ty,
            line: line_no,
        })
    }
}

/// 按顶层逗号切分类型列表。类型里的方括号可以嵌套,所以不能裸
/// `split(',')`:`DICT[STRING, INTEGER]` 里的逗号不是分隔符。
fn split_type_list(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in text.chars() {
        match ch {
            '[' | '(' => {
                depth += 1;
                current.push(ch);
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                let t = current.trim().to_string();
                if !t.is_empty() {
                    out.push(t);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let t = current.trim().to_string();
    if !t.is_empty() {
        out.push(t);
    }
    out
}

/// 把一段类型文本变成静态类型 —— **走真正的 parser**。
///
/// 实现方式是把文本包进一个合成的 `LET(__wlwl_sig_t: <文本>, 0);` 再
/// 调用 `wlwl_parser::parse`,取回注解里的 `TypeExpr`。代价是一次小
/// 解析,换来的是签名文法与注解文法**永不分叉**:以后 parser 加了新的
/// 类型产生式,签名侧自动跟上,不需要在这里同步一份。
///
/// 前置 `line_no - 1` 个换行,是为了让 parser 报出的行号**就是 `.sig`
/// 文件里的真实行号** —— 合成源码只有一行,不补的话每条语法错误都会
/// 指到第 1 行。
fn parse_type_text(text: &str, file: &str, line_no: u32) -> Result<Ty, WlwlError> {
    if text.is_empty() {
        return Err(sig_syntax_error(
            file,
            line_no,
            "expected a type".to_string(),
        ));
    }
    let padding = "\n".repeat(line_no.saturating_sub(1) as usize);
    let synthetic = format!("{padding}LET(__wlwl_sig_t: {text}, 0);");
    let ast = wlwl_parser::parse(&synthetic, file).map_err(|e| reanchor(e, file, line_no))?;
    let annotation = match &ast {
        Expr::Let {
            type_annotation: Some(a),
            ..
        } => a,
        _ => {
            return Err(sig_syntax_error(
                file,
                line_no,
                format!("`{text}` is not a type expression"),
            ))
        }
    };
    Ok(Ty::from_type_expr(&annotation.expr))
}

/// 把 parser 的错误重新锚到 `.sig` 文件:码号保持不变(那本来就是
/// 语法错误码),位置换成签名文件 + 真实行号,消息加上出处前缀。
fn reanchor(err: WlwlError, file: &str, line_no: u32) -> WlwlError {
    let original = err.diagnostic();
    let d = WlwlDiagnostic::new(
        original.code,
        format!("in module signature: {}", original.message),
        Location::point(file.to_string(), line_no, 0),
    )
    .with_severity(original.severity);
    d.into()
}

fn sig_syntax_error(file: &str, line_no: u32, message: String) -> WlwlError {
    WlwlDiagnostic::new(
        ErrorCode::E0010,
        format!("invalid module signature: {message}"),
        Location::point(file.to_string(), line_no, 0),
    )
    .into()
}

// ── 渲染(Step 7 sig-gen 的地基) ──────────────────────────────

impl fmt::Display for ModuleSig {
    /// 渲染成规范签名文本,`parse_module_sig(display(sig)) == sig`。
    ///
    /// 函数条目用 `名字(形参) : 返回` 而不是 `FUN[…] -> …` —— 后者的
    /// 箭头形式在 parser 里到不了(见模块文档),渲染它就等于渲染一个
    /// 自己解析不回来的东西。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for entry in self.entries.values() {
            match &entry.ty {
                Ty::Fun { params, ret } => {
                    let rendered: Vec<String> = params.iter().map(Ty::to_string).collect();
                    writeln!(
                        f,
                        "EXPORT {} ({}) : {}",
                        entry.name,
                        rendered.join(", "),
                        ret
                    )?;
                }
                other => writeln!(f, "EXPORT {} : {}", entry.name, other)?,
            }
        }
        Ok(())
    }
}

// ── 生成(sig-gen 的地基) ────────────────────────────────────

/// 从模块实现**反推**一份签名骨架(计划书 §4.3 C3 的生成侧)。
///
/// 导出名来自 `EXPORT` 节点,类型来自根作用域绑定表 —— 也就是同一次
/// 类型遍历的产物,不另写一套推导。查不到绑定(理论上不该发生:`EXPORT`
/// 一个未绑定的名运行期就会 `E0020`)一律落 `DYNAMIC`,即「我不表态」。
///
/// **生成物必须永远可解析**,这是硬约束:`sig-gen` 产出的文件要能被
/// `check` 读回来(锁测试 `sig_gen_roundtrip_parse_check`)。签名文法
/// 能表达的类型比实现侧的类型 IR 窄 —— 嵌套位置**写不出函数类型**
/// (方括号形式的 `FUN[…]` 解析后返回类型恒为 `DYNAMIC`,渲染它等于
/// 悄悄丢掉返回类型),所以那种位置一律降级为 `DYNAMIC`,而不是渲染成
/// 一个「看起来精确、实际更弱」的类型。顶层函数类型照常渲染成
/// `名字(形参) : 返回`。
///
/// 降级规则集中在 `sig_safe_ty`,改签名文法时它和
/// [`ModuleSig`]'s `Display` 必须一起改。
pub fn sig_from_module(program: &Expr, declared: &[DeclaredBinding]) -> ModuleSig {
    let mut entries = BTreeMap::new();
    for (name, _) in exported_names(program) {
        let ty = declared
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.ty.clone())
            .unwrap_or(Ty::Dynamic);
        entries.insert(
            name.clone(),
            SigEntry {
                name,
                // line = 0:生成物没有「签名文件里的行」可言,渲染也不
                // 打印行号,只用于回显解析结果的来源。
                ty: sig_safe_ty(ty, true),
                line: 0,
            },
        );
    }
    ModuleSig { entries }
}

/// 把类型收敛到签名文法表达得了的形状。`top_level` 为真时函数类型照旧
/// 保留(渲染成 `名字(形参) : 返回`),否则降级为 `DYNAMIC`。
fn sig_safe_ty(ty: Ty, top_level: bool) -> Ty {
    match ty {
        Ty::Fun { .. } if !top_level => Ty::Dynamic,
        Ty::Fun { params, ret } => Ty::Fun {
            params: params
                .iter()
                .map(|p| sig_safe_ty(p.clone(), false))
                .collect(),
            ret: Box::new(sig_safe_ty(*ret, false)),
        },
        // [v0.10 Step 9] 带约束的类型变量**可渲染**:`T: Comparable` 用的
        // 是方括号内的 `:`(parser 认识它),所以它能被 `.wll.sig` 原文表达
        // —— 不必像函数类型那样降级 `DYNAMIC`。
        Ty::Var { name, bound } => Ty::Var {
            name,
            bound: bound.map(|b| Box::new(sig_safe_ty(*b, true))),
        },
        Ty::Array(e) => Ty::Array(Box::new(sig_safe_ty(*e, false))),
        Ty::Dict(k, v) => Ty::Dict(
            Box::new(sig_safe_ty(*k, false)),
            Box::new(sig_safe_ty(*v, false)),
        ),
        Ty::Option(t) => Ty::Option(Box::new(sig_safe_ty(*t, false))),
        Ty::Result(t, e) => Ty::Result(
            Box::new(sig_safe_ty(*t, false)),
            Box::new(sig_safe_ty(*e, false)),
        ),
        Ty::Named { name, args } => Ty::Named {
            name,
            args: args.iter().map(|a| sig_safe_ty(a.clone(), false)).collect(),
        },
        other => other,
    }
}

// ── 契约检查 ──────────────────────────────────────────────────

/// 一个模块对外的契约:签名文件(可选)+ 密封面(可选)。
#[derive(Debug, Clone, Default)]
pub struct ModuleContract {
    /// 模块标识(文件路径),只用于诊断文案
    pub module: String,
    /// 旁路签名文件
    pub sig: Option<ModuleSig>,
    /// `SEALED([...])` 声明的公开面
    pub seal: Option<BTreeSet<String>>,
}

impl ModuleContract {
    /// 从模块 AST + 可选签名文件组装契约:`SEALED([...])` 声明直接从
    /// 程序里读,签名文件由调用方按旁路路径找到后传进来。
    pub fn from_module(program: &Expr, module: impl Into<String>, sig: Option<ModuleSig>) -> Self {
        ModuleContract {
            module: module.into(),
            sig,
            seal: sealed_names(program),
        }
    }

    /// 没有任何契约载体 = 「无签名」路径,`check_exports` 直接返回空。
    pub fn is_empty(&self) -> bool {
        self.sig.is_none() && self.seal.is_none()
    }

    /// 签名声明的名字(无签名文件时为空集)。
    fn sig_names(&self) -> Option<BTreeSet<String>> {
        self.sig.as_ref().map(ModuleSig::names)
    }
}

/// 顶层 `EXPORT` 收集出来的名字(顺序 = 首次出现序,便于诊断稳定)。
fn exported_names(program: &Expr) -> Vec<(String, Span)> {
    fn collect(e: &Expr, out: &mut Vec<(String, Span)>) {
        match e {
            Expr::Block { exprs, .. } => {
                for e in exprs {
                    collect(e, out);
                }
            }
            Expr::Export { names, span } => {
                for n in names {
                    let name = n.local_name().to_string();
                    if !out.iter().any(|(existing, _)| *existing == name) {
                        out.push((name, span.clone()));
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    collect(program, &mut out);
    out
}

/// 顶层 `SEALED` 收集出来的名字(与 eval 的 `collect_sealed` 同一套规则:
/// 只看顶层块)。
fn sealed_names(program: &Expr) -> Option<BTreeSet<String>> {
    fn collect(e: &Expr, out: &mut BTreeSet<String>, found: &mut bool) {
        match e {
            Expr::Block { exprs, .. } => {
                for e in exprs {
                    collect(e, out, found);
                }
            }
            Expr::Sealed { names, .. } => {
                *found = true;
                out.extend(names.iter().map(|n| n.local_name().to_string()));
            }
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    let mut found = false;
    collect(program, &mut out, &mut found);
    if found {
        Some(out)
    } else {
        None
    }
}

/// 模块自查:实现自己的 `EXPORT` 面 vs 签名 / 密封面。
///
/// `declared` 是同一次类型遍历交出的根作用域绑定类型(Step 6 的一部分),
/// 只用于 `E0115` 的类型比较。
///
/// 返回值自带一份**去重集**:自查报过的「不在声明面内」的名字,交给
/// [`check_imports`] 让消费方那一侧不再复述(见 [`ImportedModule`])。
///
/// **零破坏**:无签名、无 `SEALED` 时直接返回空 —— 那是 v0.9 的全部
/// 程序走的路径,一条诊断都不该有。
pub fn check_exports(
    program: &Expr,
    declared: &[DeclaredBinding],
    contract: &ModuleContract,
) -> ModuleCheck {
    if contract.is_empty() {
        return ModuleCheck::default();
    }
    let mut diags = Vec::new();
    let mut undeclared = BTreeSet::new();
    let exports = exported_names(program);

    // ---- 缺失方向:声明了但没导出 ----
    for (name, carrier) in declared_missing(contract, &exports) {
        // 名字根本不在 `EXPORT` 里,所以没有 EXPORT 的 span 可用;退到
        // 模块顶层块的 span —— 诊断指向文件,而不是指向某个不存在的
        // 表达式。
        let span = span_of(program);
        diags.push(TypeDiag::new(
            TypeDiagKind::DeclaredNotExported {
                name,
                carriers: vec![carrier],
            },
            span,
        ));
    }

    // ---- 多出方向:导出了但没声明 ----
    for (name, span) in &exports {
        let carriers = missing_carriers(contract, name);
        if !carriers.is_empty() {
            undeclared.insert(name.clone());
            diags.push(TypeDiag::new(
                TypeDiagKind::ExportNotDeclared {
                    name: name.clone(),
                    carriers,
                },
                span.clone(),
            ));
        }
    }

    // ---- 类型冲突 ----
    if let Some(sig) = &contract.sig {
        for (name, entry) in &sig.entries {
            if !exports.iter().any(|(n, _)| n == name) {
                continue; // 缺失方向已经报过了,不再叠一条类型诊断
            }
            let Some(binding) = declared.iter().find(|b| b.name == *name) else {
                continue; // 实现没给这个绑定类型(见模块文档「不误报优先」)
            };
            if !binding.ty.satisfies_annotation(&entry.ty) {
                diags.push(TypeDiag::new(
                    TypeDiagKind::SignatureTypeMismatch {
                        name: name.clone(),
                        declared: entry.ty.clone(),
                        found: binding.ty.clone(),
                    },
                    binding.span.clone(),
                ));
            }
        }
    }

    ModuleCheck { diags, undeclared }
}

/// [`check_exports`] 的产物:诊断 + 「已报过的不在声明面内的名字」。
///
/// 后者是给消费方去重用的 —— 见 [`ImportedModule::already_reported`]。
#[derive(Debug, Clone, Default)]
pub struct ModuleCheck {
    /// 契约诊断(`E0113` / `E0114` / `E0115`)
    pub diags: Vec<TypeDiag>,
    /// 自查已报过的「导出 / 导入的名字不在声明面内」
    pub undeclared: BTreeSet<String>,
}

/// 程序里所有 `IMPORT` 节点(顶层块内,以及 `IF` 分支内)。
///
/// 只走这两层:模块契约关心的是「谁依赖谁」,而 `IMPORT` 在语言里是
/// 顶层声明(spec §13),写在函数体里本来就不该有。深一层少一层递归,
/// 也少一层为「函数体里的 import」操心的地方。
pub fn import_nodes(program: &Expr) -> Vec<&Expr> {
    fn collect<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
        match e {
            Expr::Block { exprs, .. } => {
                for e in exprs {
                    collect(e, out);
                }
            }
            Expr::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect(then_branch, out);
                if let Some(e) = else_branch {
                    collect(e, out);
                }
            }
            other @ Expr::Import { .. } => out.push(other),
            _ => {}
        }
    }
    let mut out = Vec::new();
    collect(program, &mut out);
    out
}

/// [`import_nodes`] 的 spec 视图,按出现顺序去重。编译期沿 import 图
/// 走模块时用它。
pub fn import_specs(program: &Expr) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for node in import_nodes(program) {
        if let Expr::Import { path, .. } = node {
            if !out.iter().any(|p| p == path) {
                out.push(path.clone());
            }
        }
    }
    out
}

/// 一个被 import 的模块,连同「自查已经报过什么」。
#[derive(Debug, Clone, Default)]
pub struct ImportedModule {
    /// 该模块的契约载体
    pub contract: ModuleContract,
    /// 自查已经报过的「不在声明面内」的名字。
    ///
    /// 消费方这一侧**跳过**它们:同一个根因(签名漏了一个名字)已经在
    /// 模块自己的位置上说过一遍了,再在每个调用点复述一遍只会淹没真正
    /// 的新问题。签名没声明 `PI`、模块又导出了 `PI`、三个文件都用了
    /// `PI` —— 用户要做的修复只有一件:把 `PI` 写进签名。
    pub already_reported: BTreeSet<String>,
}

/// 消费方自查:`IMPORT` 的名字必须在**签名**的声明面内。
///
/// 密封面**不**参与这一侧(见模块文档「两种载体,一个概念」):密封面是
/// 模块内部的私有线,越界由模块自查那条报,不在每个调用点重复一遍。
pub fn check_imports(program: &Expr, modules: &BTreeMap<String, ImportedModule>) -> Vec<TypeDiag> {
    let mut diags = Vec::new();
    for import in import_nodes(program) {
        let Expr::Import { path, names, .. } = import else {
            continue;
        };
        let Some(imported) = modules.get(path) else {
            continue; // 没解析到契约的 spec(解析失败的分支另行上报)
        };
        let Some(sig_names) = imported.contract.sig_names() else {
            continue; // 只有密封面 = 没有对外签名,消费方无从判
        };
        for n in names {
            if sig_names.contains(&n.name) || imported.already_reported.contains(&n.name) {
                continue;
            }
            diags.push(TypeDiag::new(
                TypeDiagKind::ExportNotDeclared {
                    name: n.name.clone(),
                    carriers: vec![ContractCarrier::Signature],
                },
                n.span.clone(),
            ));
        }
    }
    diags
}

/// 声明面缺哪些名字 → 哪些载体没声明它。一个名字**只报一条**,
/// 两个载体都缺时在同一条里说清。
fn missing_carriers(contract: &ModuleContract, name: &str) -> Vec<ContractCarrier> {
    let mut out = Vec::new();
    if let Some(names) = contract.sig_names() {
        if !names.contains(name) {
            out.push(ContractCarrier::Signature);
        }
    }
    if let Some(seal) = &contract.seal {
        if !seal.contains(name) {
            out.push(ContractCarrier::Seal);
        }
    }
    out
}

/// 声明面里有、实现没导出的名字,按 `(名字, 声明它的载体)` 返回。
///
/// 两个载体**各自**返回:同一个名字既不在签名也不在密封面里,是两条
/// 诊断(「签名少了它」+「密封面少了它」),因为修法不同 —— 一个改
/// `.sig`,一个改源码里的 `SEALED`。反过来「实现多导出了」那一侧两个
/// 载体是**合并**成一条的(同一个根因,改哪边都行)。
fn declared_missing(
    contract: &ModuleContract,
    exports: &[(String, Span)],
) -> Vec<(String, ContractCarrier)> {
    let mut out = Vec::new();
    if let Some(names) = contract.sig_names() {
        for name in names {
            if !exports.iter().any(|(n, _)| *n == name) {
                out.push((name.clone(), ContractCarrier::Signature));
            }
        }
    }
    if let Some(seal) = &contract.seal {
        for name in seal {
            if !exports.iter().any(|(n, _)| n == name) {
                out.push((name.clone(), ContractCarrier::Seal));
            }
        }
    }
    out
}

/// 给「声明了但没导出」找一个落点。名字根本不在 `EXPORT` 里,所以没有
/// `EXPORT` 的 span 可用;退到模块顶层块的 span(诊断指向文件而不是
/// 指向某个不存在的表达式)。
fn span_of(program: &Expr) -> Span {
    program.span().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check_program_detailed;
    use std::collections::HashMap;
    use wlwl_parser::parse;

    /// 走一遍「解析 + 类型遍历 + 契约检查」,返回诊断码序列。
    fn contract_diags(module_src: &str, sig_text: Option<&str>) -> Vec<String> {
        let module = parse(module_src, "m.wll").expect("module must parse");
        let sig = sig_text.map(|t| parse_module_sig(t, "m.wll.sig").expect("signature must parse"));
        let out = check_program_detailed(&module, &HashMap::new());
        let contract = ModuleContract::from_module(&module, "m.wll", sig);
        check_exports(&module, &out.declared, &contract)
            .diags
            .iter()
            .map(|d| match d.kind.codes() {
                Some((e, _)) => e.as_str().to_string(),
                None => "<uncoded>".to_string(),
            })
            .collect()
    }

    // ---- 签名文法 ----

    #[test]
    fn sig_parses_value_and_function_entries() {
        let text = "\
# math.wll 的签名
EXPORT add (INTEGER, INTEGER) : INTEGER
EXPORT PI : FLOAT
EXPORT names : ARRAY[STRING]
EXPORT table : DICT[STRING, INTEGER]
EXPORT maybe_count : OPTION[INTEGER]
EXPORT split (RESULT[INTEGER, STRING]) : RESULT[STRING, INTEGER]
";
        let sig = parse_module_sig(text, "m.wll.sig").expect("signature parses");
        assert_eq!(sig.entries.len(), 6);
        assert_eq!(
            sig.entries["add"].ty,
            Ty::Fun {
                params: vec![Ty::Integer, Ty::Integer],
                ret: Box::new(Ty::Integer),
            }
        );
        assert_eq!(sig.entries["PI"].ty, Ty::Float);
        assert_eq!(sig.entries["names"].ty, Ty::Array(Box::new(Ty::String)));
        assert_eq!(
            sig.entries["table"].ty,
            Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer))
        );
        assert_eq!(
            sig.entries["maybe_count"].ty,
            Ty::Option(Box::new(Ty::Integer))
        );
        // 嵌套逗号不被当成参数分隔符。
        assert_eq!(
            sig.entries["split"].ty,
            Ty::Fun {
                params: vec![Ty::Result(Box::new(Ty::Integer), Box::new(Ty::String))],
                ret: Box::new(Ty::Result(Box::new(Ty::String), Box::new(Ty::Integer))),
            }
        );
    }

    /// 签名文法自带 `名字(形参) : 返回`,就是为了绕开箭头形式到不了
    /// parser 这个坑(Step 3 实测)。往返测试把这条钉死。
    ///
    /// 比的是**声明内容**(`名字 -> 类型`),不含行号 —— 行号是位置元
    /// 数据,渲染时按名字排序重排,换行是必然的。渲染出来的文本再解析
    /// 回来必须逐条同型,这条就是 Step 7 `sig-gen` 的地基。
    #[test]
    fn sig_display_round_trips_through_the_parser() {
        fn types(sig: &ModuleSig) -> BTreeMap<String, Ty> {
            sig.entries
                .iter()
                .map(|(k, v)| (k.clone(), v.ty.clone()))
                .collect()
        }
        let text = "\
EXPORT add (INTEGER, INTEGER) : INTEGER
EXPORT PI : FLOAT
EXPORT names : ARRAY[STRING]
EXPORT table : DICT[STRING, INTEGER]
EXPORT either : OPTION[INTEGER]
";
        let sig = parse_module_sig(text, "m.wll.sig").expect("signature parses");
        let rendered = sig.to_string();
        let reparsed = parse_module_sig(&rendered, "m.wll.sig").expect("rendered form parses");
        assert_eq!(types(&sig), types(&reparsed));
        // 再渲染一次必须逐字节相同(幂等):`sig-gen` 连续跑两次不产生 diff。
        assert_eq!(reparsed.to_string(), rendered);
        // 渲染按名字排序(诊断与 diff 都要确定),所以它不是原文的逆函数。
        assert!(rendered.starts_with("EXPORT PI : FLOAT\n"));
    }

    #[test]
    fn sig_uses_the_source_type_grammar_so_the_two_cannot_drift() {
        // 签名里的类型走真 parser:源码能写的类型写法,签名里就都能写;
        // 源码写不了(箭头形式),签名里也不假装支持。
        let sig = parse_module_sig("EXPORT f (INTEGER) : STRING", "m.wll.sig").unwrap();
        let arrow = sig.entries["f"].ty.clone();
        assert_eq!(
            arrow,
            Ty::Fun {
                params: vec![Ty::Integer],
                ret: Box::new(Ty::String),
            }
        );
        // 箭头写法会被当成一个畸形类型名报错,而不是被静默吸收。
        let err = parse_module_sig("EXPORT f (INTEGER) -> STRING", "m.wll.sig")
            .expect_err("the arrow form is not part of the signature grammar");
        assert!(err
            .diagnostic()
            .message
            .contains("invalid module signature"));
    }

    #[test]
    fn sig_reports_duplicate_declarations_and_unknown_lines() {
        let dup = parse_module_sig("EXPORT a : INTEGER\nEXPORT a : STRING", "m.wll.sig")
            .expect_err("a declared name must be unique");
        assert!(dup.diagnostic().message.contains("declared twice"));
        let junk = parse_module_sig("MODULE a", "m.wll.sig").expect_err("only EXPORT is allowed");
        assert!(junk.diagnostic().message.contains("expected `EXPORT"));
    }

    #[test]
    fn sig_syntax_errors_point_at_the_offending_line_of_the_sig_file() {
        // 合成源码只有一行,不补前缀的话每条错误都会指到第 1 行。
        let err = parse_module_sig("EXPORT ok : INTEGER\nEXPORT bad : ARRAY\n", "m.wll.sig")
            .expect_err("bare ARRAY is not a type");
        assert_eq!(err.diagnostic().location.line, 2);
        assert_eq!(err.diagnostic().location.file, "m.wll.sig");
    }

    // ---- 计划书点名的三条锁测试 ----

    /// 锁测试 1:`E0113` / `E0114` / `E0115` 三条各自的触发条件。
    ///
    /// 每个夹具只让**一个**方向出问题 —— 否则一条夹具会同时触发两三条,
    /// 掩盖掉「这条码到底管哪个条件」。
    #[test]
    fn sig_mismatch_reports_e0113_e0114_e0115() {
        // E0113 多出:实现了 `a` 与 `secret`,签名只声明了 `a`。
        assert_eq!(
            contract_diags(
                "LET(a: INTEGER, 1); LET(secret, 1); EXPORT([\"a\", \"secret\"]);",
                Some("EXPORT a : INTEGER")
            ),
            vec!["E0113"]
        );
        // E0114 缺失:签名声明了 `ghost`,实现只导出 `a`。
        assert_eq!(
            contract_diags(
                "LET(a: INTEGER, 1); EXPORT([\"a\"]);",
                Some("EXPORT a : INTEGER\nEXPORT ghost : INTEGER")
            ),
            vec!["E0114"]
        );
        // E0115 冲突:签名说 STRING,实现注解是 INTEGER。
        assert_eq!(
            contract_diags(
                "LET(a: INTEGER, 1); EXPORT([\"a\"]);",
                Some("EXPORT a : STRING")
            ),
            vec!["E0115"]
        );
    }

    /// 锁测试 2:密封面越界。签名与密封面共用两条码,但载体不同,
    /// 诊断要能说清是哪一个缺了名字。
    #[test]
    fn sealed_violation_reports_the_seal_as_the_missing_carrier() {
        let module = parse(
            "SEALED([\"open\"]); LET(open, 1); LET(sneaky, 2); EXPORT([\"open\", \"sneaky\"]);",
            "m.wll",
        )
        .expect("module must parse");
        let out = check_program_detailed(&module, &HashMap::new());
        let contract = ModuleContract::from_module(&module, "m.wll", None);
        let diags = check_exports(&module, &out.declared, &contract).diags;
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind.codes().unwrap().0.as_str(), "E0113");
        assert_eq!(
            diags[0].message(),
            "export `sneaky` is not declared in the SEALED surface"
        );
        // 密封面比导出面窄 —— 密封面声明了却没导出的名字走 E0114。
        let module = parse("SEALED([\"ghost\"]); LET(a, 1); EXPORT([\"a\"]);", "m.wll")
            .expect("module must parse");
        let out = check_program_detailed(&module, &HashMap::new());
        let contract = ModuleContract::from_module(&module, "m.wll", None);
        let diags = check_exports(&module, &out.declared, &contract).diags;
        assert_eq!(diags[0].kind.codes().unwrap().0.as_str(), "E0114");
        assert_eq!(
            diags[0].message(),
            "the SEALED surface declares `ghost`, which the module does not export"
        );
    }

    /// 锁测试 3:无签名 = v0.9 行为。这是**结构性**保证(旁路文件,语言里
    /// 没有任何标记语法),不是约定。
    #[test]
    fn no_sig_behaves_as_v09() {
        // 没有任何契约载体 → 一条诊断都没有,哪怕模块导出了一堆东西。
        assert!(contract_diags("LET(a, 1); LET(b, 2); EXPORT([\"a\", \"b\"]);", None).is_empty());
        // 签名与实现完全一致 → 静默。
        assert!(contract_diags(
            "LET(a: INTEGER, 1); EXPORT([\"a\"]);",
            Some("EXPORT a : INTEGER")
        )
        .is_empty());
    }

    // ---- E0115 的方向与不误报边界 ----

    #[test]
    fn the_implementation_must_be_able_to_stand_in_for_the_signature() {
        // 实现比签名更宽(安全放宽)不算违约。
        assert!(contract_diags(
            "LET(a: INTEGER, 1); EXPORT([\"a\"]);",
            Some("EXPORT a : FLOAT")
        )
        .is_empty());
        // 实现比签名更窄(收窄)违约:签名承诺 INTEGER,却给出 STRING。
        assert_eq!(
            contract_diags(
                "LET(a: STRING, \"x\"); EXPORT([\"a\"]);",
                Some("EXPORT a : INTEGER")
            ),
            vec!["E0115"]
        );
        // 函数按形参逆变判定:签名要 INTEGER 形参,实现收 FLOAT 也安全。
        assert!(contract_diags(
            "LET(f, FUN((a: FLOAT) : INTEGER, 1)); EXPORT([\"f\"]);",
            Some("EXPORT f (INTEGER) : INTEGER")
        )
        .is_empty());
    }

    /// 推断不出确切类型的地方不判类型:那里落 `Dynamic`,而 `Dynamic`
    /// 与任何类型可赋值。这与 Step 5「未结构化内建一律落 Dynamic」同
    /// 一条纪律 —— 猜出来的类型不许变成诊断。
    ///
    /// 注意夹具:无注解**不等于** `Dynamic`。`LET(a, 1)` 的字面量照样推
    /// 得精确定到 `INTEGER`,该报还是报(见上一条测试);真正落 `Dynamic`
    /// 的是调用结果这种「推不出来」的位置。
    #[test]
    fn an_uninferrable_implementation_is_never_judged_against_the_signature() {
        assert!(contract_diags(
            "LET(a, UNKNOWN_CALL()); EXPORT([\"a\"]);",
            Some("EXPORT a : STRING")
        )
        .is_empty());
    }

    /// 一个名字只在**一个**载体里缺时,只出一条诊断,并说清缺在哪个载体。
    #[test]
    fn one_missing_carrier_produces_one_diagnostic() {
        let module = parse(
            "SEALED([\"a\"]); LET(a: INTEGER, 1); LET(b, 2); EXPORT([\"a\", \"b\"]);",
            "m.wll",
        )
        .expect("module must parse");
        let out = check_program_detailed(&module, &HashMap::new());
        let sig = parse_module_sig("EXPORT a : INTEGER", "m.wll.sig").expect("parses");
        let contract = ModuleContract::from_module(&module, "m.wll", Some(sig));
        let diags = check_exports(&module, &out.declared, &contract).diags;
        // `b` 在签名里没有,在密封面里也没有 → 两个载体都缺,合并成一条。
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message(),
            "export `b` is not declared in the module signature or the SEALED surface"
        );
    }

    // ---- 消费方一侧 ----

    /// 造一个「被 import 的模块」条目。`already_reported` 是模块自查
    /// 已经报过的名字,用来验去重。
    fn imported(sig: Option<&str>, already_reported: &[&str]) -> ImportedModule {
        ImportedModule {
            contract: ModuleContract {
                module: "math.wll".into(),
                sig: sig.map(|t| parse_module_sig(t, "math.wll.sig").expect("signature parses")),
                seal: None,
            },
            already_reported: already_reported.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn import_diags(importer: &str, sig: Option<&str>) -> Vec<String> {
        let ast = parse(importer, "main.wll").expect("importer must parse");
        let mut modules = BTreeMap::new();
        if let Some(text) = sig {
            modules.insert("math".to_string(), imported(Some(text), &[]));
        }
        check_imports(&ast, &modules)
            .iter()
            .map(|d| match d.kind.codes() {
                Some((e, _)) => e.as_str().to_string(),
                None => "<uncoded>".to_string(),
            })
            .collect()
    }

    /// 签名是**给别人看的契约**:消费方用了一个签名没声明的名字,报
    /// E0113,并把位置指在 `IMPORT` 的那个名字上。
    #[test]
    fn importing_an_undeclared_name_is_e0113_at_the_import_site() {
        assert_eq!(
            import_diags(
                "IMPORT(\"math\", [\"add\", \"internals\"]);",
                Some("EXPORT add (INTEGER, INTEGER) : INTEGER")
            ),
            vec!["E0113"]
        );
        // 声明过的名字照常通过。
        assert!(import_diags(
            "IMPORT(\"math\", [\"add\"]);",
            Some("EXPORT add (INTEGER, INTEGER) : INTEGER")
        )
        .is_empty());
        // 没有签名文件 → v0.9 行为,消费方无从判起,静默。
        assert!(import_diags("IMPORT(\"math\", [\"whatever\"]);", None).is_empty());
    }

    /// 缺失方向与类型冲突**不在**消费方这一侧报:它们是模块自己的事,
    /// 一个根因只出一条诊断。签名声明了 `ghost` 而模块没导出时,模块
    /// 自查报 E0114;消费方导入 `ghost` 是**静默**的 —— 签名承诺过它,
    /// 是这个模块赖账,不该让每个调用点都背一条诊断。
    #[test]
    fn the_consumer_side_only_covers_the_extra_direction() {
        assert!(import_diags(
            "IMPORT(\"math\", [\"ghost\"]);",
            Some("EXPORT ghost : INTEGER")
        )
        .is_empty());
    }

    /// 密封面**不**参与消费方判定:它是模块内部的私有线,越界由模块自查
    /// 那条报,不在每个调用点重复一遍。
    #[test]
    fn a_seal_without_a_signature_says_nothing_to_consumers() {
        let ast = parse("IMPORT(\"math\", [\"internals\"]);", "main.wll").expect("parses");
        let module = parse(
            "SEALED([\"add\"]); EXPORT([\"add\", \"internals\"]);",
            "math.wll",
        )
        .expect("parses");
        let mut modules = BTreeMap::new();
        modules.insert(
            "math".to_string(),
            ImportedModule {
                contract: ModuleContract::from_module(&module, "math.wll", None),
                already_reported: BTreeSet::new(),
            },
        );
        assert!(check_imports(&ast, &modules).is_empty());
    }

    /// 一个根因只报一次:模块自查已经说过「`PI` 不在声明面内」,消费方
    /// 就不复述 —— 用户要做的修复只有一件(把 `PI` 写进签名)。
    #[test]
    fn the_consumer_side_stays_silent_where_the_module_already_spoke() {
        let ast = parse("IMPORT(\"math\", [\"PI\", \"internals\"]);", "main.wll").expect("parses");
        let mut modules = BTreeMap::new();
        // `PI` 已被自查报过 → 静默;`internals` 没报过 → E0113。
        modules.insert(
            "math".to_string(),
            imported(Some("EXPORT add : INTEGER"), &["PI"]),
        );
        let diags = check_imports(&ast, &modules);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message().contains("`internals`"),
            "only the unreported name survives: {}",
            diags[0].message()
        );
    }

    #[test]
    fn imports_nested_in_blocks_are_still_checked() {
        let ast = parse(
            "IF(<(1, 1), IMPORT(\"math\", [\"internals\"]), PRINT(1));",
            "main.wll",
        )
        .expect("parses");
        let mut modules = BTreeMap::new();
        modules.insert(
            "math".to_string(),
            imported(Some("EXPORT add : INTEGER"), &[]),
        );
        let diags = check_imports(&ast, &modules);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind.codes().unwrap().0.as_str(), "E0113");
    }

    // ---- 生成侧(sig-gen 的地基) ----

    fn generated(module_src: &str) -> String {
        let module = parse(module_src, "m.wll").expect("module must parse");
        let out = check_program_detailed(&module, &HashMap::new());
        sig_from_module(&module, &out.declared).to_string()
    }

    /// 计划书点名的验收项:**生成物可被解析回来**,且来回一致。
    #[test]
    fn sig_gen_roundtrip_parse_check() {
        let text = generated(
            "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
             LET(PI, 3);\n\
             LET(names, [\"x\"]);\n\
             EXPORT([\"add\", \"PI\", \"names\"]);\n",
        );
        assert_eq!(
            text,
            "EXPORT PI : INTEGER\n\
             EXPORT add (INTEGER, INTEGER) : INTEGER\n\
             EXPORT names : ARRAY[STRING]\n"
        );
        // 解析回来 → 再渲染,逐字节相同。
        let reparsed = parse_module_sig(&text, "m.wll.sig").expect("generated text parses");
        assert_eq!(reparsed.to_string(), text);
    }

    /// 生成物**必须**永远可解析。降级规则就是为此存在的:方括号形式的
    /// `FUN[…]` 解析后返回类型恒为 `DYNAMIC`(源码里还没有箭头形式,
    /// 见模块文档),所以在**非顶层**位置渲染函数类型等于悄悄丢掉返回
    /// 类型 —— 那里只能落 `DYNAMIC`。
    #[test]
    fn unrenderable_nested_function_types_degrade_to_dynamic() {
        // 形参注解写成 `FUN[INTEGER]` 是源码里**能到达**的函数类型形状,
        // 也是签名文法表达不了的形状。
        let text = generated(
            "LET(apply, FUN((f: FUN[INTEGER]) : STRING, \"\"));\n\
             EXPORT([\"apply\"]);\n",
        );
        assert_eq!(text, "EXPORT apply (DYNAMIC) : STRING\n");
        assert!(parse_module_sig(&text, "m.wll.sig").is_ok());
    }

    #[test]
    fn a_container_of_functions_degrades_only_the_inner_slot() {
        let text = generated(
            "LET(table, [FUN((x: INTEGER) : INTEGER, x)]);\n\
             LET(ok, 1);\n\
             EXPORT([\"table\", \"ok\"]);\n",
        );
        assert_eq!(text, "EXPORT ok : INTEGER\nEXPORT table : ARRAY[DYNAMIC]\n");
        assert!(parse_module_sig(&text, "m.wll.sig").is_ok());
    }

    /// 没导出的绑定不进签名 —— 签名描述的是公开面。
    #[test]
    fn only_exported_bindings_reach_the_signature() {
        let text = generated("LET(internal, 1); LET(public, 2); EXPORT([\"public\"]);\n");
        assert_eq!(text, "EXPORT public : INTEGER\n");
    }

    /// 导出了却没有根作用域绑定(理论上运行期会先 `E0020`)时,签名落
    /// `DYNAMIC` —— 生成器不猜,也不报错。
    #[test]
    fn an_unbound_export_falls_back_to_dynamic() {
        let module = parse("EXPORT([\"ghost\"]);", "m.wll").expect("parses");
        let out = check_program_detailed(&module, &HashMap::new());
        let sig = sig_from_module(&module, &out.declared);
        assert_eq!(sig.entries["ghost"].ty, Ty::Dynamic);
        assert_eq!(sig.to_string(), "EXPORT ghost : DYNAMIC\n");
    }

    /// 零导出的模块生成**空**签名(而不是编造一个)。
    #[test]
    fn a_module_without_exports_generates_an_empty_signature() {
        assert_eq!(generated("LET(a, 1);\n"), "");
    }

    /// [Step 9] 带约束的类型变量**能被签名表达**:`T: Comparable` 的 `:` 在
    /// 方括号内,`.wll.sig` 的类型文本走的正是同一个 parser(Step 7 的
    /// `parse_type_text`（模块内私有）)。所以 Step 7 那条「签名文法表达不了的类型一律降级
    /// `DYNAMIC`」在这里松开了一格 —— 往返仍然成立。
    #[test]
    fn bounded_variables_survive_the_signature_round_trip() {
        let text = generated(
            "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, a));\n\
             EXPORT([\"max\"]);\n",
        );
        assert_eq!(
            text,
            "EXPORT max (T: Comparable, T: Comparable) : T: Comparable\n"
        );
        let reparsed = parse_module_sig(&text, "m.wll.sig").expect("generated text parses");
        assert_eq!(reparsed.to_string(), text);
    }

    /// 零参函数是合法签名条目,往返得回来。
    #[test]
    fn a_zero_parameter_function_round_trips() {
        let text = generated("LET(answer, FUN(() : INTEGER, 42));\nEXPORT([\"answer\"]);\n");
        assert_eq!(text, "EXPORT answer () : INTEGER\n");
        let reparsed = parse_module_sig(&text, "m.wll.sig").expect("parses");
        assert_eq!(reparsed.to_string(), text);
    }

    /// 生成的签名**必须**能让 `check` 静默通过 —— 契约自洽是 sig-gen
    /// 的基本承诺,不是加分项。
    #[test]
    fn a_generated_signature_satisfies_its_own_module() {
        let module_src = "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
                          LET(PI, 3);\n\
                          EXPORT([\"add\", \"PI\"]);\n";
        let module = parse(module_src, "m.wll").expect("parses");
        let out = check_program_detailed(&module, &HashMap::new());
        let sig = sig_from_module(&module, &out.declared);
        let contract = ModuleContract::from_module(&module, "m.wll", Some(sig));
        let check = check_exports(&module, &out.declared, &contract);
        assert!(
            check.diags.is_empty(),
            "a generated signature must not condemn its own module: {:?}",
            check.diags
        );
    }
}
