//! MATCH 穷尽性 / 不可达子句(v0.10 Step 8 · 计划书 §5.1 P1-1)。
//!
//! # 两条检查
//!
//! | 检查 | 码 | 何时报 |
//! |---|---|---|
//! | 非穷尽 | `E0116` / `W0116` | 构造子空间**可枚举**、子句没盖满、且 default 臂是**被省略**补出来的 `NULL` |
//! | 不可达子句 / 死 default | `W0117`(恒警告) | 该臂接受的值已被前面的臂全盖住 |
//!
//! **可枚举**的构造子空间只有三种(都由运行时形状直接给出,不是猜的):
//! `BOOLEAN` → `{TRUE, FALSE}`、`NULL` → `{NULL}`、
//! `RESULT[T, E]` → `{OK(_), ERR(_)}`。整数 / 浮点 / 字符串是**无限域**,
//! 字面量穷举不完;`ARRAY` / `DICT` 的构造子按元数 / 键集无限展开。
//! 这几类**不报非穷尽** —— 报了就是误报。
//!
//! # 非穷尽为什么以「default 被省略」为前提
//!
//! spec §7.6 的 MATCH 是**全覆盖**的:default 臂总是存在(省略时等于
//! `NULL` 字面量)。所以「子句没盖满构造子空间」本身**不是**错误 ——
//! 作者写了 default 就是有意兜底,再报一遍纯噪声。
//!
//! 真正要报的是**省略了 default 却没盖满**:那种情况下漏掉的分支会
//! **静默得到 `NULL`**。例如
//!
//! ```wlwl
//! MATCH(r, [OK(n) -> n])   // r: RESULT[INTEGER, STRING]
//! // r 是 ERR 时什么都不打印 —— 静默 NULL,而不是报错
//! ```
//!
//! 这是本 Step 实测后对原计划的一处收敛(原计划没提 default 臂)。
//!
//! # 零误报是怎么做到的:三档置信度
//!
//! 判定一个子句「是否已被前面的子句盖住」时,答案分三档:
//!
//! - [`Verdict::Covered`] —— 盖住了(可以报不可达);
//! - [`Verdict::Uncovered`] —— 没盖住;
//! - [`Verdict::Unknown`] —— 判不了,**不报**。
//!
//! 列的种类决定能给多强的答案:
//!
//! | 列 | 空间 | 策略 |
//! |---|---|---|
//! | `Closed` | 有限可枚举 | **精确并集递归** —— 逐构造子特化,唯一能报「缺 `ERR(_)`」的路径 |
//! | `ScalarOpen` | 无限标量域 | **精确单行** —— 一个值只匹配一个字面量子句,不存在并集覆盖 |
//! | `Aggregate` | 数组 / 字典 | **保守句法** —— 只在能证明时给 `Covered`,其余 `Unknown` |
//! | `Unknown` | `Dynamic` / 函数 / 具名类型 | 保守句法 |
//!
//! 数组 / 字典只做保守判定的理由很具体:`[1, _]` 与 `[_ , 2]` 的**并集**
//! 能盖住 `[_, _]`,但**任何一行单独都盖不住**;而数组的构造子还按元数
//! 无限展开(`[]` / `[x]` / `[x, y]` / …,带 `*rest` 又是 `[n..∞)`)。
//! 把这套算准需要完整的元数分解 + 元素列递归,那是 v0.10.1 的量。
//! 现在宁可漏报,也不冒「报了一个其实可达的子句」这种更伤信任的错。
//!
//! # 与运行期语义严格对齐
//!
//! 判定规则逐条对着 `wlwl-eval` 的 `try_match` 写:
//!
//! - `Ident` / `Wildcard` 无条件匹配(所以绑定型模式在可达性上是通配);
//! - `Literal` 走 `values_equal_loose`(整数按值、字符串按值、`NULL` 相等);
//! - `Array` **无 `*rest` 时是精确元数**,有 `*rest` 时是 `[n..∞)`;
//! - `Dict` 是**键子集**匹配(值里多出来的键不影响匹配);
//! - 构造子不匹配是**软失败**(`Ok(false)`),不是硬错误 —— 所以拿
//!   `OK(..)` 去匹配一个非 `RESULT` 的值只是「不匹配」,不会中止。

use wlwl_ast::{Expr, Literal, MatchClause, Pattern, Span};

use crate::diag::{ArmSite, TypeDiag, TypeDiagKind};
use crate::ty::Ty;

