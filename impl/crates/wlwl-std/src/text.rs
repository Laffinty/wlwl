//! [v0.11.2 M2] `wlwl:std.text` — 完整 Unicode 简单大小写映射 + 规范规范化(R2)。
//!
//! ## 为什么这不是「再加几个函数」
//!
//! 全局内建 `UPPER` / `LOWER` 只做 ASCII 映射(语言规范 §10.5)。实测
//! `UPPER("straße")` = `STRAßE`,`UPPER("héllo")` = `HéLLO` —— 非 ASCII
//! 字符原样穿过。对一个把「可复现」写进卖点的语言来说,这是**静默的错误
//! 结果**而不是可见的失败:大小写折叠是排序键、去重键、比较键的常见来源。
//!
//! 同理,`==(a, b)` 在组合字符上会给出一个**可见但错误**的答案:`"é"` 一个
//! 是 U+00E1,另一个是 `U+0065 U+0301`,渲染一模一样,比较结果 FALSE。
//! 索引、去重、缓存键的标准前置是 `==(NFC(a), NFC(b))`。
//!
//! ## 大小写部分零 Unicode 表
//!
//! **大小写映射不需要任何 Unicode 数据文件。** `str::to_uppercase` /
//! `to_lowercase` 直接走 Rust `char` 实现内置的 `Uppercase` / `Lowercase`
//! 查表(含 `ß` → `SS`、希腊 final sigma、土耳其无点 i 这类特殊映射)。
//!
//! ## 规范化部分**要**表,而且正确性只有一处证据
//!
//! `NFC` / `NFD` / `NFC_QC` 走 [`crate::unicode::canon`],数据来自 UCD
//! **18.0.0**(生成器 `bin/gen-ucd` 产出入库表,3.66 MB 源数据不入库)。
//!
//! ⚠ **本文件的单测与契约表都不是正确性证据。** 规范化排错一个码点时,
//! 渲染结果**完全相同** —— 人眼、截图、字节 diff 全都看不出来。唯一
//! 证据是 `bin/norm-check` 对官方 `NormalizationTest.txt` 的**全量**自检
//! (20 171 行 × 100 855 条断言,`addendum-03` §3.2)。
//!
//! ## 为什么只有两个成员
//!
//! 原计划还有 `GRAPHEME_COUNT` / `WIDTH`,连同 `NFC` / `NFD` 一起推迟。
//! 理由是它们**共用同一捆 UCD 数据**(`GraphemeBreakProperty` +
//! `Extended_Pictographic` / `UnicodeData` + `CompositionExclusions` +
//! `EastAsianWidth`)。分批做意味着生成器要写三遍、审三遍、跟 Unicode
//! 版本对齐三遍。合成一块做,是**一次**决定而不是四次。见
//! `docs/history/20261002.md` §1(v0.11.2 构建计划归档件)§5.2 的裁决更新。
//!
//! ## [W-24 / v0.12.1] 匹配前归一化三件套
//!
//! `NFKC` / `CASE_FOLD` / `STRIP_INVISIBLE` 是 WAF 的**匹配前归一化**:
//! 兼容性字符(全角 / 连字 / 上标 / 数学字母)折回原样、大小写折平、
//! 零宽与双向控制字符剥掉 —— 三步走完,「靠字符伪装出来的关键词」就
//! 归一成了同一个串。
//!
//! ⚠ **仓内此前没有 NFKC 的数据底座**:`unicode/norm.rs` 的 `DECOMP` 表
//! **只有规范分解**(`gen-ucd.rs` 读 `UnicodeData.txt` 时把以 `<` 开头的
//! 分解字段整条跳过 —— 那正是兼容性分解)。三张新表在
//! [`crate::unicode::waf`],数据版本 **Unicode 17.0**(ICU 78.2)。
//!

