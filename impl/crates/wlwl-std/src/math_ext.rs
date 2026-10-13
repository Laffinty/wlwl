//! [v0.11.3 M7 / addendum-10 W-03] `wlwl:std.math` 的任意精度十进制内核。
//!
//! 形状与裁决见 [`ADR-0031`](../../../../docs/adr/0031-math-ext-shape.md) D1。
//!
//! ## 值形状(公开契约,字段名变更即 breaking)
//!
//! `{"sign": INTEGER, "digits": STRING, "scale": INTEGER}`,表示
//! `sign × digits × 10^-scale`。规范形式:
//!
//! * `digits` **无前导零**(零为 `"0"`);`scale ≥ 0`;`sign ∈ {-1, 1}`;
//! * **零恒为 `{1, "0", 0}`** —— 避免 `-0` 这种两个表示同一个数的状态。
//!
//! ⚠ `scale` 是**有意义的**,不是规范化掉的:`DECIMAL("1.00")` 渲染回 `"1.00"`。
//! 金额场景正是靠它保留「几位」这个业务事实。
//!
//! ## 为什么归 R2
//!
//! 每个运算都要构造**一个新 `DICT`** —— 与 D11-012 记录的数组平方级同族成本。
//! R1 版本在规模上不可用。
//!
//! ## 与既有成员的分工
//!
//! `ADR-0031` D2:本族**不含 `MEAN`**(`SUM` + `/` 已覆盖);
//! D3:统计族沿用 `SUM` 的**静默提升**口径,不引入 `E0030`。

use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

/// 内部表示:与公开 `DICT` 一一对应,但用 `Vec<u8>` 存数字便于运算。
#[derive(Clone, PartialEq, Eq)]
struct Dec {
    /// `false` = 正,`true` = 负;零**恒为** `false`。
    neg: bool,
    /// 数字位,每个元素是 `0..=9`;**无前导零**,零为 `vec![0]`。
    digits: Vec<u8>,
    scale: usize,
}

impl Dec {
    fn is_zero(&self) -> bool {
        self.digits.iter().all(|&d| d == 0)
    }

    /// 按 `scale` 渲染成规范十进制串(`"1.50"` / `"-0.25"` / `"3"`)。
    fn render(&self) -> String {
        let mut s = String::with_capacity(self.digits.len() + self.scale + 2);
        if self.neg && !self.is_zero() {
            s.push('-');
        }
        if self.scale == 0 {
            s.extend(self.digits.iter().map(|d| (b'0' + d) as char));
        } else {
            let n = self.digits.len();
            if n > self.scale {
                s.extend(
                    self.digits[..n - self.scale]
                        .iter()
                        .map(|d| (b'0' + d) as char),
                );
                s.push('.');
                s.extend(
                    self.digits[n - self.scale..]
                        .iter()
                        .map(|d| (b'0' + d) as char),
                );
            } else {
                s.push('0');
                s.push('.');
                for _ in 0..(self.scale - n) {
                    s.push('0');
                }
                s.extend(self.digits.iter().map(|d| (b'0' + d) as char));
            }
        }
        s
    }

    /// 归一到 `scale` 位(不足**追加**零)。用于比较与加减的对齐。
    ///
    /// ⚠ 补零必须**追加到末尾**(大端:末位是最小位)—— 追加即「整体左移一位、
    /// 最小位补 0」,也就是值乘 10。前置会把 `1.0` 变成 `01.0`,于是
    /// `DECIMAL_CMP(DECIMAL("1.0"), DECIMAL("1.00"))` 给 `-1` 而不是 `0`。
    fn at_scale(&self, scale: usize) -> Dec {
        if scale == self.scale {
            return self.clone();
        }
        let mut digits = self.digits.clone();
        digits.extend(vec![0u8; scale - self.scale]);
        Dec {
            neg: self.neg,
            digits,
            scale,
        }
    }

