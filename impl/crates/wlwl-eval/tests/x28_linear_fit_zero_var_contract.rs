//! [X-28] `LINEAR_FIT` 的**两个方向**都要挡零方差 —— 标准库·核心册 §7.5。
//!
//! ## 它补的是什么
//!
//! 标准库·核心册 §7.5 的成员表与「空与无定义」表**两处**都写:
//! 「`CORRELATION` / `LINEAR_FIT` **任一方向**方差为零 → `NULL`」
//! (「相关系数 / 斜率无定义,**不是 0**」)。
//!
//! 缺陷形态:`LINEAR_FIT` 只累计了 x 方向的方差 ⇒ `LINEAR_FIT([1,2,3],
//! [1,1,1])`(y 方向零方差)返回 `[slope: 0.0, intercept: 1.0]` ⇒ 与规范
//! 明文冲突,且与**同族**的 `CORRELATION`(同一份 y 零方差数据返回 `NULL`)
//! **不一致**。
//!
//! ## 负向验证
//!
//! 把 y 方向的守卫去掉 ⇒ **本表至少 2 条变红**
//! (`linear_fit.y_zero_variance` / `linear_fit.both_directions_agree`)。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 跑一段源码,返回「诊断形态」(`!CODE message`)或「值形态」(`Value::display`)。
fn actual(src: &str) -> String {
    let ast = match wlwl_parser::parse(src, "t.wll") {
        Ok(a) => a,
        Err(e) => return format!("!PARSE {}", e.diagnostic().message),
    };
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    }
}

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("wlwl_x28_linear_fit")
}

/// ⚠ `concat!` 只接受**字面量** ⇒ 导入语句**内联**在每条用例里
///   (不能提成 `const`)。
const IMPORT: &str = r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#;

const CASES: &[Case] = &[
    // ── y 方向零方差 ⇒ NULL(缺陷形态在这里返回 slope 0.0) ─────────────
    Case {
        name: "linear_fit.y_zero_variance",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"LINEAR_FIT([1, 2, 3], [1, 1, 1])"#
        ),
        expect: "NULL",
    },
    // ── x 方向零方差 ⇒ NULL(既有口径,回归守卫) ───────────────────────
    Case {
        name: "linear_fit.x_zero_variance",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"LINEAR_FIT([1, 1, 1], [1, 2, 3])"#
        ),
        expect: "NULL",
    },
    // ── 两个方向**逐字一致** ───────────────────────────────────────────
    Case {
        name: "linear_fit.both_directions_agree",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"=(LINEAR_FIT([1, 2, 3], [1, 1, 1]), LINEAR_FIT([1, 1, 1], [1, 2, 3]))"#
        ),
        expect: "TRUE",
    },
    // ── 与**同族**的 `CORRELATION` 逐字一致(同族口径守卫) ───────────
    Case {
        name: "linear_fit.agrees_with_correlation_on_y_zero_variance",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"=(CORRELATION([1, 2, 3], [1, 1, 1]), LINEAR_FIT([1, 2, 3], [1, 1, 1]))"#
        ),
        expect: "TRUE",
    },
    // ── 正常数据**逐字不变**(回归守卫) ───────────────────────────────
    Case {
        name: "linear_fit.normal_data",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"LINEAR_FIT([1, 2, 3], [2, 4, 6])"#
        ),
        expect: "[slope: 2.0, intercept: 0.0]",
    },
    Case {
        name: "linear_fit.negative_slope",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"LINEAR_FIT([1, 2, 3], [3, 2, 1])"#
        ),
        expect: "[slope: -1.0, intercept: 4.0]",
    },
    // ── 非有限值 ⇒ NULL(既有口径,回归守卫) ───────────────────────────
    // ⚠ `NaN` 只能经 `-(LN(0), LN(0))` 造(`-inf - (-inf)`)⇒ 源码里**没有**
    //   `NaN` 字面量;`0.0 / 0.0` 会撞 `E1003`(原生诊断,直接终止运行)。
    Case {
        name: "linear_fit.non_finite",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"LINEAR_FIT([1, 2], [1, -(LN(0), LN(0))])"#
        ),
        expect: "NULL",
    },
    // ── `CORRELATION` **逐字不变**(回归守卫) ──────────────────────────
    Case {
        name: "correlation.normal_data",
        src: concat!(
            r#"IMPORT("wlwl:std.math", ["LINEAR_FIT", "CORRELATION", "LN"]); "#,
            r#"CORRELATION([1, 2, 3], [2, 4, 6])"#
        ),
        expect: "1.0",
    },
];

#[test]
fn linear_fit_returns_null_when_either_direction_has_zero_variance() {
    // ⚠ `IMPORT` 是 `const` ⇒ 只为**文档可读性**保留;`concat!` 里必须用字面量。
    let _ = IMPORT;
    for c in CASES {
        let got = actual(c.src);
        assert_eq!(got, c.expect, "case {}: {}", c.name, c.src);
    }
}