use crate::unicode::{canon, grapheme, waf, width};
use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlResult;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.text",
    functions: &[
        ("TO_UPPER", to_upper as StdFn),
        ("TO_LOWER", to_lower as StdFn),
        ("NFC", nfc as StdFn),
        ("NFD", nfd as StdFn),
        ("NFC_QC", nfc_qc as StdFn),
        ("GRAPHEME_COUNT", grapheme_count as StdFn),
        ("WIDTH", width as StdFn),
        // [G-20 引入 / 规范 v1.0 §12.4 成文] **宽松**的「文本 → 整数」。
        // **不新增类型**（见下方 `to_int` 的契约）：它与全局内建 `INT` 是
        // **故意不同**的两件事 —— §12.4 的对比表钉死了那个差异。
        ("TO_INT", to_int as StdFn),
        // ── [W-24 / v0.12.1] 匹配前归一化三件套 ──────────────────────
        // ⚠ **附录 A 抄件尚未同步** ⇒ `check-appendix-a` 与
        // `stdlib_appendix_a_sync` 会红(表体多出这三行)—— 那是集成者
        // 收口时要做的,不是本项的锅。
        ("NFKC", nfkc as StdFn),
        ("CASE_FOLD", case_fold as StdFn),
        ("STRIP_INVISIBLE", strip_invisible as StdFn),
    ],
};

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: expected string, got {}", crate::value_kind(got)),
    )
}

fn case_map(
    host: &mut dyn StdHost,
    name: &str,
    args: Vec<Value>,
    upper: bool,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() != 1 {
        return Err(arity(host, name, args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(type_err(host, name, &args[0]));
    };
    let mapped = if upper {
        s.to_uppercase()
    } else {
        s.to_lowercase()
    };
    Ok(wlwl_value::Outcome::normal(Value::String(mapped)))
}

pub fn to_upper(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    case_map(host, "TO_UPPER", args, true)
}

pub fn to_lower(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    case_map(host, "TO_LOWER", args, false)
}

// ── 规范化(v0.11.3 M6 / addendum-03 W-03)──────────────────────────
//
// 算法与四张 UCD 表都在 [`crate::unicode`];这里只做 `String` ↔ 码点序列
// 的转接与诊断。**正确性不在本文件证明** —— `NFC` / `NFD` 的正确性只由
// `bin/norm-check` 对官方 `NormalizationTest.txt` 的**全量**自检说了算
// (`addendum-03` §3.2)。下面这层薄包装不该有自己的「看起来对」的断言。

/// 取唯一实参;不是 `STRING` 就发 `E0030`,元数不对发 `E0022`。
fn str_arg<'a>(
    host: &mut dyn StdHost,
    name: &str,
    args: &'a [Value],
) -> wlwl_error::WlwlResult<&'a str> {
    if args.len() != 1 {
        return Err(arity(host, name, args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(type_err(host, name, &args[0]));
    };
    Ok(s)
}

/// 码点序列 → `String`。
///
/// `None` 分支**不可达**:入参来自一个合法 `String`(没有代理码点),而规范
/// 分解 / 组合 / 排序都不会凭空造出新码点,UCD 表里也不存在代理区。这里
/// 选择**跳过**而不是 panic —— 一份坏表不该把整个运行打挂 —— 同时用
/// `debug_assert` 让它在测试与开发构建里叫出来。
fn to_string(cps: &[u32]) -> String {
    let mut s = String::with_capacity(cps.len());
    for &cp in cps {
        match char::from_u32(cp) {
            Some(c) => s.push(c),
            None => debug_assert!(false, "规范化产出了非标量值 U+{cp:04X}"),
        }
    }
    s
}

/// `NFC(s)` —— 规范组合。`"e"` + U+0301 → `"é"`(**长度变短**)。
pub fn nfc(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "NFC", &args)?;
    // 快检命中时直接借原串:不进 `Vec<u32>`、不重排、不查表(纯扫描)。
    // ⚠ 成员边界仍要产出 owned `STRING`,所以那次 `to_owned()` 拷贝省不掉 ——
    // 「快检不分配」是**算法层**(`canon::nfc` 返回 `Cow::Borrowed`)的性质,
    // 不是成员层的。
    let out = if quick_ok(s) {
        s.to_owned()
    } else {
        to_string(&canon::nfc(&s.chars().map(u32::from).collect::<Vec<_>>()))
    };
    Ok(wlwl_value::Outcome::normal(Value::String(out)))
}

/// `NFD(s)` —— 规范分解。`"é"` → `"e"` + U+0301(**长度变长**)。
pub fn nfd(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "NFD", &args)?;
    let cps: Vec<u32> = s.chars().map(u32::from).collect();
    Ok(wlwl_value::Outcome::normal(Value::String(to_string(
        &canon::nfd(&cps),
    ))))
}

