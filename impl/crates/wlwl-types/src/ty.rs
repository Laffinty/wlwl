//! 静态类型 IR(ADR-0020 A1 · build plan Step 1)。
//!
//! **分层纪律**:本模块的 [`Ty`] 是**编译期**类型表示,与运行时 `TYPE(x)`
//! 的字符串比对路径(ADR-0010 的 transient cast)**严格分层**。
//! `wlwl-eval` 不依赖本 crate,运行时行为不受本模块影响(ADR-0020 S1/S3)。
//!
//! **语法面零扩张**:本模块只**消费** `wlwl-ast::TypeExpr` 的既有三形态
//! (`Ident` / `Array` / `Generic`),不改 AST。新类型名一律走
//! `Generic { name, args }` 既有槽位(ADR-0020 A1 设计约束)。
//!
//! **记法**:`Display` 用**方括号**(`ARRAY[INTEGER]`),与 parser 实际
//! 产出的 `TypeExpr` 一致,因此 `parse(display(ty)) == ty` 可往返。
//! build plan 正文用的尖括号记法(`ARRAY<T>`)只是散文简写,不是词法形式。

use std::fmt;

use wlwl_ast::TypeExpr;

/// 静态类型表示(ADR-0020 A1 最小集)。
///
/// 变体集合**刻意**等于 build plan §3.1「静态类型最小集」,不多不少:
/// 运行时存在但本层不结构化建模的类型(`TASK` / `CHANNEL` / `CLASS` /
/// `INSTANCE`)一律落到 [`Ty::Named`],而不是各开一个变体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// spec v0.9 §2.1 `INTEGER` — 有符号 64 位整数
    Integer,
    /// spec v0.9 §2.1 `FLOAT` — IEEE 754 双精度
    Float,
    /// spec v0.9 §2.1 `STRING`
    String,
    /// spec v0.9 §2.1 `BOOLEAN`
    Boolean,
    /// spec v0.9 §2.1 `NULL`
    Null,
    /// `ARRAY[T]` — 元素类型已知的数组
    Array(Box<Ty>),
    /// `DICT[K, V]` — 键值类型已知的字典(spec §2.1:键限 `STRING` / `INTEGER`)
    Dict(Box<Ty>, Box<Ty>),
    /// `OPTION[T]` — **仅注解侧**的可选包装。
    ///
    /// v0.9 运行时的「无值」用 `NULL` 或 `RESULT` 表达,没有对应的运行时
    /// 类型名(§2.1 的 13 个类型里没有 `OPTION`)。因此本变体是**注解糖**,
    /// 与运行时值的匹配规则由 A4 决定,本层不定。
    Option(Box<Ty>),
    /// `RESULT[T, E]` — `OK[T]` 折叠为 `Result[T, Dynamic]`,
    /// `ERR[E]` 折叠为 `Result[Dynamic, E]`
    Result(Box<Ty>, Box<Ty>),
    /// `FUN(T...) -> U` — 函数类型。
    ///
    /// **本版不可从 parser 到达**:`wlwl-ast/src/lib.rs:83-84` 把
    /// `FUN(...) -> T` 标为 "reserved for v0.4",至今未实现;lexer 也
    /// 没有 `->` 终结符(`-` 与 `>` 是两个独立 token)。本变体先落 IR,
    /// 供 A4 边界检查与 P1-2 限形泛型使用;接 parser 语法待决策点 D-5。
    Fun {
        /// 形参类型,按位置排列
        params: Vec<Ty>,
        /// 返回类型
        ret: Box<Ty>,
    },
    /// 已知但本层不结构化建模的具名类型。
    ///
    /// 覆盖两种情形:(1) 运行时真实类型但不在最小集内
    /// (`TASK` / `CHANNEL` / `CLASS` / `INSTANCE`);(2) 未知或用户自造
    /// 类型名。**保名保参**以便 A4 报出可诊断的失配,不做静默丢弃。
    Named {
        /// 原始类型名(不做大小写折叠,便于诊断回显源码)
        name: String,
        /// 类型参数;裸标识符为空
        args: Vec<Ty>,
    },
    /// 未标注处的回退类型 / 顶底元。
    ///
    /// 与任何类型双向可赋值(ADR-0020 A1:`Dynamic` 是顶/底元)。
    Dynamic,
}