    fn from_parts(neg: bool, digits: Vec<u8>, scale: usize) -> Dec {
        // 去前导零 —— 一次切片而不是在 `Vec` 头上 `remove(0)`
        // (那是 O(n²),而 `DECIMAL_ADD` 每调一次就走这里)。
        let first = digits
            .iter()
            .position(|&d| d != 0)
            .unwrap_or(digits.len() - 1);
        let digits = digits[first..].to_vec();
        let zero = digits.iter().all(|&d| d == 0);
        Dec {
            // 零不带负号 —— 消灭 `-0` 这种双表示。
            neg: neg && !zero,
            digits,
            scale,
        }
    }

    /// 数值比较:返回 `-1 / 0 / 1`。两者的 `scale` 不同也没关系(先对齐)。
    /// ⚠ **必须用 `mag_cmp` 而不是 `Vec::cmp`**。字典序在**位数不同时**是错的:
    /// `[1,0,0]` 字典序小于 `[9,5]`,数值却是 100 > 95。第一版直接用
    /// `a.digits.cmp(&b.digits)`,后果是 `DECIMAL_CMP(DECIMAL("10.0"),
    /// DECIMAL("9.5"))` 给出 `-1`,而 `DECIMAL_SUB` 也会走错分支。
    fn cmp(&self, other: &Dec) -> i32 {
        let s = self.scale.max(other.scale);
        let a = self.at_scale(s);
        let b = other.at_scale(s);
        match (a.is_zero(), b.is_zero()) {
            (true, true) => return 0,
            (true, false) => return if b.neg { 1 } else { -1 },
            (false, true) => return if a.neg { -1 } else { 1 },
            _ => {}
        }
        let mag = match mag_cmp(&a.digits, &b.digits) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        };
        match (a.neg, b.neg) {
            (false, false) => mag,
            (true, true) => -mag,
            (false, true) => 1,
            (true, false) => -1,
        }
    }

    fn neg(&self) -> Dec {
        Dec {
            neg: !self.neg && !self.is_zero(),
            digits: self.digits.clone(),
            scale: self.scale,
        }
    }

    fn add(&self, other: &Dec) -> Dec {
        let s = self.scale.max(other.scale);
        let a = self.at_scale(s);
        let b = other.at_scale(s);
        let (r, neg) = if a.neg == b.neg {
            (mag_add(&a.digits, &b.digits), a.neg)
        } else {
            // ⚠ 同 `cmp`:**必须 `mag_cmp`**。`Vec::cmp` 在位数不同时是字典序,
            // `[1,2,5]` 会被判成小于 `[5,0]`,于是 `1.25 - 0.5` 走了「b 大于 a」
            // 分支并给出 `-0.25`。
            match mag_cmp(&a.digits, &b.digits) {
                std::cmp::Ordering::Less => (mag_sub(&b.digits, &a.digits), b.neg),
                std::cmp::Ordering::Greater => (mag_sub(&a.digits, &b.digits), a.neg),
                std::cmp::Ordering::Equal => (vec![0], false),
            }
        };
        Dec::from_parts(neg, r, s)
    }

    fn sub(&self, other: &Dec) -> Dec {
        self.add(&other.neg())
    }

    fn mul(&self, other: &Dec) -> Dec {
        if self.is_zero() || other.is_zero() {
            return Dec::from_parts(false, vec![0], 0);
        }
        let r = mag_mul(&self.digits, &other.digits);
        Dec::from_parts(self.neg != other.neg, r, self.scale + other.scale)
    }

    /// 除法:`scale` 是**必填**的保留位数(ADR-0031 D1:不许静默截断),
    /// 舍入一律 `HALF_EVEN`(与 Python `decimal` 默认对齐)。
    fn div(&self, other: &Dec, scale: usize) -> Dec {
        if other.is_zero() {
            // 调用方负责先判,这里只是兜底(防御性:不该走到)。
            return Dec::from_parts(false, vec![0], 0);
        }
        if self.is_zero() {
            return Dec::from_parts(false, vec![0], scale);
        }
        // 要的是整数 Q,使 `Q / 10^scale == self/other`,即
        //   Q = round( A × 10^(sb + scale - sa) / B )
        // ⇒ `sb + scale >= sa` 时放大**分子**,否则放大**分母**(避免负指数)。
        // ⚠ 第一版用「统一放大再按位截取」,多放大了一位数,结果系统性偏了
        // 一个 10 的幂(1.25/0.5 得 5.00)。这个式子是推出来的,不是凑的。
        let neg = self.neg != other.neg;
        let need = other.scale + scale;
        let q = if need >= self.scale {
            let k = need - self.scale;
            let mut num = self.digits.clone();
            num.extend(vec![0u8; k]);
            long_div_round(&num, &other.digits)
        } else {
            let k = self.scale - need;
            let mut den = other.digits.clone();
            den.extend(vec![0u8; k]);
            long_div_round(&self.digits, &den)
        };
        Dec::from_parts(neg, q, scale)
    }

    /// 按 `mode` 在 `scale` 位上舍入。`scale` 大于当前 `scale` 时补零而非舍入。
    ///
    /// ⚠ **`q` 必须真的被重新赋值** —— 第一版写成 `mag_add(&q, &[1]);` 把返回值
    /// **丢了**,而 clippy 的 `unused_mut` 正在提示这件事(`mut` 不必要 ⇒ 根本没发生
    /// 变更)。所有 `2.5` 的用例因此一律不进位,`HALF_UP(2.5)` 甚至给出 `2`。
    fn round_to(&self, scale: usize, mode: Round) -> Dec {
        if scale >= self.scale {
            return self.at_scale(scale);
        }
        let drop = self.scale - scale;
        // ⚠ **`digits.len()` 可能小于 `scale`** —— `from_parts` 去掉了前导零,而
        // 零位是在 `scale` 里的。`DECIMAL("0.005")` 归一后是 `digits=[5]`、
        // `scale=3`。所以 `drop >= digits.len()` 时结果就是「全舍掉」,必须先判,
        // 否则切片下溢。
        if drop >= self.digits.len() {
            return Dec::from_parts(false, vec![0], scale);
        }
        let q: Vec<u8> = self.digits[..self.digits.len() - drop].to_vec();
        let dropped: Vec<u8> = self.digits[self.digits.len() - drop..].to_vec();
        let anything = dropped.iter().any(|&d| d != 0);
        // ⚠ **四个方向模式的差别只在负数上**。`Dec` 存的是「绝对值 + 符号」,所以
        // 「进位」永远意味着**远离零**,符号最后才贴回去 ⇒
        // `DOWN` 向零;`FLOOR` 向 -inf(负数要多退);`UP` 远离零;`CEILING` 向 +inf。
        // 第一版把 `FLOOR` 写成「向零」,于是 `FLOOR(-2.1)` 给出 `-2` 而不是 `-3`。
        let up = match mode {
            Round::Down => false,
            Round::Floor => self.neg && anything,
            Round::Up => anything,
            Round::Ceiling => !self.neg && anything,
            // 大端存储 ⇒ `dropped[0]` 是被丢位里**最高**的那一位
            Round::HalfUp => dropped[0] >= 5,
            Round::HalfEven => match dropped[0].cmp(&5) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Less => false,
                // 恰好 5:后面还有非零 ⇒ 超过一半;否则是**恰好一半** ⇒ 看保留段末位奇偶
                std::cmp::Ordering::Equal => {
                    dropped[1..].iter().any(|&d| d != 0)
                        || q.last().map(|d| d % 2 == 1).unwrap_or(false)
                }
            },
        };
        let q = if up { mag_add(&q, &[1]) } else { q };
        Dec::from_parts(self.neg, q, scale)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Round {
    HalfEven,
    HalfUp,
    Down,
    Up,
    Floor,
    Ceiling,
}