/// 一列的构造子空间。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Col {
    /// 有限且可枚举:唯一能报「缺哪个构造子」的列。
    Closed(Vec<Ctor>),
    /// 无限标量域(`INTEGER` / `FLOAT` / `STRING`)。
    ScalarOpen,
    /// 数组 / 字典:构造子空间无限展开,只做保守判定。
    Aggregate,
    /// 不知道(`Dynamic` / 函数 / 具名类型 / `OPTION` 糖)。
    Unknown,
}

/// `Closed` 列里的一个构造子。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ctor {
    key: CtorKey,
    /// 实参数(0 或 1)。`OK` / `ERR` 各带一个载荷,`TRUE` / `FALSE` / `NULL`
    /// 不带 —— 元数不同,特化时走的路径也不同。
    arity: usize,
    /// 诊断文案:缺的是哪个构造子就写哪个。
    display: &'static str,
}

impl Ctor {
    fn nil(key: CtorKey, display: &'static str) -> Ctor {
        Ctor {
            key,
            arity: 0,
            display,
        }
    }

    fn unary(key: CtorKey, display: &'static str) -> Ctor {
        Ctor {
            key,
            arity: 1,
            display,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CtorKey {
    True,
    False,
    Null,
    Ok,
    Err,
}

/// 覆盖性判定的三档答案(见模块文档「三档置信度」)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// 前面的子句盖住了查询模式接受的全部值。
    Covered,
    /// 存在查询模式接受、但前面子句都没盖住的值。
    Uncovered,
    /// 判不了 —— 调用方必须**不报**。
    Unknown,
}

impl Verdict {
    /// 「全部都盖住」的合并:任一项 `Uncovered` 就是 `Uncovered`
    /// (有一个反例就够);没有反例但有 `Unknown` 就是 `Unknown`;
    /// 全 `Covered` 才是 `Covered`。
    fn all_covered(self, other: Verdict) -> Verdict {
        match (self, other) {
            (_, Verdict::Uncovered) | (Verdict::Uncovered, _) => Verdict::Uncovered,
            (Verdict::Covered, Verdict::Covered) => Verdict::Covered,
            _ => Verdict::Unknown,
        }
    }
}

/// 模式在第一列上的「头」形态。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Head {
    /// `Ident` / `Wildcard`:无条件匹配。
    Any,
    /// 字面量头(整数 / 字符串 / 布尔 / `NULL`)。
    Lit(LitKey),
    /// 构造子头(`OK` / `ERR`)。
    Ctor(CtorKey),
    /// 数组 / 字典头。
    Aggregate,
    /// 认不出来的头(理论上 parser 已经挡掉,这里防御到底)。
    Other,
}

/// 可比较的字面量键。`Interpolated` 不是编译期常量,拿它判覆盖只会
/// 猜,所以走 `Unknown` 分支。
#[derive(Debug, Clone, PartialEq, Eq)]
enum LitKey {
    Int(i64),
    Float(u64),
    Str(String),
    Bool(bool),
    Null,
    /// 插值字面量:值编译期未知。
    Interpolated,
}

impl LitKey {
    fn of(lit: &Literal) -> LitKey {
        match lit {
            Literal::Integer(i) => LitKey::Int(*i),
            // `f64` 没有 `Eq`;按位比较才是精确的「同一个值」。
            Literal::Float(f) => LitKey::Float(f.to_bits()),
            Literal::String(s) => LitKey::Str(s.clone()),
            Literal::Boolean(b) => LitKey::Bool(*b),
            Literal::Null => LitKey::Null,
            Literal::Interpolated(_) => LitKey::Interpolated,
        }
    }

