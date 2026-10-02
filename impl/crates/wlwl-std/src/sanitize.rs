//! [v0.11.3 M1] `wlwl:std.sanitize` — 输出安全(R2,旗舰特色功能)。
//!
//! ## 威胁模型(规范 §13 措辞纪律)
//!
//! 本模块防的是「不可信数据在**输出点**被解释为代码或结构」。它**不清洗
//! 输入**(OWASP 现行口径:输入侧「清洗」不是 XSS / 注入的防治手段),不做
//! 语义校验,不防逻辑漏洞。上下文分工(URL → `std.encode.URL_ENCODE`、
//! 标记 → [`html_escape`]、富 HTML → `HTML_SANITIZE`)是**规范性**的,三者
//! 互不替代 —— 选错上下文的转义不是保守,是错误。
//!
//! ## 为什么全员 R2
//!
//! 业主 2026-10-03 裁决:`std.sanitize` 是旗舰特色功能,性能是契约的一部分。
//! 解释器侧字符串与数组构建是超线性的(D11-012 / D12-009 的已归档证据):
//! `+(s, …)` 逐次全量拷贝、`PUSH` 逐次全量拷贝,R1 实现的转义在文档级输入上
//! **必然 O(n²)** —— 与 M5 把 `RANGE` 沉回 R2 同一根因。归层基准
//! (R1 原型 vs R2,四档语料)见 `benches/baseline.txt` M1 段(计划 W-08)。
//! 实现要点:直通 `Value`(不经 `wrap()` / compat,零转换);单趟扫描 +
//! `with_capacity` 预分配;树构建(M3)用索引池与迭代栈,禁止递归。
//!
//! ## 实体表是全表,不是半表
//!
//! [`entities`] 由生成器从 WHATWG `entities.json` 官方数据产出(数据文件
//! 不入库,生成器入库 —— `src/bin/gen-entities.rs`)。半张表在安全语境下
//! 不是「documented 局限」而是**调用方拿到的错误数据**(`&copy;` 原样穿过),
//! 这是 D12-004「不做半张表」教训在安全语义下的加倍成立。未知命名实体
//! **原样保留**是 HTML5 自身的容错行为;数字引用按 WHATWG 规则映射
//! (越界 / 代理区 → U+FFFD,溢出有检查,不得 panic)。

pub mod entities;

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.sanitize",
    functions: &[
        ("HTML_ESCAPE", html_escape as StdFn),
        ("HTML_UNESCAPE", html_unescape as StdFn),
        // [v0.11.3 M3] `HTML_SANITIZE` 随 M3 落地时注册。
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

/// `HTML_ESCAPE(data) -> STRING`:标记上下文转义(HTML 文本节点与**双引号**
/// 属性值,HTML5 同口径)。转义集恰为 `& < > " '` 五个 ASCII 字符
/// (`'` → `&#39;`,旧渲染器兼容口径);非 ASCII 码点逐字节原样保留。
/// 未加引号的属性值不在契约内 —— 那是调用方产出的畸形 HTML,不是转义能救的。
pub fn html_escape(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "HTML_ESCAPE", args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(type_err(host, "HTML_ESCAPE", &args[0]));
    };
    Ok(Outcome::normal(Value::String(escape_str(s))))
}

/// 转义核心(纯函数,供单测与 `StdFn` 包装共用)。单趟扫描,输出一次分配:
/// 容量取「输入字节长 + 16」—— 五个转义里膨胀最大的是单字符 → 6 字节,但
/// 纯文本段占绝对多数,`+16` 的过分配远好于二次分配;最坏情况(全 `&`)由
/// `String` 的倍增兜底,仍是一次线性摊销。
fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// `HTML_UNESCAPE(data) -> STRING`:HTML 实体解码,**全表**(WHATWG 官方数据,
/// 生成器产出)。语义见规范 §13.1 与模块文档 —— 要点:命名实体分号形式优先,
/// legacy 子集允许缺分号且按**最长前缀**匹配(HTML5 文本行为,`&copyx` →
/// `©x`);数字引用按 WHATWG 规则映射(越界 / 代理区 → U+FFFD,溢出有检查);
/// 未知命名实体**原样保留**。
pub fn html_unescape(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity(host, "HTML_UNESCAPE", args.len(), 1));
    }
    let Value::String(s) = &args[0] else {
        return Err(type_err(host, "HTML_UNESCAPE", &args[0]));
    };
    Ok(Outcome::normal(Value::String(unescape_str(s))))
}