impl Ty {
    /// 从 `wlwl-ast::TypeExpr` 映射到静态类型。
    ///
    /// 大小写不敏感 —— 与运行时 `E0033` 路径的既有行为一致
    /// (`wlwl-eval/src/lib.rs:8587` 注释:case-insensitive, top-level
    /// shape only)。
    ///
    /// **不 panic、不返回 Result**:映射是全函数。识别得出的头 + 正确元数
    /// 落到具体变体;裸头(无类型参数)落到该头的 `Dynamic` 参数形式;
    /// 识别得出但**元数错误**的头**保留为 [`Ty::Named`]** 而非丢弃
    /// 参数,让 A4 能报出「元数不对」而不是「类型不认识」。
    pub fn from_type_expr(expr: &TypeExpr) -> Ty {
        match expr {
            TypeExpr::Array { element, .. } => Ty::Array(Box::new(Ty::from_type_expr(element))),
            TypeExpr::Ident { name, .. } => Ty::from_head(name, &[]),
            TypeExpr::Generic { name, args, .. } => {
                let mapped: Vec<Ty> = args.iter().map(Ty::from_type_expr).collect();
                Ty::from_head(name, &mapped)
            }
        }
    }

    /// 统一的头名分派。`args` 为已映射好的类型参数(裸标识符为空)。
    fn from_head(name: &str, args: &[Ty]) -> Ty {
        let upper = name.to_ascii_uppercase();
        match upper.as_str() {
            // ---- 基类型 ----
            "INTEGER" | "INT" if args.is_empty() => Ty::Integer,
            "FLOAT" | "DOUBLE" if args.is_empty() => Ty::Float,
            "STRING" | "STR" if args.is_empty() => Ty::String,
            "BOOLEAN" | "BOOL" if args.is_empty() => Ty::Boolean,
            "NULL" | "NONE" if args.is_empty() => Ty::Null,

            // ---- 回退 / 顶底元 ----
            "DYNAMIC" | "ANY" if args.is_empty() => Ty::Dynamic,

            // ---- 容器与包装 ----
            // 注:`ARRAY` 裸写(无方括号)被 parser 特判拒绝
            // (`wlwl-parser/src/lib.rs:2407-2410`),所以下面这条
            // `args.is_empty()` 分支今天**不可从源码到达**,属防御性分支 ——
            // 保持映射全函数,不因将来 parser 放宽而 panic。
            "ARRAY" if args.is_empty() => Ty::Array(Box::new(Ty::Dynamic)),

            "ARRAY" if args.len() == 1 => Ty::Array(Box::new(args[0].clone())),
            "DICT" | "MAP" if args.is_empty() => {
                Ty::Dict(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic))
            }
            "DICT" | "MAP" if args.len() == 2 => {
                Ty::Dict(Box::new(args[0].clone()), Box::new(args[1].clone()))
            }
            "OPTION" | "MAYBE" if args.is_empty() => Ty::Option(Box::new(Ty::Dynamic)),
            "OPTION" | "MAYBE" if args.len() == 1 => Ty::Option(Box::new(args[0].clone())),
            "RESULT" if args.is_empty() => Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic)),
            "RESULT" if args.len() == 2 => {
                Ty::Result(Box::new(args[0].clone()), Box::new(args[1].clone()))
            }
            // `OK[T]` / `ERR[E]`(AST 注释点名的既有用法):
            // 未提及的那一侧保持 Dynamic,不做无依据的补全。
            "OK" if args.len() == 1 => Ty::Result(Box::new(args[0].clone()), Box::new(Ty::Dynamic)),
            "ERR" if args.len() == 1 => {
                Ty::Result(Box::new(Ty::Dynamic), Box::new(args[0].clone()))
            }

            // ---- 函数类型 ----
            // 暂定映射:参数表取全部类型参数,返回类型 Dynamic。**今天不可达**
            // (parser 不产 `FUN` 头),D-5 拍板后随语法一并定稿。
            "FUN" | "FUNCTION" => Ty::Fun {
                params: args.to_vec(),
                ret: Box::new(Ty::Dynamic),
            },

            // ---- 已知名但元数不对:保名保参,交给 A4 诊断 ----
            _ if is_known_head(&upper) => Ty::Named {
                name: name.to_string(),
                args: args.to_vec(),
            },

            // ---- 未知头 ----
            _ => Ty::Named {
                name: name.to_string(),
                args: args.to_vec(),
            },
        }
    }

    /// 是否为回退类型 `Dynamic`。
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Ty::Dynamic)
    }

    /// 源类型能否赋给目标类型。
    ///
    /// 规则:
    /// - 任一侧为 [`Ty::Dynamic`] 即为真(顶/底元双向互通);
    /// - 容器 / 包装逐位置递归;
    /// - 函数类型形参**逆变**、返回类型协变;
    /// - [`Ty::Named`] 要求同名、同元数、参数逐位置可赋值。
    ///
    /// **刻意不含** `INTEGER -> FLOAT` 数值放宽:运行时走 `E0031`
    /// numeric coercion,静态层的放宽规则由 A4 决定,本层不定。
    pub fn is_assignable_to(&self, target: &Ty) -> bool {
        if self.is_dynamic() || target.is_dynamic() {
            return true;
        }
        match (self, target) {
            (Ty::Integer, Ty::Integer)
            | (Ty::Float, Ty::Float)
            | (Ty::String, Ty::String)
            | (Ty::Boolean, Ty::Boolean)
            | (Ty::Null, Ty::Null) => true,
            (Ty::Array(a), Ty::Array(b)) => a.is_assignable_to(b),
            (Ty::Dict(k1, v1), Ty::Dict(k2, v2)) => {
                k1.is_assignable_to(k2) && v1.is_assignable_to(v2)
            }
            (Ty::Option(a), Ty::Option(b)) => a.is_assignable_to(b),
            (Ty::Result(t1, e1), Ty::Result(t2, e2)) => {
                t1.is_assignable_to(t2) && e1.is_assignable_to(e2)
            }
            (
                Ty::Fun {
                    params: p1,
                    ret: r1,
                },
                Ty::Fun {
                    params: p2,
                    ret: r2,
                },
            ) => {
                p1.len() == p2.len()
                    // 形参逆变:目标形参能喂给源形参才算安全。
                    && p1.iter().zip(p2.iter()).all(|(a, b)| b.is_assignable_to(a))
                    && r1.is_assignable_to(r2)
            }
            (Ty::Named { name: n1, args: a1 }, Ty::Named { name: n2, args: a2 }) => {
                n1 == n2
                    && a1.len() == a2.len()
                    && a1.iter().zip(a2.iter()).all(|(a, b)| a.is_assignable_to(b))
            }
            _ => false,
        }
    }
}