    /// 这个字面量在闭空间里对应哪个构造子 —— `TRUE` / `FALSE` / `NULL`
    /// **既是字面量模式也是闭空间构造子**,两者必须认作同一个东西,
    /// 否则 `MATCH(TRUE, [[TRUE, 1]])` 会因为「列里出现了字面量头」
    /// 被当成类型不一致而放弃分析。
    fn ctor_key(&self) -> Option<CtorKey> {
        match self {
            LitKey::Bool(true) => Some(CtorKey::True),
            LitKey::Bool(false) => Some(CtorKey::False),
            LitKey::Null => Some(CtorKey::Null),
            _ => None,
        }
    }
}

/// 模式头对应的闭空间构造子(`Any` 通配不算 —— 它匹配所有构造子)。
fn head_ctor_key(h: &Head) -> Option<CtorKey> {
    match h {
        Head::Ctor(k) => Some(*k),
        Head::Lit(l) => l.ctor_key(),
        _ => None,
    }
}

/// 模式第一列的头。
fn head_of(p: &Pattern) -> Head {
    match p {
        Pattern::Ident(..) | Pattern::Wildcard(_) => Head::Any,
        Pattern::Literal(lit, _) => Head::Lit(LitKey::of(lit)),
        Pattern::Array(..) | Pattern::Dict(..) => Head::Aggregate,
        Pattern::Constructor { name, .. } => {
            if name == "OK" {
                Head::Ctor(CtorKey::Ok)
            } else if name == "ERR" {
                Head::Ctor(CtorKey::Err)
            } else {
                Head::Other
            }
        }
    }
}

/// 被检查值对应的列,由静态类型 + 模式共同决定。
///
/// **只有类型给不出信息时,模式才允许揭示空间**:被检查值是 `Dynamic`
/// (函数返回值最常见)时,出现 `OK(..)` / `ERR(..)` 模式说明作者显然在
/// 匹配一个 `RESULT` —— 这时按 `RESULT` 分析比直接放弃有价值得多,
/// 而且不会误报:`OK(..)` 对非 `RESULT` 的值只是软不匹配(见模块文档末节)。
///
/// 反过来,**类型已知且不是 `RESULT`** 却写了构造子模式,那是程序类型
/// 自相矛盾 → 判不了(落到 `Unknown` 一条不报),而不是硬说「缺
/// `ERR(_)`」那种在坏程序上看似有用的噪声。
fn column_for(scrutinee: &Ty, patterns: &[&Pattern]) -> Col {
    let has_ctor = patterns.iter().any(|p| matches!(head_of(p), Head::Ctor(_)));
    match scrutinee {
        // 类型不可知:模式是唯一的线索。
        Ty::Dynamic | Ty::Fun { .. } | Ty::Named { .. } => {
            if has_ctor {
                Col::Closed(result_ctors())
            } else {
                Col::Unknown
            }
        }
        Ty::Boolean => known_or_inconsistent(
            has_ctor,
            vec![
                Ctor::nil(CtorKey::True, "TRUE"),
                Ctor::nil(CtorKey::False, "FALSE"),
            ],
        ),
        Ty::Null => known_or_inconsistent(has_ctor, vec![Ctor::nil(CtorKey::Null, "NULL")]),
        Ty::Result(..) => Col::Closed(result_ctors()),
        Ty::Integer | Ty::Float | Ty::String => {
            if has_ctor {
                Col::Unknown
            } else {
                Col::ScalarOpen
            }
        }
        Ty::Array(_) | Ty::Dict(..) => {
            if has_ctor {
                Col::Unknown
            } else {
                Col::Aggregate
            }
        }
        // `Ty::Option` **故意**不枚举:它是注解糖(spec §2.1 没有这个运行时
        // 类型),而 `strict_types` 默认关着、注解并没有被运行时强制 ——
        // 把它当成 `{NULL, OK(_), ERR(_)}` 是不成立的。
        _ => Col::Unknown,
    }
}

/// 类型已知的闭空间:空间照给,但出现构造子模式就判不了。
fn known_or_inconsistent(has_ctor: bool, ctors: Vec<Ctor>) -> Col {
    if has_ctor {
        Col::Unknown
    } else {
        Col::Closed(ctors)
    }
}

fn result_ctors() -> Vec<Ctor> {
    vec![
        Ctor::unary(CtorKey::Ok, "OK(_)"),
        Ctor::unary(CtorKey::Err, "ERR(_)"),
    ]
}

/// `rows` 是否盖住 `query` 接受的全部值。
fn cover(rows: &[&Pattern], query: &Pattern, col: &Col) -> Verdict {
    // **基例**:没有任何前序子句时,查询模式接受的值必然逃出(rows 之外) ——
    // 每个 WLWL 模式都至少匹配一个值,所以这里恒是 `Uncovered`。
    // 漏掉这条会让「第一条子句」被判成不可达。
    if rows.is_empty() {
        return Verdict::Uncovered;
    }
    match col {
        Col::Closed(ctors) => {
            let mut verdict = Verdict::Covered;
            for ctor in ctors {
                // 查询模式接受不到这个构造子 → 对本题无贡献。
                let Some(q_spec) = specialize_query(query, ctor) else {
                    continue;
                };
                let Some(scoped) = scope_rows(rows, ctors, ctor.key) else {
                    return Verdict::Unknown;
                };
                verdict = verdict.all_covered(cover_ctor(&scoped, &q_spec, ctor.key));
                if verdict == Verdict::Uncovered {
                    return Verdict::Uncovered;
                }
            }
            verdict
        }
        // 标量域:一个值只匹配一个字面量子句,不存在并集覆盖,单行判定
        // 就是精确判定。
        Col::ScalarOpen => {
            if rows.iter().any(|r| matches!(head_of(r), Head::Any)) {
                return Verdict::Covered;
            }
            if rows.iter().any(|r| matches!(head_of(r), Head::Other)) {
                return Verdict::Unknown;
            }
            let qh = head_of(query);
            match qh {
                Head::Lit(l) => {
                    if l == LitKey::Interpolated {
                        return Verdict::Unknown;
                    }
                    if rows
                        .iter()
                        .any(|r| matches!(&head_of(r), Head::Lit(rl) if *rl == l))
                    {
                        Verdict::Covered
                    } else {
                        Verdict::Uncovered
                    }
                }
                // 通配查询没被任何字面量行盖住 → 还有别的值(域无限)。
                Head::Any => Verdict::Uncovered,
                // 拿数组 / 构造子去匹配一个标量列:程序本身类型不一致。
                _ => Verdict::Unknown,
            }
        }
        // 聚合列 / 不知道的列:只在**能证明**时给 `Covered`。
        Col::Aggregate | Col::Unknown => {
            if rows.iter().any(|r| matches!(head_of(r), Head::Any)) {
                return Verdict::Covered;
            }
            if rows.iter().any(|r| syntactically_covers(r, query)) {
                return Verdict::Covered;
            }
            // 存在并集覆盖的可能(`[1, _]` ∪ `[_ , 2]` ⊇ `[_, _]`),
            // 判不准就不判。
            Verdict::Unknown
        }
    }
}

/// 在闭空间里筛出「能匹配构造子 `key` 的值」的那些行。
///
/// 三种行的处理:
/// - 通配行 —— 对该构造子的值全盖,收;
/// - 同一闭空间里**别的**构造子行 —— 匹配不到,不收(这只是不相关);
/// - 闭空间里压根不存在的头(别的字面量 / 聚合 / 认不出来的)—— 类型自相
///   矛盾,`None` 表示整个 MATCH 判不了。
///
/// **必须拿整个空间来判「在不在空间里」**:只看当前这一列的单个构造子,
/// 同一个空间里的兄弟构造子会被误判成外来头。
fn scope_rows<'a>(rows: &[&'a Pattern], space: &[Ctor], key: CtorKey) -> Option<Vec<&'a Pattern>> {
    let mut scoped = Vec::with_capacity(rows.len());
    for r in rows {
        let include = match head_of(r) {
            Head::Any => true,
            other => match head_ctor_key(&other) {
                Some(k) if space.iter().any(|c| c.key == k) => k == key,
                _ => return None,
            },
        };
        if include {
            scoped.push(*r);
        }
    }
    Some(scoped)
}

