//! 注入 R1 门面的 R2 内核 —— 标准库底座 v0.11 M3-0(ADR-0021 §层间规则)。
//!
//! **本文件不是命名空间。** 它没有 `SPEC`、不进 `resolve()`、不进附录 A,
//! `IMPORT` 方永远看不到这里的任何名字。它是 [`crate::StdSource::kernels`]
//! 注入槽的取值表 —— eval 侧在 R1 源码求值**之前**把表里的名字绑进子
//! 求值器的局部环境(见 `wlwl_eval::ModuleLoader::load_std_lang`),而返回
//! 给 `IMPORT` 方的模块 env 只收 `EXPORT` 名单。所以**门面仍是唯一对外
//! 契约层**(ADR-0021:90 / stdlib 规范 §0.2)。
//!
//! ## 为什么要这条通道
//!
//! ADR-0021:89 定了「R1 可以调用 R0 与 R2」,M1 只落了 R1→R0 那一半
//! (闭包调全局内建)。M3 的两个模块各有一类东西**纯 wlwl 表达不出来**:
//!
//! 1. **浮点内核** —— `SQRT` / `POW` 依赖浮点指令(ADR-0021:84 已预告);
//! 2. **指定码的原生诊断** —— wlwl 源码无法指定诊断码。实测(2026-10-01,
//!    `target/debug/wlwl` 跑临时脚本):`PANIC(msg)` → `E0100`;`LEN(1)` →
//!    `E0030` 但消息被 `LEN` 固定;wlwl 闭包元数错 → `E0022` 但消息**无函数
//!    名前缀**。而 `E0038`(`RANGE` 步长为零,stdlib 规范 §5)在 M3 之前的
//!    全仓唯一发射点就是 `collection.rs` 里的 Rust 代码 —— 删掉它就没有
//!    任何发射点了。
//!
//! > **[D11-019] 补记**:`E0038` 当初正是这条通道的**第一个用例**,但 M5
//! > 裁决把 `RANGE` 沉回 R2 之后,它的唯一发射点回到了 Rust 侧
//! > (`collection.rs` 的 `kernel_range`),门面里的 `_DIAG_E0038` 注入与
//! > 本表的对应条目一起删除 —— 它已无发射点,留着就是「注入了没人用」的
//! > 死代码,而那正是 D11-011 的反向守卫要防的东西。其余发射器仍有门面
//! > 发射点(元数 / 类型 / 回调三类),留在表内。
//!
//! 被否决的替代方案:让 R1 靠「误撞」一个恰好会发 `E0030` 的内建来报类型
//! 错。码对了,但消息指向 `LEN` 而不是 `MAP`,诊断反而更糟 —— 用一条私有
//! 通道换一个**指名道姓**的消息,是划算的。
//!
//! ## 命名与不外泄
//!
//! kernel 名一律 `_` 前缀 UPPER_SNAKE:既不与全局内建撞名(撞名会让门面
//! 里的 `LET(X, ...)` 触发 `W0030` 遮蔽警告),又在读 `.wll` 源码时一眼
//! 分辨出「这不是导出面」。`wlwl-eval` 侧有一条守卫断言 kernel 名不出现在
//! 任一 R1 模块的 `EXPORT` 里,也不与全局内建重名。

use crate::StdFn;
use wlwl_error::{ErrorCode, WlwlResult};
use wlwl_value::{type_name, Outcome, StdHost, Value};

// ─────────────────────────────────────────────────────────────────────
// 共享助手
// ─────────────────────────────────────────────────────────────────────

/// 元数不符。消息带 kernel 自身的名字(与 R2 各模块的 `arity` 助手同
/// 措辞惯例),便于从诊断里一眼看出是门面调用写错了实参个数。
fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

