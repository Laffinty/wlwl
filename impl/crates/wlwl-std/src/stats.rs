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

use std::collections::HashMap;

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

/// 取原数组的**原始元素**(不经 `f64` 转换)。
///
/// `series()` 会把 `INTEGER` 直接抹成 `f64`,原始 `Value` 的静态类型就在
/// 那一步丢了。`MODE` 要「回查原数组取那个元素的原值」,所以这里另借一份
/// **只读视图** —— 不改 `series()` 的对外行为(其余统计成员仍只拿 `f64`)。
fn raw_items(v: &Value) -> &[Value] {
    match v {
        Value::Array(items) => items.as_slice(),
        // `series()` 已把非数组判成 `E0030` ⇒ 非数组到不了这里。
        _ => &[],
    }
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
///
/// ⚠ **返回「数组中那个元素的原值」,原样保留其静态类型**(`F-05` 裁决,
/// 2026-10-20):纯 `INTEGER` 数组 ⇒ `INTEGER`、纯 `FLOAT` 数组 ⇒ `FLOAT`、
/// **混装 ⇒ `FLOAT`**(语言规范 §2.2 的混装提升)。
/// 此前返回类型由**内核的 `f64` 表示**决定(与输入类型无关),于是
/// `MODE([1, 1, 2])` 得 `1.0` / `FLOAT`,而同族 `SUM([1, 2, 3])` 保持
/// `INTEGER` —— 两兄弟口径不对称。
pub fn mode(host: &mut dyn StdHost, args: Vec<Value>) -> R<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "MODE", args.len(), 1));
    }
    let xs = series(host, "MODE", &args[0])?;
    if xs.is_empty() || !finite(&xs) {
        return Ok(null());
    }
    // [D13-015] 单趟 O(n) 计数。原实现对**每个**元素全量扫一遍数组数它的出现
    // 次数 ⇒ Θ(n²)(每元素成本随 n 线性上涨,n = 100 000 外推约 1.36 s)。
    //
    // ⚠ **键不能直接用 `to_bits()`**:`0.0` 与 `-0.0` 的位模式不同
    // (`0x0000…` vs `0x8000…`),会把它们拆成两组 ⇒ `MODE([0.0, NEG(0.0), 1.0])`
    // 从 `0.0` 退化成 `NULL`。而 IEEE 下 `0.0 == -0.0` 为真,旧实现正是按 `==`
    // 分组的,Python `statistics.mode` 也把它们当一组(dict 把 `-0.0` 哈希成 `0.0`)。
    // ⇒ 键必须先把有符号零规范化掉,见 `mode_key`。
    //
    // ⚠ `f64` **没有实现 `Eq` / `Hash`**(`NaN` 不自反),所以 `HashMap<f64, _>`
    // 连编译都过不了 —— 实现者必然要自己挑键表示,而两种显然的挑法
    // (`to_bits()` 与 `total_cmp` 次序)**都**按符号区分零 ⇒ 这个坑是必踩的,
    // 不是「小心一点就能避开」。契约 `mode_treats_signed_zeros_as_one_group_and_
    // keeps_the_first_sign` 把它钉死,并做过负向验证(换回 `to_bits()` 即变红)。
    let mut buckets: HashMap<u64, (usize, f64, usize)> = HashMap::new();
    for (i, &x) in xs.iter().enumerate() {
        match buckets.get_mut(&mode_key(x)) {
            // 命中已有桶:只加计数,**保留首次出现的下标与原值**(符号就在原值里)。
            Some(slot) => slot.2 += 1,
            None => {
                buckets.insert(mode_key(x), (i, x, 1));
            }
        }
    }
    // 选「全局最大计数里的第一个」= 计数最大、并列时首次下标最小。
    // ⚠ `buckets` 是 HashMap,`values()` 顺序不确定 ⇒ 并列必须**显式**按下标比,
    // 不能靠遍历顺序蒙对。旧实现的增量扫描恰好等价于此(第一次达到全局最大值的
    // 元素把 `best` 顶上去,之后再也没有更大的计数能顶掉它),语义逐字保持。
    let mut best: Option<(usize, f64, usize)> = None;
    // ⚠ `buckets` 是 HashMap,`values()` 顺序不确定 ⇒ 并列必须**显式**按下标比,
    // 不能靠遍历顺序蒙对。旧实现的增量扫描恰好等价于此(第一次达到全局最大值的
    // 元素把 `best` 顶上去,之后再也没有更大的计数能顶掉它),语义逐字保持。
    //
    // ⚠ 写法从 `for (_, &(i, x, c)) in buckets.iter()` 改成 `values()`
    // ([D13-053]):clippy 1.99 的 `for_kv_map`(由 `-D warnings` 提升为 error)
    // 判它「在遍历 map 的 values 却解构键值对」。⚠ **改写不影响遍历顺序** ——
    // `values()` 与 `iter().map(|(_, v)| v)` 顺序完全相同,而上面那条注释已
    // 明说顺序根本不承重。
    for &(i, x, c) in buckets.values() {
        match best {
            None => best = Some((i, x, c)),
            Some((bi, _, bc)) if c > bc || (c == bc && i < bi) => best = Some((i, x, c)),
            Some(_) => {}
        }
    }
    // ⚠ `c > 1` 是**与 Python 的有意分歧**(`statistics.mode([1])` 返回 `1`):
    // 只出现一次 ⇒ 没有众数 ⇒ `NULL`。这一条不许在重写时"顺手对齐"。
    match best {
        Some((i, x, c)) if c > 1 => Ok(Outcome::normal(mode_value(&args[0], i, x))),
        _ => Ok(null()),
    }
}

