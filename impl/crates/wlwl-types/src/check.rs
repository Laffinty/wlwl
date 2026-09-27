//! 编译期静态 pass(ADR-0020 A3 · build plan Step 2)。
//!
//! # 定位
//!
//! 把 `wlwl check` 从「只 parse」升级为「parse → 可选静态 check」。
//! 本 pass 只做**边界契约检查**,范围限缩为 ADR-0020 Decision 2:
//! **不是**全程序推断,**不是** HM,**不做**流敏感类型收窄。
//!
//! # 三条硬纪律
//!
//! 1. **不误报优先**。任何推导不出确切类型的位置一律落
//!    [`Ty::Dynamic`],而 `Dynamic` 与任何类型可赋值,所以「不确定」永远
//!    不产生诊断。A3 的验收项是「未标注程序在开启模式下不误报」。
//! 2. **不碰内建返回类型**。注册表结构化签名归 A6′(Step 5),在此之前
//!    内建调用一律返回 `Dynamic` —— 所以 A3 只能抓到**用户定义的、带注解
//!    的**函数边界,抓不到内建。这是范围裁剪,不是缺陷。
//! 3. **不报未定义名字**。那由 parser 的 `lint` pass 负责(`W0001`),
//!    静态 pass 重复报会让一个问题出两张诊断。
//!
//! # 与运行时 `E0033` 的关系
//!
//! 两者正交、并存:本 pass 编译期拦,`strict_types` 运行时兜底。本 pass
//! **不修改** `wlwl-eval` 的任何代码,`E0033` 的触发路径与语义完全不变
//! (ADR-0020 S3)。

use wlwl_ast::{Expr, FunParam, Literal, Pattern, Span, StrPart};

use crate::diag::{TypeDiag, TypeDiagKind};
use crate::env::TypeEnv;
use crate::ty::Ty;

/// 对一棵已解析的 AST 跑静态 pass,返回全部诊断(按发现顺序)。
///
/// 调用方在 `gradual_typing = "off"` 时**不应调用本函数**(那才是
/// 「零开销」的保证);`off` 档必须整条静态链路都不执行。
pub fn check_program(expr: &Expr) -> Vec<TypeDiag> {
    let mut checker = Checker::new();
    checker.check_expr(expr, None);
    checker.diags
}

struct Checker {
    env: TypeEnv,
    diags: Vec<TypeDiag>,
    /// 当前函数的返回值注解栈。最内层生效 —— 嵌套函数的 `RETURN` 不会
    /// 被外层函数的注解误伤。
    returns: Vec<Ty>,
}

impl Checker {
    fn new() -> Checker {
        Checker {
            env: TypeEnv::new(),
            diags: Vec::new(),
            returns: Vec::new(),
        }
    }

    fn report(&mut self, kind: TypeDiagKind, span: &Span) {
        self.diags.push(TypeDiag::new(kind, span.clone()));
    }

    /// 「实际类型 vs 期望类型」的统一判定。`Dynamic` 在任一侧都不报 ——
    /// 这是「不误报优先」纪律的落点。
    fn require(
        &mut self,
        expected: &Ty,
        found: &Ty,
        kind_of: impl Fn(Ty, Ty) -> TypeDiagKind,
        span: &Span,
    ) {
        if !found.is_assignable_to(expected) {
            self.report(kind_of(expected.clone(), found.clone()), span);
        }
    }