/// 取字符串实参;非 `STRING` 时退回 `fallback`(诊断消息绝不能自己再抛)。
fn str_arg(args: &[Value], i: usize, fallback: &str) -> String {
    match args.get(i) {
        Some(Value::String(s)) => s.clone(),
        _ => fallback.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────
// 1. _KIND(x) -> STRING
// ─────────────────────────────────────────────────────────────────────

/// 值的**诊断用**种类名。
///
/// 单一来源是 `wlwl_value::type_name`。M3 之前仓里有两套并存的措辞 ——
/// `collection::value_kind`(`"function closure"` / `"native fn"` /
/// `"RESULT err"` / `"task handle"`)与 `wlwl_value::type_name`
/// (`"function"` / `"native-function"` / `"err"` / `"task-handle"`),同一个
/// `E0030` 在 `std.collection` 和 `std.test` 里读起来不一样。这里收敛到
/// 后者(既有单源),措辞变化登记 `deviations-v0.11.md`。
///
/// 注意与 `TYPE(x)`(wlwl 全局内建,§10.3)的区别:`TYPE` 返回 §2.1 的
/// **大写**类型名(`"FUNCTION"` / `"RESULT"`),本 kernel 返回诊断措辞用的
/// 小写种类名 —— R1 门面拼诊断消息时要的是后者。
pub fn kernel_kind(_host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    let Some(x) = args.first() else {
        return Err(arity(_host, "_KIND", 0, 1));
    };
    Ok(Outcome::normal(Value::String(type_name(x).into())))
}

// ─────────────────────────────────────────────────────────────────────
// 2. _DIAG_E0020(msg) / _DIAG_E0030(msg) / _DIAG_E0038(msg)  ——  发射器
// ─────────────────────────────────────────────────────────────────────

/// 发射一个指定码的原生诊断并**上抛**(`Err`),不给调用方返回值。
///
/// 这三个是同一件事的三个码,故共用一个签名:第一个实参是消息文本,门
/// 面负责拼出指名道姓的消息(`"MAP: expected array, got integer"`),
/// 这里只负责附上正确的码与当前 span。
macro_rules! diag_kernel {
    ($fn_name:ident, $kernel:literal, $code:expr, $default_msg:literal, $doc:literal) => {
        #[doc = $doc]
        pub fn $fn_name(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
            if args.len() != 1 {
                return Err(arity(host, $kernel, args.len(), 1));
            }
            Err(host.diag($code, str_arg(&args, 0, $default_msg)))
        }
    };
}

diag_kernel!(
    kernel_diag_e0020,
    "_DIAG_E0020",
    ErrorCode::E0020,
    "callback is not callable",
    "发射 `E0020`(成员存在但不可调用)。R1 门面用它报「回调不是函数值」。"
);

diag_kernel!(
    kernel_diag_e0022,
    "_DIAG_E0022",
    ErrorCode::E0022,
    "arity mismatch",
    "发射 `E0022`(元数不符)。给**变长**元数的成员用(`SORT` 1–2 参、`RANGE`\n    1–3 参、`ZIP` ≥1 参):`FUN((*args), ..)` 接受任意元数,解释器自己的\n    元数检查永远不会触发,所以元数检查必须由门面自己做 —— 而 wlwl\n    源码发不出指定码的诊断,只能走这里。固定元数的成员不需要它:\n    解释器的 `E0022` 码相同,只是消息里没有函数名前缀。"
);

diag_kernel!(
    kernel_diag_e0030,
    "_DIAG_E0030",
    ErrorCode::E0030,
    "type error",
    "发射 `E0030`(类型错误)。R1 门面用它报实参形态不符(`arr` 非 `ARRAY` 等)。"
);

// [D11-019] 这里原有第四个发射器 `_DIAG_E0038`(步长为零),是这条通道的
// 第一个用例。M5 把 `RANGE` 沉回 R2 之后,`E0038` 的唯一发射点回到
// `collection.rs` 的 `kernel_range`,门面不再有发射点 ⇒ kernel、注入条目与
// KERNELS 表项一并删除(留着就是死注入,而 D11-011 的反向守卫正是为防这个)。

// ─────────────────────────────────────────────────────────────────────
// 3. _SQRT(x) / _POW(a, b)  ——  std.math 浮点内核
// ─────────────────────────────────────────────────────────────────────

/// 域违例载荷。stdlib 规范 §7 规定域违例返回
/// `ERR(["kind": "DomainError", ...])` —— 那是**值**(不是原生诊断),
/// 所以走 `Ok(Outcome::normal(Value::Err(..)))`,由门面原样返回。
fn domain_error(op: &str, detail: String) -> Value {
    Value::Err(Box::new(Value::Dict(vec![
        (
            Value::String("kind".into()),
            Value::String("DomainError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(detail)),
    ])))
}

/// 取出 `FLOAT` 实参:`INTEGER` 按 §2.2 提升,其余形态报 `E0030`。
///
/// `FLOAT(x)`(§10.3)对整数提升、对非数字返回 `ERR(["kind":
/// "ParseError", ..])` 值而不是诊断,所以类型错必须自己判。
fn float_arg(host: &mut dyn StdHost, kernel: &str, v: &Value) -> WlwlResult<f64> {
    match v {
        Value::Integer(i) => Ok(*i as f64),
        Value::Float(f) => Ok(*f),
        other => Err(host.diag(
            ErrorCode::E0030,
            format!("{kernel}: expected number, got {}", type_name(other)),
        )),
    }
}

/// `SQRT(x) -> FLOAT`:平方根;`-0.0` 得 `-0.0`(stdlib 规范 §7)。
/// 负实参 → `ERR(["kind": "DomainError", ...])`。
pub fn kernel_sqrt(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "_SQRT", args.len(), 1));
    }
    let x = float_arg(host, "_SQRT", &args[0])?;
    if x < 0.0 {
        return Ok(Outcome::normal(domain_error(
            "SQRT",
            "SQRT: expected x >= 0".to_string(),
        )));
    }
    // `f64::sqrt` 对 `-0.0` 返回 `-0.0`,与 §7 的约定一致。
    Ok(Outcome::normal(Value::Float(x.sqrt())))
}

/// `POW(a, b) -> FLOAT`:幂;两参提升为 `FLOAT`(stdlib 规范 §7)。
/// `0` 的负次幂、负底的非整数指数 → `ERR(["kind": "DomainError", ...])`。
pub fn kernel_pow(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "_POW", args.len(), 2));
    }
    let a = float_arg(host, "_POW", &args[0])?;
    let b = float_arg(host, "_POW", &args[1])?;
    if a == 0.0 && b < 0.0 {
        return Ok(Outcome::normal(domain_error(
            "POW",
            "POW: zero raised to a negative power".to_string(),
        )));
    }
    if a < 0.0 && b.fract() != 0.0 {
        return Ok(Outcome::normal(domain_error(
            "POW",
            "POW: negative base with a non-integer exponent".to_string(),
        )));
    }
    Ok(Outcome::normal(Value::Float(a.powf(b))))
}