impl From<&TypeExpr> for Ty {
    fn from(expr: &TypeExpr) -> Ty {
        Ty::from_type_expr(expr)
    }
}

impl fmt::Display for Ty {
    /// 规范渲染,使用**方括号**(与 parser 产出一致,可往返)。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Integer => f.write_str("INTEGER"),
            Ty::Float => f.write_str("FLOAT"),
            Ty::String => f.write_str("STRING"),
            Ty::Boolean => f.write_str("BOOLEAN"),
            Ty::Null => f.write_str("NULL"),
            Ty::Dynamic => f.write_str("DYNAMIC"),
            Ty::Array(e) => write!(f, "ARRAY[{}]", e),
            Ty::Dict(k, v) => write!(f, "DICT[{}, {}]", k, v),
            Ty::Option(t) => write!(f, "OPTION[{}]", t),
            Ty::Result(t, e) => write!(f, "RESULT[{}, {}]", t, e),
            Ty::Fun { params, ret } => {
                let rendered: Vec<String> = params.iter().map(Ty::to_string).collect();
                write!(f, "FUN[{}] -> {}", rendered.join(", "), ret)
            }
            Ty::Named { name, args } if args.is_empty() => f.write_str(name),
            Ty::Named { name, args } => {
                let rendered: Vec<String> = args.iter().map(Ty::to_string).collect();
                write!(f, "{}[{}]", name, rendered.join(", "))
            }
        }
    }
}

