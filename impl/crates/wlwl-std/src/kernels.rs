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
// 3b. v0.11.2 M4 —— 超越函数族与 TRUNC(同一个 R2 浮点通道)
// ─────────────────────────────────────────────────────────────────────
//
// 纯 wlwl 表达不出这些浮点指令(ADR-0021 §0),故与 `_SQRT` / `_POW` 同走
// kernel 通道。**一条也不进 EXPORT**,故不落附录 A、不在 IMPORT 面上。
//
// 域违例的口径与 `SQRT` / `POW` 一致(§7):返回 `ERR([kind: DomainError])`
// **值**,而不是原生诊断。类型错由门面先判(指名 `LN` 而不是 `_LN`)。

/// 一元浮点 kernel 的共用收口:元数 → 类型 → 域 → 施加。
///
/// 写这个助手而不是手抄十五遍,是因为十五遍里只要有一遍忘了域检查或忘了
/// `Value::Float` 包装,那一条就成了「行为与文档不符且没人发现」的成员。
/// `domain` 返回 `Some(reason)` 时发 DomainError 值。
fn unary_f64(
    host: &mut dyn StdHost,
    kernel: &str,
    op: &str,
    args: &[Value],
    domain: impl Fn(f64) -> Option<String>,
    f: impl Fn(f64) -> f64,
) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, kernel, args.len(), 1));
    }
    let x = float_arg(host, kernel, &args[0])?;
    if let Some(reason) = domain(x) {
        return Ok(Outcome::normal(domain_error(op, reason)));
    }
    Ok(Outcome::normal(Value::Float(f(x))))
}

/// 二元浮点 kernel 的共用收口(目前只有 `ATAN2` 用)。
///
/// `kernel` 与 `op` 分开,理由同 [`unary_f64`]:前者进诊断(消息要指名
/// `_ATAN2` 这类**私有** kernel 名,因为那是解释器内部的元数/类型错),
/// 后者进 ERR 载荷(那是要给用户看的,必须是 `ATAN2`)。合成一个参数的话,
/// 载荷里就会漏出下划线开头的私有名。
fn binary_f64(
    host: &mut dyn StdHost,
    kernel: &str,
    op: &str,
    args: &[Value],
    domain: impl Fn(f64, f64) -> Option<String>,
    f: impl Fn(f64, f64) -> f64,
) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, kernel, args.len(), 2));
    }
    let a = float_arg(host, kernel, &args[0])?;
    let b = float_arg(host, kernel, &args[1])?;
    if let Some(reason) = domain(a, b) {
        return Ok(Outcome::normal(domain_error(op, reason)));
    }
    Ok(Outcome::normal(Value::Float(f(a, b))))
}