impl Round {
    fn parse(s: &str) -> Option<Round> {
        Some(match s {
            "HALF_EVEN" => Round::HalfEven,
            "HALF_UP" => Round::HalfUp,
            "DOWN" => Round::Down,
            "UP" => Round::Up,
            "FLOOR" => Round::Floor,
            "CEILING" => Round::Ceiling,
            _ => return None,
        })
    }
}

fn mag_add(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0u8;
    for i in 0..a.len().max(b.len()) {
        // ⚠ 下标要在**越界判断之内**算 —— 先算 `b.len() - 1 - i` 再判 `i < b.len()`
        // 会在不等长时**下溢**(第一版就栽在这里,表现为 `attempt to subtract with
        // overflow`)。
        let mut s = carry;
        if i < a.len() {
            s += a[a.len() - 1 - i];
        }
        if i < b.len() {
            s += b[b.len() - 1 - i];
        }
        out.push(s % 10);
        carry = s / 10;
    }
    if carry > 0 {
        out.push(carry);
    }
    out.reverse();
    out
}

/// `a - b`,要求 `a ≥ b`。
fn mag_sub(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(a.len());
    let mut borrow = 0i8;
    for i in 0..a.len() {
        let x = a[a.len() - 1 - i] as i8;
        let y = if i < b.len() {
            b[b.len() - 1 - i] as i8
        } else {
            0
        };
        let mut d = x - y - borrow;
        if d < 0 {
            d += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out.push(d as u8);
    }
    out.reverse();
    out
}

fn mag_mul(a: &[u8], b: &[u8]) -> Vec<u8> {
    // ⚠ **先翻成小端、卷积、再翻回来**。直接在大端数组上写 `acc[i + j]` 的权重
    // 是错的(大端下标 0 是最高位,`a` 的第 i 位权重是 `10^(la-1-i)`,与 `b` 的
    // 第 j 位相乘后的落点**不是** `i + j`)。第一版就这么写的,后果是
    // `125 × 5` 算成 **5062** 而不是 625。翻成小端后下标回到教科书形式。
    let ar: Vec<u8> = a.iter().rev().copied().collect();
    let br: Vec<u8> = b.iter().rev().copied().collect();
    let mut acc = vec![0u32; ar.len() + br.len()];
    for (i, &x) in ar.iter().enumerate() {
        for (j, &y) in br.iter().enumerate() {
            acc[i + j] += (x * y) as u32;
        }
    }
    // 小端进位
    for k in 0..acc.len() - 1 {
        acc[k + 1] += acc[k] / 10;
        acc[k] %= 10;
    }
    let last = acc.len() - 1;
    if acc[last] >= 10 {
        let carry = acc[last] / 10;
        acc[last] %= 10;
        acc.push(carry);
    }
    acc.iter().rev().map(|&v| v as u8).collect()
}

/// 整数长除法 + `HALF_EVEN` 舍入:返回 `round(num / den)` 的整数商。
///
/// 逐位长除(大端存储,新位**追加到末尾**即是 `rem × 10 + d`)。不变量:
/// 每轮开始时 `rem < den`,故 `rem × 10 + d < den × 10` ⇒ 试商必在 `0..=9`。
///
/// 舍入判据用**余数**而非位串:余数 ×2 与 `den` 比 —— 大于则进、等于则看
/// 商末位奇偶(恰好一半时的 HALF_EVEN 规则)、小于则不进。
fn long_div_round(num: &[u8], den: &[u8]) -> Vec<u8> {
    let mut quotient: Vec<u8> = Vec::with_capacity(num.len());
    let mut rem: Vec<u8> = vec![0];
    for &d in num {
        rem.push(d); // `rem × 10 + d`(大端:追加到末尾)
        let mut q = 0u8;
        while mag_cmp(&rem, den) != std::cmp::Ordering::Less && q < 10 {
            rem = mag_sub(&rem, den);
            q += 1;
        }
        quotient.push(q);
    }
    let twice = mag_add(&rem, &rem);
    let up = match mag_cmp(&twice, den) {
        std::cmp::Ordering::Greater => true,
        // 恰好一半 ⇒ 看商末位奇偶(`2.5 → 2`、`3.5 → 4`)
        std::cmp::Ordering::Equal => quotient.last().map(|d| d % 2 == 1).unwrap_or(false),
        std::cmp::Ordering::Less => false,
    };
    if up {
        mag_add(&quotient, &[1])
    } else {
        quotient
    }
}

fn mag_cmp(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let mut x = a;
    let mut y = b;
    while x.len() > 1 && x[0] == 0 {
        x = &x[1..];
    }
    while y.len() > 1 && y[0] == 0 {
        y = &y[1..];
    }
    x.len().cmp(&y.len()).then_with(|| x.cmp(y))
}
// ── 字面量解析 ────────────────────────────────────────────────────────

/// 解析十进制字面量:可选 `-`、整数位、可选 `.` + 小数位。
///
/// 拒绝:空串、只有 `.`、`-` 后无数字、非 ASCII 数字、多个 `.`。
/// 接受:`"0"`、`"-0"`(归一到零)、`"1.00"`(`scale` 保留 2)、`".5"`(`scale` 1)。
fn parse_literal(s: &str) -> Option<Dec> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let mut neg = false;
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        neg = bytes[i] == b'-';
        i += 1;
    }
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut scale = 0usize;
    let mut seen_dot = false;
    let mut any = false;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'0'..=b'9' => {
                digits.push(c - b'0');
                if seen_dot {
                    scale += 1;
                }
                any = true;
            }
            b'.' => {
                if seen_dot {
                    return None;
                }
                seen_dot = true;
            }
            _ => return None,
        }
        i += 1;
    }
    if !any || digits.is_empty() {
        return None;
    }
    Some(Dec::from_parts(neg, digits, scale))
}