    /// 遍历并检查一个表达式,返回它推导出的类型。
    ///
    /// `expected` 是**期望类型向下传播**的入口参数:从这里传下去的字面量
    /// 会按期望定型(如 `INTEGER` 字面量喂给 `FLOAT` 形参直接定为
    /// `FLOAT`)。传播的完整规则归 A2′(Step 4),A3 只做字面量一级。
    fn check_expr(&mut self, expr: &Expr, expected: Option<&Ty>) -> Ty {
        match expr {
            Expr::Literal(lit, _) => self.check_literal(lit, expected),

            Expr::Var(name, _) => self.env.lookup(name).cloned().unwrap_or(Ty::Dynamic),
            Expr::Call { name, args, span } => self.check_call(name, args, span),
            Expr::Block { exprs, .. } => {
                let mut last = Ty::Dynamic;
                for (i, e) in exprs.iter().enumerate() {
                    let is_last = i + 1 == exprs.len();
                    let t = if is_last {
                        self.check_expr(e, expected)
                    } else {
                        self.check_expr(e, None)
                    };
                    if is_last {
                        last = t;
                    }
                }
                last
            }
            Expr::Array { items, .. } => {
                // 元素类型:全部相同才保留,含任何 Dynamic 或不一致即 Dynamic。
                let mut unified: Option<Ty> = None;
                for item in items {
                    let t = self.check_expr(item, None);
                    unified = Some(match unified {
                        None => t,
                        Some(prev) if prev == t => prev,
                        Some(_) => Ty::Dynamic,
                    });
                }
                Ty::Array(Box::new(unified.unwrap_or(Ty::Dynamic)))
            }
            Expr::Dict { entries, .. } => {
                let mut keys: Option<Ty> = None;
                let mut vals: Option<Ty> = None;
                for (k, v) in entries {
                    let kt = self.check_expr(k, None);
                    let vt = self.check_expr(v, None);
                    keys = unify(keys, kt);
                    vals = unify(vals, vt);
                }
                Ty::Dict(
                    Box::new(keys.unwrap_or(Ty::Dynamic)),
                    Box::new(vals.unwrap_or(Ty::Dynamic)),
                )
            }
            Expr::Let {
                name,
                type_annotation,
                value,
                span,
                ..
            } => {
                let declared = type_annotation
                    .as_ref()
                    .map(|a| Ty::from_type_expr(&a.expr));
                let actual = self.check_expr(value, declared.as_ref());
                if let Some(d) = &declared {
                    let at = type_annotation.as_ref().map(|a| &a.span).unwrap_or(span);
                    self.require(
                        d,
                        &actual,
                        |expected, found| TypeDiagKind::AnnotationMismatch { expected, found },
                        at,
                    );
                }
                let bound = declared.unwrap_or(actual);
                self.env.bind(name.clone(), bound.clone());
                bound
            }
            Expr::LetPattern {
                pattern,
                type_annotation,
                value,
                ..
            } => {
                let declared = type_annotation
                    .as_ref()
                    .map(|a| Ty::from_type_expr(&a.expr));
                let actual = self.check_expr(value, declared.as_ref());
                if let Some(d) = &declared {
                    self.require(
                        d,
                        &actual,
                        |expected, found| TypeDiagKind::AnnotationMismatch { expected, found },
                        &actual_span(value),
                    );
                }
                self.bind_pattern(pattern, &actual);
                let _ = declared;
                actual
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_expr(cond, None);
                let then_ty = self.check_expr(then_branch, expected);
                match else_branch {
                    Some(e) => {
                        let else_ty = self.check_expr(e, expected);
                        // 合流:相同才保留。A2′ 才做 lub,这里不猜。
                        if then_ty == else_ty {
                            then_ty
                        } else {
                            Ty::Dynamic
                        }
                    }
                    None => Ty::Dynamic,
                }
            }
            Expr::While { cond, body, .. } => {
                self.check_expr(cond, None);
                self.check_expr(body, None);
                Ty::Dynamic
            }
            Expr::For {
                var, iter, body, ..
            } => {
                self.check_expr(iter, None);
                self.env.push_scope();
                // FOR 变量的元素类型要等「可迭代协议」落地才推得出来,
                // A3 一律 Dynamic。
                self.env.bind(var.clone(), Ty::Dynamic);
                self.check_expr(body, None);
                self.env.pop_scope();
                Ty::Dynamic
            }
            Expr::Return { value, .. } => {
                let declared = self.returns.last().cloned();
                if let Some(v) = value {
                    let span = expr_span(expr);
                    let actual = self.check_expr(v, declared.as_ref());
                    if let Some(d) = &declared {
                        self.require(
                            d,
                            &actual,
                            |expected, found| TypeDiagKind::ReturnMismatch { expected, found },
                            &span,
                        );
                    }
                }
                Ty::Dynamic
            }
            Expr::Break { .. } | Expr::Continue { .. } => Ty::Dynamic,
            Expr::Fun {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                let param_tys: Vec<Ty> = params.iter().map(param_type).collect();
                let declared_ret = return_type.as_ref().map(|a| Ty::from_type_expr(&a.expr));
                let fun_ty = Ty::Fun {
                    params: param_tys.clone(),
                    ret: Box::new(declared_ret.clone().unwrap_or(Ty::Dynamic)),
                };
                // 具名函数先把签名绑进**外层**作用域,函数体里才能自递归。
                if let Some(n) = name {
                    self.env.bind(n.clone(), fun_ty.clone());
                }
                self.env.push_scope();
                for (p, t) in params.iter().zip(param_tys) {
                    self.env.bind(p.name.clone(), t);
                }
                // 无返回注解时压入 Dynamic:`RETURN` 自然不会报。
                self.returns
                    .push(declared_ret.clone().unwrap_or(Ty::Dynamic));
                let body_ty = self.check_expr(body, declared_ret.as_ref());
                // 块的**尾表达式**就是返回值(spec v0.9 §4.4:括号序列的值
                // 即最后一个表达式的值),只查显式 `RETURN` 会漏掉
                // `FUN((a): STRING, 42)` 这种最常见的写法。
                if let Some(d) = &declared_ret {
                    let span = match body.as_ref() {
                        Expr::Block { span, .. } => span.clone(),
                        other => expr_span(other),
                    };
                    self.require(
                        d,
                        &body_ty,
                        |expected, found| TypeDiagKind::ReturnMismatch { expected, found },
                        &span,
                    );
                }
                self.returns.pop();
                self.env.pop_scope();
                fun_ty
            }
            Expr::Ok { value, .. } => {
                let t = self.check_expr(value, None);
                Ty::Result(Box::new(t), Box::new(Ty::Dynamic))
            }
            Expr::Err { value, .. } => {
                let t = self.check_expr(value, None);
                Ty::Result(Box::new(Ty::Dynamic), Box::new(t))
            }
            Expr::Panic { value, .. } | Expr::Try { value, .. } => {
                self.check_expr(value, None);
                Ty::Dynamic
            }
            Expr::IsOk { value, .. } | Expr::IsErr { value, .. } => {
                self.check_expr(value, None);
                Ty::Boolean
            }
            Expr::OrDie { value, default, .. } => {
                let inner = self.check_expr(value, None);
                self.check_expr(default, None);
                // OR_DIE 拆封:OK 变体脱掉 Dynamic 错误侧。
                match inner {
                    Ty::Result(t, e) if e.is_dynamic() => *t,
                    other => other,
                }
            }
            Expr::Match {
                value,
                clauses,
                default,
                ..
            } => {
                let scrutinee = self.check_expr(value, None);
                let mut unified: Option<Ty> = None;
                for clause in clauses {
                    // 每个子句的模式绑定在自己的作用域里(spec §7.6)。
                    self.env.push_scope();
                    self.bind_pattern(&clause.pattern, &scrutinee);
                    let t = self.check_expr(&clause.body, expected);
                    self.env.pop_scope();
                    unified = unify(unified, t);
                }
                let d = self.check_expr(default, expected);
                match unified {
                    Some(u) if u == d => u,
                    _ => Ty::Dynamic,
                }
            }
            // IMPORT / EXPORT 的类型契约归 C1(Step 6 的模块签名),
            // A3 不预判。
            Expr::Import { .. } | Expr::Export { .. } => Ty::Dynamic,
        }
    }