/// `NFC_QC(s)` —— 规范快速检查(**性能闸门,不是语义糖**)。
///
/// 语义是**单方向**的:
/// - `TRUE` ⇒ 该串**必定**已是 NFC 形态(可安全跳过规范化);
/// - `FALSE` ⇒ **不保证** —— 它可能已经是,也可能不是。要判定得走 `NFC`。
///
/// 文面必须这么写。反方向不成立:官方测试文件里 202 行的串本来就是 NFC
/// 形态(`Maybe` 类单码点行,如单独一个 U+0301),快检照样返回 `FALSE`。
/// 写成「返回 `FALSE` 即表示不是 NFC 形态」会教调用方一个假事实。
pub fn nfc_qc(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "NFC_QC", &args)?;
    Ok(wlwl_value::Outcome::normal(Value::Boolean(quick_ok(s))))
}

/// 从 `&str` 跑 D65 判据,一次 `Vec` 都不构造。
fn quick_ok(s: &str) -> bool {
    let mut last = 0u8;
    s.chars()
        .all(|ch| canon::quick_step(u32::from(ch), &mut last))
}

// ── 字素簇与显示宽度(v0.11.3 M6 / addendum-03 W-04)─────────────────
//
// 两条口径(规范 §12.3)都**已经算进** `unicode::width::cp_width` 的表里,成员层
// 只做转接。正确性证据同样不在本文件:字素簇见 `bin/grapheme-check`(官方
// `GraphemeBreakTest.txt` 全量 853 行),宽度见 `bin/width-check`(EAW 全表
// 1 114 112 码点逐条比对)。

/// `GRAPHEME_COUNT(s) -> INTEGER` —— 字素簇数(用户看到一个字符 = 1)。
///
/// 空串是 `0`。⚠ 坐标系是**字素簇**不是码点:`"👨‍👩‍👧"`(三个 emoji +
/// 两个 ZWJ)得 **1**;`LEN` 给的是码点数(5),两者**不可混用**。
pub fn grapheme_count(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "GRAPHEME_COUNT", &args)?;
    let n = grapheme::count_str(s);
    Ok(wlwl_value::Outcome::normal(Value::Integer(n as i64)))
}

/// `WIDTH(s) -> INTEGER` —— 终端显示宽度(列数)。
///
/// ⚠ **簇折叠**:按 UAX #29 切簇,**每簇取簇内码点列宽的最大值**再求和。
/// `"👨‍👩‍👧"` 得 **2** 而不是 8 —— 后者是逐码点求和,不是终端实际渲染的列数。
/// ⚠ 三条口径(规范 §12.3):`A`(Ambiguous)= **1**;默认 emoji 呈现的 `N` = **2**;
/// `Mn` / `Me` / `Cf` = **0**(`Mc` **不**计 —— 它是带间距的组合记号,占宽度)。
/// ⚠ **不做**任何规范化折叠:U+2122 与 U+00A5 是同一字符,这里两者都恰好
/// 得 1,但那是**各自独立查表**的结果,不是识别出了等价关系。
pub fn width(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let s = str_arg(host, "WIDTH", &args)?;
    let n = width::width_str(s);
    Ok(wlwl_value::Outcome::normal(Value::Integer(n as i64)))
}

