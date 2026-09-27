//! 静态检查诊断类型(ADR-0020 A1)。
//!
//! # 错误码映射(决策 D-1)
//!
//! D-1 选定「新建静态段 `E0110+` / `W0110+`」,不占用 `E0030-E0039`
//! ——该段 10/10 已被运行时类型 / 数值 / 集合语义占满(`E0030` type error、
//! `E0031` 键类型、`E0033` strict_types、`E0036` 越界…)。一个码号只对应
//! **一种**条件:编译期诊断与运行时 `E0033` 语义正交,复用会让同一码号随
//! 开关含义漂移,「运行时码不变」这条兼容性承诺就无法验证。
//!
//! | 条件 | `error` 档 | `warn` 档 |
//! |---|---|---|
//! | 注解失配(边界) | `E0110` | `W0110` |
//! | 调用失配(个数 / 第 n 个实参) | `E0111` | `W0111` |
//! | 返回类型失配 | `E0112` | `W0112` |
//!
//! # 严重级不归本层
//!
//! `off` / `warn` / `error` 三档的**开关语义**归 `wlwl-toml` 的
//! `[features] gradual_typing`(ADR-0020 A3)。本层只负责「一个条件对应
//! 哪两个码」,由调用方按档位取用。
//!
//! # 尚未分配码的条件
//!
//! [`TypeDiagKind::UnresolvedTypeName`] / [`TypeDiagKind::TypeArityMismatch`]
//! 归 A4 引入,但 A4 未落地前**不预分配码号** —— [`TypeDiagKind::codes`]
//! 对它们返回 `None`。宁可不给码,也不猜一个:猜错的码会进 spec §11.2 码表
//! 并被锁测试固定下来,事后改号是破坏性变更。
//!
//! [`TypeDiagKind::UndefinedName`] 同样无码:未定义名字由 parser 的
//! `lint` pass 负责(`W0001`),静态 pass **不重复报告**,避免一条问题
//! 出两张诊断。

use std::fmt;

use wlwl_ast::Span;
use wlwl_error::{ErrorCode, Location, Severity, WlwlDiagnostic};

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
    /// [v0.10 Step 6 / plan §4.1 C1、`§4.2` C2] 实现 `EXPORT` 了
    /// (或外部 `IMPORT` 了)契约**没有声明**的名字。
    ///
    /// 方向固定为「多出来」:名字是实现侧有的、契约侧没有的。反方向是
    /// [`TypeDiagKind::DeclaredNotExported`],两者不可互换。
    ExportNotDeclared {
        /// 越界的名字
        name: String,
        /// 哪个契约载体没声明它(签名文件 / 密封面;可同时缺)
        carriers: Vec<ContractCarrier>,
    },
    /// [v0.10 Step 6 / plan §4.1 C1] 契约声明了实现**没有导出**的名字。
    DeclaredNotExported {
        /// 声明了但没导出的名字
        name: String,
        /// 声明它的契约载体
        carriers: Vec<ContractCarrier>,
    },
    /// [v0.10 Step 6 / plan §4.1 C1] 签名里的类型与实现注解冲突。
    SignatureTypeMismatch {
        /// 出问题的导出名
        name: String,
        /// 签名声明的类型
        declared: Ty,
        /// 实现侧的类型(注解,或无注解时的推断结果)
        found: Ty,
    },
}

/// 契约的载体:同一个「声明面」有两种写法,诊断要能说清是哪一个缺了名字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractCarrier {
    /// 旁路签名文件 `<module>.wll.sig`(决策 D-2)
    Signature,
    /// 源内 `SEALED([...])` 声明(决策 D-3)
    Seal,
}

impl ContractCarrier {
    /// 诊断文案用的短语。**渲染必须确定**。
    pub fn as_str(self) -> &'static str {
        match self {
            ContractCarrier::Signature => "the module signature",
            ContractCarrier::Seal => "the SEALED surface",
        }
    }
}

impl fmt::Display for ContractCarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 把一组载体渲染成「A」「A or B」这样的短语,供诊断消息使用。
///
/// 空列表渲染成 `the module contract` —— 那是「有契约但这个载体没提它」
/// 的兜底说法,不该在正常路径上出现。
fn describe_carriers(carriers: &[ContractCarrier]) -> String {
    match carriers {
        [] => "the module contract".to_string(),
        [one] => one.as_str().to_string(),
        [a, b] => format!("{} or {}", a.as_str(), b.as_str()),
        _ => format!("{} and {} more", carriers[0].as_str(), carriers.len() - 1),
    }
}