// 这两个域谓词捕获 `op` 字面量,所以要求 `'static`:调用点全传字符串字面量。
fn neg_domain(op: &'static str) -> impl Fn(f64) -> Option<String> + 'static {
    move |x| (x < 0.0).then(|| format!("{op}: expected x >= 0"))
}

fn unit_domain(op: &'static str) -> impl Fn(f64) -> Option<String> + 'static {
    move |x| (x.abs() > 1.0).then(|| format!("{op}: expected -1 <= x <= 1, got {x}"))
}

/// `LN(x) -> FLOAT`:自然对数。`x < 0` → DomainError。
///
/// **`LN(0.0)` 不报错,返回 `-inf`** —— 这是数学上正确的结果(也与 C /
/// Python / Go 一致),不是疏漏。§7 的「NaN / ±inf 原样」口径同样适用:
/// `LN(1e300)` 正常返回一个有限的 `690.8`。
pub fn kernel_ln(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_LN", "LN", &args, neg_domain("LN"), f64::ln)
}

pub fn kernel_log2(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_LOG2", "LOG2", &args, neg_domain("LOG2"), f64::log2)
}

pub fn kernel_log10(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(
        host,
        "_LOG10",
        "LOG10",
        &args,
        neg_domain("LOG10"),
        f64::log10,
    )
}

/// `EXP(x) -> FLOAT`:自然指数。**全定义域**,无 DomainError。
/// 上溢按 IEEE-754 给 `+inf`(`EXP(1000.0)`),与「NaN/±inf 原样」一致。
pub fn kernel_exp(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_EXP", "EXP", &args, |_| None, f64::exp)
}

/// `TRUNC(x) -> FLOAT`:向零取整。
///
/// **与全局 `INT` 的唯一区别在界外**:`|x| >= 2^53` 时本成员原样返回 x,
/// 而 `INT(1e20)` 抛 **E0035 并中止整个运行**。界内两者逐字相同
/// (都是向零截断)。这正是 `FLOOR` / `CEIL` / `ROUND` 存在的同款理由 ——
/// 一个取整函数因为边界而中止程序,比返回边界值难用得多。
///
/// 非数值实参:本成员经门面的 `_NEED_NUM` 报 **E0030 诊断**;
/// `INT("abc")` 返的是 `ERR([kind: ParseError])` **值**。两类别混。
pub fn kernel_trunc(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    // `f64::trunc` 对 |x| >= 2^53 原样返回(那些值本来就是整数),不需要
    // 像 FLOOR/CEIL 那样先判界再走 `INT` + 符号修正。
    unary_f64(host, "_TRUNC", "TRUNC", &args, |_| None, f64::trunc)
}

pub fn kernel_sin(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_SIN", "SIN", &args, |_| None, f64::sin)
}

pub fn kernel_cos(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_COS", "COS", &args, |_| None, f64::cos)
}

pub fn kernel_tan(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_TAN", "TAN", &args, |_| None, f64::tan)
}

/// `ASIN` / `ACOS` 的定义域是 `[-1, 1]`;域外无实值 → DomainError。
pub fn kernel_asin(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_ASIN", "ASIN", &args, unit_domain("ASIN"), f64::asin)
}

pub fn kernel_acos(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_ACOS", "ACOS", &args, unit_domain("ACOS"), f64::acos)
}

pub fn kernel_atan(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_ATAN", "ATAN", &args, |_| None, f64::atan)
}

/// `ATAN2(y, x) -> FLOAT`:按象限定义的反正切,**全定义域**。
/// `(0, 0)` 按 IEEE 754 / Rust 的约定返回 `0.0`;实轴上的 `±0` 也因此
/// 有确定值(不报错)。
pub fn kernel_atan2(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    binary_f64(host, "_ATAN2", "ATAN2", &args, |_, _| None, f64::atan2)
}

pub fn kernel_sinh(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_SINH", "SINH", &args, |_| None, f64::sinh)
}

pub fn kernel_cosh(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_COSH", "COSH", &args, |_| None, f64::cosh)
}

pub fn kernel_tanh(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    unary_f64(host, "_TANH", "TANH", &args, |_| None, f64::tanh)
}

/// `POW_MOD(base, exp, mod) -> INTEGER`:模幂,平方-乘算法,`O(log exp)`。
///
/// **为什么是 kernel 而不是 R1 门面**:门面算不出来又不炸的中间积。
/// 平方-乘的中间值最大是 `m²`;`m` 可以是 `2^61 - 1` 这样的密码学素数,
/// 那样 `m²` 在 i64 里必然溢出。R1 门面只有两条路,都不可接受:
/// 先乘再判 ⇒ 溢出先发生(语言规范 §2.2 抛 `E0035`,整个运行中止);
/// 或者人为把模数限制在 `sqrt(i64::MAX) ≈ 3.04e9` 以下 ⇒ 废掉大半用途。
/// 这里用 `i128` 算中间积,溢出前就看得见,于是 DomainError 是**主动
/// 报告**的而不是副作用。
///
/// 域违例:`mod = 0`、负指数(模逆不在本成员范围)、`base` 为最小整数时
/// 取模溢出。
pub fn kernel_pow_mod(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity(host, "_POW_MOD", args.len(), 3));
    }
    let Value::Integer(base) = args[0] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("_POW_MOD: expected number, got {}", type_name(&args[0])),
        ));
    };
    let Value::Integer(exp) = args[1] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("_POW_MOD: expected number, got {}", type_name(&args[1])),
        ));
    };
    let Value::Integer(m) = args[2] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("_POW_MOD: expected number, got {}", type_name(&args[2])),
        ));
    };
    if m == 0 {
        return Ok(Outcome::normal(domain_error(
            "POW_MOD",
            "POW_MOD: modulus must not be zero".to_string(),
        )));
    }
    if exp < 0 {
        return Ok(Outcome::normal(domain_error(
            "POW_MOD",
            "POW_MOD: exponent must be >= 0".to_string(),
        )));
    }
    let modulus = i128::from(m);
    // `rem_euclid` 而不是 Rust 的 `%`:后者的结果符号跟**左**操作数走,
    // 于是 `POW_MOD(-7, 3, 5)` 会得 -1 而不是 4。取模运算的惯例是结果与
    // 模数同号(Go / Python / C 的 `%` 在被除数为负时给出负余数,那是另一种
    // 约定,但**这个成员的名字是 mod 而不是 rem**,所以按欧几里得余数)。
    let mut result: i128 = 1i128.rem_euclid(modulus);
    let mut cur: i128 = (i128::from(base)).rem_euclid(modulus);
    let mut k = i128::from(exp);
    while k > 0 {
        if k % 2 == 1 {
            result = (result * cur).rem_euclid(modulus);
        }
        k /= 2;
        if k > 0 {
            cur = (cur * cur).rem_euclid(modulus);
        }
    }
    // 模数在 i64 范围内 ⇒ 余数也必然落在 i64 范围内。
    Ok(Outcome::normal(Value::Integer(result as i64)))
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
    // [v0.11.2 M4] 超越函数族 + TRUNC。表从 6 条涨到 21 条,这是「把纯 wlwl
    // 表达不出的东西放 R2」的记账成本,不是退化 —— 表存在的意义是给守卫
    // 一个**共享 kernel 的名册**作为反方向的事实基准。
    ("_LN", kernel_ln as StdFn),
    ("_LOG2", kernel_log2 as StdFn),
    ("_LOG10", kernel_log10 as StdFn),
    ("_EXP", kernel_exp as StdFn),
    ("_TRUNC", kernel_trunc as StdFn),
    ("_SIN", kernel_sin as StdFn),
    ("_COS", kernel_cos as StdFn),
    ("_TAN", kernel_tan as StdFn),
    ("_ASIN", kernel_asin as StdFn),
    ("_ACOS", kernel_acos as StdFn),
    ("_ATAN", kernel_atan as StdFn),
    ("_ATAN2", kernel_atan2 as StdFn),
    ("_SINH", kernel_sinh as StdFn),
    ("_COSH", kernel_cosh as StdFn),
    ("_TANH", kernel_tanh as StdFn),
    ("_POW_MOD", kernel_pow_mod as StdFn),
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