// ── `Value` ↔ `Dec` ───────────────────────────────────────────────────

/// 校验并取出 `DICT` 形状的十进制值。
///
/// ⚠ 非法 `DICT` → `E0030`,**不修复、不报错成别的形状**
/// (`ADR-0031` D1:这是明码标价的妥协,规范要原样写明)。
fn dec_from_value(v: &Value) -> Option<Dec> {
    let Value::Dict(pairs) = v else {
        return None;
    };
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == key))
            .map(|(_, v)| v)
    };
    let (Some(Value::Integer(sign)), Some(Value::String(digits)), Some(Value::Integer(scale))) =
        (get("sign"), get("digits"), get("scale"))
    else {
        return None;
    };
    if *scale < 0 {
        return None;
    }
    let neg = match sign {
        1 => false,
        -1 => true,
        _ => return None,
    };
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // ⚠ **前导零必须在归一之前判** —— `from_parts` 会把前导零剥掉,所以判在
    // 之后就永远不成立(第一版就因此让 `{digits: "012"}` 静默通过)。
    if digits.len() > 1 && digits.starts_with('0') {
        return None; // 非法形状,不修复(ADR-0031 D1)
    }
    let ds: Vec<u8> = digits.bytes().map(|c| c - b'0').collect();
    let mut d = Dec::from_parts(neg, ds, *scale as usize);
    // 规范形式要求零的 scale 归零。
    if d.is_zero() {
        d.scale = 0;
    }
    Some(d)
}