/// 构造子 `key` 的值有没有逃出所有行?
///
/// 这是「非穷尽」的判定入口:拿一个通配当查询,问这个构造子的值是否
/// 全被行盖住。答案 `Uncovered` 就意味着这个构造子**缺**。
fn ctor_escapes(rows: &[&Pattern], space: &[Ctor], key: CtorKey) -> Verdict {
    let Some(ctor) = space.iter().find(|c| c.key == key) else {
        return Verdict::Covered;
    };
    let anything = Pattern::Wildcard(Span::dummy());
    let Some(q_spec) = specialize_query(&anything, ctor) else {
        return Verdict::Covered;
    };
    let Some(scoped) = scope_rows(rows, space, key) else {
        return Verdict::Unknown;
    };
    cover_ctor(&scoped, &q_spec, key)
}

/// `Closed` 列里单个构造子的覆盖性。`rows` 已限定为「能匹配该构造子值」
/// 的行,`q_spec` 是查询模式按该构造子特化后的载荷(0 元构造子没有载荷)。
fn cover_ctor(rows: &[&Pattern], q_spec: &Spec, key: CtorKey) -> Verdict {
    // 同 `cover` 的基例:没有能匹配该构造子的行 → 该构造子的值全部逃出。
    if rows.is_empty() {
        return Verdict::Uncovered;
    }
    // 0 元构造子(`TRUE` / `FALSE` / `NULL`)没有载荷列,只看「有没有行
    // 盖住它」—— 那已经由 `scoped` 的筛选与空/非空判定回答了。
    let Spec::Arity1(q_inner) = q_spec else {
        return Verdict::Covered;
    };
    let mut verdict = Verdict::Covered;
    for r in rows {
        let v = match head_of(r) {
            // 通配行对该构造子的值全盖。
            Head::Any => Verdict::Covered,
            other if head_ctor_key(&other) == Some(key) => match ctor_inner(r) {
                // 载荷类型不可知(`OK` / `ERR` 没有泛型实参),走保守判定。
                Some(inner) => cover(&[&inner], q_inner, &Col::Unknown),
                None => Verdict::Covered,
            },
            // 闭空间里出现别的构造子 = 特化阶段没滤干净,兜底不参与判定。
            _ => Verdict::Covered,
        };
        verdict = verdict.all_covered(v);
        if verdict == Verdict::Uncovered {
            return Verdict::Uncovered;
        }
    }
    verdict
}