/// `MODE` 的返回值:**原数组里那个元素的原值**,静态类型原样保留。
///
/// - 纯 `INTEGER` 数组 ⇒ `Value::Integer`(取原值,不做 `f64` 往返 ⇒ 大整数不丢精度);
/// - 含任一 `FLOAT`(纯浮点或混装)⇒ `Value::Float`(语言规范 §2.2 的混装提升)。
///
/// ⚠ 混装时用的是**桶里存的那个 `f64`**(首次出现者的值):有符号零的符号就在
/// 里面(见 `mode_key` 的注释),改成从原 `Value` 转换会把符号抹掉。
/// ⚠ `i` 是胜出分组的**首次出现下标** ⇒ 纯整数路径回查到的就是该分组的
/// 首个元素,符号 / 数值都原样。
fn mode_value(arr: &Value, i: usize, x: f64) -> Value {
    let items = raw_items(arr);
    // ⚠ 混装判据看**整个输入**而不是只看胜出元素:输入里有 `FLOAT` 就是混装,
    // 哪怕那个 `FLOAT` 不是众数(否则 `MODE([1, 1, 2.0])` 会返回 `INTEGER` 的 `1`)。
    let mixed_float = items.iter().any(|v| matches!(v, Value::Float(_)));
    if mixed_float {
        return Value::Float(x);
    }
    match items.get(i) {
        Some(Value::Integer(n)) => Value::Integer(*n),
        // 理论到不了:非混装 ⇒ 全部元素都是 `INTEGER`。
        _ => Value::Float(x),
    }
}

/// `MODE` 的桶键:把 `-0.0` 规范化成 `+0.0`,其余按位模式。
///
/// `finite()` 已在调用前排除 `NaN`,所以 `x == 0.0` 恰好就是「x 是某个有符号零」
/// 的判定,不会误伤别的值。返回 `+0.0` 的位模式而非常量 `0`,是为了与分支里的
/// `x.to_bits()` 保持同一种表示。
#[inline]
fn mode_key(x: f64) -> u64 {
    if x == 0.0 {
        0.0f64.to_bits()
    } else {
        x.to_bits()
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
    Ok(Outcome::normal(Value::dict(vec![
        (Value::String("slope".into()), Value::Float(slope)),
        (
            Value::String("intercept".into()),
            Value::Float(my - slope * mx),
        ),
    ])))
}