// ─────────────────────────────────────────────────────────────────────
// 4. 注入表
// ─────────────────────────────────────────────────────────────────────

/// 全部注入 kernel。**`StdSource::kernels` 并不引用本表** —— 每个模块在
/// 自己的 `StdSource` 里逐条手写注入清单(模块无关的共享 kernel 与模块
/// 专属 kernel 混在一起时,表无法表达)。本表的身份是「模块无关的共享
/// kernel 名册」,作用是给守卫当反方向的事实基准。
///
/// 顺序不承载语义。`wlwl-eval` 侧的守卫测试断言表里的名字既不与全局内建
/// 重名,也不出现在任一 R1 模块的 `EXPORT` 里(见
/// `wlwl-eval/src/stdlib_mirror.rs` 的 `r1_kernels_never_leak_to_the_export_surface`),
/// 另一条反向守卫断言「表里每个 kernel 都真的有人注入」(防死注入)。
pub static KERNELS: &[(&str, StdFn)] = &[
    ("_KIND", kernel_kind as StdFn),
    ("_DIAG_E0020", kernel_diag_e0020 as StdFn),
    ("_DIAG_E0022", kernel_diag_e0022 as StdFn),
    ("_DIAG_E0030", kernel_diag_e0030 as StdFn),
    ("_SQRT", kernel_sqrt as StdFn),
    ("_POW", kernel_pow as StdFn),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// 最小宿主:只实现 `diag`(其余方法 panic,本文件的 kernel 不会走到)。
    struct Bare;

    impl StdHost for Bare {
        fn ctx(&mut self) -> &mut wlwl_value::StdCtx {
            panic!("kernel tests never touch the per-call context")
        }
        fn call(
            &mut self,
            _f: &Value,
            _args: Vec<Value>,
            _name: &str,
        ) -> wlwl_error::WlwlResult<Outcome> {
            panic!("kernel tests never call back into the host")
        }
        fn diag(&mut self, code: ErrorCode, message: String) -> wlwl_error::WlwlError {
            wlwl_error::WlwlDiagnostic::new(
                code,
                message,
                wlwl_error::Location::point("<test>", 0, 0),
            )
            .into()
        }
    }

    fn run(f: StdFn, args: Vec<Value>) -> Result<Outcome, wlwl_error::WlwlError> {
        f(&mut Bare, args)
    }

    #[test]
    fn kind_uses_the_single_wlwl_value_source() {
        for v in [
            Value::Integer(1),
            Value::Float(1.5),
            Value::String("x".into()),
            Value::Boolean(true),
            Value::Null,
            Value::Array(vec![]),
            Value::Dict(vec![]),
            Value::Ok(Box::new(Value::Integer(1))),
            Value::Err(Box::new(Value::Null)),
        ] {
            let out = run(kernel_kind, vec![v.clone()]).expect("kind evaluates");
            assert_eq!(
                out.value,
                Value::String(type_name(&v).into()),
                "kernel_kind must agree with wlwl_value::type_name for {v:?}"
            );
        }
    }

    #[test]
    fn kind_arity_mismatch_is_e0022() {
        let err = run(kernel_kind, vec![]).expect_err("zero args is a diagnostic");
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    /// 三个发射器(E0020 / E0022 / E0030)共用一个宏,写串一个编译器不会
    /// 报;这条把三个码各自钉住。
    ///
    /// [D11-019] 原先这里还有第四个 `E0038` —— M5 把 `RANGE` 沉回 R2 后它
    /// 成了死 kernel,已随注入条目一并删除,故本表随之少一项。
    #[test]
    fn each_diag_kernel_carries_its_own_code() {
        for (f, want) in [
            (kernel_diag_e0020 as StdFn, ErrorCode::E0020),
            (kernel_diag_e0022 as StdFn, ErrorCode::E0022),
            (kernel_diag_e0030 as StdFn, ErrorCode::E0030),
        ] {
            let err = run(f, vec![Value::String("msg".into())]).expect_err("always a diagnostic");
            assert_eq!(err.diagnostic().code, want);
            assert_eq!(err.diagnostic().message, "msg");
        }
    }

    /// 消息不是 `STRING` 时退回固定文案,**不得**自己再抛(诊断消息里再
    /// 抛一个诊断会让调用方拿到错误的码)。
    #[test]
    fn diag_kernel_never_throws_on_a_non_string_message() {
        let err = run(kernel_diag_e0030, vec![Value::Integer(7)]).expect_err("always a diagnostic");
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        assert_eq!(err.diagnostic().message, "type error");
    }

    #[test]
    fn sqrt_matches_the_spec_edges() {
        let ok = |x: f64| {
            let outcome = run(kernel_sqrt, vec![Value::Float(x)]).expect("no diagnostic");
            outcome.value
        };
        assert_eq!(ok(4.0), Value::Float(2.0));
        assert_eq!(ok(0.0), Value::Float(0.0));
        // §7: `-0.0` 得 `-0.0`。`f64` 相等分不出 ±0,所以比符号位。
        let neg_zero = match ok(-0.0) {
            Value::Float(f) => f,
            other => panic!("expected float, got {other:?}"),
        };
        assert!(
            neg_zero == 0.0 && neg_zero.is_sign_negative(),
            "SQRT(-0.0) must be -0.0, got {neg_zero:?}"
        );
        // 整数按 §2.2 提升。
        assert_eq!(
            run(kernel_sqrt, vec![Value::Integer(9)])
                .expect("no diagnostic")
                .value,
            Value::Float(3.0)
        );
    }

    #[test]
    fn sqrt_of_a_negative_is_a_domain_error_value_not_a_diagnostic() {
        let out = run(kernel_sqrt, vec![Value::Float(-1.0)]).expect("domain error is a value");
        match out.value {
            Value::Err(p) => {
                let Value::Dict(entries) = *p else {
                    panic!("payload must be a DICT, got {p:?}")
                };
                let find = |k: &str| {
                    entries
                        .iter()
                        .find(|(key, _)| matches!(key, Value::String(s) if s == k))
                        .map(|(_, v)| v.clone())
                };
                assert_eq!(
                    find("kind"),
                    Some(Value::String("DomainError".into())),
                    "stdlib spec §7 requires kind=DomainError"
                );
            }
            other => panic!("expected ERR, got {other:?}"),
        }
    }

    #[test]
    fn pow_edges_follow_the_spec() {
        let ok = |a: f64, b: f64| {
            run(kernel_pow, vec![Value::Float(a), Value::Float(b)])
                .expect("no diagnostic")
                .value
        };
        assert_eq!(ok(2.0, 10.0), Value::Float(1024.0));
        // 负底 + 整数指数合法(§7 只禁「负底的非整数指数」)。
        assert_eq!(ok(-2.0, 3.0), Value::Float(-8.0));
        // 两参提升为 FLOAT。
        assert_eq!(
            run(kernel_pow, vec![Value::Integer(2), Value::Integer(3)])
                .expect("no diagnostic")
                .value,
            Value::Float(8.0)
        );
    }

    #[test]
    fn pow_domain_violations_are_domain_error_values() {
        for (a, b) in [(0.0, -1.0), (-8.0, 0.5)] {
            let out = run(kernel_pow, vec![Value::Float(a), Value::Float(b)])
                .expect("domain error is a value, not a diagnostic");
            assert!(
                matches!(out.value, Value::Err(_)),
                "POW({a}, {b}) must be an ERR value, got {:?}",
                out.value
            );
        }
    }

    #[test]
    fn non_numeric_argument_is_e0030() {
        for (f, args) in [
            (kernel_sqrt as StdFn, vec![Value::String("x".into())]),
            (kernel_pow as StdFn, vec![Value::Float(1.0), Value::Null]),
        ] {
            let err = run(f, args).expect_err("type error is a diagnostic");
            assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        }
    }

    /// kernel 表的「应然」侧:名字唯一 + `_` 前缀 + UPPER_SNAKE。
    ///
    /// `_` 前缀不是审美:与全局内建重名会让门面里同名的 `LET` 触发 `W0030`
    /// 遮蔽警告,而撞名本身要到 eval 侧才查得出来(守卫在
    /// `wlwl-eval/src/stdlib_mirror.rs`)。
    #[test]
    fn kernel_table_is_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in KERNELS {
            assert!(!name.is_empty(), "empty kernel name");
            assert!(
                name.starts_with('_'),
                "kernel `{name}` must be `_`-prefixed"
            );
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                "kernel `{name}` is not UPPER_SNAKE"
            );
            assert!(seen.insert(*name), "kernel `{name}` listed twice");
        }
        assert!(!KERNELS.is_empty(), "the table must not be empty");
    }
}