    fn check_literal(&mut self, lit: &Literal, expected: Option<&Ty>) -> Ty {
        match lit {
            // 期望类型向下传播的字面量一级:INTEGER 字面量喂 FLOAT 时
            // 直接定为 FLOAT,避免「1 传给 (f: FLOAT)」被误判为收窄。
            Literal::Integer(_) => match expected {
                Some(Ty::Float) => Ty::Float,
                _ => Ty::Integer,
            },
            Literal::Float(_) => Ty::Float,
            Literal::String(_) => Ty::String,
            Literal::Boolean(_) => Ty::Boolean,
            Literal::Null => Ty::Null,
            Literal::Interpolated(parts) => {
                // `${...}` 内是完整表达式,必须继续遍历 —— 插值里藏一个
                // 注解失配不能被漏掉。
                for part in parts {
                    if let StrPart::Expr(inner) = part {
                        self.check_expr(inner.as_ref(), Some(&Ty::String));
                    }
                }
                Ty::String
            }
        }
    }

    fn check_call(&mut self, name: &str, args: &[Expr], span: &Span) -> Ty {
        // 实参先各自推导(无期望类型:实参之间的期望传播属 A2′)。
        let arg_tys: Vec<Ty> = args.iter().map(|a| self.check_expr(a, None)).collect();

        // 只有**本模块内可见、且带注解**的函数才有签名。内建建与未知名字
        // 一律 Dynamic —— A6′ 之前无从得知内建返回类型。
        let Some(Ty::Fun { params, ret }) = self.env.lookup(name).cloned() else {
            return Ty::Dynamic;
        };

        if params.len() != arg_tys.len() {
            self.report(
                TypeDiagKind::CallArityMismatch {
                    expected: params.len(),
                    found: arg_tys.len(),
                },
                span,
            );
            return *ret;
        }
        for (position, (expected_t, found_t)) in params.iter().zip(&arg_tys).enumerate() {
            if !found_t.is_assignable_to(expected_t) {
                self.report(
                    TypeDiagKind::CallArgMismatch {
                        position,
                        expected: expected_t.clone(),
                        found: found_t.clone(),
                    },
                    span,
                );
            }
        }
        *ret
    }