/// 查询模式按某个构造子特化后的形态。
enum Spec {
    /// 0 元:匹配该构造子的任何值(`TRUE` / `FALSE` / `NULL`)。
    Arity0,
    /// 1 元:载荷还要被继续判定(`OK` / `ERR`)。
    Arity1(Pattern),
}

/// 特化查询模式:`None` = 查询模式接受不到该构造子的值。
fn specialize_query(q: &Pattern, ctor: &Ctor) -> Option<Spec> {
    let head = head_of(q);
    match &head {
        // 通配接受任何构造子 → 用通配当载荷(不施加任何约束)。
        Head::Any => Some(if ctor.arity == 0 {
            Spec::Arity0
        } else {
            Spec::Arity1(Pattern::Wildcard(Span::dummy()))
        }),
        other if head_ctor_key(other) == Some(ctor.key) => match ctor_inner(q) {
            Some(inner) => Some(Spec::Arity1(inner)),
            None => Some(Spec::Arity0),
        },
        _ => None,
    }
}

fn ctor_inner(p: &Pattern) -> Option<Pattern> {
    match p {
        Pattern::Constructor { inner, .. } => Some((**inner).clone()),
        _ => None,
    }
}

/// **保守的**句法覆盖:只有「显然盖住」才返回 `true`。
///
/// 宁可漏报不可达子句,也不能报一个其实可达的子句 —— 后者会让用户
/// 删掉正确代码。
fn syntactically_covers(row: &Pattern, query: &Pattern) -> bool {
    if row == query {
        // 结构完全相同:两个模式接受的集合相同。
        return true;
    }
    match (row, query) {
        // 数组:元数相同、且行内每一项都是通配 → 盖住查询的全部同元数组。
        (Pattern::Array(row_items, None, _), Pattern::Array(q_items, None, _))
            if row_items.len() == q_items.len() =>
        {
            row_items
                .iter()
                .all(|r| matches!(r, Pattern::Wildcard(_) | Pattern::Ident(..)))
        }
        // 构造子:同名且载荷被盖住。
        (
            Pattern::Constructor {
                name: rn,
                inner: ri,
                ..
            },
            Pattern::Constructor {
                name: qn,
                inner: qi,
                ..
            },
        ) if rn == qn => syntactically_covers(ri, qi),
        // 字典:行的键是查询键的子集,且每个子模式都盖住(行是更弱的约束)。
        (Pattern::Dict(row_entries, _), Pattern::Dict(q_entries, _)) => {
            row_entries.iter().all(|(rk, rsub)| {
                q_entries
                    .iter()
                    .any(|(qk, qsub)| keys_match(rk, qk) && syntactically_covers(rsub, qsub))
            })
        }
        _ => false,
    }
}