/// 头名是否在已知集合内(用于区分「元数错误」与「类型不认识」)。
///
/// 运行时真实类型(spec §2.1)里本层不结构化建模的 `TASK` / `CHANNEL` /
/// `CLASS` / `INSTANCE` 也在内 —— 它们走 `Named` 是**有意的**,不是不认识。
fn is_known_head(upper: &str) -> bool {
    matches!(
        upper,
        "TASK"
            | "CHANNEL"
            | "CLASS"
            | "INSTANCE"
            | "ARRAY"
            | "DICT"
            | "MAP"
            | "OPTION"
            | "MAYBE"
            | "RESULT"
            | "OK"
            | "ERR"
            | "FUN"
            | "FUNCTION"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_ast::Expr;
    use wlwl_parser::parse;

    /// 经**真实 parser** 走一圈再映射 —— 这样锁的是「源码 → IR」的实际
    /// 通路,而不是手工构造的 `TypeExpr`。
    ///
    /// 包装成 `FUN((x: <annotation>), x);` 是因为 `parse_type_annotation`
    /// 只在形参 / 返回值位置可达(它以顶层 `,` / `)` 收尾)。
    fn ty_of_annotation(annotation: &str) -> Ty {
        let src = format!("FUN((x: {annotation}), x);");
        let expr = parse(&src, "ty.wll").expect("annotation must parse");
        let Expr::Fun { params, .. } = expr else {
            panic!("expected a FUN expression");
        };
        let ann = params[0]
            .type_annotation
            .as_ref()
            .expect("param carries an annotation");
        Ty::from_type_expr(&ann.expr)
    }

    /// 返回值注解的映射(参数表收尾后紧跟 `: Type`,再跟 `, body`)。
    fn ty_of_return_annotation(annotation: &str) -> Ty {
        let src = format!("FUN((x): {annotation}, x);");
        let expr = parse(&src, "ty.wll").expect("return annotation must parse");
        let Expr::Fun { return_type, .. } = expr else {
            panic!("expected a FUN expression");
        };
        let ann = return_type.expect("return type annotation present");
        Ty::from_type_expr(&ann.expr)
    }

    /// 直接构造 `TypeExpr::Ident` —— 用于锁「parser 够不到」的防御性分支。
    ///
    /// parser 对 `ARRAY` 特判、强制要求方括号
    /// (`wlwl-parser/src/lib.rs:2407-2410`:命中 `ARRAY` 就直接进
    /// `parse_braced`),所以**裸 `ARRAY` 注解在 v0.9 源码里写不出来**;
    /// 其余头(`DICT` / `OPTION` / `RESULT`)走通用路径,裸写能解析成 `Ident`。
    fn ty_of_bare_ident(name: &str) -> Ty {
        Ty::from_type_expr(&TypeExpr::Ident {
            name: name.to_string(),
            span: wlwl_ast::Span::new("ty.wll", 1, 1),
        })
    }

    // ---- 锁测试 1:TypeExpr → IR 映射与往返 ----

    #[test]
    fn type_ir_roundtrip() {
        let cases: &[(&str, Ty)] = &[
            ("INTEGER", Ty::Integer),
            ("FLOAT", Ty::Float),
            ("STRING", Ty::String),
            ("BOOLEAN", Ty::Boolean),
            ("NULL", Ty::Null),
            ("ARRAY[INTEGER]", Ty::Array(Box::new(Ty::Integer))),
            (
                "DICT[STRING, INTEGER]",
                Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer)),
            ),
            ("OPTION[STRING]", Ty::Option(Box::new(Ty::String))),
            (
                "RESULT[INTEGER, STRING]",
                Ty::Result(Box::new(Ty::Integer), Box::new(Ty::String)),
            ),
            (
                "OK[INTEGER]",
                Ty::Result(Box::new(Ty::Integer), Box::new(Ty::Dynamic)),
            ),
            (
                "ERR[STRING]",
                Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::String)),
            ),
            (
                "ARRAY[ARRAY[INTEGER]]",
                Ty::Array(Box::new(Ty::Array(Box::new(Ty::Integer)))),
            ),
            ("int", Ty::Integer),
            ("String", Ty::String),
        ];

        for (source, expected) in cases {
            assert_eq!(&ty_of_annotation(source), expected, "source: {source}");
        }
    }

    /// 渲染回源码必须能被 parser 读回同一个类型 —— 这是选方括号记法
    /// 而不是尖括号的**唯一理由**,必须锁住。
    #[test]
    fn display_roundtrips_through_parser() {
        let tys = [
            Ty::Integer,
            Ty::Array(Box::new(Ty::Integer)),
            Ty::Dict(Box::new(Ty::String), Box::new(Ty::Integer)),
            Ty::Option(Box::new(Ty::String)),
            Ty::Result(Box::new(Ty::Integer), Box::new(Ty::String)),
            Ty::Array(Box::new(Ty::Dict(
                Box::new(Ty::String),
                Box::new(Ty::Array(Box::new(Ty::Float))),
            ))),
        ];
        for ty in tys {
            let rendered = ty.to_string();
            assert_eq!(ty_of_annotation(&rendered), ty, "rendered: {rendered}");
        }
    }

    #[test]
    fn return_annotation_maps_through_same_path() {
        assert_eq!(ty_of_return_annotation("INTEGER"), Ty::Integer);
        assert_eq!(
            ty_of_return_annotation("ARRAY[STRING]"),
            Ty::Array(Box::new(Ty::String))
        );
    }

    #[test]
    fn named_types_keep_name_and_args() {
        assert_eq!(
            ty_of_annotation("MY_TYPE"),
            Ty::Named {
                name: "MY_TYPE".into(),
                args: vec![]
            }
        );
        // 运行时真实类型(spec §2.1)但本层不结构化建模 -> 有意走 Named。
        assert_eq!(
            ty_of_annotation("CHANNEL"),
            Ty::Named {
                name: "CHANNEL".into(),
                args: vec![]
            }
        );
        // 识别得出但元数不对:保名保参,不静默丢弃。
        assert_eq!(
            ty_of_annotation("DICT[INTEGER]"),
            Ty::Named {
                name: "DICT".into(),
                args: vec![Ty::Integer]
            }
        );
    }

    // ---- 锁测试 2:Dynamic 回退 ----

    #[test]
    fn dynamic_fallback() {
        // 裸容器头:形状已知、元素未知 -> 形状 + Dynamic 参数。
        // `DICT` / `OPTION` / `RESULT` 裸写能过 parser,走真实通路。
        assert_eq!(
            ty_of_annotation("DICT"),
            Ty::Dict(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic))
        );
        assert_eq!(
            ty_of_annotation("RESULT"),
            Ty::Result(Box::new(Ty::Dynamic), Box::new(Ty::Dynamic))
        );
        assert_eq!(
            ty_of_annotation("OPTION"),
            Ty::Option(Box::new(Ty::Dynamic))
        );
        assert_eq!(ty_of_annotation("ANY"), Ty::Dynamic);
        assert_eq!(ty_of_annotation("DYNAMIC"), Ty::Dynamic);
        // 裸 `ARRAY` 被 parser 拒(见下),只能直接构造 Ident 验证
        // 映射的防御性分支同样给出「形状 + Dynamic」。
        assert_eq!(ty_of_bare_ident("ARRAY"), Ty::Array(Box::new(Ty::Dynamic)));

        // 顶/底元:双向互通。
        assert!(Ty::Dynamic.is_assignable_to(&Ty::Integer));
        assert!(Ty::Integer.is_assignable_to(&Ty::Dynamic));
        assert!(Ty::Dynamic.is_dynamic());
        assert!(!Ty::Integer.is_dynamic());
        // Dynamic 可穿过容器参数。
        assert!(
            Ty::Array(Box::new(Ty::Dynamic)).is_assignable_to(&Ty::Array(Box::new(Ty::Integer)))
        );
        // 未标注处的回退类型就是 Dynamic。
        assert!(Ty::Dynamic.is_assignable_to(&Ty::Array(Box::new(Ty::String))));
    }

    /// 锁住「裸 `ARRAY` 注解在 v0.9 语法里写不出来」这一事实。
    ///
    /// 该特判是 parser 的既有行为(`wlwl-parser/src/lib.rs:2407-2410`),
    /// 不是本 crate 的选择。spec v0.10 补写 `Type` 产生式时需要知道:
    /// 容器头的类型参数在 v0.9 语法里**强制**出现。
    #[test]
    fn bare_array_annotation_is_rejected_by_the_parser() {
        let err = parse("FUN((x: ARRAY), x);", "ty.wll").expect_err("bare ARRAY is rejected");
        assert!(
            err.to_string().contains("expected `[` after `ARRAY`"),
            "unexpected diagnostic: {err}"
        );
    }

    #[test]
    fn assignability_is_structural_without_numeric_widening() {
        assert!(Ty::Integer.is_assignable_to(&Ty::Integer));
        assert!(!Ty::Integer.is_assignable_to(&Ty::Float));
        assert!(!Ty::Float.is_assignable_to(&Ty::Integer));
        assert!(
            Ty::Array(Box::new(Ty::Integer)).is_assignable_to(&Ty::Array(Box::new(Ty::Integer)))
        );
        assert!(
            !Ty::Array(Box::new(Ty::Integer)).is_assignable_to(&Ty::Array(Box::new(Ty::String)))
        );
        assert!(!Ty::Integer.is_assignable_to(&Ty::Array(Box::new(Ty::Integer))));
    }

    #[test]
    fn fun_type_is_ir_only_and_never_reachable_from_parser() {
        // parser 产不出 `FUN` 头(wlwl-ast 标注 reserved for v0.4),
        // 因此本变体只能直接构造;锁住「IR 存在但语法未接」这一事实。
        let ty = ty_of_annotation("FUN[INTEGER, STRING]");
        assert_eq!(
            ty,
            Ty::Fun {
                params: vec![Ty::Integer, Ty::String],
                ret: Box::new(Ty::Dynamic)
            }
        );
        assert_eq!(ty.to_string(), "FUN[INTEGER, STRING] -> DYNAMIC");
        assert!(!ty.is_assignable_to(&Ty::Integer));
        assert!(Ty::Fun {
            params: vec![Ty::Integer],
            ret: Box::new(Ty::String)
        }
        .is_assignable_to(&Ty::Fun {
            params: vec![Ty::Integer],
            ret: Box::new(Ty::String)
        }));
    }
}
