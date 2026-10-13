//! [v0.11.3 M7 / addendum-10 W-05] `wlwl:std.math` 的描述统计内核。
//!
//! ## 三个已经定死的口径
//!
//! * **不含 `MEAN`**(`ADR-0031` D2):`std.collection::SUM` 加 `/` 已覆盖。
//! * **混装沿用 `SUM` 的静默提升**(`ADR-0031` D3):`ARRAY[INTEGER]` 与
//!   `ARRAY[FLOAT]` 混装**不**报 `E0030`。实测既有的 `SUM([1, 2.5])` 得 `3.5`,
//!   统计族若收紧就会与它分裂,而分裂意味着其中一处是错的。
//! * **空数组 → `NULL`**(沿用 `std.collection` §5 口径),本模块**不发明**新口径。
//!
//! ## 未定义 ↔ 空 的区分(容易读混,写在这里)
//!
//! 「空数组」与「统计量在该输入上无定义」是两回事:
//!
//! | 输入 | 结果 | 为什么 |
//! |---|---|---|
//! | `[]` | `NULL` | 没有样本,与 `std.collection` §5 一致 |
//! | `[x]` 的**样本**标准差 / 方差 | `NULL` | 分母是 `n-1 = 0`,**数学上无定义**,不是错误 |
//! | `[x]` 的**总体**标准差 / 方差 | `0.0` | 分母是 `n = 1`,有定义 |
//!
//! `NaN` 会渗进统计量吗?—— **不会**:任一元素是 `NaN` 或 `±inf` 时,本模块一律返回
//! `NULL`。理由是 wlwl 没有浮点异常的传播约定(`/(0.0, 0.0)` 直接 `E1003`),让
//! `NaN` 渗进统计量会让调用方**看不出**结果不可信,而 `NULL` 是看得见的。

use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

type R<T> = wlwl_error::WlwlResult<T>;

/// 取实数序列。
///
/// ⚠ 混装**静默提升**(`ADR-0031` D3);非数组 / 非数值元素 ⇒ `E0030`。
fn series(host: &mut dyn StdHost, name: &str, v: &Value) -> R<Vec<f64>> {
    let Value::Array(items) = v else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{name}: expected ARRAY, got {}", wlwl_value::type_name(v)),
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        match it {
            Value::Integer(i) => out.push(*i as f64),
            Value::Float(f) => out.push(*f),
            other => {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!(
                        "{name}: element must be INTEGER or FLOAT, got {}",
                        wlwl_value::type_name(other)
                    ),
                ));
            }
        }
    }
    Ok(out)
}

/// 取两个**等长**序列;不等长 ⇒ `E0030`(计划 §2.3)。
fn pair(host: &mut dyn StdHost, name: &str, a: &Value, b: &Value) -> R<(Vec<f64>, Vec<f64>)> {
    let xs = series(host, name, a)?;
    let ys = series(host, name, b)?;
    if xs.len() != ys.len() {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "{name}: the two arrays must have equal length, got {} and {}",
                xs.len(),
                ys.len()
            ),
        ));
    }
    Ok((xs, ys))
}

/// 含 `NaN` / `±inf` ⇒ 该序列的统计量无意义(见文件头)。
fn finite(xs: &[f64]) -> bool {
    xs.iter().all(|v| v.is_finite())
}

fn ok_f(v: f64) -> Outcome {
    Outcome::normal(Value::Float(v))
}

fn null() -> Outcome {
    Outcome::normal(Value::Null)
}

fn sorted(xs: Vec<f64>) -> Vec<f64> {
    let mut v = xs;
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v
}

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

// ── MEDIAN ────────────────────────────────────────────────────────────

/// `MEDIAN(arr)` —— 偶数长度取**中间两个的平均**(计划 §3.5「写死」)。
///
/// ⚠ 平均那一步必须走 `f64`:整数数组下用整数 `/` 会被**截断**
/// (`/(10, 4)` 得 2 而不是 2.5,见 `ADR-0031` §3)。
pub fn median(host: &mut dyn StdHost, args: Vec<Value>) -> R<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "MEDIAN", args.len(), 1));
    }
    let xs = series(host, "MEDIAN", &args[0])?;
    if xs.is_empty() || !finite(&xs) {
        return Ok(null());
    }
    let v = sorted(xs);
    let n = v.len();
    Ok(if n % 2 == 1 {
        ok_f(v[n / 2])
    } else {
        ok_f((v[n / 2 - 1] + v[n / 2]) / 2.0)
    })
}

// ── MODE ──────────────────────────────────────────────────────────────

/// `MODE(arr)` —— 出现次数最多的值。
///
/// ⚠ **并列取数组中先出现的那个**(与 Python `statistics.mode` 一致)。
/// 「取最小」更确定但会让 `MODE([2, 1, 2, 1])` 与 `MODE([1, 2, 1, 2])` 分道扬镳;
/// 「并列就报错」在对称输入上过于苛刻。先出现者至少是**可预测**的。
/// ⚠ **全部只出现一次 ⇒ 没有众数 ⇒ `NULL`**(Python `statistics.multimode` 的情形)。
pub fn mode(host: &mut dyn StdHost, args: Vec<Value>) -> R<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "MODE", args.len(), 1));
    }
    let xs = series(host, "MODE", &args[0])?;
    if xs.is_empty() || !finite(&xs) {
        return Ok(null());
    }
    let mut best: Option<f64> = None;
    let mut best_n = 0usize;
    for &x in &xs {
        let n = xs.iter().filter(|y| **y == x).count();
        if n > best_n {
            best_n = n;
            best = Some(x);
        }
    }
    match (best, best_n) {
        (Some(x), n) if n > 1 => Ok(Outcome::normal(Value::Float(x))),
        _ => Ok(null()),
    }
}

