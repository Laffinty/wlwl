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

use std::collections::HashMap;

use wlwl_ast::{Expr, FunParam, Literal, Pattern, Span, StrPart};

use crate::diag::{TypeDiag, TypeDiagKind};
use crate::env::TypeEnv;
use crate::matchx;
use crate::ty::Ty;

/// 对一棵已解析的 AST 跑静态 pass,返回全部诊断(按发现顺序)。
///
/// 调用方在 `gradual_typing = "off"` 时**不应调用本函数**(那才是
/// 「零开销」的保证);`off` 档必须整条静态链路都不执行。
pub fn check_program(expr: &Expr) -> Vec<TypeDiag> {
    check_program_with_builtins(expr, &HashMap::new())
}

/// 同 [`check_program`],但带一张**内建签名表**(Step 5 · A6′)。
///
/// 这条入口存在的理由是**分层**:内建签名住在 `wlwl-eval` 的注册表里,
/// 而 ADR-0020 Decision 1 规定 `wlwl-eval` **不依赖** `wlwl-types`、
/// 反之亦然。所以映射由同时依赖两者的 `wlwl-cli` 那一层完成,本 crate
/// 只接受**已经转成 `Ty` 的**表。
///
/// 首批(A6′)未结构化的 50 条**不在表里** —— 它们等价于「查不到签名」,
/// 静态层落 `Dynamic`、**不产生诊断**。
pub fn check_program_with_builtins(expr: &Expr, builtins: &HashMap<String, Ty>) -> Vec<TypeDiag> {
    check_program_detailed(expr, builtins).diags
}

/// 一个**根作用域**绑定的静态类型 —— 模块契约检查(Step 6)的输入。
///
/// 只收根作用域(`TypeEnv::depth() == 1`),也就是模块顶层能看到的那些
/// 名字:函数体内的局部变量、模式匹配的子句绑定一律不进。理由是模块
/// 契约只对**公开面**说话,而公开面只能是顶层绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredBinding {
    /// 绑定名
    pub name: String,
    /// 静态类型:有注解取注解,无注解取推断结果(常常是 `Dynamic`)
    pub ty: Ty,
    /// 定位:有类型注解时指向注解,否则指向绑定表达式
    pub span: Span,
}

/// [`check_program_detailed`] 的产物。
#[derive(Debug, Clone, Default)]
pub struct CheckOutput {
    /// 全部类型诊断(按发现顺序)
    pub diags: Vec<TypeDiag>,
    /// 根作用域绑定的静态类型,同名取最后一次绑定(与重绑定语义一致)
    pub declared: Vec<DeclaredBinding>,
}

/// [`check_program`] 的完整版:除了诊断,还交出根作用域的绑定类型。
///
/// Step 6 的模块契约检查需要后者 —— `E0115`(签名类型 vs 实现注解)要比的
/// 就是这份表。把它做成同一次遍历的副产品而不是另写一遍顶层扫描,是为了
/// 让「推断出来的类型」只有**一个**来源:两处各自推断必然漂移。
pub fn check_program_detailed(expr: &Expr, builtins: &HashMap<String, Ty>) -> CheckOutput {
    check_program_with_options(
        expr,
        &CheckOptions {
            builtins,
            // Step 8 之前 MATCH 检查不存在,所以历史调用方式等价于「开着」。
            match_exhaustiveness: true,
        },
    )
}

/// 一次静态遍历的开关(Step 8 起静态层有**两个**子系统,各管各的档位)。
#[derive(Debug, Clone, Copy)]
pub struct CheckOptions<'a> {
    /// 内建签名表(Step 5 · A6′)。空表 = 内建返回类型不可知 → 落
    /// `Dynamic`,不产生诊断。
    pub builtins: &'a HashMap<String, Ty>,
    /// Step 8:是否跑 MATCH 穷尽性 / 可达性检查。`false` = **完全不跑**
    /// —— 这是「零开销」在 MATCH 侧的落点。
    pub match_exhaustiveness: bool,
}

/// [`check_program_detailed`] 的开关版。
pub fn check_program_with_options(expr: &Expr, opts: &CheckOptions<'_>) -> CheckOutput {
    let mut checker = Checker::new(opts.builtins);
    checker.match_exhaustiveness = opts.match_exhaustiveness;
    checker.check_expr(expr, None);
    CheckOutput {
        diags: checker.diags,
        declared: dedupe_last_binding(checker.declared),
    }
}

/// 同名绑定取最后一次:模块顶层 `LET(f, ...)` 覆盖前者,与 §3.2 重绑定
/// 语义一致,也与 [`TypeEnv`] 的 `bind` 覆盖行为一致。顺序保持首次出现
/// 的位置,便于诊断输出稳定。
fn dedupe_last_binding(bindings: Vec<DeclaredBinding>) -> Vec<DeclaredBinding> {
    let mut out: Vec<DeclaredBinding> = Vec::with_capacity(bindings.len());
    for b in bindings {
        match out.iter_mut().find(|p| p.name == b.name) {
            Some(prev) => *prev = b,
            None => out.push(b),
        }
    }
    out
}

struct Checker<'a> {
    env: TypeEnv,
    diags: Vec<TypeDiag>,
    /// 当前函数的返回值注解栈。最内层生效 —— 嵌套函数的 `RETURN` 不会
    /// 被外层函数的注解误伤。
    returns: Vec<Ty>,
    /// 内建签名表(Step 5 · A6′)。查不到 = 没有签名。
    builtins: &'a HashMap<String, Ty>,
    /// 根作用域绑定(Step 6)。见 [`DeclaredBinding`]。
    declared: Vec<DeclaredBinding>,
    /// Step 8:是否跑 MATCH 检查。关掉时那条 pass **完全不执行**。
    match_exhaustiveness: bool,
    /// Step 9:本次调用积累的类型变量绑定(形参 → 实参),用于把返回类型
    /// 里的变量代入。**每次调用前清空** —— 绑定只在一个调用的实参之间成立。
    pending_substitutions: Vec<(String, Ty)>,
}