/// WHATWG 数字引用解析表:0x80–0x9F 按 windows-1252 映射(HTML5
/// "numeric character reference end state" 表),缺项与 0x00 一律 U+FFFD。
fn numeric_replacement(cp: u32) -> Option<char> {
    let mapped = match cp {
        0x00 => None,
        // windows-1252 映射表的五个「洞」:0x81 / 0x8D / 0x8F / 0x90 / 0x9D
        // 在 WHATWG 数字引用表里就是 U+FFFD,绝不能原样穿过。
        0x81 | 0x8D | 0x8F | 0x90 | 0x9D => None,
        0x80 => Some(0x20AC),
        0x82 => Some(0x201A),
        0x83 => Some(0x0192),
        0x84 => Some(0x201E),
        0x85 => Some(0x2026),
        0x86 => Some(0x2020),
        0x87 => Some(0x2021),
        0x88 => Some(0x02C6),
        0x89 => Some(0x2030),
        0x8A => Some(0x0160),
        0x8B => Some(0x2039),
        0x8C => Some(0x0152),
        0x8E => Some(0x017D),
        0x91 => Some(0x2018),
        0x92 => Some(0x2019),
        0x93 => Some(0x201C),
        0x94 => Some(0x201D),
        0x95 => Some(0x2022),
        0x96 => Some(0x2013),
        0x97 => Some(0x2014),
        0x98 => Some(0x02DC),
        0x99 => Some(0x2122),
        0x9A => Some(0x0161),
        0x9B => Some(0x203A),
        0x9C => Some(0x0153),
        0x9E => Some(0x017E),
        0x9F => Some(0x0178),
        // 其余码点原样;代理区交给 from_u32 的 None → U+FFFD。
        other => Some(other),
    };
    mapped.and_then(char::from_u32).or(Some('\u{FFFD}'))
}

/// 实体名查找(纯名称字节序比较;表按名称排序,查询走 `binary_search_by`)。
/// 分号不进比较键 —— 输入里的 `;` 由调用方在游程终点单独判定并消费。
fn lookup_semicolon(name: &[u8]) -> Option<&'static [u32]> {
    entities::SEMICOLON
        .binary_search_by(|(n, _)| n.as_bytes().cmp(name))
        .ok()
        .map(|i| entities::SEMICOLON[i].1)
}

/// legacy 缺分号形式。
fn lookup_legacy(name: &[u8]) -> Option<&'static [u32]> {
    entities::LEGACY
        .binary_search_by(|(n, _)| n.as_bytes().cmp(name))
        .ok()
        .map(|i| entities::LEGACY[i].1)
}