/// 字典键的字面量比较。拿不到常量键(表达式键)时返回 `false` ——
/// 判不准就不判。
fn keys_match(a: &Expr, b: &Expr) -> bool {
    let key = |e: &Expr| match e {
        Expr::Literal(l, _) => Some(LitKey::of(l)),
        _ => None,
    };
    match (key(a), key(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// 一条 MATCH 的穷尽性 / 可达性诊断结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchCheck {
    /// 省略了 default 且构造子空间有缺口 —— 漏掉的分支会静默得到
    /// `NULL`。值为构造子名(`OK(_)` / `TRUE` / …)。
    pub missing: Vec<String>,
    /// 第 i 个子句不可达(0 起算)。
    pub unreachable_clauses: Vec<usize>,
    /// 显式写了 default,但子句已经盖满整个空间 → default 永远跑不到。
    pub default_unreachable: bool,
}

/// 检查一个 MATCH:逐子句判可达性,再看构造子空间有没有缺口。
///
/// `default_synthetic` = 源码省略了 default 臂(parser 补了 `NULL` 字面量)。
/// 它是**非穷尽**报不报的分水岭,理由见模块文档。
pub fn check_match(scrutinee: &Ty, clauses: &[MatchClause], default_synthetic: bool) -> MatchCheck {
    let patterns: Vec<&Pattern> = clauses.iter().map(|c| &c.pattern).collect();
    let col = column_for(scrutinee, &patterns);

    // ---- 逐子句:被前面的子句盖住 = 不可达 ----
    let mut unreachable_clauses = Vec::new();
    for (i, p) in patterns.iter().enumerate() {
        let earlier: Vec<&Pattern> = patterns[..i].to_vec();
        if cover(&earlier, p, &col) == Verdict::Covered {
            unreachable_clauses.push(i);
        }
    }

    // ---- 构造子空间有没有缺口 ----
    // 对每个构造子问一句「它的值有没有逃出所有子句」。注意判定必须拿到
    // **整个**闭空间(见 `scope_rows` 的文档):只拿单个构造子去比,同一个
    // 空间里的兄弟构造子会被误判成「外来头」。
    let mut missing: Vec<String> = Vec::new();
    let mut fully_covered = false;
    if let Col::Closed(ctors) = &col {
        let mut unknown = false;
        for ctor in ctors {
            match ctor_escapes(&patterns, ctors, ctor.key) {
                Verdict::Covered => {}
                Verdict::Uncovered => missing.push(ctor.display.to_string()),
                Verdict::Unknown => unknown = true,
            }
        }
        fully_covered = missing.is_empty() && !unknown;
    }

    MatchCheck {
        // 写了 default 就是有意兜底,不算非穷尽;只有省略 default 时
        // 漏掉的分支才会**静默得到 NULL**,那才是要报的。
        missing: if default_synthetic {
            missing
        } else {
            Vec::new()
        },
        unreachable_clauses,
        default_unreachable: !default_synthetic && fully_covered,
    }
}

/// 把检查结果变成诊断。
pub fn to_diags(check: &MatchCheck, match_span: &Span, clause_spans: &[Span]) -> Vec<TypeDiag> {
    let mut out = Vec::new();
    for &i in &check.unreachable_clauses {
        out.push(TypeDiag::new(
            TypeDiagKind::UnreachableArm {
                what: ArmSite::Clause(i),
                reason: "an earlier clause already matches every value it accepts".to_string(),
            },
            clause_spans
                .get(i)
                .cloned()
                .unwrap_or_else(|| match_span.clone()),
        ));
    }
    if !check.missing.is_empty() {
        out.push(TypeDiag::new(
            TypeDiagKind::NonExhaustive {
                missing: check.missing.clone(),
            },
            match_span.clone(),
        ));
    }
    if check.default_unreachable {
        out.push(TypeDiag::new(
            TypeDiagKind::UnreachableArm {
                what: ArmSite::Default,
                reason: "the clauses already match every value".to_string(),
            },
            match_span.clone(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use wlwl_parser::parse;

    /// 跑一遍「解析 + 静态层 + MATCH 检查」,返回诊断码序列。
    fn codes(src: &str) -> Vec<String> {
        let ast = parse(src, "m.wll").expect("fixture must parse");
        let out = crate::check::check_program(&ast);
        out.iter()
            .map(|d| match d.emitted_code(wlwl_error::Severity::Error) {
                Some(c) => c.as_str().to_string(),
                None => "<uncoded>".to_string(),
            })
            .collect()
    }

    /// 计划书 §5.1 的验收:**缺失模式列出具体构造**。
    #[test]
    fn missing_result_constructor_is_named() {
        assert_eq!(codes(r#"MATCH(OK(1), [[OK(n), n]]);"#), vec!["E0116"],);
        let ast = parse(r#"MATCH(OK(1), [[OK(n), n]]);"#, "m.wll").unwrap();
        let msgs: Vec<String> = crate::check::check_program(&ast)
            .iter()
            .map(|d| d.message())
            .collect();
        assert!(
            msgs[0].contains("ERR(_)"),
            "the diagnostic must name the missing constructor: {}",
            msgs[0]
        );
        // 消息要讲清漏了会怎样(静默 NULL),否则用户以为程序会报错。
        assert!(msgs[0].contains("yields NULL"), "{}", msgs[0]);
    }

    /// 补上 `ERR` 之后穷尽 —— 但仍然没有 default 臂,所以不该再报。
    #[test]
    fn a_complete_result_match_is_silent() {
        assert!(codes(r#"MATCH(OK(1), [[OK(n), n], [ERR(e), 0]]);"#).is_empty());
    }

    /// 写了 default 就是有意兜底:**不算非穷尽**。这是本 Step 对原计划
    /// 的一处收敛(spec §7.6 的 MATCH 恒有 default)。
    #[test]
    fn an_explicit_default_is_not_a_non_exhaustive_error() {
        assert!(codes(r#"MATCH(OK(1), [[OK(n), n]], 0);"#).is_empty());
    }

    /// 显式 default + 子句已盖满 → default 永远跑不到(`W0117` 恒警告)。
    #[test]
    fn a_dead_default_arm_is_flagged_as_a_warning_even_in_error_mode() {
        let out = codes(r#"MATCH(OK(1), [[OK(n), n], [ERR(e), 0]], 99);"#);
        assert_eq!(out, vec!["W0117"]);
        // 恒警告:任何档位下严重级都是 Warning,不阻塞退出码。
        let ast = parse(r#"MATCH(OK(1), [[OK(n), n], [ERR(e), 0]], 99);"#, "m.wll").unwrap();
        let diags = crate::check::check_program(&ast);
        let rendered = diags[0].to_diagnostic(wlwl_error::Severity::Error).unwrap();
        assert_eq!(rendered.severity, wlwl_error::Severity::Warning);
        assert_eq!(rendered.code.as_str(), "W0117");
    }

    /// 通配之后的子句不可达 —— 最常见的一种死代码。
    #[test]
    fn a_clause_after_a_wildcard_is_unreachable() {
        let out = codes(r#"MATCH(1, [[_, 1], [1, 2]]);"#);
        assert_eq!(out, vec!["W0117"]);
        let ast = parse(r#"MATCH(1, [[_, 1], [1, 2]]);"#, "m.wll").unwrap();
        let msg = crate::check::check_program(&ast)[0].message();
        assert!(msg.contains("match clause 2"), "{}", msg);
    }

    /// 重复字面量:第二条子句永远匹配不到。
    #[test]
    fn a_duplicate_literal_clause_is_unreachable() {
        assert_eq!(codes(r#"MATCH(1, [[1, "a"], [1, "b"]]);"#), vec!["W0117"]);
    }

    /// 闭空间的**并集**覆盖:单看任何一行都盖不住 `[_, _]`,但
    /// `TRUE` + `FALSE` 合起来盖住了整个布尔空间。
    #[test]
    fn the_union_of_boolean_clauses_covers_the_space() {
        // 少一个分支 → 非穷尽。
        assert_eq!(codes(r#"MATCH(TRUE, [[TRUE, 1]]);"#), vec!["E0116"]);
        // 两个都在,且写了 default → default 死了。
        assert_eq!(
            codes(r#"MATCH(TRUE, [[TRUE, 1], [FALSE, 0]], 2);"#),
            vec!["W0117"]
        );
        // 两个都在,default 被省略 → 穷尽,静默。
        assert!(codes(r#"MATCH(TRUE, [[TRUE, 1], [FALSE, 0]]);"#).is_empty());
    }

    /// **模式可以揭示空间**:被检查值是 `Dynamic`(函数返回值),但出现
    /// `OK(..)` 模式说明作者在匹配 `RESULT`。
    #[test]
    fn constructor_patterns_reveal_a_result_scrutinee() {
        let ast = parse(
            r#"LET(f, FUN(() : DYNAMIC, 0)); MATCH(f(), [[OK(n), n]]);"#,
            "m.wll",
        )
        .expect("parses");
        let out = crate::check::check_program(&ast);
        let codes: Vec<&str> = out
            .iter()
            .filter_map(|d| d.emitted_code(wlwl_error::Severity::Error))
            .map(|c| c.as_str())
            .collect();
        assert!(codes.contains(&"E0116"), "expected E0116, got {codes:?}");
    }

    /// 数组的**精确元数**语义:行与查询元数不同 → 没盖住(不报,判不准);
    /// 元数相同且行内全是通配 → 盖住(报不可达)。
    #[test]
    fn array_arity_is_respected_where_it_is_decidable() {
        // `[_, _]` 盖住 `[1, 2]`。
        assert_eq!(
            codes(r#"MATCH([1, 2], [[[_, _], 1], [[1, 2], 2]]);"#),
            vec!["W0117"]
        );
        // 元数不同 → 判不准,**不报**。宁可漏报也不误报。
        assert!(codes(r#"MATCH([1, 2], [[[1, 2, 3], 1], [[1, 2], 2]]);"#).is_empty());
    }

    /// 无限域(整数 / 字符串)不报非穷尽 —— 穷举不完,报了就是误报。
    #[test]
    fn infinite_domains_never_report_non_exhaustive() {
        for src in [
            r#"MATCH(1, [[1, "a"]]);"#,
            r#"MATCH("x", [["y", 1]]);"#,
            r#"MATCH(1, [[1, "a"], [2, "b"]]);"#,
        ] {
            assert!(
                !codes(src).contains(&"E0116".to_string()),
                "must not claim non-exhaustiveness on an infinite domain: {src}"
            );
        }
    }

    /// `Dynamic` 且没有任何构造子提示 → 判不了,一条都不报。
    #[test]
    fn an_unanalysable_scrutinee_stays_silent() {
        let ast = parse(
            r#"LET(f, FUN(() : DYNAMIC, 0)); MATCH(f(), [[1, 1]]);"#,
            "m.wll",
        )
        .expect("parses");
        let out = crate::check::check_program(&ast);
        let codes: Vec<&str> = out
            .iter()
            .filter_map(|d| d.emitted_code(wlwl_error::Severity::Error))
            .map(|c| c.as_str())
            .collect();
        assert!(
            codes.is_empty(),
            "Dynamic scrutinee must stay silent: {codes:?}"
        );
    }

    /// `Ty::Option` **故意**不当作 `{NULL, OK(_), ERR(_)}`:它是注解糖,
    /// 而 `strict_types` 默认关着,注解没被运行时强制。
    #[test]
    fn option_annotated_scrutinees_are_not_enumerated() {
        let ast = parse(
            r#"LET(m: OPTION[INTEGER], NULL); MATCH(m, [[OK(n), n]]);"#,
            "m.wll",
        )
        .expect("parses");
        let out = crate::check::check_program(&ast);
        let codes: Vec<&str> = out
            .iter()
            .filter_map(|d| d.emitted_code(wlwl_error::Severity::Error))
            .map(|c| c.as_str())
            .collect();
        assert!(codes.is_empty(), "OPTION must not be enumerated: {codes:?}");
    }

    /// 类型自相矛盾的 MATCH(拿数组模式去匹配 BOOLEAN)→ 判不了,不报。
    #[test]
    fn an_inconsistent_match_is_left_alone() {
        let ast = parse(
            r#"LET(b: BOOLEAN, TRUE); MATCH(b, [[1, "int"], [TRUE, "bool"]]);"#,
            "m.wll",
        )
        .expect("parses");
        let out = crate::check::check_program(&ast);
        let codes: Vec<&str> = out
            .iter()
            .filter_map(|d| d.emitted_code(wlwl_error::Severity::Error))
            .map(|c| c.as_str())
            .collect();
        assert!(
            codes.is_empty(),
            "inconsistent match must stay silent: {codes:?}"
        );
    }

    /// v0.10 的默认路径:全部 MATCH 一条诊断都不许多出来(零破坏)。
    #[test]
    fn v09_programs_stay_silent() {
        for src in [
            r#"MATCH(1, [[1, "a"], [2, "b"]], "other");"#,
            r#"MATCH(OK(1), [[OK(n), n], [ERR(e), 0]]);"#,
            r#"MATCH([1, 2], [[[a, b], a], [[c], c], [[d, e, f], d]], 0);"#,
            r#"MATCH("k", [[[["k": v]], v]], 0);"#,
        ] {
            assert!(
                codes(src).is_empty(),
                "a v0.9 program must produce no MATCH diagnostics: {src}"
            );
        }
    }
}