    /// 把模式里的名字绑进当前作用域。`ty` 是被匹配值的类型。
    fn bind_pattern(&mut self, pattern: &Pattern, ty: &Ty) {
        match pattern {
            Pattern::Ident(name, _) => self.env.bind(name.clone(), ty.clone()),
            Pattern::Wildcard(_) | Pattern::Literal(..) => {}
            Pattern::Array(items, rest, _) => {
                let elem = match ty {
                    Ty::Array(e) => (**e).clone(),
                    _ => Ty::Dynamic,
                };
                for item in items {
                    self.bind_pattern(item, &elem);
                }
                if let Some(r) = rest {
                    self.bind_pattern(r, &Ty::Array(Box::new(elem)));
                }
            }
            Pattern::Dict(entries, _) => {
                // spec §2.1:键限 STRING / INTEGER。键是这两种字面量时
                // 值类型可知,否则 Dynamic。
                let value_ty = match ty {
                    Ty::Dict(_, v) => (**v).clone(),
                    _ => Ty::Dynamic,
                };
                for (key_expr, sub) in entries {
                    self.check_expr(key_expr, None);
                    let v = match key_expr {
                        Expr::Literal(Literal::String(_) | Literal::Integer(_), _) => {
                            value_ty.clone()
                        }
                        _ => Ty::Dynamic,
                    };
                    self.bind_pattern(sub, &v);
                }
            }
            Pattern::Constructor { name, inner, .. } => {
                let t = match ty {
                    Ty::Result(ok_t, err_t) => {
                        if name == "OK" {
                            (**ok_t).clone()
                        } else {
                            (**err_t).clone()
                        }
                    }
                    _ => Ty::Dynamic,
                };
                self.bind_pattern(inner, &t);
            }
        }
    }
}

/// 逐位置合流:一致才保留,否则退回 `Dynamic`。
fn unify(prev: Option<Ty>, next: Ty) -> Option<Ty> {
    Some(match prev {
        None => next,
        Some(p) if p == next => p,
        Some(_) => Ty::Dynamic,
    })
}

/// 形参的类型:有注解用注解,否则 `Dynamic`(未标注即回退)。
fn param_type(p: &FunParam) -> Ty {
    p.type_annotation
        .as_ref()
        .map_or(Ty::Dynamic, |a| Ty::from_type_expr(&a.expr))
}

/// 表达式节点的 span(AST 里每个变体都带一个)。
fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Literal(_, s)
        | Expr::Var(_, s)
        | Expr::Call { span: s, .. }
        | Expr::Block { span: s, .. }
        | Expr::Array { span: s, .. }
        | Expr::Dict { span: s, .. }
        | Expr::Let { span: s, .. }
        | Expr::LetPattern { span: s, .. }
        | Expr::If { span: s, .. }
        | Expr::While { span: s, .. }
        | Expr::For { span: s, .. }
        | Expr::Return { span: s, .. }
        | Expr::Break { span: s }
        | Expr::Continue { span: s }
        | Expr::Fun { span: s, .. }
        | Expr::Ok { span: s, .. }
        | Expr::Err { span: s, .. }
        | Expr::Panic { span: s, .. }
        | Expr::Try { span: s, .. }
        | Expr::IsOk { span: s, .. }
        | Expr::IsErr { span: s, .. }
        | Expr::OrDie { span: s, .. }
        | Expr::Match { span: s, .. }
        | Expr::Import { span: s, .. }
        | Expr::Export { span: s, .. } => s.clone(),
    }
}