fn unescape_str(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'&' {
            // 非 '&' 的字节段整段拷贝(UTF-8 安全:'&' 是 ASCII,切片边界
            // 落在 ASCII 字符上必然是字符边界)。
            let start = i;
            while i < b.len() && b[i] != b'&' {
                i += 1;
            }
            out.push_str(&s[start..i]);
            continue;
        }
        // —— '&' 开头的候选 ——
        match b.get(i + 1) {
            // 数字引用:`&#DDDD;` / `&#xHHHH;`(缺分号也解码,HTML5 legacy)。
            Some(b'#') => {
                let (mut j, radix) = match b.get(i + 2) {
                    Some(b'x' | b'X') => (i + 3, 16u64),
                    _ => (i + 2, 10u64),
                };
                let mut acc: u64 = 0;
                let mut too_big = false;
                let mut digits = 0usize;
                while let Some(&c) = b.get(j) {
                    let d = match (radix, c) {
                        (10, b'0'..=b'9') => (c - b'0') as u64,
                        (16, b'0'..=b'9') => (c - b'0') as u64,
                        (16, b'a'..=b'f') => (c - b'a') as u64 + 10,
                        (16, b'A'..=b'F') => (c - b'A') as u64 + 10,
                        _ => break,
                    };
                    digits += 1;
                    if !too_big {
                        acc = acc.saturating_mul(radix).saturating_add(d);
                        if acc > 0x10FFFF {
                            too_big = true;
                        }
                    }
                    j += 1;
                }
                if digits == 0 {
                    // `&#` 后面没有数字 → 字面量。只消费 '&',让后续字节
                    // 走正常通道(`&#;` 里的 ';' 也会被原样吐出)。
                    out.push('&');
                    i += 1;
                    continue;
                }
                if b.get(j) == Some(&b';') {
                    j += 1;
                }
                let cp = if too_big || acc == 0 {
                    None
                } else {
                    Some(acc as u32)
                };
                let ch = cp.and_then(numeric_replacement).unwrap_or('\u{FFFD}');
                out.push(ch);
                i = j;
            }
            // 命名引用:取 ASCII 字母数字游程,分号形式优先,legacy 按最长
            // 前缀匹配;都没有 → '&' 原样,游程留在原地走正常通道。
            Some(c) if c.is_ascii_alphanumeric() => {
                let mut end = i + 1;
                while end < b.len()
                    && end - i <= entities::MAX_NAME_LEN
                    && b[end].is_ascii_alphanumeric()
                {
                    end += 1;
                }
                let run = &b[i + 1..end];
                let decoded = if b.get(end) == Some(&b';') {
                    lookup_semicolon(run).map(|cps| (cps, end + 1))
                } else {
                    None
                }
                .or_else(|| {
                    // legacy 缺分号:最长前缀优先(HTML5 文本行为;
                    // `&copyx` → `©x`,`&copyz;` → `©z;`)。
                    let max = run.len().min(entities::MAX_NAME_LEN);
                    (1..=max)
                        .rev()
                        .find_map(|len| lookup_legacy(&run[..len]).map(|cps| (cps, i + 1 + len)))
                });
                match decoded {
                    Some((cps, consumed)) => {
                        for &cp in cps {
                            // 命名实体不经过数字引用映射表;表里的码点全部合法。
                            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                        }
                        i = consumed;
                    }
                    None => {
                        out.push('&');
                        i += 1;
                    }
                }
            }
            // `&` 后面不是字母数字(空格 / 结尾 / 另一个实体等)→ 字面量。
            _ => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{escape_str, unescape_str};

    // ── escape ── 期望值取自规范 §13.1 的映射表,不是实现输出。

    #[test]
    fn escape_five_characters() {
        assert_eq!(escape_str("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&#39;");
        assert_eq!(escape_str("\"'\""), "&quot;&#39;&quot;");
    }

    #[test]
    fn escape_passes_non_ascii_through() {
        // 非 ASCII 原样 —— 转义集就是五个 ASCII 字符。
        assert_eq!(escape_str("héllo 世界 🌍"), "héllo 世界 🌍");
        assert_eq!(escape_str(""), "");
    }

    #[test]
    fn escape_is_idempotent_safe_input() {
        // 无特殊字符的串转义一次与两次结果相同(有 & 的串不成立 —— 那正是
        // 「转义后不要再转义」的原因,也是 roundtrip 只定义在 UNESCAPE∘ESCAPE
        // 上的原因)。
        assert_eq!(escape_str("plain"), escape_str(&escape_str("plain")));
    }

    // ── unescape ── 全表 / legacy 最长前缀 / 数字引用 / 字面量回退。

    #[test]
    fn unescape_full_table_entities() {
        // 全表口径:&copy; 必须正确解码(半表提案的反例)。
        assert_eq!(unescape_str("&lt;b&amp;&copy; &#65;&#x41;"), "<b&© AA");
        // 双码点实体。
        assert_eq!(unescape_str("&NotNestedGreaterGreater;"), "⪢̸");
    }

    #[test]
    fn unescape_legacy_longest_prefix() {
        // HTML5 文本行为:legacy 名缺分号也解码,按最长前缀。
        assert_eq!(unescape_str("&copyx"), "©x");
        assert_eq!(unescape_str("&copyz;"), "©z;");
        assert_eq!(unescape_str("&amp"), "&");
        // 分号形式优先。
        assert_eq!(unescape_str("&amp;"), "&");
        // notin;是分号实体,not 是 legacy —— 两者都要能解。
        assert_eq!(unescape_str("&notin;"), "∉");
        assert_eq!(unescape_str("&not"), "¬");
    }

    #[test]
    fn unescape_unknown_named_entity_is_verbatim() {
        assert_eq!(unescape_str("&unknownent;"), "&unknownent;");
        assert_eq!(unescape_str("&Dummy;"), "&Dummy;");
        assert_eq!(unescape_str("&;"), "&;");
        assert_eq!(unescape_str("&"), "&");
        assert_eq!(unescape_str("100% & up"), "100% & up");
    }

    #[test]
    fn unescape_numeric_references() {
        assert_eq!(unescape_str("&#65;"), "A");
        assert_eq!(unescape_str("&#x41;"), "A");
        assert_eq!(unescape_str("&#X41;"), "A");
        assert_eq!(unescape_str("&#x1F600;"), "\u{1F600}");
        // 双重编码不发生:数字引用解出的 '&' **不再**被二次解码。
        assert_eq!(unescape_str("&#38;amp;"), "&amp;");
        // 越界 / 代理区 / 0 → U+FFFD(WHATWG 规则),不 panic。
        assert_eq!(unescape_str("&#x110000;"), "\u{FFFD}");
        assert_eq!(unescape_str("&#0;"), "\u{FFFD}");
        assert_eq!(unescape_str("&#xD800;"), "\u{FFFD}");
    }

    #[test]
    fn unescape_numeric_windows1252_mapping() {
        // 0x80–0x9F 段走 WHATWG 的 windows-1252 映射表。
        assert_eq!(unescape_str("&#151;"), "—"); // 0x97 → U+2014
        assert_eq!(unescape_str("&#128;"), "€"); // 0x80 → U+20AC
                                                 // 缺项(0x81 等)→ U+FFFD。
        assert_eq!(unescape_str("&#129;"), "\u{FFFD}");
    }

    #[test]
    fn unescape_malformed_is_verbatim_prefix() {
        // `&#` 无数字 → 字面量;后续内容正常处理。
        assert_eq!(unescape_str("&#;x"), "&#;x");
        assert_eq!(unescape_str("&#x;"), "&#x;");
        assert_eq!(unescape_str("&#"), "&#");
    }

    #[test]
    fn roundtrip_theorem_holds() {
        // §13.1:HTML_UNESCAPE(HTML_ESCAPE(s)) == s 对一切 s 成立。
        for s in [
            "a<b>&\"'",
            "héllo 世界 🌍",
            "&amp; &lt; &copy; &#65;",
            "",
            "&&&>>>\"\"'''",
        ] {
            assert_eq!(unescape_str(&escape_str(s)), s);
        }
    }
}
