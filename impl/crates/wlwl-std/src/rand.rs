//! [v0.11.3 M6 / addendum-08] `wlwl:std.rand` —— **显式播种**的 RNG(R2)。
//!
//! 这是本语言**第一个**随机数能力,而它一直是「**没想清楚**」而不是「不重要」
//! —— 规范 §14.1 的原话:「一个**默认播种**的 RNG 会削弱可复现这个卖点,而一个
//! **要求显式播种**的 RNG 需要先裁决『全局状态 vs 显式传递』—— 那是**设计
//! 决策**,不是加几个函数」。[`ADR-0027`](../../../docs/adr/0027-std-rand-shape.md)
//! 已 **Accepted**,那个争点随之结项。
//!
//! ## 四条不能破的纪律(全部来自 ADR)
//!
//! 1. **状态是显式值,不是隐藏状态。** `SEED(entropy)` 造出一个值,之后每个成员
//!    的**第一个实参**都收它。**没有 `SET_SEED()`、没有「全局当前 RNG」。**
//!    契约里「同一 seed 建两个 RNG,两条序列完全相同」就是这条的守卫 —— 有隐藏
//!    状态时它不可能成立。
//! 2. **熵只从显式实参进来。** `SEED` **内部不读操作系统熵源**。要真随机就去
//!    `std.encode` 的 `RANDOM_BYTES` / `RANDOM_HEX` 取,然后显式传进来 ——
//!    `std.encode` 是熵源、`std.rand` 单向消费它(`addendum-04` §3.3)。
//!    **对把可复现当卖点的语言,「依赖可见」本身就是功能。**
//! 3. **只做密码学随机是错的、不做也是错的** —— 那是 `RANDOM_BYTES` 的活。
//!    两个命名空间、两套用途,**不合并**。
//! 4. **可复现是硬承诺**:同一 `(熵, 调用序列)` ⇒ 同一输出,**逐字节**。
//!    ⇒ 算法换掉就是 breaking,必须同批更新向量并在 CHANGELOG 显式记录。
//!
//! ## 算法(规范文本必须写全这一段,否则第三方无法复现向量)
//!
//! - **播种**:熵字节 → **FNV-1a-64** 折叠成一个 `u64` → **SplitMix64** 展开成
//!   **256 bit 状态**(4 × `u64`)。展开后**四字全零**则按固定常数
//!   `0x9E3779B97F4A7C15` 再展开一次 —— xoshiro 的全零态是**吸收态**(恒输出 0),
//!   不守卫就会静默退化成一条无意义序列。
//! - **生成**:**xoshiro256++**。
//! - **浮点**:`UNIFORM` 取 `(next_u64() >> 11) as f64 * 2^-53`。右移 11 位拿到
//!   恰好 53 bit 尾数 ⇒ 结果**可被保证**落在 `[0, 1)`,而不是「恰好不会返 1.0」。
//! - **整数**:走**无偏取模拒绝**(`u128` 算 `2^64 mod range` 的截断点),
//!   **绝不经过 `FLOAT`** —— 浮点只有 53 bit 尾数,中转会引入舍入偏差,且对大区间
//!   **静默收窄上界**(不报错、只给偏斜序列)。
//!
//! ## ⚠ 状态字按 `i64` 位模式存放(实现层事实)
//!
//! xoshiro 的状态是 `u64`(最大 18 446 744 073 709 551 615),而 wlwl 的
//! `INTEGER` 是 **`i64`**。⇒ 状态字以 **`i64` 位模式**存放,显示为**有符号数**
//! (多数会是负数);成员读回时按**位重解释**为 `u64`。**逐字节往返无损**,且
//! 用户手写正数状态(`42` → `42u64`)同样合法。这是「不加内建类型」(ADR A2)
//! 唯一的代价,写在这里而不是留给下一个人猜。
//!
//! ## 成员只有四个(A7:本批先落 `std.rand`)
//!
//! `SEED` / `UNIFORM` / `INT_RANGE` / `CODEPOINT`。
//! ⚠️ **「降级为官方包」是过渡形态**(规范 §14 已登记「`std.ai` / `std.agent`
//! 降级为官方包」的方向)—— 官方包机制一旦存在,`std.rand` 按 §14 降级。
//! **不把这句话写进规范,它就会变成永久事实。**

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.rand",
    functions: &[
        ("SEED", seed as StdFn),
        ("UNIFORM", uniform as StdFn),
        ("INT_RANGE", int_range as StdFn),
        ("CODEPOINT", codepoint as StdFn),
    ],
};

// ── 助手 ───────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, want: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected {want}, got {}", crate::value_kind(got)),
    )
}

fn bad(host: &mut dyn StdHost, name: &str, msg: String) -> wlwl_error::WlwlError {
    host.diag(ErrorCode::E0030, format!("{name}: {msg}"))
}