/// 一条静态类型诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDiag {
    /// 诊断种类
    pub kind: TypeDiagKind,
    /// 源码位置;合成诊断(无对应源码)用 `Span::dummy()`
    pub span: Span,
}

/// 诊断种类 → 错误码(决策 D-1)。
impl TypeDiagKind {
    /// `(error 档, warn 档)` 码对。
    ///
    /// `None` = 该条件尚未分配码号(见模块文档「尚未分配码的条件」)。
    pub fn codes(&self) -> Option<(ErrorCode, ErrorCode)> {
        match self {
            TypeDiagKind::AnnotationMismatch { .. } => Some((ErrorCode::E0110, ErrorCode::W0110)),
            TypeDiagKind::CallArityMismatch { .. } | TypeDiagKind::CallArgMismatch { .. } => {
                Some((ErrorCode::E0111, ErrorCode::W0111))
            }
            TypeDiagKind::ReturnMismatch { .. } => Some((ErrorCode::E0112, ErrorCode::W0112)),
            // [v0.10 Step 6 / plan §4] 模块契约段。E0113/E0114 是公开面
            // 的两个方向(多出 / 缺失),E0115 是类型冲突 —— 三个码各管
            // 一种条件,和 E0110-E0112 一样只由编译期发出。
            TypeDiagKind::ExportNotDeclared { .. } => Some((ErrorCode::E0113, ErrorCode::W0113)),
            TypeDiagKind::DeclaredNotExported { .. } => Some((ErrorCode::E0114, ErrorCode::W0114)),
            TypeDiagKind::SignatureTypeMismatch { .. } => {
                Some((ErrorCode::E0115, ErrorCode::W0115))
            }
            TypeDiagKind::UndefinedName { .. }
            | TypeDiagKind::UnresolvedTypeName { .. }
            | TypeDiagKind::TypeArityMismatch { .. } => None,
        }
    }
}

/// 诊断 → `WlwlDiagnostic`。
impl TypeDiag {
    /// 无源码位置可依的诊断(例如整模块级汇总)。
    pub fn synthetic(kind: TypeDiagKind) -> TypeDiag {
        TypeDiag {
            kind,
            span: Span::dummy(),
        }
    }

    /// 按严重级产出可渲染 / 可 JSON 序列化的诊断。
    ///
    /// `None` = 该 `kind` 尚未分配码号(见 [`TypeDiagKind::codes`])。
    pub fn to_diagnostic(&self, severity: Severity) -> Option<WlwlDiagnostic> {
        let (error_code, warning_code) = self.kind.codes()?;
        let code = match severity {
            Severity::Warning => warning_code,
            _ => error_code,
        };
        let mut d = WlwlDiagnostic::new(code, self.message(), self.location());
        d.severity = severity;
        Some(d)
    }

    /// `gradual_typing = "error"` 档:硬诊断,阻塞退出码。
    pub fn to_error(&self) -> Option<WlwlDiagnostic> {
        self.to_diagnostic(Severity::Error)
    }

    /// `gradual_typing = "warn"` 档:软诊断,不阻塞退出码。
    pub fn to_warning(&self) -> Option<WlwlDiagnostic> {
        self.to_diagnostic(Severity::Warning)
    }

    /// AST `Span` → 诊断 `Location`。
    pub fn location(&self) -> Location {
        Location::range(
            self.span.file.clone(),
            self.span.line_start,
            self.span.col_start,
            self.span.line_end,
            self.span.col_end,
        )
    }
}

impl TypeDiag {
    /// 构造一条诊断。
    pub fn new(kind: TypeDiagKind, span: Span) -> TypeDiag {
        TypeDiag { kind, span }
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
            TypeDiagKind::ExportNotDeclared { name, carriers } => {
                format!(
                    "export `{name}` is not declared in {}",
                    describe_carriers(carriers)
                )
            }
            TypeDiagKind::DeclaredNotExported { name, carriers } => {
                format!(
                    "{} declares `{name}`, which the module does not export",
                    describe_carriers(carriers)
                )
            }
            TypeDiagKind::SignatureTypeMismatch {
                name,
                declared,
                found,
            } => format!(
                "module signature type mismatch for `{name}`: expected `{declared}`, found `{found}`"
            ),
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
