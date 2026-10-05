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
//! `entities` 由生成器从 WHATWG `entities.json` 官方数据产出(数据文件
//! 不入库,生成器入库 —— `src/bin/gen-entities.rs`)。半张表在安全语境下
//! 不是「documented 局限」而是**调用方拿到的错误数据**(`&copy;` 原样穿过),
//! 这是 D12-004「不做半张表」教训在安全语义下的加倍成立。未知命名实体
//! **原样保留**是 HTML5 自身的容错行为;数字引用按 WHATWG 规则映射
//! (越界 / 代理区 → U+FFFD,溢出有检查,不得 panic)。

pub(crate) mod entities;
pub(crate) mod filter;
pub(crate) mod serialize;
pub(crate) mod tokenize;
pub(crate) mod tree;

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.sanitize",
    functions: &[
        ("HTML_ESCAPE", html_escape as StdFn),
        ("HTML_UNESCAPE", html_unescape as StdFn),
        ("HTML_SANITIZE", html_sanitize as StdFn),
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
pub(crate) fn escape_str(s: &str) -> String {
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
    Ok(Outcome::normal(Value::String(entities::decode(s))))
}

/// `HTML_SANITIZE(data, policy?) -> STRING`:富 HTML 白名单净化(旗舰成员)。
///
/// 形态:容错解析(tokenize)→ 树构建(白名单子集 tree-construction)→
/// 白名单过滤 → 序列化。违规标签删除而非转义;raw-text / 外来内容整棵
/// 删除;`policy` 缺省为保守策略(§13.2),形态不符 `E0030` 带名。
/// **安全不变量:重解析幂等**(`SANITIZE(SANITIZE(x, p), p) == SANITIZE(x, p)`,
/// W-12 测试守护);**不承诺**与浏览器解析逐位一致(§13.2 已登记差异)。
/// 性能:三趟各 O(n),深嵌套迭代栈(病态契约),基准见 baseline.txt M3 段。
pub fn html_sanitize(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    if args.is_empty() || args.len() > 2 {
        // 不用共享的 `arity()`:它的模板恒为「expects {want} argument(s)」,
        // 而本成员的 `policy` 可省 —— 照搬会对着合法的单参调用说「expects 2」。
        return Err(host.diag(
            ErrorCode::E0022,
            format!(
                "HTML_SANITIZE: function expects 1 or 2 argument(s), got {}",
                args.len()
            ),
        ));
    }
    let Value::String(src) = &args[0] else {
        return Err(type_err(host, "HTML_SANITIZE", &args[0]));
    };
    let policy = match args.get(1) {
        None => filter::Policy::default(),
        // policy 形态错必须**说 policy**:`type_err` 是给 data 用的
        // 「expected string」模板,用在 policy 上会指错参数(W-13 契约表
        // 当场抓到 `[1, 2]` 报 "expected string, got array")。
        Some(v @ Value::Dict(_)) => filter::parse_policy(host, v)?,
        Some(other) => {
            return Err(host.diag(
                ErrorCode::E0030,
                format!(
                    "HTML_SANITIZE: policy must be a DICT, got {}",
                    crate::value_kind(other)
                ),
            ))
        }
    };
    let tokens = tokenize::tokenize(src);
    let mut tree = tree::build(tokens);
    filter::filter_tree(&mut tree, &policy);
    let mut out = String::with_capacity(src.len() + 16);
    serialize::serialize(&tree, &mut out);
    Ok(Outcome::normal(Value::String(out)))
}

#[cfg(test)]
mod tests {
    use super::entities::decode as unescape_str;
    use super::escape_str;
    use super::{filter, serialize, tokenize, tree};

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

    // ── sanitize ── 走与成员同一条流水线(tokenize → build → filter →
    // serialize),缺省策略。**期望值是人工裁定的输出,不是实现回读。**

    fn sanitize_default(src: &str) -> String {
        let tokens = tokenize::tokenize(src);
        let mut tree = tree::build(tokens);
        filter::filter_tree(&mut tree, &filter::Policy::default());
        let mut out = String::with_capacity(src.len() + 16);
        serialize::serialize(&tree, &mut out);
        out
    }

    #[test]
    fn sanitize_keeps_siblings_after_a_dropped_raw_text() {
        // D13-003 回归:tokenizer 吞掉 `</script>` 不发 End,若树构建不在
        // 收到 RawText 时弹栈,后续内容会挂进被整删的 script 里一并消失。
        assert_eq!(
            sanitize_default("<b>x</b><script>evil</script>after"),
            "<b>x</b>after"
        );
        // 未闭合的 raw-text 吞到串尾(HTML5 同款),其后无内容可丢。
        assert_eq!(sanitize_default("<b>x</b><script>evil"), "<b>x</b>");
        // 多个 raw-text 交替,每个都不能吃掉它的后继。
        assert_eq!(
            sanitize_default("<b>1</b><style>a{}</style><i>2</i><script>b</script><u>3</u>"),
            "<b>1</b><i>2</i><u>3</u>"
        );
    }

    #[test]
    fn sanitize_keeps_content_of_unwrapped_elements() {
        // D13-005 回归:非白名单元素是「拆壳留内容」(unwrap),不是整删。
        // 挂载点算错时这层内容会静默消失(两代错法分别表现为:整段丢失 /
        // 被踢到元素外面)。
        assert_eq!(sanitize_default("<div onclick=\"x\">y</div>"), "y");
        assert_eq!(sanitize_default("<table><td>x</td></table>"), "x");
        // 拆壳保内容的同时,保留标签的嵌套结构不能被拆坏。
        assert_eq!(sanitize_default("<div><b>x</b></div>"), "<b>x</b>");
        assert_eq!(
            sanitize_default("<section><p>a</p><ul><li>b</li></ul></section>"),
            "<p>a</p><ul><li>b</li></ul>"
        );
        // 未知属性随拆壳一并消失,但内容留下。
        assert_eq!(sanitize_default("<span class=\"c\" id=\"i\">t</span>"), "t");
    }

    #[test]
    fn sanitize_drops_noembed_with_content() {
        // D13-004 回归:规范 §13.2 把 noembed 列入 raw-text 整删集,实现漏了
        // 会让它的 raw-text 正文被当标记解析(mXSS 面)。
        assert_eq!(
            sanitize_default("<noembed><b>x</b></noembed>after"),
            "after"
        );
        assert_eq!(sanitize_default("<NOEMBED>x</NOEMBED>"), "");
    }

    #[test]
    fn plaintext_is_unwrapped_not_dropped() {
        // D13-008 回归:规范 §13.2 的整删集只列 8 个 raw-text 元素,`plaintext`
        // 不在其中,且明写「按普通文本处理」⇒ 拆壳留内容,不是整删。
        // (浏览器把 `<plaintext>` 之后全部当文本,本成员不复制该行为 ——
        //  已登记的解析差分;但也不能因此把用户内容整段丢掉。)
        assert_eq!(sanitize_default("<plaintext>hello"), "hello");
        assert_eq!(sanitize_default("<b>a</b><plaintext>b"), "<b>a</b>b");
        // 拆壳不削弱安全性:其内的危险元素照常按各自规则处置。
        assert_eq!(sanitize_default("<plaintext><script>alert(1)</script>"), "");
        assert_eq!(
            sanitize_default("<plaintext><img src=x onerror=alert(1)>"),
            ""
        );
        assert_eq!(
            sanitize_default("<plaintext><a href=\"javascript:alert(1)\">x</a>"),
            "<a>x</a>"
        );
    }
}
