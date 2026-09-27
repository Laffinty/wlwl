//! 静态检查诊断类型(ADR-0020 A1)。
//!
//! **与 `wlwl-error` 的关系刻意尚未建立**。`TypeDiag` 描述「哪里、什么
//! 类型的错」,不含错误码;映射到 `wlwl_error::WlwlDiagnostic` / `EC` 需要
//! 决策点 **D-1**(静态诊断码段:新建 `E0110+` 段 / 复用 `E0030-E0039` /
//! 只用 W 码),尚未拍板。build plan §3.2 给了 `E0110`-`E0112` 的**草案**,
//! 本模块**不预先**采纳 —— Step 2(A3)落地时再接线。
//!
//! 同理,严重级(`off` / `warn` / `error`)属于 A3 的开关语义,不在本层。

use std::fmt;

use wlwl_ast::Span;

use crate::ty::Ty;

/// 静态诊断的种类。
///
/// 覆盖 A1–A4 会用到的全部失配形态;每个变体都自带足够信息渲染
/// [`TypeDiag::message`],不需要再回头看 AST。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDiagKind {
    /// 边界处的注解失配(形参注解 vs 传入值)。
    AnnotationMismatch {
        /// 注解声明的类型
        expected: Ty,
        /// 实际拿到的类型
        found: Ty,
    },
    /// 调用实参个数与形参表不符。
    CallArityMismatch {
        /// 被调方要求的实参个数
        expected: usize,
        /// 调用点实际给了几个
        found: usize,
    },
    /// 第 `position` 个实参的类型不符(按 0 起算,便于回显源码)。
    CallArgMismatch {
        /// 实参位置(0 起算)
        position: usize,
        /// 被调方形参类型
        expected: Ty,
        /// 实际传入类型
        found: Ty,
    },
    /// 返回值类型与注解不符。
    ReturnMismatch {
        /// 注解声明的返回类型
        expected: Ty,
        /// 实际返回的类型
        found: Ty,
    },
    /// 引用了不存在的名字。
    UndefinedName {
        /// 源码里写的名字
        name: String,
    },
    /// 注解里的类型名不可解析。
    UnresolvedTypeName {
        /// 源码里写的类型名
        name: String,
    },
    /// 泛型头元数不对(如 `DICT[INTEGER]`)—— `Ty::from_type_expr`
    /// 把它保留为 `Ty::Named`,本诊断负责把它讲清楚。
    TypeArityMismatch {
        /// 泛型头名
        name: String,
        /// 该头要求的元数
        expected: usize,
        /// 注解里实际写了几个
        found: usize,
    },
}

/// 一条静态类型诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDiag {
    /// 诊断种类
    pub kind: TypeDiagKind,
    /// 源码位置;合成诊断(无对应源码)用 `Span::dummy()`
    pub span: Span,
}

impl TypeDiag {
    /// 构造一条诊断。
    pub fn new(kind: TypeDiagKind, span: Span) -> TypeDiag {
        TypeDiag { kind, span }
    }

    /// 无源码位置可依的诊断(例如整模块级汇总)。
    pub fn synthetic(kind: TypeDiagKind) -> TypeDiag {
        TypeDiag {
            kind,
            span: Span::dummy(),
        }
    }

    /// 人类可读的一行消息。**渲染必须确定**(无 HashMap 序、无浮点格式
    /// 抖动),以便快照锁测试。
    pub fn message(&self) -> String {
        match &self.kind {
            TypeDiagKind::AnnotationMismatch { expected, found } => {
                format!("annotation mismatch: expected `{expected}`, found `{found}`")
            }
            TypeDiagKind::CallArityMismatch { expected, found } => {
                format!("call arity mismatch: expected {expected} argument(s), found {found}")
            }
            TypeDiagKind::CallArgMismatch {
                position,
                expected,
                found,
            } => {
                format!("call argument {position} mismatch: expected `{expected}`, found `{found}`")
            }
            TypeDiagKind::ReturnMismatch { expected, found } => {
                format!("return type mismatch: expected `{expected}`, found `{found}`")
            }
            TypeDiagKind::UndefinedName { name } => format!("undefined name `{name}`"),
            TypeDiagKind::UnresolvedTypeName { name } => {
                format!("unresolved type name `{name}`")
            }
            TypeDiagKind::TypeArityMismatch {
                name,
                expected,
                found,
            } => format!("type `{name}` takes {expected} type argument(s), found {found}"),
        }
    }
}

impl fmt::Display for TypeDiag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy() -> Span {
        Span::new("t.wll", 3, 7)
    }

    #[test]
    fn messages_are_deterministic_and_carry_the_span() {
        let diags = [
            TypeDiag::new(
                TypeDiagKind::AnnotationMismatch {
                    expected: Ty::Integer,
                    found: Ty::String,
                },
                dummy(),
            ),
            TypeDiag::new(
                TypeDiagKind::CallArityMismatch {
                    expected: 2,
                    found: 1,
                },
                dummy(),
            ),
            TypeDiag::new(
                TypeDiagKind::CallArgMismatch {
                    position: 0,
                    expected: Ty::Array(Box::new(Ty::Integer)),
                    found: Ty::Array(Box::new(Ty::String)),
                },
                dummy(),
            ),
            TypeDiag::new(
                TypeDiagKind::ReturnMismatch {
                    expected: Ty::Boolean,
                    found: Ty::Null,
                },
                dummy(),
            ),
            TypeDiag::new(
                TypeDiagKind::UndefinedName {
                    name: "missing".into(),
                },
                dummy(),
            ),
            TypeDiag::new(
                TypeDiagKind::UnresolvedTypeName {
                    name: "NOPE".into(),
                },
                dummy(),
            ),
            TypeDiag::new(
                TypeDiagKind::TypeArityMismatch {
                    name: "DICT".into(),
                    expected: 2,
                    found: 1,
                },
                dummy(),
            ),
        ];

        let rendered: Vec<String> = diags
            .iter()
            .map(|d| format!("{}:{} {}", d.span.line_start, d.span.col_start, d))
            .collect();
        assert_eq!(
            rendered,
            vec![
                "3:7 annotation mismatch: expected `INTEGER`, found `STRING`",
                "3:7 call arity mismatch: expected 2 argument(s), found 1",
                "3:7 call argument 0 mismatch: expected `ARRAY[INTEGER]`, found `ARRAY[STRING]`",
                "3:7 return type mismatch: expected `BOOLEAN`, found `NULL`",
                "3:7 undefined name `missing`",
                "3:7 unresolved type name `NOPE`",
                "3:7 type `DICT` takes 2 type argument(s), found 1",
            ]
        );
    }

    #[test]
    fn message_is_stable_across_repeated_renders() {
        let diag = TypeDiag::new(
            TypeDiagKind::ReturnMismatch {
                expected: Ty::Result(Box::new(Ty::Integer), Box::new(Ty::String)),
                found: Ty::Dynamic,
            },
            dummy(),
        );
        assert_eq!(diag.message(), diag.message());
        assert_eq!(
            diag.message(),
            "return type mismatch: expected `RESULT[INTEGER, STRING]`, found `DYNAMIC`"
        );
    }

    #[test]
    fn synthetic_diag_uses_dummy_span() {
        let diag = TypeDiag::synthetic(TypeDiagKind::UndefinedName { name: "x".into() });
        assert_eq!(diag.span, Span::dummy());
    }
}