// ── VARIANCE / STDDEV ─────────────────────────────────────────────────

/// 样本 / 总体两套**显式分开的成员**(计划 §2.3)。
///
/// ⚠ **样本的 `n = 1` 返回 `NULL` 而不是报错**:分母 `n-1 = 0`,数学上无定义。
fn variance(xs: &[f64], population: bool) -> Option<f64> {
    if xs.is_empty() || !finite(xs) {
        return None;
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let ss: f64 = xs.iter().map(|x| (x - mean) * (x - mean)).sum();
    let denom = if population { n } else { n - 1.0 };
    if denom <= 0.0 {
        return None;
    }
    Some(ss / denom)
}

macro_rules! variance_member {
    ($name:ident, $fn_name:literal, $population:expr, $sqrt:expr) => {
        pub fn $name(host: &mut dyn StdHost, args: Vec<Value>) -> R<Outcome> {
            if args.len() != 1 {
                return Err(arity(host, $fn_name, args.len(), 1));
            }
            let xs = series(host, $fn_name, &args[0])?;
            Ok(match variance(&xs, $population) {
                Some(v) if $sqrt => ok_f(if v < 0.0 { 0.0 } else { v.sqrt() }),
                Some(v) => ok_f(v),
                None => null(),
            })
        }
    };
}

variance_member!(variance_s, "VARIANCE_S", false, false);
variance_member!(variance_p, "VARIANCE_P", true, false);
variance_member!(stddev_s, "STDDEV_S", false, true);
variance_member!(stddev_p, "STDDEV_P", true, true);

// ── PERCENTILE ────────────────────────────────────────────────────────

/// `PERCENTILE(arr, p)` —— `p ∈ [0, 100]`,**线性插值**(含端点,等价于
/// NumPy 的 `method="linear"` / Python `statistics.quantiles(method="inclusive")`)。
///
/// ⚠ 插值方式必须写明,因为它是**可观察的**:同样一组数据,`p` 落在两个观测点之间时,
/// 线性 / 较低 / 较高 / 中点 / 哈德利五种方法给出**不同**的值。
/// ⚠ `p` 越界 / 空数组 / 含非有限值 ⇒ `NULL`(不是 `E0030` —— `p` 是**数据**不是参数错)。
pub fn percentile(host: &mut dyn StdHost, args: Vec<Value>) -> R<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "PERCENTILE", args.len(), 2));
    }
    let xs = series(host, "PERCENTILE", &args[0])?;
    let Value::Float(p) = args[1] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "PERCENTILE: p must be FLOAT, got {}",
                wlwl_value::type_name(&args[1])
            ),
        ));
    };
    if !(0.0..=100.0).contains(&p) || xs.is_empty() || !finite(&xs) {
        return Ok(null());
    }
    let v = sorted(xs);
    let n = v.len();
    if n == 1 {
        return Ok(ok_f(v[0]));
    }
    // rank = p/100 × (n-1),线性插值
    let rank = (p / 100.0) * (n - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return Ok(ok_f(v[lo]));
    }
    let frac = rank - lo as f64;
    Ok(ok_f(v[lo] * (1.0 - frac) + v[hi] * frac))
}

// ── CORRELATION / LINEAR_FIT ──────────────────────────────────────────

/// `CORRELATION(xs, ys)` —— Pearson 相关系数。
///
/// ⚠ `n < 2` 或任一方向方差为零 ⇒ `NULL`(相关系数**无定义**,不是 0)。
/// ⚠ `n = 2` 时相关系数恒为 `±1`(两点永远共线),这是数学事实不是缺陷。
pub fn correlation(host: &mut dyn StdHost, args: Vec<Value>) -> R<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "CORRELATION", args.len(), 2));
    }
    let (xs, ys) = pair(host, "CORRELATION", &args[0], &args[1])?;
    let n = xs.len();
    if n < 2 || !finite(&xs) || !finite(&ys) {
        return Ok(null());
    }
    let nf = n as f64;
    let mx = xs.iter().sum::<f64>() / nf;
    let my = ys.iter().sum::<f64>() / nf;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in xs.iter().zip(ys.iter()) {
        let (dx, dy) = (x - mx, y - my);
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    let den = (sxx * syy).sqrt();
    if den == 0.0 {
        return Ok(null()); // 任一方向无方差 ⇒ 相关系数无定义
    }
    Ok(ok_f(sxy / den))
}

/// `LINEAR_FIT(xs, ys)` → `["slope": FLOAT, "intercept": FLOAT]`。
///
/// ⚠ **`x` 方向方差为零 ⇒ `NULL`**(斜率无定义,不是 0)—— 与 `CORRELATION` 同款理由。
/// ⚠ 只做**最小二乘直线**,返回**系数**,不是拟合后的点集。
pub fn linear_fit(host: &mut dyn StdHost, args: Vec<Value>) -> R<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "LINEAR_FIT", args.len(), 2));
    }
    let (xs, ys) = pair(host, "LINEAR_FIT", &args[0], &args[1])?;
    let n = xs.len();
    if n < 2 || !finite(&xs) || !finite(&ys) {
        return Ok(null());
    }
    let nf = n as f64;
    let mx = xs.iter().sum::<f64>() / nf;
    let my = ys.iter().sum::<f64>() / nf;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (x, y) in xs.iter().zip(ys.iter()) {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
    }
    if sxx == 0.0 {
        return Ok(null()); // x 无方差 ⇒ 斜率无定义
    }
    let slope = sxy / sxx;
    Ok(Outcome::normal(Value::Dict(vec![
        (Value::String("slope".into()), Value::Float(slope)),
        (
            Value::String("intercept".into()),
            Value::Float(my - slope * mx),
        ),
    ])))
}