fn dec_to_value(d: &Dec) -> Value {
    Value::Dict(vec![
        (
            Value::String("sign".into()),
            Value::Integer(if d.neg { -1 } else { 1 }),
        ),
        (
            Value::String("digits".into()),
            Value::String(d.digits.iter().map(|x| (b'0' + x) as char).collect()),
        ),
        (
            Value::String("scale".into()),
            Value::Integer(d.scale as i64),
        ),
    ])
}

// ── 诊断 ──────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!(
            "{name}: expected a DECIMAL DICT {{\"sign\": INT, \"digits\": STRING, \"scale\": INT}}, got {}",
            wlwl_value::type_name(got)
        ),
    )
}

fn bad_literal(host: &mut dyn StdHost, name: &str, s: &str) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: {s:?} is not a decimal literal (expected [+-]?digits[.digits])"),
    )
}

// ── 成员 ──────────────────────────────────────────────────────────────

/// `DECIMAL(literal) -> DICT`。
pub fn decimal(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "DECIMAL", args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "DECIMAL: expected STRING, got {}",
                wlwl_value::type_name(&args[0])
            ),
        ));
    };
    match parse_literal(s) {
        Some(d) => Ok(Outcome::normal(dec_to_value(&d))),
        None => Err(bad_literal(host, "DECIMAL", s)),
    }
}

macro_rules! binary {
    ($name:ident, $fn_name:literal, $op:expr) => {
        pub fn $name(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
            if args.len() != 2 {
                return Err(arity(host, $fn_name, args.len(), 2));
            }
            let (Some(a), Some(b)) = (dec_from_value(&args[0]), dec_from_value(&args[1])) else {
                let bad = if dec_from_value(&args[0]).is_none() {
                    &args[0]
                } else {
                    &args[1]
                };
                return Err(type_err(host, $fn_name, bad));
            };
            #[allow(clippy::redundant_closure_call)]
            let f: fn(&Dec, &Dec) -> Dec = $op;
            Ok(Outcome::normal(dec_to_value(&f(&a, &b))))
        }
    };
}