// ── 常量 ───────────────────────────────────────────────────────────────

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Unicode 标量值上界(不含)。
const MAX_SCALAR: u64 = 0x11_0000;

// ── 熵 → 状态 ──────────────────────────────────────────────────────────

/// FNV-1a 64:把任意长度的熵字节折叠成一个 `u64`。
///
/// 为什么需要一个折叠:熵是**变长**的(`RANDOM_BYTES(16)` 是 16 字节、`""` 是 0
/// 字节),而 SplitMix64 要的是一个 `u64` 种子。
/// ⚠️ FNV **不是**密码学哈希,这里是**熵已经均匀**之后的一次归一化,不承担任何
/// 安全职责 —— 真熵来自 `std.encode` 的 OS 熵源。
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GOLDEN);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// 熵字节 → xoshiro256++ 的 4 × `u64` 状态。
///
/// **零状态守卫在这里**:四字全零是 xoshiro 的**吸收态**(输出恒为 0)。概率是
/// 2^-256,但那个输入一旦出现,冻结向量照样绿,而 `std.rand` 静默退化 —— 所以
/// 它被**显式挡掉**,并且读状态时再挡一次(`rng_from_value`)。
fn state_from_entropy(entropy: &[u8]) -> [u64; 4] {
    let mut sm = SplitMix64(fnv1a64(entropy));
    let mut s = [sm.next(), sm.next(), sm.next(), sm.next()];
    if s == [0; 4] {
        let mut fixed = SplitMix64(GOLDEN);
        s = [fixed.next(), fixed.next(), fixed.next(), fixed.next()];
    }
    s
}

// ── 生成器 ─────────────────────────────────────────────────────────────

/// `[0, n)` 上的**无偏**整数 —— 取模拒绝。
///
/// 朴素 `x % n` 在 `2^64 % n != 0` 时是**有偏**的:`0..=u64::MAX` 的末尾有一块
/// 不完整的剩余,取模后那一段的值会**多占**概率。这里先算出截断点
/// `2^64 - (2^64 % n)`,抽到截断点以上就**重抽** —— 平均多抽不到一次。
///
/// `u128` 只在这个算式里出现(避免 `2^64` 与 `hi - lo` 溢出),不进热路径。
///
/// ⚠️ **这个函数刻意对取源 `F` 泛型**,不是为了漂亮,是为了让「拒绝」这条
/// 路径可以被**确定性地**测:`n = 7` 时 `2^64 mod 7 = 2`,两个最高的 u64 必然
/// 被拒 —— 用一个手写的取源就能验证它**确实重抽了**,而统计测试**永远抓不到**
/// 这个缺陷(偏差量级是 `n / 2^64`,任何抽样都看不出来)。
fn unbiased_with<F: FnMut() -> u64>(mut draw: F, n: u64) -> u64 {
    assert!(n > 0, "unbiased_with: n must be positive");
    if n == 1 {
        return 0;
    }
    let n128 = u128::from(n);
    let cutoff = (1u128 << 64) - ((1u128 << 64) % n128);
    loop {
        let x = u128::from(draw());
        if x < cutoff {
            return (x % n128) as u64;
        }
    }
}

/// xoshiro256++ 的**内部**状态机。
///
/// ⚠️ `#[doc(hidden)] pub` 的唯一理由:`benches/rand.rs` 在另一个 crate 里,
/// 而 ADR-0027 A2 明写「`DICT` 的字典构造**要进基准,不得假设它免费**」——
/// 要量那个代价,就必须能跑**绕过 `DICT`** 的同一序列作对照。
/// ⇒ **它不是 API 面**,外部代码不要直接依赖(签名随时可能变)。
#[doc(hidden)]
pub struct Xoshiro([u64; 4]);

impl Xoshiro {
    /// 从 4 个状态字构造。仅供基准的对照路径使用(成员一律走
    /// `SEED`,那才是「熵 → 状态」的唯一正门)。
    #[doc(hidden)]
    pub fn new(s: [u64; 4]) -> Self {
        Self(s)
    }