/// [G-20 引入 / 规范 v1.0 §12.4 成文] `TO_INT(s) -> INTEGER` —— **宽松**的「文本 → 整数」。
///
/// §12.4 的宽松口径（**故意**与全局内建 `INT` 不同 —— 二者**不是**同一件事的
/// 两种写法，见 §12.4 的对比表）：
///
/// | 形态 | 口径 |
/// |---|---|
/// | 空白裁剪 | 裁掉**首尾**空白；裁剪**只**作用于首尾 ⇒ 串**内部**的空白使解析失败 |
/// | 十六进制文本 | 接受 `0x` / `0X` 前缀的十六进制整数，十六进制数字大小写不敏感 |
/// | 小数截断 | 含**一个**小数点的文本**向零截断**（小数部分被丢掉，不报「不是整数」） |
/// | 前导符号 | `+` / `-` **至多一个**，且符号之后**必须紧跟**数字 |
/// | 范围 | 结果必须落在 `INTEGER` 的有符号 64 位区间内（**含** `i64::MIN`）⇒ 越界 ⇒ 解析失败 |
///
/// ⚠ 失败**不是**诊断：返回 `ERR([kind:"ParseError", input, reason])` **值**
///   （载荷是**字典**，三个字段：`kind` / `input` / `reason`；`input` 是**裁剪前**
///   的原始实参）⇒ 程序**不**中止；调用方按语言核心册 §8.3 的 `IS_ERR` /
///   `ERR_PAYLOAD` / `UNWRAP_OR` 分支处理该 `ERR`。
///
/// ⚠ **不新增类型**，只补一个成员；`INT` 本身**不动**（它是 v0.9 的冻结契约）。
pub fn to_int(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    const NAME: &str = "TO_INT";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "{NAME}: expected string, got {}",
                crate::value_kind(&args[0])
            ),
        ));
    };
    match parse_lenient_int(s) {
        Ok(v) => Ok(Outcome::normal(Value::Integer(v))),
        // ⚠ `input` 传的是**原始**实参（裁剪前）—— §12.4 的载荷口径。
        Err(reason) => Ok(Outcome::normal(parse_error_value(s, reason))),
    }
}

/// `i64::MIN` 的**幅度**（= `i64::MAX + 1`）—— 唯一允许幅度超出 `i64::MAX`
/// 的那一个值，且**只对负号**成立（正号下它是越界）。
const I64_MIN_MAGNITUDE: u64 = 9_223_372_036_854_775_808;

/// §12.4 的**宽松**解析。`Ok(v)` = 解析结果；`Err(reason)` = 失败原因文本
/// （措辞**不由**规范冻结 —— §12.4「失败口径（内容级）」）。
///
/// 接受的形态：**十进制**（至多一个小数点）/ **`0x`·`0X` 十六进制文本**；
/// 前置**至多一个** `+` / `-`，且符号之后**必须紧跟**数字。
fn parse_lenient_int(raw: &str) -> Result<i64, String> {
    // ① 裁剪首尾空白。裁剪**只**作用于首尾 ⇒ 串**内部**的空白不匹配任何
    //    形态 ⇒ 解析失败（`TO_INT("1 2")`）。
    let t = raw.trim();
    if t.is_empty() {
        return Err("empty string after trimming whitespace".to_string());
    }
    // ② 前导符号**至多一个**；符号之后**必须紧跟**数字 ⇒ `TO_INT("- 42")`
    //    失败（符号与数字之间的空白**不算**裁剪 —— 裁剪只作用于**首尾**）。
    let (negative, rest, signed) = match t.strip_prefix('-') {
        Some(r) => (true, r, true),
        None => match t.strip_prefix('+') {
            Some(r) => (false, r, true),
            None => (false, t, false),
        },
    };
    if signed && !rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err("sign must be immediately followed by a digit".to_string());
    }
    // ③ 形态：**十六进制文本** 或 **十进制**（至多一个小数点）。
    let magnitude: u64 = match rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        Some(hex) => {
            if hex.is_empty() {
                return Err("hexadecimal literal has no digits".to_string());
            }
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("invalid character in the hexadecimal digits".to_string());
            }
            u64::from_str_radix(hex, 16)
                .map_err(|_| "number out of range for INTEGER".to_string())?
        }
        None => {
            // 小数点**至多一个**：第二个 `.` 落在小数部分里 ⇒ 小数部分不是
            // 全数字 ⇒ 失败（`TO_INT("1.2.3")`）。
            let (int_part, frac_part) = match rest.split_once('.') {
                Some((i, f)) => (i, Some(f)),
                None => (rest, None),
            };
            if int_part.is_empty() {
                return Err("no integer digits before the decimal point".to_string());
            }
            if !int_part.chars().all(|c| c.is_ascii_digit()) {
                return Err("invalid character in the integer part".to_string());
            }
            if let Some(frac) = frac_part {
                // 小数部分**被丢掉**（向零截断）⇒ 它**必须**全是数字 ⇒
                // `TO_INT("3.9abc")` 失败。
                if !frac.chars().all(|c| c.is_ascii_digit()) {
                    return Err("invalid character in the fractional part".to_string());
                }
            }
            int_part
                .parse::<u64>()
                .map_err(|_| "number out of range for INTEGER".to_string())?
        }
    };
    // ④ 幅度 → 带符号的值。`i64::MIN` 的幅度是 `i64::MAX + 1` ⇒ 只对
    //    **负号**成立（`TO_INT("-9223372036854775808")` = `i64::MIN`）；
    //    正号下它是**越界**（`TO_INT("9223372036854775808")` 失败）。
    if magnitude > I64_MIN_MAGNITUDE {
        return Err("number out of range for INTEGER".to_string());
    }
    if magnitude == I64_MIN_MAGNITUDE {
        return if negative {
            Ok(i64::MIN)
        } else {
            Err("number out of range for INTEGER".to_string())
        };
    }
    let v = magnitude as i64;
    Ok(if negative { -v } else { v })
}