binary!(decimal_add, "DECIMAL_ADD", Dec::add);
binary!(decimal_sub, "DECIMAL_SUB", Dec::sub);
binary!(decimal_mul, "DECIMAL_MUL", Dec::mul);

/// `DECIMAL_CMP(a, b) -> -1 | 0 | 1`。**永不失败**。
pub fn decimal_cmp(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity(host, "DECIMAL_CMP", args.len(), 2));
    }
    let (Some(a), Some(b)) = (dec_from_value(&args[0]), dec_from_value(&args[1])) else {
        let bad = if dec_from_value(&args[0]).is_none() {
            &args[0]
        } else {
            &args[1]
        };
        return Err(type_err(host, "DECIMAL_CMP", bad));
    };
    Ok(Outcome::normal(Value::Integer(a.cmp(&b) as i64)))
}

/// `DECIMAL_DIV(a, b, scale)`。**`scale` 必填** —— 静默截断是精度谎言
/// (`ADR-0031` D1);`b` 为零 ⇒ `ERR([kind: DomainError])`;舍入恒 `HALF_EVEN`。
pub fn decimal_div(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity(host, "DECIMAL_DIV", args.len(), 3));
    }
    let (Some(a), Some(b)) = (dec_from_value(&args[0]), dec_from_value(&args[1])) else {
        let bad = if dec_from_value(&args[0]).is_none() {
            &args[0]
        } else {
            &args[1]
        };
        return Err(type_err(host, "DECIMAL_DIV", bad));
    };
    let Value::Integer(scale) = args[2] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "DECIMAL_DIV: scale must be INTEGER, got {}",
                wlwl_value::type_name(&args[2])
            ),
        ));
    };
    if scale < 0 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("DECIMAL_DIV: scale must be >= 0, got {scale}"),
        ));
    }
    if b.is_zero() {
        return Ok(Outcome::normal(Value::Err(Box::new(Value::Dict(vec![
            (
                Value::String("kind".into()),
                Value::String("DomainError".into()),
            ),
            (Value::String("op".into()), Value::String("DIV".into())),
        ])))));
    }
    Ok(Outcome::normal(dec_to_value(&a.div(&b, scale as usize))))
}

/// `DECIMAL_TO_STRING(a) -> STRING`。**永不失败**。
pub fn decimal_to_string(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "DECIMAL_TO_STRING", args.len(), 1));
    }
    let Some(a) = dec_from_value(&args[0]) else {
        return Err(type_err(host, "DECIMAL_TO_STRING", &args[0]));
    };
    Ok(Outcome::normal(Value::String(a.render())))
}

/// `DECIMAL_ROUND(a, scale, mode) -> DICT`。
///
/// `mode` **无默认**(`ADR-0031` / 计划 §3.3):银行家舍入与四舍五入是**业务决定**,
/// 给默认等于替业务方选了一个。未知 `mode` → `E0030`。
pub fn decimal_round(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity(host, "DECIMAL_ROUND", args.len(), 3));
    }
    let Some(a) = dec_from_value(&args[0]) else {
        return Err(type_err(host, "DECIMAL_ROUND", &args[0]));
    };
    let (Value::Integer(scale), Value::String(mode)) = (&args[1], &args[2]) else {
        return Err(host.diag(
            ErrorCode::E0030,
            "DECIMAL_ROUND: expects (DECIMAL, INTEGER, STRING mode)".to_string(),
        ));
    };
    let Some(mode) = Round::parse(mode) else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "DECIMAL_ROUND: unknown mode {mode:?} (want HALF_EVEN / HALF_UP / \
                 DOWN / UP / FLOOR / CEILING)"
            ),
        ));
    };
    if *scale < 0 {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("DECIMAL_ROUND: scale must be >= 0, got {scale}"),
        ));
    }
    Ok(Outcome::normal(dec_to_value(
        &a.round_to(*scale as usize, mode),
    )))
}