    fn next_u64(&mut self) -> u64 {
        let s = &mut self.0;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// `[0, n)` 上的**无偏**整数(取模拒绝)。
    ///
    /// 朴素 `x % n` 在 `2^64 % n != 0` 时是**有偏**的(短的那块多占概率)。
    #[doc(hidden)]
    pub fn unbiased_below(&mut self, n: u64) -> u64 {
        unbiased_with(|| self.next_u64(), n)
    }

    /// `[0, 1)` 的 `f64`,取**恰好 53 bit** 尾数。
    ///
    /// ⚠️ `#[doc(hidden)] pub` 只为基准的对照路径(见类型注释)—— 外部代码
    /// 应当走成员,而不是自己推进内部状态机。
    #[doc(hidden)]
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
}

/// 一个码点是不是「可以安全交付」的 Unicode **标量值**。
///
/// - 排除 **UTF-16 代理区** `0xD800..=0xDFFF`:它们**不是**标量值,任何合法
///   UTF-8 编码都不可能产出,`char::from_u32` 也会返回 `None`。
/// - 排除 **永久非字符** `0xFDD0..=0xFDEF` 与尾数位非零的 `U+xFFFE` / `U+xFFFF`
///   (Unicode 3.9 起归为 Noncharacter):它们能编码、能进 `char`,但**在任何
///   交换文本里都不该出现**,交给终端只会显示成替换符。
fn is_scalar_value(cp: u64) -> bool {
    if (0xd800..=0xdfff).contains(&cp) {
        return false;
    }
    if (0xfdd0..=0xfdef).contains(&cp) {
        return false;
    }
    cp & 0xfffe != 0xfffe
}

// ── `i64` ↔ `u64` 的位模式桥 ───────────────────────────────────────────

/// `Value::Integer` 是 `i64`,xoshiro 状态是 `u64` ⇒ 按**位**互转,不经数值。
///
/// 注意是 `wrapping` 而不是饱和:一个正数被当成位模式时(例如用户手写
/// `s0 = 42`),`42i64 as u64` 就是 `42`;而一个真 u64 的高位字(如
/// `0x9e37…`)会显示成负数。两者都**往返无损**,这正是要的性质。
fn to_value(u: u64) -> Value {
    Value::Integer(u as i64)
}

fn state_to_value(s: [u64; 4]) -> Value {
    Value::Dict(
        ["s0", "s1", "s2", "s3"]
            .into_iter()
            .zip(s)
            .map(|(k, v)| (Value::String(k.into()), to_value(v)))
            .collect(),
    )
}

fn rng_to_value(r: Xoshiro) -> Value {
    state_to_value(r.0)
}

fn rng_from_value(
    host: &mut dyn StdHost,
    name: &str,
    v: &Value,
) -> wlwl_error::WlwlResult<Xoshiro> {
    let Value::Dict(entries) = v else {
        return Err(type_err(host, name, "a RNG state dictionary", v));
    };
    let mut s = [0u64; 4];
    for (i, key) in ["s0", "s1", "s2", "s3"].into_iter().enumerate() {
        let found = entries
            .iter()
            .find(|(k, _)| matches!(k, Value::String(kk) if kk == key))
            .map(|(_, val)| val);
        match found {
            Some(Value::Integer(n)) => s[i] = *n as u64,
            Some(other) => {
                return Err(bad(
                    host,
                    name,
                    format!(
                        "state word `{key}` must be an integer, got {}",
                        crate::value_kind(other)
                    ),
                ))
            }
            None => {
                return Err(bad(
                    host,
                    name,
                    format!("state dictionary is missing `{key}` (see std.rand::SEED)"),
                ))
            }
        }
    }
    // 读侧**再挡一次**零状态:播种时已经挡过,但用户可以手写这个字典,
    // 而它的输出恒为 0 —— 与其静默给出一条无意义序列,不如报错。
    if s == [0; 4] {
        return Err(bad(
            host,
            name,
            "state is the xoshiro all-zero state, which is absorbing (always 0); \
             re-seed with SEED"
                .into(),
        ));
    }
    Ok(Xoshiro(s))
}

/// 把熵实参归一化成字节。
fn entropy_bytes(host: &mut dyn StdHost, name: &str, v: &Value) -> wlwl_error::WlwlResult<Vec<u8>> {
    match v {
        Value::String(s) => Ok(s.as_bytes().to_vec()),
        // 字节数组 = `RANDOM_BYTES(n)` 的返回。逐元素校验:超范围的整数
        // 静默截断会让「同一个熵」有两种解释。
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                let Value::Integer(b) = it else {
                    return Err(type_err(host, name, "a byte array of integers", it));
                };
                if !(0..=255).contains(b) {
                    return Err(bad(host, name, format!("byte {b} is outside 0..=255")));
                }
                out.push(*b as u8);
            }
            Ok(out)
        }
        other => Err(type_err(host, name, "a string or a byte array", other)),
    }
}

/// 端点既收 `FLOAT` 也收 `INTEGER` —— `UNIFORM(rng, 0, 10)` 是最自然的写法,
/// 让它报 `E0030` 是刻意的用户敌意。返回 `(f64, f64)`。
fn float_endpoint(host: &mut dyn StdHost, name: &str, v: &Value) -> wlwl_error::WlwlResult<f64> {
    match v {
        Value::Float(f) => Ok(*f),
        Value::Integer(i) => Ok(*i as f64),
        other => Err(type_err(host, name, "a number", other)),
    }
}

// ── 成员 ───────────────────────────────────────────────────────────────