/// 值表达式的 span(用于 `LET([a,b], v)` 注解失配的定位)。
fn actual_span(e: &Expr) -> Span {
    expr_span(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_parser::parse;

    fn check(src: &str) -> Vec<TypeDiag> {
        let ast = parse(src, "check.wll").expect("fixture must parse");
        check_program(&ast)
    }

    fn codes(src: &str) -> Vec<String> {
        check(src)
            .iter()
            .map(|d| match d.kind.codes() {
                Some((e, _)) => e.as_str().to_string(),
                None => "<unmapped>".to_string(),
            })
            .collect()
    }

    // ---- 默认关闭时零诊断(锁测试:default_off_zero_diag 的库侧) ----

    #[test]
    fn unannotated_program_produces_no_diagnostics() {
        // A3 的核心验收:未标注程序开启后也不误报。Dynamic 顶/底元保证
        // 这一点,这里锁住它。
        let src = r#"
        LET(x, 1);
        LET(y, "s");
        LET(add, FUN((a, b), +(a, b)));
        LET(z, add(x, y));
        PRINT(z);
        "#;
        assert_eq!(check(src), vec![]);
    }

    #[test]
    fn builtin_calls_are_not_checked_without_a6prime() {
        // 纪律 2:A6′ 之前内建返回类型不可知 -> Dynamic -> 不报。
        let src = r#"
        LET(x: INTEGER, INT("42"));
        LET(s: STRING, STR(42));
        "#;
        assert_eq!(check(src), vec![]);
    }

    // ---- E0110:注解失配 ----

    #[test]
    fn annotation_mismatch_is_reported() {
        let diags = check(r#"LET(x: INTEGER, "not an int");"#);
        assert_eq!(codes(r#"LET(x: INTEGER, "not an int");"#), ["E0110"]);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message(),
            "annotation mismatch: expected `INTEGER`, found `STRING`"
        );
    }

    #[test]
    fn integer_widening_to_float_is_accepted() {
        // ADR-0010 的 silent upcast:静态层必须一致。
        assert_eq!(check("LET(x: FLOAT, 1);"), vec![]);
        // 收窄方向仍然拒绝。
        assert_eq!(codes("LET(x: INTEGER, 1.5);"), ["E0110"]);
    }

    #[test]
    fn nested_container_mismatch_is_reported() {
        assert_eq!(codes(r#"LET(x: ARRAY[INTEGER], ["a"]);"#), ["E0110"]);
        assert_eq!(
            codes(r#"LET(x: DICT[STRING, INTEGER], [1, 2]);"#),
            ["E0110"]
        );
        // 元素类型一致则通过。
        assert_eq!(check(r#"LET(x: ARRAY[INTEGER], [1, 2, 3]);"#), vec![]);
    }

    // ---- E0111:调用失配 ----

    #[test]
    fn call_arg_mismatch_is_reported() {
        let src = r#"
        LET(f, FUN((a: INTEGER), a));
        f("nope");
        "#;
        assert_eq!(codes(src), ["E0111"]);
        assert_eq!(
            check(src)[0].message(),
            "call argument 0 mismatch: expected `INTEGER`, found `STRING`"
        );
    }

    #[test]
    fn call_arity_mismatch_is_reported() {
        let src = r#"
        LET(f, FUN((a: INTEGER, b: INTEGER), a));
        f(1);
        "#;
        assert_eq!(codes(src), ["E0111"]);
        assert_eq!(
            check(src)[0].message(),
            "call arity mismatch: expected 2 argument(s), found 1"
        );
    }

    #[test]
    fn never_called_annotated_function_is_still_checked() {
        // 静态检查的全部意义:没被调用过的函数,签名错误也必须报出来
        // ——这正是运行时 `E0033` 永远发现不了的情况。
        assert_eq!(
            codes("LET(f, FUN((a: INTEGER) : INTEGER, \"wrong\"));"),
            ["E0112"]
        );
        // 没有返回注解就不判返回 —— 未标注即回退 Dynamic。
        assert_eq!(check(r#"LET(f, FUN((a: INTEGER), "wrong"));"#), vec![]);
    }

    // ---- E0112:返回类型失配 ----

    #[test]
    fn return_mismatch_is_reported() {
        let src = r#"
        LET(f, FUN((a): STRING, 42));
        "#;
        assert_eq!(codes(src), ["E0112"]);
        assert_eq!(
            check(src)[0].message(),
            "return type mismatch: expected `STRING`, found `INTEGER`"
        );
    }

    #[test]
    fn nested_function_return_is_not_judged_by_outer_signature() {
        // returns 栈的作用:内层 RETURN 不受外层返回注解误伤。
        let src = concat!(
            "LET(outer, FUN(() : STRING, ",
            "LET(inner, FUN(() : INTEGER, 7)); ",
            "IF(==(inner(), 7), \"ok\", \"no\");",
            "));"
        );
        assert_eq!(check(src), vec![]);
    }

    // ---- 不重复报告未定义名字(纪律 3) ----

    #[test]
    fn undefined_names_are_left_to_the_parser_lint_pass() {
        assert_eq!(check("PRINT(nonexistent);"), vec![]);
    }

    // ---- 作用域正确性 ----

    #[test]
    fn shadowed_names_resolve_to_the_innermost_binding() {
        let src = r#"
        LET(f, FUN((a: INTEGER),
            LET(g, FUN((b: STRING), b));
            g(1)
        ));
        f(2);
        "#;
        assert_eq!(codes(src), ["E0111"]);
    }

    #[test]
    fn recursive_named_function_binds_its_own_signature() {
        let src = concat!(
            "LET(fact, FUN((n: INTEGER) : INTEGER, ",
            "IF(<(n, 1), 1, *(n, fact(-(n, 1))));",
            "));"
        );
        assert_eq!(check(src), vec![]);
    }

    #[test]
    fn block_tail_expression_is_the_return_value() {
        // spec v0.9 §4.4:括号序列的值 = 最后一个表达式的值。
        // 尾表达式也必须过返回注解这一关。
        // 尾表达式是 "ok" -> String,与注解一致,不报。
        assert_eq!(check("LET(f, FUN(() : STRING, (1; \"ok\";)));"), vec![]);
        // 尾表达式是 42 -> Integer,失配。
        assert_eq!(codes("LET(f, FUN(() : STRING, (1; 42);));"), ["E0112"]);
    }

    #[test]
    fn explicit_return_and_tail_are_not_double_reported() {
        // 显式 `RETURN` 走 returns 栈,尾表达式是 RETURN 时值为 Dynamic,
        // 两条路径只应报一次。
        let src = "LET(f, FUN(() : STRING, RETURN(42)));";
        assert_eq!(codes(src), ["E0112"]);
    }

    #[test]
    fn diagnostics_carry_the_source_location() {
        let diags = check("LET(x, 1);\nLET(y: INTEGER, \"s\");");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].span.file, "check.wll");
        assert_eq!(diags[0].span.line_start, 2);
    }

    #[test]
    fn interpolation_is_traversed() {
        // `${...}` 内藏的注解失配不能被漏掉。
        // 插值里嵌一个调用,实参失配必须被报出来。
        // (不能写 `${g("bad")}` —— 嵌套双引号会打断 WLWL 的词法。)
        let src = concat!(
            "LET(bad, \"oops\"); ",
            "LET(g, FUN((a: INTEGER), a)); ",
            "LET(s, \"value: ${g(bad)}\");"
        );
        assert_eq!(codes(src), ["E0111"]);
    }

    // ---- 码映射(D-1) ----

    #[test]
    fn every_emitted_diag_maps_to_a_code() {
        let src = r#"
        LET(a: INTEGER, "s");
        LET(f, FUN((x: INTEGER), x));
        f("s");
        LET(g, FUN((y): STRING, 1));
        "#;
        let diags = check(src);
        assert_eq!(diags.len(), 3);
        for d in &diags {
            assert!(d.to_error().is_some(), "unmapped kind: {:?}", d.kind);
            assert!(d.to_warning().is_some());
        }
    }

    #[test]
    fn warn_and_error_档_use_different_codes() {
        let d = check(r#"LET(x: INTEGER, "s");"#).remove(0);
        assert_eq!(d.to_error().expect("error code").code.as_str(), "E0110");
        let w = d.to_warning().expect("warn code");
        assert_eq!(w.code.as_str(), "W0110");
        assert!(w.code.is_warning());
        assert!(!d.to_error().expect("error").code.is_warning());
    }

    #[test]
    fn unmapped_kinds_decline_to_invent_a_code() {
        // A4 未落地前不预分配码号。
        let d = TypeDiag::synthetic(TypeDiagKind::TypeArityMismatch {
            name: "DICT".into(),
            expected: 2,
            found: 1,
        });
        assert!(d.to_error().is_none());
        assert!(d.to_warning().is_none());
        // UndefinedName 同理(parser lint 的地盘)。
        let d = TypeDiag::synthetic(TypeDiagKind::UndefinedName { name: "x".into() });
        assert!(d.to_error().is_none());
    }
}
