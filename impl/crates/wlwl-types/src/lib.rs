//! WLWL 静态类型层 —— v0.10.0 引入的**编译期** pass(ADR-0020,
//! build plan Step 1 / A1)。
//!
//! # 存在理由
//!
//! 类型注解从 v0.3 起就在语法里(`wlwl-ast::TypeAnnotation`),但一直**没有
//! 语义**:唯一消费者是运行时按 `TYPE` 名做顶层形状比对
//! (`wlwl-eval/src/lib.rs:8586-8600`,失配 `E0033`),而 `wlwl check`
//! 只 parse(`wlwl-cli/src/main.rs:51-58`)。ADR-0020 决定把静态方向
//! **限缩重开**为「边界契约检查 + 期望类型向下传播」,范围**不是**
//! ADR-0010 当年否决的 HM / 全程序推断。
//!
//! # 分层纪律
//!
//! - 本 crate 只依赖 `wlwl-ast`(读 `TypeExpr` / `Span`);
//! - `wlwl-eval` **不依赖**本 crate,运行时 `strict_types` / `E0033` 路径
//!   一个字都不动(ADR-0020 S3);
//! - 诊断到 `wlwl_error::WlwlDiagnostic` 的映射**尚未建立** —— 等决策点
//!   D-1 拍板(见 [`diag`] 模块文档)。
//!
//! # 本 Step(A1)交付的范围
//!
//! - [`ty::Ty`] 静态类型 IR + 从 `TypeExpr` 的映射 + 结构化可赋值判定
//! - [`env::TypeEnv`] 词法作用域链
//! - [`diag::TypeDiag`] 诊断类型
//!
//! **不含**(留给后续 Step):`check.rs` 遍历与边界检查(A3)、期望类型向下
//! 传播(A2′)、容器 / 函数类型的完整检查规则(A4)、注册表结构化签名(A6′)。
//!
//! 门禁命令:`cargo test -p wlwl-types`;全量回归 `cargo test --workspace`
//! 必须保持 v0.9 的 1517 项全绿、0 failed(ADR-0020 S1)。

pub mod check;
pub mod diag;
pub mod env;
pub mod ty;

pub use check::{check_program, check_program_with_builtins};
pub use diag::{TypeDiag, TypeDiagKind};
pub use env::TypeEnv;
pub use ty::Ty;

#[cfg(test)]
mod integration_tests {
    //! 跨模块的最小闭环:类型注解 → IR → 名字环境 → 诊断。
    use super::*;

    #[test]
    fn unannotated_binding_falls_back_to_dynamic_and_stays_assignable() {
        // A2′ 的验收前提:未标注处回退 Dynamic,且 Dynamic 不得产生误报。
        let mut env = TypeEnv::new();
        env.bind("unannotated", Ty::Dynamic);
        env.bind("annotated", Ty::Array(Box::new(Ty::Integer)));

        let annotated = env.lookup("annotated").expect("bound");
        let unannotated = env.lookup("unannotated").expect("bound");

        assert!(unannotated.is_assignable_to(annotated));
        assert!(annotated.is_assignable_to(unannotated));
    }

    #[test]
    fn annotated_mismatch_is_representable_as_a_diag() {
        // 静态层唯一在 A1 承诺的事:失配**可被表达**,不误报。
        let declared = Ty::Array(Box::new(Ty::Integer));
        let actual = Ty::Array(Box::new(Ty::String));
        assert!(!declared.is_assignable_to(&actual));

        let diag = TypeDiag::new(
            TypeDiagKind::AnnotationMismatch {
                expected: declared,
                found: actual,
            },
            wlwl_ast::Span::new("t.wll", 1, 9),
        );
        assert_eq!(
            diag.message(),
            "annotation mismatch: expected `ARRAY[INTEGER]`, found `ARRAY[STRING]`"
        );
    }
}