/// `SEED(entropy) -> DICT`
///
/// 从**显式熵**造一个 RNG 状态值:`["s0": …, "s1": …, "s2": …, "s3": …]`。
/// 之后每个成员的第一个实参都收它 —— **没有 `SET_SEED()`、没有全局当前 RNG**。
///
/// 熵接受 `STRING`(按 **UTF-8 字节**取)或 `ARRAY`(字节数组,即
/// `RANDOM_BYTES(n)` 的返回)。⚠️ 写死这一条:否则「同一个熵」在两种字面量下会
/// 给出两个不同序列,而契约冻的就是字节。
pub fn seed(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "SEED";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let entropy = entropy_bytes(host, NAME, &args[0])?;
    Ok(Outcome::normal(state_to_value(state_from_entropy(
        &entropy,
    ))))
}

/// `UNIFORM(rng) -> [FLOAT, DICT]`([0, 1)) / `UNIFORM(rng, a, b) -> [FLOAT, DICT]`([a, b))
///
/// 两种形态是**同一个成员**:`args.len() ∈ {1, 3}`。
/// ⚠️ **2 个实参是 `E0022`**,不是「`a` 取默认」—— `StdFn` 签名层没有可选形参
/// 构造(它是裸函数指针,元数由每个成员自己 `args.len()` 判),而把 2 参当成
/// 「合法的第三形态」正是语义漏洞最容易长出来的地方。
///
/// 区间是**左闭右开** `[a, b)`。`a >= b` 是空区间 → `E0030`;端点是 `NaN`
/// 也走同一条(`NaN` 是合法的 `FLOAT`,而 `[a, b)` 对它没有定义)。
///
/// ## ⚠️ 为什么返回 `[值, 新状态]` 而不是标量(ADR-0027 补强 4)
///
/// **`DICT` 是不可变的**(语言规范 §2.1),而 `SET_PROP` / `INDEX_SET` 对字典
/// 也是「返回**新**字典、接收者不变」(§13.4)。⇒ 若成员只返回标量,**状态永不
/// 前进**:同一个 `rng` 值反复调用拿到的是**同一个数**(实测 2000 次全部相同)。
///
/// 所以状态与值**一起**返回,用语言**已有**的数组模式绑定消费:
///
/// ```
/// LET(r0, SEED("seed"));
/// LET([v, r1], UNIFORM(r0));        // r1 是**已推进**的状态
/// LET([w, r2], UNIFORM(r1));
/// ```
///
/// 这样「忘了推进状态」**不再是可能的惯用写法** —— 拿不到值,除非同时拿到新状态。
pub fn uniform(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "UNIFORM";
    if args.len() != 1 && args.len() != 3 {
        // 2 参落到这里 —— 消息里点明它不是默认形态。
        return Err(arity(host, NAME, args.len(), 1));
    }
    let mut rng = rng_from_value(host, NAME, &args[0])?;
    let u = rng.unit();
    if args.len() == 1 {
        return Ok(Outcome::normal(Value::Array(vec![
            Value::Float(u),
            rng_to_value(rng),
        ])));
    }
    let a = float_endpoint(host, NAME, &args[1])?;
    let b = float_endpoint(host, NAME, &args[2])?;
    // ⚠ 这里**必须**是「不是小于」而不是「大于等于」:`a` / `b` 是 `f64`,
    // 端点可以传 `NaN`(它是合法的 `FLOAT` 值)。`!(a < b)` 对 NaN 为 **真**
    // (⇒ 报错,而不是静默返回一个 NaN 结果),而 `a >= b` 对 NaN 为 **假**
    // (⇒ NaN 会一路穿到结果里)。两者语义不同,所以这里走 `partial_cmp`
    // 把「小于」单独判出来 —— 这也是 clippy 那条 `neg_cmp_op_on_partial_ord`
    // 想让我们显式表达的东西。
    match a.partial_cmp(&b) {
        Some(std::cmp::Ordering::Less) => {}
        _ => {
            return Err(bad(
                host,
                NAME,
                format!(
                    "[a, b) is empty when a >= b (or an endpoint is NaN), got a = {a}, b = {b}"
                ),
            ));
        }
    }
    let mut v = a + u * (b - a);
    // 舍入可能把结果**顶到 b**:`a` 很大而区间比 `a` 的一个 ulp 还窄时
    // (如 `UNIFORM(rng, 1e9, 1e9+1)`),`(b - a)` 的量级会低于半个 ulp。
    // ⇒ 「两端不包含」这条契约必须**被保证**,所以这里夹一次。
    if v >= b {
        v = f64::from_bits(b.to_bits() - 1);
    }
    if v < a {
        v = a;
    }
    Ok(Outcome::normal(Value::Array(vec![
        Value::Float(v),
        rng_to_value(rng),
    ])))
}