/// `ERR([kind:"ParseError", input, reason])` —— 与全局内建 `INT` 同族形状。
fn parse_error_value(input: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("ParseError".into()),
        ),
        (Value::String("input".into()), Value::String(input.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

// ── W-24 · 匹配前归一化(v0.12.1 / D-2)─────────────────────────────────────
//
// **为什么这三个成员在同一个文件里**:它们是 WAF 的「匹配前归一化」三件套 ——
// 先把**兼容性字符**折回它本来的样子(`NFKC`),再把**大小写**折平
// (`CASE_FOLD`),最后把**看不见的字符**剥掉(`STRIP_INVISIBLE`)。三步都走,
// 「ｓｅｌｅｃｔ」「SELECT」「s\u{200B}elect」就都归一成同一个串了。
//
// ## ⚠ 本文件**不含**任何 Unicode 数据表
//
// 三张表在 [`crate::unicode::waf`]:兼容性分解(`COMPAT_DECOMP`)、大小写折叠
// (`FOLD_SIMPLE` / `FOLD_FULL`)、不可见格式字符(`INVISIBLE_CF`)。
// ⚠ 它们的**数据版本是 Unicode 17.0**(ICU 78.2),而仓内 `unicode/norm.rs`
// 的是 **18.0.0** —— 差集是「Unicode 16.0 / 17.0 新增且带兼容性分解的
// 码点」(实测 53 个,例 U+1CCD6 起)。见该文件头部。
//
// ## ⚠ **正确性证据不在本文件**
//
// 这三个成员是**查表 + 转发**,表是**数据**不是证据 —— 与 `NFC` / `NFD`
// 同款:`NFC` / `NFD` 的正确性只由 `bin/norm-check` 对官方
// `NormalizationTest.txt` 的全量自检说了算,本文件的单测只是**机制钉子**。
//
// ## ⚠ 失败口径(与本文件既有成员一致)
//
// **元数 / 类型错** ⇒ 原生诊断 `E0022` / `E0030`(程序员错误,不可预期)。
// **运行期失败** ⇒ **没有**:这三个成员是纯「串 → 串」的映射,不访问文件
// 系统、不做 I/O ⇒ 不存在可预期的运行期失败 ⇒ **不返回 `ERR` 值**。

/// `NFKC(s) -> STRING` —— **兼容性分解 + 规范组合**(UAX #15 的 NFKC)。
///
/// 与 [`nfc`] 的差别只有一处,但那一处就是它存在的理由:`NFC` 只做**规范**
/// 分解 / 组合,`NFKC` 额外做**兼容性**分解 ⇒ 那些「长得像 ASCII、其实不是」
/// 的字符被折回 ASCII:
///
/// | 输入 | `NFC` | `NFKC` |
/// |---|---|---|
/// | `ｓ`(U+FF21 全角 s) | `ｓ`(不动) | `s` |
/// | `ﬁ`(U+FB01 连字) | `ﬁ` | `fi` |
/// | `²`(U+00B2 上标二) | `²` | `2` |
/// | `𝐬`(U+1D42C 数学小写 s) | `𝐬` | `s` |
/// | `①`(U+2460 带圈一) | `①` | `1` |
///
/// ⚠ **组合记号不动**(`Mn` / `Me`):它们带语义(重音),折掉会毁内容。
/// ⚠ **格式字符不动**(`Cf`:零宽 / 双向控制):那是 [`strip_invisible`] 的活。
/// ⚠ **可能变长也可能变短**(全角 → 半角变短,连字 → 两个字母变长)。
/// ⚠ **幂等**:`NFKC(NFKC(x)) == NFKC(x)`。
pub fn nfkc(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    let s = str_arg(host, "NFKC", &args)?;
    // ① 兼容性分解:逐码点查表(`COMPAT_DECOMP` 里是**递归展开后**的分解)。
    let mut cps: Vec<u32> = Vec::with_capacity(s.len());
    for ch in s.chars() {
        match compat_decomp(u32::from(ch)) {
            Some(exp) => cps.extend_from_slice(exp),
            None => cps.push(u32::from(ch)),
        }
    }
    // ② 规范组合:走仓内自带的 UAX #15 内核(`unicode::canon`)。
    // ⚠ 这里喂进去的**已经**是 NFKD 形态 ⇒ `canon::nfd` 那一步是恒等的,
    // `canon::nfc` 做的正是「规范组合」⇒ 整体就是 NFKC。
    let composed = canon::nfc(&cps);
    Ok(Outcome::normal(Value::String(to_string(&composed))))
}

/// `CASE_FOLD(s) -> STRING` —— **Unicode 大小写折叠**(CaseFolding 的 C + F)。
///
/// 折叠是「**大小写不敏感的比较键**」:两个串折叠之后相等 ⇒ 它们只差大小写。
/// 与 [`to_lower`] 的差别在**三处**,且都是折叠该做而小写映射不该做的事:
///
/// | 输入 | `TO_LOWER` | `CASE_FOLD` |
/// |---|---|---|
/// | `ß` | `ß` | `ss` |
/// | `ẞ`(U+1E9E) | `ß` | `ss` |
/// | `ſ`(U+017F 长 s) | `ſ` | `s` |
/// | `ς`(U+03C2 尾 sigma) | `ς` | `σ` |
/// | `ﬁ`(U+FB01) | `ﬁ` | `fi` |
/// | `İ`(U+0130) | `i` + U+0307 | `i` + U+0307 |
///
/// ⚠ **尾 sigma 一律折成 `σ`**:`TO_LOWER` 按上下文给 `ς`(词尾)还是 `σ`
///   (词中),折叠**没有上下文** —— 比较键不能随上下文变。
/// ⚠ **可能变长**(`ß` → `ss`):折叠键的长度不等于原串长度。
/// ⚠ **土耳其语的 `T` 状态映射不含**:`I` / `ı` 的土耳其语折叠是 locale
///   相关的,默认折叠用 `I` → `i`、`ı` → `ı`。
pub fn case_fold(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    let s = str_arg(host, "CASE_FOLD", &args)?;
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let cp = u32::from(ch);
        if let Ok(i) = waf::FOLD_SIMPLE.binary_search_by_key(&cp, |&(c, _)| c) {
            // 折叠目标是一个码点。
            let target = waf::FOLD_SIMPLE[i].1;
            match char::from_u32(target) {
                Some(t) => out.push(t),
                // ⚠ 不可达:表里的目标都来自合法的 Unicode 码点。
                None => debug_assert!(false, "折叠目标不是合法码点 U+{target:04X}"),
            }
        } else if let Ok(i) = waf::FOLD_FULL.binary_search_by_key(&cp, |&(c, _)| c) {
            // 折叠目标是多个码点(`ß` → `ss` 这类)。
            for &target in waf::FOLD_FULL[i].1 {
                match char::from_u32(target) {
                    Some(t) => out.push(t),
                    None => debug_assert!(false, "折叠目标不是合法码点 U+{target:04X}"),
                }
            }
        } else {
            // 表里没有 ⇒ 该码点**没有**折叠映射 ⇒ 小写映射就是折叠结果
            // (Unicode 的默认折叠 = 小写映射;C/F 两态覆盖了全部例外)。
            out.extend(ch.to_lowercase());
        }
    }
    Ok(Outcome::normal(Value::String(out)))
}

/// `STRIP_INVISIBLE(s) -> STRING` —— 剥掉**不可见字符**(general category = `Cf`)。
///
/// 剥的是这一族:零宽空格(U+200B)/ 零宽不连字(U+200C)/ 零宽连接符(U+200D)/
/// 字连接符(U+2060)/ BOM(U+FEFF)/ 软连字(U+00AD)/ **双向控制符**
/// (U+202A–U+202E 的 LRE·RLE·PDF·LRO·RLO 与 U+2066–U+2069 的
/// LRI·RLI·FSI·PDI)/ 方向标记(U+200E·U+200F)。
///
/// ⚠ **组合记号不动**(`Mn` / `Me`):它们带语义(重音),剥掉会把 `é` 变成
///   `e`、把内容改掉。变体选择符(U+FE00–U+FE0F)也是 `Mn` ⇒ 不动。
/// ⚠ **边界:ZWJ 与 emoji 标签序列会被剥掉** ⇒ 家族 emoji
///   (`👨‍👩‍👧`,ZWJ 连接)会散成三个 emoji,emoji 标签序列(U+E0020–U+E007F)
///   的**语义**也没了。要保留 emoji 语义就别用它 —— 它是给
///   「关键词匹配」用的,不是给「显示」用的。
pub fn strip_invisible(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<Outcome> {
    let s = str_arg(host, "STRIP_INVISIBLE", &args)?;
    let out: String = s
        .chars()
        .filter(|ch| !waf::INVISIBLE_CF.binary_search(&u32::from(*ch)).is_ok())
        .collect();
    Ok(Outcome::normal(Value::String(out)))
}

/// 兼容性分解查表:码点 → **递归展开后**的兼容性分解。
///
/// 表里**只**有「NFKD ≠ NFD」的码点 —— 没有兼容性分解的码点不在表里
/// (它们的分解由 `unicode::canon` 的规范分解那一半负责)。
fn compat_decomp(cp: u32) -> Option<&'static [u32]> {
    waf::COMPAT_DECOMP
        .binary_search_by_key(&cp, |&(c, _)| c)
        .ok()
        .map(|i| waf::COMPAT_DECOMP[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(upper: bool, s: &str) -> String {
        if upper {
            s.to_uppercase()
        } else {
            s.to_lowercase()
        }
    }

    /// 全局内建 `UPPER` / `LOWER` 只做 ASCII —— 这几条是本模块存在的理由,
    /// 期望值取自 Unicode 的简单大小写映射规则,不是从实现输出抄的。
    #[test]
    fn non_ascii_actually_moves() {
        // Latin-1 补充
        assert_eq!(run(true, "héllo"), "HÉLLO");
        assert_eq!(run(false, "ÉÀÜ"), "éàü");
        // Latin 扩展 A(带变音符的拉丁字母)
        assert_eq!(run(true, "āáǎà"), "ĀÁǍÀ");
        // 西里尔
        assert_eq!(run(true, "привет"), "ПРИВЕТ");
        assert_eq!(run(false, "ПРИВЕТ"), "привет");
        // 希腊
        assert_eq!(run(true, "αβγ"), "ΑΒΓ");
    }

    /// 简单映射的**长度会变**的情形。这是最容易被「逐字符 to_upper」的
    /// 实现漏掉的一类。
    #[test]
    fn sharp_s_expands() {
        // § SpecialCasing.txt:ß 的大写是 "SS"(两个码点)
        assert_eq!(run(true, "straße"), "STRASSE");
        // 土耳其无点 i / 点上 i
        assert_eq!(run(false, "İ"), "i̇");
    }

    #[test]
    fn greek_final_sigma_is_contextual() {
        // Σ 在词尾小写为 ς,词中为 σ
        assert_eq!(run(false, "ΑΣ"), "ας");
        assert_eq!(run(false, "ΣΑ"), "σα");
    }

    #[test]
    fn ascii_is_unchanged_in_shape() {
        assert_eq!(run(true, "Hello, World 123"), "HELLO, WORLD 123");
        assert_eq!(run(false, "Hello"), "hello");
        assert_eq!(run(true, ""), "");
        assert_eq!(run(false, ""), "");
    }

    // ── [W-24] 归一化三成员的机制钉子 ────────────────────────────────────
    //
    // ⚠ **不是**正确性证据 —— 契约表在 `wlwl-eval` 的
    // `path_contains_contract.rs`;这里钉的是**机制**(查表走对分支、
    // 幂等、边界不被误伤)。

    fn fold(s: &str) -> String {
        // 直接跑成员层的算法(不经过宿主):失败口径由契约表钉。
        let mut cps: Vec<u32> = Vec::new();
        for ch in s.chars() {
            let cp = u32::from(ch);
            if let Ok(i) = waf::FOLD_SIMPLE.binary_search_by_key(&cp, |&(c, _)| c) {
                cps.push(waf::FOLD_SIMPLE[i].1);
            } else if let Ok(i) = waf::FOLD_FULL.binary_search_by_key(&cp, |&(c, _)| c) {
                cps.extend_from_slice(waf::FOLD_FULL[i].1);
            } else {
                cps.extend(ch.to_lowercase().map(u32::from));
            }
        }
        to_string(&cps)
    }

    /// 兼容性分解 + 规范组合(= UAX #15 的 NFKC)。
    fn nfkc(s: &str) -> String {
        let mut cps: Vec<u32> = Vec::new();
        for ch in s.chars() {
            match compat_decomp(u32::from(ch)) {
                Some(exp) => cps.extend_from_slice(exp),
                None => cps.push(u32::from(ch)),
            }
        }
        to_string(&canon::nfc(&cps))
    }

    fn strip(s: &str) -> String {
        s.chars()
            .filter(|ch| !waf::INVISIBLE_CF.binary_search(&u32::from(*ch)).is_ok())
            .collect()
    }

    /// 期望值取自 **Unicode 标准本身**(UAX #15 / CaseFolding /
    /// DerivedGeneralCategory),不是从实现输出抄的。
    #[test]
    fn nfkc_folds_compatibility_characters() {
        // 全角 ASCII
        assert_eq!(
            nfkc("\u{ff53}\u{ff45}\u{ff4c}\u{ff45}\u{ff43}\u{ff54}"),
            "select"
        );
        // 连字 ﬁ
        assert_eq!(nfkc("\u{fb01}"), "fi");
        // 上标 ²
        assert_eq!(nfkc("x\u{b2}"), "x2");
        // 数学小写字母 𝐬
        assert_eq!(nfkc("\u{1d42c}"), "s");
        // 不换行空格
        assert_eq!(nfkc("a\u{a0}b"), "a b");
    }

    #[test]
    fn nfkc_composes_but_does_not_drop_combining_marks() {
        // 分解形 ⇒ 组合形(标记**没**被丢)。
        assert_eq!(nfkc("e\u{301}"), "\u{e9}");
        // 单独的组合记号原样保留。
        assert_eq!(nfkc("\u{301}"), "\u{301}");
        // 零宽空格**不动**(那是 STRIP_INVISIBLE 的活)。
        assert_eq!(nfkc("a\u{200b}b"), "a\u{200b}b");
    }

    #[test]
    fn nfkc_is_idempotent() {
        let s = "\u{ff53}\u{ff45}\u{ff4c}\u{ff45}\u{ff43}\u{ff54}\u{301}";
        assert_eq!(nfkc(&nfkc(s)), nfkc(s));
    }

    #[test]
    fn case_fold_uses_the_full_folding_not_the_lowercase_mapping() {
        // ß → ss(全量折叠 F;小写映射是它自己)
        assert_eq!(fold("\u{df}"), "ss");
        // ẞ → ss
        assert_eq!(fold("\u{1e9e}"), "ss");
        // 长 s ſ → s(简单折叠 C)
        assert_eq!(fold("\u{17f}"), "s");
        // 尾 sigma ς → σ(**没有**上下文)
        assert_eq!(fold("\u{3c2}"), "\u{3c3}");
        assert_eq!(fold("\u{3a3}"), "\u{3c3}");
        // 连字 ﬁ → fi
        assert_eq!(fold("\u{fb01}"), "fi");
        // 土耳其语的点上 I(默认折叠不含 T 态)
        assert_eq!(fold("\u{130}"), "i\u{307}");
    }

    #[test]
    fn strip_invisible_removes_format_characters_but_keeps_marks() {
        assert_eq!(strip("sel\u{200b}ect"), "select");
        assert_eq!(strip("a\u{200d}b"), "ab");
        assert_eq!(strip("\u{feff}select"), "select");
        // 双向控制符(Trojan Source 那族)
        assert_eq!(strip("sel\u{202e}ect"), "select");
        // ⚠ 组合记号**不动**(剥掉会把 `é` 变成 `e`、毁掉内容)。
        assert_eq!(strip("e\u{301}"), "e\u{301}");
    }
}