impl<'a> Checker<'a> {
    fn new(builtins: &'a HashMap<String, Ty>) -> Checker<'a> {
        Checker {
            env: TypeEnv::new(),
            diags: Vec::new(),
            returns: Vec::new(),
            builtins,
            declared: Vec::new(),
            // 由 `check_program_with_options` 覆写;默认值等于「关」——
            // 构造器本身不该顺手把一条新 pass 打开。
            match_exhaustiveness: false,
            pending_substitutions: Vec::new(),
        }
    }

    /// 记录一个根作用域绑定。非根作用域静默忽略 —— 契约只管公开面。
    fn record(&mut self, name: &str, ty: Ty, span: Span) {
        if self.env.depth() == 1 {
            self.declared.push(DeclaredBinding {
                name: name.to_string(),
                ty,
                span,
            });
        }
    }

    fn report(&mut self, kind: TypeDiagKind, span: &Span) {
        self.diags.push(TypeDiag::new(kind, span.clone()));
    }

    /// 「实际类型 vs 期望类型」的统一判定。`Dynamic` 在任一侧都不报 ——
    /// 这是「不误报优先」纪律的落点。
    ///
    /// 判定走 [`Ty::satisfies_annotation`] 而非 `is_assignable_to`:前者
    /// 额外承认 `OPTION[T]` 注解糖的运行时见证(A4)。
    fn require(
        &mut self,
        expected: &Ty,
        found: &Ty,
        kind_of: impl Fn(Ty, Ty) -> TypeDiagKind,
        span: &Span,
    ) {
        if !found.satisfies_annotation(expected) {
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
            // A2′:把**期望元素类型**向下推进容器字面量的每个元素,并逐个校验。
            //
            // 无期望类型时退回 Step 2 的「统一」推断(全同才保留,含 Dynamic
            // 或不一致即 Dynamic)。有期望类型时**逐元素报**,而不是等整体失配
            // —— `LET(x: ARRAY[INTEGER], [1, "a"])` 应当指出是 `"a"` 那一项,
            // 整体比对只能给一个笼统的 `ARRAY[INTEGER]` vs `ARRAY[DYNAMIC]`。
            //
            // 校验通过后直接返回**声明的元素类型**:既让整体检查自动通过
            // (元素级诊断已经覆盖了整体失配,避免同一问题报两次),又让推断
            // 结果比 `Dynamic` 更精确 —— 这正是 A2′ 的主要收益。
            Expr::Array { items, .. } => {
                let elem_expected = match expected {
                    Some(Ty::Array(t)) => Some(t.as_ref()),
                    _ => None,
                };
                let mut unified: Option<Ty> = None;
                for item in items {
                    let t = self.check_expr(item, elem_expected);
                    if let Some(want) = elem_expected {
                        self.require(
                            want,
                            &t,
                            |expected, found| TypeDiagKind::AnnotationMismatch { expected, found },
                            &expr_span(item),
                        );
                    }
                    unified = Some(match unified {
                        None => t,
                        Some(prev) => Ty::lub(&prev, &t),
                    });
                }
                Ty::Array(Box::new(
                    elem_expected.map_or_else(|| unified.unwrap_or(Ty::Dynamic), |e| e.clone()),
                ))
            }
            // A2′:**值**侧收到期望值类型并逐条校验。**键**侧不下推期望类型,
            // 但键类型仍按字面量原样推断后参与整体比较 ——
            // 下推会让字面量被错误地按期望定型,而完全不检查又会漏掉
            // 「键声明 STRING 却是整数」这类真实失配。折中做法:
            // 键不接收期望(保持 `1` 就是 INTEGER),但**返回推断出的键类型**,
            // 让外层的整体比较照常把关。
            Expr::Dict { entries, .. } => {
                let val_expected = match expected {
                    Some(Ty::Dict(_, v)) => Some(v.as_ref()),
                    _ => None,
                };
                let mut keys: Option<Ty> = None;
                let mut vals: Option<Ty> = None;
                for (k, v) in entries {
                    let kt = self.check_expr(k, None);
                    let vt = self.check_expr(v, val_expected);
                    if let Some(want) = val_expected {
                        self.require(
                            want,
                            &vt,
                            |expected, found| TypeDiagKind::AnnotationMismatch { expected, found },
                            &expr_span(v),
                        );
                    }
                    keys = Some(match keys {
                        None => kt,
                        Some(prev) => Ty::lub(&prev, &kt),
                    });
                    vals = Some(match vals {
                        None => vt,
                        Some(prev) => Ty::lub(&prev, &vt),
                    });
                }
                Ty::Dict(
                    Box::new(keys.unwrap_or(Ty::Dynamic)),
                    Box::new(
                        val_expected.map_or_else(|| vals.unwrap_or(Ty::Dynamic), |v| v.clone()),
                    ),
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
                self.record(
                    name,
                    bound.clone(),
                    type_annotation
                        .as_ref()
                        .map(|a| a.span.clone())
                        .unwrap_or_else(|| span.clone()),
                );
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
                        // A2′:取最小公共上界,而不是「相同才保留」。
                        // `IF(c, 1, 2.5)` 从此推断为 `FLOAT` 而非 `Dynamic`。
                        Ty::lub(&then_ty, &else_ty)
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
                    self.record(
                        n,
                        fun_ty.clone(),
                        return_type
                            .as_ref()
                            .map(|a| a.span.clone())
                            .unwrap_or_else(|| expr_span(expr)),
                    );
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
            Expr::Panic { value, .. } => {
                self.check_expr(value, None);
                Ty::Dynamic
            }
            // `TRY(x)`(spec §8.3):`OK(v)` 时取 `v`,`ERR` 时等价于
            // `RETURN(err)`。所以静态类型就是**成功侧载荷**。
            // 错误侧具体化时(`RESULT[T, STRING]`)不给结论 —— `OK`/`ERR`
            // 是运行期分支,判断哪一支属于类型收窄(A5,已推迟)。
            Expr::Try { value, .. } => {
                let inner = self.check_expr(value, None);
                inner.unwrap_result().cloned().unwrap_or(Ty::Dynamic)
            }
            Expr::IsOk { value, .. } | Expr::IsErr { value, .. } => {
                self.check_expr(value, None);
                Ty::Boolean
            }
            Expr::OrDie { value, default, .. } => {
                let inner = self.check_expr(value, None);
                self.check_expr(default, None);
                // 同样只在错误侧未知时解包。
                inner.unwrap_result().cloned().unwrap_or(Ty::Dynamic)
            }
            Expr::Match {
                value,
                clauses,
                default,
                default_synthetic,
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
                    unified = Some(match unified {
                        None => t,
                        Some(prev) => Ty::lub(&prev, &t),
                    });
                }
                let d = self.check_expr(default, expected);
                // A2′:所有子句体与 default 一起取 lub。default 参与合流是
                // 因为它在无子句命中时就是结果值,漏掉它会让「只有 default
                // 有类型」的情形退化成 Dynamic。
                let unified = Ty::lub(&unified.unwrap_or(Ty::Dynamic), &d);
                // [v0.10 Step 8 / P1-1] 穷尽性 / 不可达子句。类型已经推完,
                // 所以这一步不碰类型环境,纯只读判定。开关关着时**不调用**
                // —— 与 `gradual_typing = "off"` 同一套「不跑就是零开销」。
                if self.match_exhaustiveness {
                    let spans: Vec<Span> = clauses.iter().map(|c| c.span.clone()).collect();
                    self.diags.extend(matchx::to_diags(
                        &matchx::check_match(&scrutinee, clauses, *default_synthetic),
                        &expr_span(expr),
                        &spans,
                    ));
                }
                unified
            }
            // IMPORT / EXPORT / SEALED 的**类型**契约归 C1/C2(Step 6 的
            // 模块签名与密封面),但它们的名字面契约不需要类型:
            // 导出集与声明集的差集是纯名字比较,走 [`crate::sig`]。
            // 这里一律落 `Dynamic`,不预判。
            Expr::Import { .. } | Expr::Export { .. } | Expr::Sealed { .. } => Ty::Dynamic,
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
        // `RESULT` 解包族(spec §8.3 消费者注册表)。`IS_OK` / `IS_ERR` /
        // `OR_DIE` / `TRY` 是 **lexer 关键字**,由 parser 降级成 `Expr::*`,已在
        // 上面按节点处理;只有 `UNWRAP` / `UNWRAP_OR` / `ERR_PAYLOAD`
        // 走通用 `Expr::Call` 路径(参见
        // `wlwl-eval/src/lib.rs:5459-5467` 的同名注记),所以必须在
        // 这里特判 —— 否则它们的返回类型永远是
        // `Dynamic`,`RESULT` 的注解糖语义就丢了。
        //
        // 这些是**全局内建**,签名不在类型环境里,故实参没有期望类型可下推。
        if matches!(name, "UNWRAP" | "UNWRAP_OR" | "ERR_PAYLOAD") {
            let arg_tys: Vec<Ty> = args.iter().map(|a| self.check_expr(a, None)).collect();
            return match (name, arg_tys.len()) {
                ("UNWRAP", 1) | ("UNWRAP_OR", 2) => unwrap_side(&arg_tys[0], Side::Ok),
                ("ERR_PAYLOAD", 1) => unwrap_side(&arg_tys[0], Side::Err),
                // 元数不对:交回运行时报(它的 arity 诊断更准,
                // 且不在 A4 范围)。
                _ => Ty::Dynamic,
            };
        }

        // 签名解析:**局部作用域优先**,其次内建签名表(A6′),最后 Dynamic。
        //
        // 遮蔽优先是必须的:spec §3.5 的 `allow_builtin_shadow` 允许用户
        // 重新绑定内建名,此时内建签名**不适用**,否则会对用户的定义报错。
        if let Some(local) = self.env.lookup(name).cloned() {
            let Ty::Fun { params, ret } = local else {
                // 局部绑定不是函数(普通变量被当函数调)——交给运行时报。
                return Ty::Dynamic;
            };
            return self.check_call_with_sig(args, span, &params, *ret, true);
        }
        let Some(builtin) = self.builtins.get(name).cloned() else {
            // 既无局部签名也无内建签名(首批未覆盖的 50 条 / 未知名字)。
            return Ty::Dynamic;
        };
        let Ty::Fun { params, ret } = builtin else {
            return Ty::Dynamic;
        };
        // 内建首批只给返回类型,`params` 为空 → 逐位都拿不到期望,
        // 实参各自独立推导,元数也不检查(可选形参与变长实参会让精确
        // 元数必然误报,见 `wlwl-eval::registry::BuiltinSig::params`)。
        self.check_call_with_sig(args, span, &params, *ret, false)
    }

    /// 拿到签名后的共用路径:下推期望类型 → 推导实参 → 比对。
    ///
    /// `check_arity` 只对**局部**签名为真 —— 内建首批的形参未知,
    /// 元数检查会误报(见调用点)。
    fn check_call_with_sig(
        &mut self,
        args: &[Expr],
        span: &Span,
        params: &[Ty],
        ret: Ty,
        check_arity: bool,
    ) -> Ty {
        // A2′:实参按签名的形参类型**逐位下推**期望类型。必须在推导
        // 实参**之前**拿到签名 —— 字面量与容器字面量只有拿到期望类型才会
        // 按期望定型。元数不足时 `params.get(i)` 给出 `None`,不会越界。
        //
        // [Step 9] 变量绑定是**每次调用独立**的:先清空上一次残留,否则
        // 两次无关的调用会把变量串起来。
        self.pending_substitutions.clear();
        let arg_tys: Vec<Ty> = args
            .iter()
            .enumerate()
            .map(|(i, a)| self.check_expr(a, params.get(i)))
            .collect();

        if check_arity && params.len() != arg_tys.len() {
            self.report(
                TypeDiagKind::CallArityMismatch {
                    expected: params.len(),
                    found: arg_tys.len(),
                },
                span,
            );
            return ret;
        }
        for (position, (expected_t, found_t)) in params.iter().zip(&arg_tys).enumerate() {
            // [v0.10 Step 9 / P1-2] 泛型实例化:声明类型里带类型变量时,
            // 先按实参把变量绑定起来(顺带验约束),再判实参填不填得进。
            //
            // 报出来的 `expected` 是**未代入的**声明类型 —— 这样消息里留着
            // `T: Comparable`,而不是被抹成实参类型。约束不满足时它同样
            // 走这条既有的调用失配路径:**本 Step 不新增错误码**,约束不
            // 满足就是「实参不符合被调方声明的形参类型」。
            if expected_t.vars().is_empty() {
                if !found_t.satisfies_annotation(expected_t) {
                    self.report(
                        TypeDiagKind::CallArgMismatch {
                            position,
                            expected: expected_t.clone(),
                            found: found_t.clone(),
                        },
                        span,
                    );
                }
                continue;
            }
            let mut bindings: Vec<(String, Ty)> = Vec::new();
            let ok = crate::ty::instantiate(expected_t, found_t, &mut bindings);
            if !ok {
                self.report(
                    TypeDiagKind::CallArgMismatch {
                        position,
                        expected: expected_t.clone(),
                        found: found_t.clone(),
                    },
                    span,
                );
            }
            // 记下这次实例化,返回类型要用它代入(见下面)。
            self.pending_substitutions.extend(bindings);
        }
        crate::ty::substitute(&ret, &self.pending_substitutions)
    }

    /// 把模式里的名字绑进当前作用域。`ty` 是被匹配值的类型。
    fn bind_pattern(&mut self, pattern: &Pattern, ty: &Ty) {
        match pattern {
            Pattern::Ident(name, span) => {
                self.env.bind(name.clone(), ty.clone());
                self.record(name, ty.clone(), span.clone());
            }
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

/// 取 `RESULT` 的哪一侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// `OK(v)` 的载荷 —— `UNWRAP` / `UNWRAP_OR` / `TRY` / `OR_DIE` 的结果。
    Ok,
    /// `ERR(e)` 的载荷 —— `ERR_PAYLOAD` 的结果。
    Err,
}

/// 解包族的返回类型。
///
/// 只在**另一侧未知**（`Dynamic`）时给出结论:另一侧一旦
/// 具体化,`OK` / `ERR` 就是**运行期分支**,静态层无从知道
/// 实际走了哪一支(属于类型收窄,A5 已推迟)。给不出结论
/// 就回退 `Dynamic` → 不报,符合「不误报优先」。
fn unwrap_side(ty: &Ty, side: Side) -> Ty {
    match ty {
        Ty::Result(ok, err) => {
            let known = ok.is_dynamic() || err.is_dynamic();
            if !known {
                return Ty::Dynamic;
            }
            match side {
                Side::Ok if err.is_dynamic() => (**ok).clone(),
                Side::Err if ok.is_dynamic() => (**err).clone(),
                _ => Ty::Dynamic,
            }
        }
        other if other.is_dynamic() => Ty::Dynamic,
        // 非 `RESULT` 交给运行时报(`E0030`),静态层不重复报。
        _ => Ty::Dynamic,
    }
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
        | Expr::Export { span: s, .. }
        | Expr::Sealed { span: s, .. } => s.clone(),
    }
}

/// 值表达式的 span(用于 `LET([a,b], v)` 注解失配的定位)。
fn actual_span(e: &Expr) -> Span {
    expr_span(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_ast::TypeAnnotation;
    use wlwl_parser::parse;

    fn check(src: &str) -> Vec<TypeDiag> {
        let ast = parse(src, "check.wll").expect("fixture must parse");
        check_program(&ast)
    }

    /// 取一个形参上的类型注解。
    fn ann_of(annotation: &str) -> TypeAnnotation {
        let src = format!("FUN((x: {annotation}), x);");
        let expr = parse(&src, "check.wll").expect("annotation must parse");
        let Expr::Fun { params, .. } = expr else {
            panic!("expected a FUN expression");
        };
        params[0]
            .type_annotation
            .clone()
            .expect("param carries an annotation")
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

    // ---- A4:OPTION[T] 注解糖语义 ----

    #[test]
    fn option_accepts_its_two_runtime_witnesses() {
        // 规范端见证:`NULL`("就是没有")与 `RESULT`("或者是一个 T")。
        // spec §2.1 的 13 个运行时类型里没有 OPTION,所以 `Option<T>`
        // 必须有运行时见证,否则用户写不出、也判不了。
        assert_eq!(check("LET(x: OPTION[INTEGER], NULL);"), vec![]);
        assert_eq!(check("LET(x: OPTION[INTEGER], OK(1));"), vec![]);
        // 容器层上的 OPTION 也成立(递归使用一致规则)。
        assert_eq!(check("LET(x: OPTION[ARRAY[INTEGER]], OK([1, 2]));"), vec![]);
    }

    #[test]
    fn option_rejects_a_bare_t() {
        // 反向保护:若裸 `T` 也通过,OPTION 就退化成 T 的别名,
        // 失去全部意义。
        assert_eq!(codes("LET(x: OPTION[INTEGER], 1);"), ["E0110"]);
        assert_eq!(codes(r#"LET(x: OPTION[INTEGER], "s");"#), ["E0110"]);
        // OK 侧类型不匹配时仍报失配。
        assert_eq!(codes(r#"LET(x: OPTION[INTEGER], OK("s"));"#), ["E0110"]);
    }

    #[test]
    fn option_sugar_does_not_leak_into_other_annotations() {
        // `satisfies_annotation` 只多认 OPTION 一种糖,其余全部走
        // `is_assignable_to` 的结构规则 —— 两层不得混淆。
        // 空值就是不能填进 RESULT。
        assert_eq!(codes("LET(x: RESULT[INTEGER, STRING], NULL);"), ["E0110"]);
        // 基础类型之间的失配与 OPTION 无关,照报。
        assert_eq!(codes("LET(x: INTEGER, NULL);"), ["E0110"]);
        assert_eq!(codes(r#"LET(x: INTEGER, "s");"#), ["E0110"]);
    }

    // ---- A4:RESULT[T, E] 解包族 ----

    /// 一个错误侧未知的 `RESULT` 形参。
    /// 只能靠注解构造:`OK[e]` 的映射是 `Result(T, Dynamic)`,
    /// `ERR[e]` 是 `Result(Dynamic, E)`,源码里没有写法把**两侧都**
    /// 具体化的 RESULT —— 这也正是下面那条 deferral 的依据。
    const R_DYN: &str = "RESULT[INTEGER, DYNAMIC]";

    #[test]
    fn try_unwraps_the_ok_payload() {
        // spec §8.3:`TRY(x)` 在 `OK(v)` 时取 `v`。运行时
        // `Value::Ok(v) => Outcome::normal(*v)`(eval lib.rs:7301),静态层必须一致。
        let ok = format!("LET(f, FUN((r: {R_DYN}) : INTEGER, TRY(r)));");
        assert_eq!(check(&ok), vec![]);
        // 反证:改成 STRING 就必须报 E0112。如果 TRY 不解包
        // (落 Dynamic),这两条的行为会完全一样,差异本身就是
        // 解包生效的证明。
        let bad = format!("LET(f, FUN((r: {R_DYN}) : STRING, TRY(r)));");
        assert_eq!(codes(&bad), ["E0112"]);
    }

    #[test]
    fn unwrap_family_is_special_cased_through_the_call_path() {
        // UNWRAP / UNWRAP_OR / ERR_PAYLOAD 是**普通内建**(不是 lexer 关键字),
        // 必须在 check_call 特判才能推导出载荷类型。
        let ok_unwrap = format!("LET(f, FUN((r: {R_DYN}) : INTEGER, UNWRAP(r)));");
        assert_eq!(check(&ok_unwrap), vec![]);
        let ok_or = format!("LET(f, FUN((r: {R_DYN}) : INTEGER, UNWRAP_OR(r, 0)));");
        assert_eq!(check(&ok_or), vec![]);
        let err = r#"LET(f, FUN((r: RESULT[DYNAMIC, STRING]) : STRING, ERR_PAYLOAD(r)));"#;
        assert_eq!(check(err), vec![]);
        let err_wrong_ret =
            r#"LET(f, FUN((r: RESULT[DYNAMIC, STRING]) : INTEGER, ERR_PAYLOAD(r)));"#;
        assert_eq!(codes(err_wrong_ret), ["E0112"]);
        let wrong_ret = format!("LET(f, FUN((r: {R_DYN}) : STRING, UNWRAP(r)));");
        assert_eq!(codes(&wrong_ret), ["E0112"]);
        // 反过来,取「另一侧未知」的那一侧**不给结论**:值可能根本不是
        // `ERR`,此时运行时会报 `E0030`。静态层不猜。
        let inconclusive = format!("LET(f, FUN((r: {R_DYN}) : STRING, ERR_PAYLOAD(r)));");
        assert_eq!(check(&inconclusive), vec![]);
    }

    #[test]
    fn unwrap_defers_when_both_result_sides_are_concrete() {
        // 两侧都具体化 → OK/ERR 是运行期分支,静态层无从知
        // 走哪一支 → 不报(落 Dynamic)。此路径源码里到不了
        // (注解构造两侧都具体化的 RESULT 需要两个构造式),
        // 故直接测 `unwrap_side`。
        let both = Ty::Result(Box::new(Ty::Integer), Box::new(Ty::String));
        assert_eq!(unwrap_side(&both, Side::Ok), Ty::Dynamic);
        assert_eq!(unwrap_side(&both, Side::Err), Ty::Dynamic);
        // 对照:只有一侧未知时才有结论。
        let ok_known = Ty::Result(Box::new(Ty::Integer), Box::new(Ty::Dynamic));
        assert_eq!(unwrap_side(&ok_known, Side::Ok), Ty::Integer);
        assert_eq!(unwrap_side(&ok_known, Side::Err), Ty::Dynamic);
        let err_known = Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::String));
        assert_eq!(unwrap_side(&err_known, Side::Err), Ty::String);
        assert_eq!(unwrap_side(&err_known, Side::Ok), Ty::Dynamic);
    }

    #[test]
    fn unwrap_on_non_result_or_wrong_arity_is_left_to_the_runtime() {
        // 非 RESULT 交给运行时报 `E0030`;静态层不重复报,也不猜。
        assert_eq!(check("LET(f, FUN((r) : INTEGER, UNWRAP(1)));"), vec![]);
        // 元数不对也交给运行时。
        let both = format!("LET(f, FUN((r: {R_DYN}) : INTEGER, UNWRAP(r, 0)));");
        assert_eq!(check(&both), vec![]);
    }

    // ---- A4:容器嵌套边界 ----

    #[test]
    fn container_mismatch_is_reported_at_the_failing_depth() {
        // 递归一层层比,错在哪层就在哪层报。
        assert_eq!(
            codes(r#"LET(x: ARRAY[ARRAY[INTEGER]], [["a"]]);"#),
            ["E0110"]
        );
        // A2′(Step 4)起改为**逐元素**报,所以两个坏元素是两条诊断。
        assert_eq!(codes("LET(x: ARRAY[STRING], [1, 2]);"), ["E0110", "E0110"]);
        // 键侧不接收期望类型(下推会把字面量按期望错误定型),但键类型仍
        // 参与整体比较 —— 所以「声明 STRING 键却是整数」这类失配照报。
        assert_eq!(codes("LET(x: DICT[STRING, INTEGER], [1, 2]);"), ["E0110"]);
        assert_eq!(codes("LET(x: INTEGER, [1, 2]);"), ["E0110"]);
        // 深层正确的容器不报。
        assert_eq!(check("LET(x: ARRAY[ARRAY[INTEGER]], [[1], [2]]);"), vec![]);
        assert_eq!(
            check(r#"LET(x: DICT[STRING, ARRAY[INTEGER]], ["a": [1]]);"#),
            vec![]
        );
    }

    #[test]
    fn heterogeneous_nesting_is_silent_only_without_a_declared_element_type() {
        // A2′(Step 4)把这条的性质**分成了两半**,本测试随之修订:
        //
        // 1. **无声明元素类型**时,异构元素统一为 `Dynamic` → 静默。
        //    这是「不误报优先」的直接后果,必须锁住:高度嵌套下的异构值
        //    不应被误报。
        assert_eq!(codes(r#"LET(x, [[1], ["b"]]);"#), Vec::<String>::new());
        assert_eq!(codes(r#"LET(x, [1, "b"]);"#), Vec::<String>::new());
        // 2. **有声明元素类型**时,异构不再被 `Dynamic` 吞掉 —— A2′ 把期望
        //    元素类型逐项下推,`ARRAY[INTEGER]` 里放数组会被逐元素报出来。
        //    这是 A2′ 带来的**新增检出能力**,不是回归。两个元素都违规,
        //    所以是两条(逐元素定位,而不是笼统的整体一条)。
        assert_eq!(
            codes(r#"LET(x: ARRAY[INTEGER], [[1], ["b"]]);"#),
            ["E0110", "E0110"]
        );
        // 嵌套数组的声明元素类型必须是数组,不是元素类型。
        assert_eq!(codes(r#"LET(x: ARRAY[INTEGER], ["a"]);"#), ["E0110"]);
    }

    #[test]
    fn fun_type_contravariance_is_enforced_on_params() {
        // 形参逆变:被调方的形参能接住实参,才算安全。
        // 用真实的数值 ladder(INTEGER → FLOAT 安全放宽)做见证 ——
        // 不能用 Dynamic:它是顶底元,只要任一侧是 Dynamic,逆变
        // 就退化为恒真,那时逆变不可验证。
        let accepts_int = Ty::Fun {
            params: vec![Ty::Integer],
            ret: Box::new(Ty::String),
        };
        let accepts_float = Ty::Fun {
            params: vec![Ty::Float],
            ret: Box::new(Ty::String),
        };
        // 接受 Float 的函数可以充当接受 Integer 的函数(调用方传
        // Integer,被调方持为 Float 处理)—— 安全的方向。
        assert!(accepts_float.is_assignable_to(&accepts_int));
        // 反向不安全:只能处理 Integer 的函数冒充一个会传 Float 的位置。
        assert!(!accepts_int.is_assignable_to(&accepts_float));
        // 返回类型协变,同样用数值 ladder 做见证。
        let ret_int = Ty::Fun {
            params: vec![Ty::Integer],
            ret: Box::new(Ty::Integer),
        };
        let ret_float = Ty::Fun {
            params: vec![Ty::Integer],
            ret: Box::new(Ty::Float),
        };
        // 返回 Integer 的可以充当返回 Float 的(调用方按 Float 接收,
        // 而 INTEGER -> FLOAT 是安全放宽)。
        assert!(ret_int.is_assignable_to(&ret_float));
        // 返回 Float 的不能充当返回 Integer 的:那是收窄。
        assert!(!ret_float.is_assignable_to(&ret_int));
        // 返回 Dynamic 时双向皆真 —— 这是 `Dynamic` 作为顶底元的必然结果,
        // 不是逆变/协变规则的例外,故此处不作为见证。
    }

    // ---- A4 明确提后 / 需注意的现状 ----

    #[test]
    fn dict_key_type_constraint_is_not_checked_statically() {
        // spec §2.1:字典键必须是 STRING / INTEGER。A4 **不**做这项
        // 静态检查 —— 因为 D-1 只分配了 E0110-E0112 / W0110-W0112
        // 三个条件,本项无码可用;而 build plan §4.1 草案已预留
        // E0113-E0115 给模块签名。报错时必须先分配码号,否则就违
        // 破了本 ADR「凭空分配诊断码是破坏性变更」这一纪律。
        assert_eq!(
            check(r#"LET(m: DICT[BOOLEAN, STRING], [TRUE: "x"]);"#),
            vec![]
        );
    }

    #[test]
    fn arrow_function_type_syntax_silently_degrades_instead_of_erroring() {
        // 重要的现状记录。原判断是「写 `FUN(T) -> U` 会报
        // `E0010`」——**错的**。实测:`parse_type_annotation` 把类型区采集成
        // 平铺 token 列,而 `parse_type_expr_from_pieces` 在解析完 `FUN` 头后把剩余
        // pieces 直接拼成一个 **`Named`**,不报错。
        // 即:箭头形式不是被拒绝,而是**静默地被吸收成一个无意义类型名**。
        // 用户不会看到任何错误,也不会得到任何检查。
        let ast = wlwl_parser::parse("FUN((f: FUN(INTEGER) -> STRING), 1);", "check.wll")
            .expect("the arrow form is NOT rejected — it parses");
        let Expr::Fun { params, .. } = ast else {
            panic!("expected FUN");
        };
        let ann = params[0].type_annotation.as_ref().expect("annotation");
        let ty = Ty::from_type_expr(&ann.expr);
        // 它落成了一个将整段原文拼进去的无意义名。
        assert_eq!(
            ty,
            Ty::Named {
                name: "( INTEGER ) - > STRING".into(),
                args: vec![]
            },
            "arrow form must keep degrading silently until D-5 adds `->`"
        );
        // 对照:方括号形式 `FUN[T, ...]` 是**能解析**的,并映射到
        // `Ty::Fun`(形参取全部类型参数,返回类型为 Dynamic)。
        assert_eq!(
            Ty::from_type_expr(&ann_of("FUN[INTEGER, STRING]").expr),
            Ty::Fun {
                params: vec![Ty::Integer, Ty::String],
                ret: Box::new(Ty::Dynamic)
            }
        );
    }

    // ---- A2′:期望类型向下传播(Step 4) ----

    #[test]
    fn a2p_container_elements_are_typed_by_the_declared_element_type() {
        // A2′ 的主要收益是**推断精度**,不只是多抓错。声明了元素类型时,
        // 字面量按期望定型,推断结果不再是笼统的 `Dynamic`。
        // `1` 遇 `ARRAY[FLOAT]` 期望 → 直接定为 FLOAT,整体通过。
        assert_eq!(check("LET(x: ARRAY[FLOAT], [1]);"), vec![]);
        // 逐元素失配现在指向**具体那一项**,而不是笼统的整体类型差。
        let diags = check(r#"LET(x: ARRAY[INTEGER], [1, "a"]);"#);
        assert_eq!(
            diags.len(),
            1,
            "one bad element must yield exactly one diagnostic"
        );
        assert_eq!(
            diags[0].message(),
            "annotation mismatch: expected `INTEGER`, found `STRING`"
        );
        // 同一个问题**不得**被报两次(元素级 + 整体级)。
        assert_eq!(
            codes(r#"LET(x: ARRAY[ARRAY[INTEGER]], [["a"]]);"#),
            ["E0110"]
        );
    }

    #[test]
    fn a2p_call_arguments_receive_the_callee_param_types() {
        // 实参按被调签名的形参类型下推。
        let src = concat!(
            "LET(g, FUN((xs: ARRAY[INTEGER]) : INTEGER, LEN(xs))); ",
            "g([1, \"a\"]);"
        );
        let diags = check(src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message().contains("annotation mismatch"),
            "{:?}",
            diags[0].message()
        );
        // 正确调用不受影响。
        assert_eq!(check("LET(g, FUN((n: FLOAT), n)); g(1);"), vec![]);
        // 无签名的调用(内建 / 未知名字)不下推,也不误报。
        assert_eq!(check("PRINT(1);"), vec![]);
    }

    #[test]
    fn a2p_lub_joins_numeric_widening_but_never_narrows_dynamic() {
        // 合流取最小公共上界:`INTEGER` 与 `FLOAT` → `FLOAT`。
        assert_eq!(Ty::lub(&Ty::Integer, &Ty::Float), Ty::Float);
        assert_eq!(Ty::lub(&Ty::Float, &Ty::Integer), Ty::Float);
        assert_eq!(Ty::lub(&Ty::String, &Ty::String), Ty::String);
        // 互不相关 → 不猜。
        assert_eq!(Ty::lub(&Ty::String, &Ty::Integer), Ty::Dynamic);
        // 容器按元素合流。
        assert_eq!(
            Ty::lub(
                &Ty::Array(Box::new(Ty::Integer)),
                &Ty::Array(Box::new(Ty::Float))
            ),
            Ty::Array(Box::new(Ty::Float))
        );
        // **Dynamic 只能被合流稀释,不能被收窄**。这是 lub 里唯一一条
        // 反向规则:某一支「不知道」时,结论也必须是「不知道」——
        // 否则会把未标注分支的 Dynamic 凭空变成具体类型。
        assert_eq!(Ty::lub(&Ty::Dynamic, &Ty::Integer), Ty::Dynamic);
        assert_eq!(Ty::lub(&Ty::Integer, &Ty::Dynamic), Ty::Dynamic);
        assert_eq!(Ty::lub(&Ty::Dynamic, &Ty::Dynamic), Ty::Dynamic);
    }

    #[test]
    fn a2p_if_branches_are_joined_instead_of_degrading() {
        // Step 2 时 `IF` 只做「相同才保留」,这里两个分支都会落 Dynamic。
        // A2′ 之后推断出 FLOAT,因而能被 FLOAT 注解接住。
        let src = "LET(x: FLOAT, IF(TRUE, 1, 2.5));";
        assert_eq!(check(src), vec![]);
        // 分支类型不相关 → Dynamic,不报。
        assert_eq!(check("LET(x, IF(TRUE, 1, \"s\"));"), vec![]);
        // 分支合流出 FLOAT,但注解要 STRING → 报。
        assert_eq!(codes("LET(x: STRING, IF(TRUE, 1, 2.5));"), ["E0110"]);
        // 一支**真的**推不出 → 落 Dynamic,不收窄。注意:未标注的
        // `LET(y, 1)` 绑的是**推断出的 INTEGER**,不是 Dynamic;真正落到
        // Dynamic 的是推导不出的调用(内建返回类型要等 A6′)。
        assert_eq!(check("LET(x: STRING, IF(TRUE, LEN([]), 2.5));"), vec![]);
        // 对照:两支都推得出(INTEGER 与 FLOAT)→ 合流 FLOAT,不报。
        assert_eq!(check("LET(y, 1); LET(x: FLOAT, IF(TRUE, y, 2.5));"), vec![]);
    }

    #[test]
    fn a2p_still_never_reports_on_unannotated_programs() {
        // A2′ 的头号验收项:传播深度加大**不得**削弱「不误报」保证。
        let src = r#"
        LET(add, FUN((a, b), +(a, b)));
        LET(f, FUN((xs), add(1, 2)));
        LET(m, ["a": 1, "b": 2]);
        LET(n, f([1, 2, 3]));
        PRINT(m);
        PRINT(n);
        IF(TRUE, PRINT(m), PRINT(n));
        "#;
        assert_eq!(check(src), vec![]);
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

    // ---- Step 6:根作用域绑定表(模块契约的输入) ----

    fn declared_of(src: &str) -> Vec<(String, Ty)> {
        let ast = parse(src, "check.wll").expect("fixture must parse");
        check_program_detailed(&ast, &HashMap::new())
            .declared
            .into_iter()
            .map(|b| (b.name, b.ty))
            .collect()
    }

    #[test]
    fn root_bindings_are_reported_and_use_the_annotation_when_present() {
        assert_eq!(
            declared_of("LET(x: INTEGER, 1); LET(s: STRING, \"a\");"),
            vec![
                ("x".to_string(), Ty::Integer),
                ("s".to_string(), Ty::String),
            ]
        );
        // 无注解 → 走推断(字面量仍可知)。
        assert_eq!(declared_of("LET(n, 1);"), vec![("n".into(), Ty::Integer)]);
    }

    #[test]
    fn only_root_scope_bindings_are_reported() {
        // 函数体内的局部变量不是公开面,不能进契约表 —— 只有外层的 `f` 进。
        assert_eq!(
            declared_of("LET(f, FUN(() : INTEGER, LET(local, 1); local));").len(),
            1
        );
        // 顶层 LET 的右值里、函数体内的嵌套绑定同样不进。
        assert!(declared_of("LET(f, FUN((a), LET(inner, a); inner));")
            .iter()
            .all(|(n, _)| n == "f"));
    }

    #[test]
    fn a_named_function_at_the_top_level_lands_in_the_table() {
        // 具名函数形如 `FUN(name(params), body)`,名字绑在**外层**作用域,
        // 所以它是公开面的一部分。
        let src = "LET(f, FUN((a: INTEGER) : INTEGER, a));";
        assert_eq!(
            declared_of(src),
            vec![(
                "f".to_string(),
                Ty::Fun {
                    params: vec![Ty::Integer],
                    ret: Box::new(Ty::Integer),
                }
            )]
        );
    }

    #[test]
    fn rebinding_keeps_the_last_type_at_the_first_position() {
        // 与 TypeEnv 的覆盖语义一致:同名取最后一次,位置保持首次。
        assert_eq!(
            declared_of("LET(x: INTEGER, 1); LET(y, 2); LET(x: STRING, \"a\");"),
            vec![
                ("x".to_string(), Ty::String),
                ("y".to_string(), Ty::Integer),
            ]
        );
    }

    // ---- Step 9 (P1-2 / D-5): 泛型实例化与显式约束 ----

    fn codes_with(src: &str) -> Vec<String> {
        let ast = parse(src, "check.wll").expect("fixture must parse");
        check_program(&ast)
            .iter()
            .map(|d| match d.emitted_code(wlwl_error::Severity::Error) {
                Some(c) => c.as_str().to_string(),
                None => "<uncoded>".to_string(),
            })
            .collect()
    }

    /// 计划书的验收项:**编译期实例化错误**。约束不满足就是那处的类型
    /// 失配,复用既有的 `E0110` / `E0111` —— 本 Step **不新增错误码**。
    #[test]
    fn a_bounded_parameter_rejects_a_non_comparable_argument() {
        let src = "\
LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, IF(<(a, b), a, b)));
max(1, TRUE);
";
        assert_eq!(codes_with(src), vec!["E0111"]);
        // 消息里留着约束 —— 报的是**未代入的声明类型**。
        let ast = parse(src, "check.wll").unwrap();
        let msg = check_program(&ast)[0].message();
        assert!(msg.contains("T: Comparable"), "{}", msg);
    }

    #[test]
    fn a_satisfying_argument_passes_the_bound() {
        for (a, b) in [("1", "2"), ("1.5", "2"), ("\"a\"", "\"b\"")] {
            let src = format!(
                "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, IF(<(a, b), a, b))); \
                 max({a}, {b});"
            );
            assert!(
                codes_with(&src).is_empty(),
                "comparable arguments must pass: {a}, {b}"
            );
        }
    }

    /// 变量在返回类型里被**代入** —— 这就是泛型在无量化语法下的精度收益
    /// (无泛型的语言只能落 `DYNAMIC`)。
    #[test]
    fn a_variable_in_the_return_type_is_instantiated_with_the_argument() {
        let src = "\
LET(id, FUN((x: T: Comparable) : T: Comparable, x));
LET(y: INTEGER, id(1));
";
        assert!(codes_with(src).is_empty());
        // 代入后返回类型是 STRING,所以喂 INTEGER 会被抓住。
        let bad = "\
LET(id, FUN((x: T: Comparable) : T: Comparable, x));
LET(y: INTEGER, id(\"s\"));
";
        assert_eq!(codes_with(bad), vec!["E0110"]);
    }

    /// 容器里的约束逐元素生效(计划书说的「`ARRAY[INTEGER]` 与
    /// `ARRAY[STRING]` 误用」那一类)。
    #[test]
    fn a_bounded_variable_inside_a_container_is_checked_per_element() {
        let ok = "\
LET(count, FUN((xs: ARRAY[T: Comparable]) : INTEGER, 0));
count([1, 2, 3]);
";
        {
            let ast = parse(ok, "check.wll").expect("parses");
            let diags = check_program(&ast);
            assert!(
                diags.is_empty(),
                "{:?}",
                diags.iter().map(|d| d.message()).collect::<Vec<_>>()
            );
        }
        let bad = "\
LET(count, FUN((xs: ARRAY[T: Comparable]) : INTEGER, 0));
count([1, TRUE]);
";
        assert_eq!(codes_with(bad), vec!["E0110"]);
    }

    /// 无约束变量接受一切(不报),`Dynamic` 实参也不参与绑定(不误报优先)。
    #[test]
    fn an_unconstrained_variable_and_dynamic_never_produce_diagnostics() {
        for src in [
            "LET(id, FUN((x: T: Comparable) : T: Comparable, x)); LET(y, id(PRINT(1)));",
            "LET(id, FUN((x: T: Comparable) : T: Comparable, x)); LET(y: INTEGER, id(id(1)));",
            "LET(f, FUN((a: ARRAY[T: Comparable]) : INTEGER, 0)); LET(n, f(PRINT([1])));",
        ] {
            assert!(
                codes_with(src).is_empty(),
                "unconstrained / unknown types must stay silent: {src}"
            );
        }
    }

    /// 带泛型标注但用得对的模块**不该**多出诊断(零破坏的静态侧)。
    #[test]
    fn v09_programs_with_generic_annotations_stay_silent() {
        assert!(codes_with(
            "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, a)); max(1, 2);"
        )
        .is_empty());
    }
}