/// `INT_RANGE(rng, lo, hi) -> [INTEGER, DICT]`([lo, hi))
///
/// 整数区间上的均匀抽样,**无偏**。
///
/// ⚠️ **它不走上浮点的路**(ADR-0027 补强 2):`FLOAT` 只有 53 bit 尾数,
/// `a + u * (b - a)` 那条路径对大区间引入舍入偏差,而且超过 2^53 的区间**静默
/// 收窄上界** —— 不报错、只给一条偏斜序列。⇒ 这里全程 `u128`/`u64` 算术。
///
/// `lo >= hi` 是空区间 → `E0030`。区间**可以跨负数**。
pub fn int_range(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "INT_RANGE";
    if args.len() != 3 {
        return Err(arity(host, NAME, args.len(), 3));
    }
    let mut rng = rng_from_value(host, NAME, &args[0])?;
    let Value::Integer(lo) = &args[1] else {
        return Err(type_err(host, NAME, "an integer", &args[1]));
    };
    let Value::Integer(hi) = &args[2] else {
        return Err(type_err(host, NAME, "an integer", &args[2]));
    };
    let (lo, hi) = (*lo, *hi);
    if lo >= hi {
        return Err(bad(
            host,
            NAME,
            format!("[lo, hi) is empty when lo >= hi, got lo = {lo}, hi = {hi}"),
        ));
    }
    // ⚠️ 中转必须用 `i128`:`lo as u128` 对 `i64::MIN` 是**符号扩展**后的巨值,
    // `hi as u128 - lo as u128` 会下溢 panic(实测过)。i64 的全域差是 2^64-1,
    // 在 i128 里精确可表示。
    let span = (i128::from(hi) - i128::from(lo)) as u128;
    debug_assert!(span > 0 && span <= u128::from(u64::MAX));
    let m = rng.unbiased_below(span as u64);
    Ok(Outcome::normal(Value::Array(vec![
        Value::Integer(lo.wrapping_add(m as i64)),
        rng_to_value(rng),
    ])))
}

/// `CODEPOINT(rng) -> [STRING, DICT]`(**第一项是恰好一个字符的字符串**)
///
/// 抽一个 Unicode **标量值**,返回它的单字符字符串。
///
/// **为什么不能拿 `UNIFORM` 复用**:码点不是均匀分布 —— 代理区
/// `0xD800..=0xDFFF` 必须排除(它们**不是**标量值),永久非字符也一并排除。
/// 所以它需要一个**显式成员**,分布规则写在成员表上而不是藏在乘法里。
///
/// 返回 `STRING` 而不是 `INTEGER`(码点数值):调用方几乎总要用它拼字符串,
/// 而数值形态没有配套的「码点 → 字符串」成员,反倒要用户自己处理。
pub fn codepoint(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "CODEPOINT";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let mut rng = rng_from_value(host, NAME, &args[0])?;
    let cp = loop {
        let cp = rng.unbiased_below(MAX_SCALAR);
        if is_scalar_value(cp) {
            break cp;
        }
    };
    // 上面保证了是标量值,`char::from_u32` 不可能返回 `None`;真返回了就是
    // `is_scalar_value` 与 std 的口径不一致 —— 那必须响,不能 `unwrap_or('?')`
    // 悄悄换一个字符。
    let ch = char::from_u32(cp as u32).ok_or_else(|| {
        bad(
            host,
            NAME,
            format!("internal: {cp} passed is_scalar_value but is not a char"),
        )
    })?;
    Ok(Outcome::normal(Value::Array(vec![
        Value::String(ch.to_string()),
        rng_to_value(rng),
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 与 `time.rs` 同款:本模块**不碰宿主**(纯计算),这个 panic 把
    /// 「不经宿主」变成可执行断言。
    struct NullHost;

    impl StdHost for NullHost {
        fn ctx(&mut self) -> &mut crate::StdCtx {
            panic!("std.rand must not touch StdCtx (it is pure computation)")
        }
        fn call(
            &mut self,
            _f: &Value,
            _args: Vec<Value>,
            _name: &str,
        ) -> wlwl_error::WlwlResult<Outcome> {
            panic!("std.rand has no member that calls back into the host")
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

    fn seeded(entropy: &[u8]) -> Xoshiro {
        Xoshiro(state_from_entropy(entropy))
    }

    fn state_values(entropy: &[u8]) -> [u64; 4] {
        state_from_entropy(entropy)
    }

    // ── 逐字节冻结的向量(取自独立第二实现,见计划 §3.6)────────────────
    //
    // 这是本成员**最强的承诺**:算法一换这些就红 ⇒ 换算法必须同批更新并在
    // CHANGELOG 显式记录(ADR-0027 A3)。

    #[test]
    fn frozen_u64_streams_match_the_reference() {
        struct Case {
            entropy: &'static [u8],
            words: [&'static str; 10],
        }
        let cases = [
            Case {
                entropy: b"wlwl",
                words: [
                    "3e6d5f811478f85b",
                    "cbcf9d845044c892",
                    "96a266b053fa17d9",
                    "37146c2ec5ad6d08",
                    "ee8aa9bcd987fb15",
                    "3a7b86269746580b",
                    "4ebba9d2e0e42c7b",
                    "a66ff0a409220152",
                    "3b026b739d310b3a",
                    "168cffe0ae8a6ae0",
                ],
            },
            Case {
                entropy: b"",
                words: [
                    "c7c9810f3d1cf2b7",
                    "1bc47e2107165ab2",
                    "2563b8b8ca5bdbb5",
                    "d38b15572668e0c7",
                    "8e2a2de1dfce0b0a",
                    "8bbfeb09158d13fd",
                    "0a986c68f6429310",
                    "a0c05c09ad6a7593",
                    "26e620840b0673e8",
                    "d6cfc0a7545f34f2",
                ],
            },
            Case {
                entropy: "种子".as_bytes(),
                words: [
                    "e06e4f7e0f084223",
                    "204c67f99e94e7cf",
                    "76af84a3bfc50736",
                    "370b1d290e9bbc6f",
                    "a269b0602e1c6e96",
                    "ad928d2ffaeef133",
                    "08518bb09e65c26b",
                    "d3901cbf6cea3586",
                    "3d473c4a9df3a174",
                    "8d75b734d4e84e3c",
                ],
            },
            Case {
                entropy: &[
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                    22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
                ],
                words: [
                    "cb83e42ba5943fc3",
                    "f9c1462c0043e035",
                    "e700038f5c6d5a28",
                    "ad16137fde8bb171",
                    "219ed1f220085138",
                    "a6bdb596693c323a",
                    "ca5d1614ab0d90d6",
                    "43d3092075b67d58",
                    "6a1bd5cbffd51dc0",
                    "cfd289b2d0ab832a",
                ],
            },
        ];
        for c in cases {
            let mut r = seeded(c.entropy);
            let got: Vec<String> = (0..10).map(|_| format!("{:016x}", r.next_u64())).collect();
            assert_eq!(got, c.words, "frozen stream for entropy {:?}", c.entropy);
        }
    }

    #[test]
    fn frozen_states_match_the_reference() {
        let cases: [(&[u8], [i64; 4]); 4] = [
            (
                b"wlwl",
                [
                    6764085530779015468,
                    -6853809058185053683,
                    -3836445681651209747,
                    -5665664888633704124,
                ],
            ),
            (
                b"",
                [
                    -4359066618775142608,
                    1156539639830188822,
                    6107907394010225435,
                    3199123669689638002,
                ],
            ),
            (
                "种子".as_bytes(),
                [
                    -8439152912609301908,
                    -5503592631781513654,
                    1254119890887343224,
                    1534974890001599172,
                ],
            ),
            (
                &[
                    0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                    22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
                ],
                [
                    9195056892158945210,
                    -6857248315756199786,
                    6891966661367945896,
                    6959375414474383947,
                ],
            ),
        ];
        for (entropy, want) in cases {
            let got: Vec<i64> = state_values(entropy).iter().map(|v| *v as i64).collect();
            assert_eq!(got, want, "frozen state for entropy {entropy:?}");
        }
    }

    // ── 零状态守卫(ADR-0027 补强 1)────────────────────────────────────
    //
    // 概率 2^-256,所以**不能**靠某个熵碰出来 —— 直接构造那个状态。
    #[test]
    fn the_zero_state_is_rejected_on_both_ends() {
        // 写侧:构造一个全零状态字典,`UNIFORM` 必须报错而不是静默返 0。
        let zero = state_to_value([0u64; 4]);
        let e = uniform(&mut NullHost, vec![zero]).unwrap_err();
        assert!(
            e.diagnostic().message.contains("absorbing"),
            "an all-zero state must be rejected, got: {}",
            e.diagnostic().message
        );
        // 读侧:播种函数本身对全零展开有守卫 —— 直接验证展开结果。
        let s = state_from_entropy(b"wlwl");
        assert_ne!(s, [0u64; 4], "seeded state must never be all zero");
    }

    // ── 无隐式状态(ADR A1 的守卫)──────────────────────────────────────

    #[test]
    fn the_same_entropy_yields_two_identical_sequences() {
        let mut a = seeded(b"wlwl");
        let mut b = seeded(b"wlwl");
        for _ in 0..64 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    // ── 边界语义 ───────────────────────────────────────────────────────

    #[test]
    fn uniform_unit_interval_never_reaches_one() {
        let mut r = seeded(b"wlwl");
        for _ in 0..200_000 {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u), "UNIFORM(rng) left [0, 1): {u}");
        }
    }

    /// `[a, b)` 的「右端不包含」必须**被保证**。`a` 远大于区间宽度时,
    /// `(b - a)` 的量级会低于半个 ulp,不做夹取就会返回恰好 `b`。
    #[test]
    fn uniform_never_returns_the_exclusive_upper_bound() {
        let a = 1.0e9_f64;
        let b = a + 1.0;
        let st = state_to_value(state_from_entropy(b"wlwl"));
        let mut host = NullHost;
        let mut cur = st;
        for _ in 0..5_000 {
            let Value::Array(pair) = uniform(
                &mut host,
                vec![cur.clone(), Value::Float(a), Value::Float(b)],
            )
            .unwrap()
            .value
            else {
                panic!("UNIFORM must return [value, rng]")
            };
            let Value::Float(v) = pair[0] else { panic!() };
            assert!(v < b, "UNIFORM(a, b) returned the exclusive bound {v}");
            assert!(v >= a, "UNIFORM(a, b) returned {v} < a = {a}");
            cur = pair[1].clone();
        }
    }

    /// 「拒绝」这条路径的**确定性**测试。
    ///
    /// `n = 7` ⇒ `2^64 mod 7 = 2` ⇒ 截断点 `= 2^64 - 2`,所以最高的两个 `u64`
    /// (`u64::MAX` 与 `u64::MAX - 1`)**必然**落在被拒区。手写一个「先给两个
    /// 被拒值、再给 41」的取源:`unbiased_with` 必须重抽两次并收下 `41 % 7 = 6`。
    ///
    /// ⚠️ **这条测试不可省**:朴素 `x % 7` 的偏差量级是 `7 / 2^64`,**任何统计
    /// 测试都抓不到它**。这是本模块唯一能证明「无偏」那条承诺成立的测试形态
    /// —— 统计测试只能证明「看起来没崩」。
    #[test]
    fn unbiased_rejects_the_incomplete_tail_verbatim() {
        let mut draws = vec![u64::MAX, u64::MAX - 1, 41].into_iter();
        let got = unbiased_with(
            || draws.next().expect("the draw source must not be exhausted"),
            7,
        );
        assert_eq!(
            got,
            41 % 7,
            "the two values above the cutoff must be rejected, not folded in"
        );
        // `n = 1` 是退化区间,必须**不抽**就返回 0(否则白烧一个 u64)。
        let mut never = || panic!("n = 1 must not draw");
        assert_eq!(unbiased_with(&mut never, 1), 0);
    }

    /// 分布形状的**统计**测试。
    ///
    /// ⚠️ 这里刻意**不**断言「每个值恰好出现 10 000 次」—— 无偏抽样是**多项
    /// 分布**,即使完全无偏,70 000 次抽 7 个值也会有 O(σ) 的起伏
    /// (σ = √(n·p·(1-p)) ≈ 90.6)。把统计量写成确定性断言就是制造一条**必然
    /// 偶尔变红**的假门禁(实测过一次:计数 `[10006, 9967, 10153, …]`,最大偏差
    /// 1.7σ,完全健康)。
    ///
    /// 所以这里是 4σ 的**容差**,并且明确写出「这条抓不到朴素取模」——
    /// 抓那个的是上面那条确定性测试。
    #[test]
    fn the_distribution_is_roughly_uniform_within_tolerance() {
        const N: usize = 70_000;
        const K: usize = 7;
        let mut r = seeded(b"wlwl");
        let mut counts = [0usize; K];
        for _ in 0..N {
            counts[r.unbiased_below(K as u64) as usize] += 1;
        }
        let expected = N as f64 / K as f64;
        let sigma = (N as f64 * (1.0 / K as f64) * (1.0 - 1.0 / K as f64)).sqrt();
        for (i, c) in counts.iter().enumerate() {
            let dev = (*c as f64 - expected).abs();
            assert!(
                dev < 4.0 * sigma,
                "bucket {i} = {c}, expected ~{expected:.0} ± {sigma:.1} (4σ = {:.0}) — \
                 counts = {counts:?}",
                4.0 * sigma
            );
        }
    }

    #[test]
    fn code_point_never_returns_a_surrogate_or_noncharacter() {
        let mut r = seeded(b"wlwl");
        for _ in 0..100_000 {
            let cp = loop {
                let cp = r.unbiased_below(MAX_SCALAR);
                if is_scalar_value(cp) {
                    break cp;
                }
            };
            assert!(
                is_scalar_value(cp),
                "code point {cp:#x} is not a scalar value"
            );
            assert!(char::from_u32(cp as u32).is_some());
        }
    }

    // ── 形态契约 ───────────────────────────────────────────────────────

    #[test]
    fn two_argument_uniform_is_an_error_not_a_default() {
        let st = state_to_value(state_from_entropy(b"wlwl"));
        let e = uniform(
            &mut NullHost,
            vec![st, Value::Float(0.0), Value::Integer(1), Value::Float(1.0)],
        )
        .unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn bad_shapes_are_diagnosed() {
        let st = state_to_value(state_from_entropy(b"wlwl"));
        // 熵类型错
        let e = seed(&mut NullHost, vec![Value::Integer(1)]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        // 字节数组里越界
        let e = seed(&mut NullHost, vec![Value::Array(vec![Value::Integer(256)])]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        assert!(e.diagnostic().message.contains("0..=255"));
        // rng 不是字典
        let e = uniform(&mut NullHost, vec![Value::Integer(1)]).unwrap_err();
        assert_eq!(e.diagnostic().code, ErrorCode::E0030);
        // 状态缺键
        let e = uniform(&mut NullHost, vec![Value::Dict(vec![])]).unwrap_err();
        assert!(e.diagnostic().message.contains("missing `s0`"));
        // 空区间
        let e = int_range(
            &mut NullHost,
            vec![st.clone(), Value::Integer(5), Value::Integer(5)],
        )
        .unwrap_err();
        assert!(e.diagnostic().message.contains("empty when lo >= hi"));
        let e = uniform(
            &mut NullHost,
            vec![st.clone(), Value::Float(1.0), Value::Float(1.0)],
        )
        .unwrap_err();
        assert!(e.diagnostic().message.contains("empty when a >= b"));
        // 元数
        assert_eq!(
            int_range(&mut NullHost, vec![st.clone()])
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0022
        );
        assert_eq!(
            codepoint(&mut NullHost, vec![])
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0022
        );
    }

    /// 跨 `i64` 全域的区间:偏移量在 i64 里会溢出,这条守住 `u128`/`i128` 那条路。
    ///
    /// ⚠️ 它同时是**方案 甲的核心证据**:`DICT` 不可变,所以**必须把返回的
    /// 新状态喂回去**才能拿到不同的数。早期版本反复用**同一个** `st` 调用
    /// 2000 次,拿回的是同一个值 —— 那不是实现慢,那是「值传递 + 不可变状态
    /// = 永不前进」这条语言事实的直接后果。
    #[test]
    fn int_range_spans_the_whole_i64_domain_when_the_state_is_threaded() {
        let mut host = NullHost;
        let mut cur = state_to_value(state_from_entropy(b"wlwl"));
        let lo = i64::MIN;
        let hi = i64::MAX;
        let mut seen_neg = false;
        let mut seen_pos = false;
        for _ in 0..2_000 {
            let Value::Array(pair) = int_range(
                &mut host,
                vec![cur.clone(), Value::Integer(lo), Value::Integer(hi)],
            )
            .unwrap()
            .value
            else {
                panic!("INT_RANGE must return [value, rng]")
            };
            let Value::Integer(v) = pair[0] else { panic!() };
            assert!((lo..hi).contains(&v), "{v} outside [{lo}, {hi})");
            seen_neg |= v < 0;
            seen_pos |= v > 0;
            cur = pair[1].clone();
        }
        assert!(
            seen_neg && seen_pos,
            "both halves of the domain must be reachable once the state is threaded"
        );
    }

    /// **状态确实在推进** —— 「抽两次要拿到两个不同的数」这条承诺的最小形态。
    /// 若某个成员忘了把推进后的状态交回去(或者原样交回入参),这条立刻红。
    #[test]
    fn drawing_twice_advances_the_state() {
        let mut host = NullHost;
        let st = state_to_value(state_from_entropy(b"wlwl"));
        let Value::Array(first) = int_range(
            &mut host,
            vec![st.clone(), Value::Integer(0), Value::Integer(1_000_000_000)],
        )
        .unwrap()
        .value
        else {
            panic!()
        };
        let Value::Array(second) = int_range(
            &mut host,
            vec![
                first[1].clone(),
                Value::Integer(0),
                Value::Integer(1_000_000_000),
            ],
        )
        .unwrap()
        .value
        else {
            panic!()
        };
        assert_ne!(
            first[1], second[1],
            "the returned state must differ after a draw"
        );
        // 同一个状态反复抽 ⇒ 同一个值(值语义,不是 bug,是承诺)。
        let Value::Array(again) = int_range(
            &mut host,
            vec![st, Value::Integer(0), Value::Integer(1_000_000_000)],
        )
        .unwrap()
        .value
        else {
            panic!()
        };
        assert_eq!(
            first[0], again[0],
            "the same state value must yield the same draw"
        );
    }

    /// 值传递的回报:状态存进 `DICT` 再取出,续跑序列**不变**。
    #[test]
    fn state_round_trips_through_a_plain_dictionary() {
        let st = state_to_value(state_from_entropy(b"wlwl"));
        let mut r = rng_from_value(&mut NullHost, "UNIFORM", &st).unwrap();
        let first: Vec<u64> = (0..4).map(|_| r.next_u64()).collect();

        // 手工拆开 → 重新组装 → 再抽,必须是同一批。
        let Value::Dict(entries) = &st else { panic!() };
        let rebuilt = Value::Dict(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .rev()
                .collect(),
        );
        let mut r2 = rng_from_value(&mut NullHost, "UNIFORM", &rebuilt).unwrap();
        let second: Vec<u64> = (0..4).map(|_| r2.next_u64()).collect();
        assert_eq!(
            first, second,
            "rebuilding the dictionary changed the sequence"
        );
    }
}
