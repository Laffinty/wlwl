//! `wlwl:std.sanitize` 成员契约表 —— 标准库规范 §13(v0.11.3 M1 / M3)。
//!
//! 沿三份既有契约(collection / str_math / test)的形态:
//!
//! 1. **规范表对拍**(四件套第 3 条):从规范 §13 的成员表**解析**出成员面,
//!    与 `resolve("wlwl:std.sanitize")` 的实现清单对拍 —— 两侧不同源,
//!    规范被改坏而实现没动时这里变红;
//! 2. **冻结用例**:诊断码 + 消息逐字 + 结果 `display()`;
//! 3. **反向守卫**:成员集非空、点名成员在列、**两侧不得空转**(集合比较
//!    本身也要能被「多出 / 少掉 / 改名」打红)。
//!
//! M3 追加的三块(计划 W-12 / W-13):
//!
//! - [`SANITIZE_CASES`] —— OWASP XSS Filter Evasion 经典向量 + mXSS 教科书
//!   样本 + 畸形输入 + 非缺省 policy,**逐条冻结期望值并注明裁定依据**;
//! - 幂等性三组 —— 契约语料 / 324 条变异 / mXSS 样本,断言
//!   `SANITIZE(SANITIZE(x, p), p) == SANITIZE(x, p)` 字节级(规范 §13.2 的
//!   规范性安全不变量);
//! - 病态契约 —— 10 万层嵌套不栈溢出、1 MB 文本 / 2 万属性 / 10 万实体,
//!   这些是**断言**(`benches/` 里的基准只出数字,不会让 CI 变红)。
//!
//! SQL 转义的**否决记录**(规范 §13.3)也被本文件钉住:删掉那段规范的
//! 尝试会在这里变红 —— 「不做」与「做」同权,都是契约。

use std::path::{Path, PathBuf};
use wlwl_eval::Evaluator;
use wlwl_value::Value;

const CASES: &[Case] = &[
    // ── HTML_ESCAPE(§13.1)──
    Case {
        name: "escape_five_characters",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("a<b>&\"'")"#,
        expect: "a&lt;b&gt;&amp;&quot;&#39;",
    },
    Case {
        name: "escape_apostrophe_is_numeric_ref",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("\"'\"")"#,
        expect: "&quot;&#39;&quot;",
    },
    Case {
        name: "escape_non_ascii_passthrough",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("héllo 世界 🌍")"#,
        expect: "héllo 世界 🌍",
    },
    Case {
        name: "escape_double_amp",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("&&")"#,
        expect: "&amp;&amp;",
    },
    Case {
        name: "escape_non_string",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE(1)"#,
        expect: "!E0030 HTML_ESCAPE: expected string, got integer",
    },
    Case {
        name: "escape_arity_zero",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE()"#,
        expect: "!E0022 HTML_ESCAPE: function expects 1 argument(s), got 0",
    },
    // ── HTML_UNESCAPE(§13.1:全表 / legacy 最长前缀 / 数字引用 / 字面量回退)──
    Case {
        name: "unescape_full_table",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&lt;b&amp;&copy;&#65;&#x41;")"#,
        expect: "<b&©AA",
    },
    Case {
        name: "unescape_two_codepoint_entity",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&NotNestedGreaterGreater;")"#,
        expect: "⪢̸",
    },
    Case {
        name: "unescape_legacy_longest_prefix",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&copyx")"#,
        expect: "©x",
    },
    Case {
        name: "unescape_legacy_before_semicolon_literal",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&copyz;")"#,
        expect: "©z;",
    },
    Case {
        name: "unescape_semicolon_form_preferred",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&amp;")"#,
        expect: "&",
    },
    Case {
        name: "unescape_legacy_no_semicolon",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&amp")"#,
        expect: "&",
    },
    Case {
        name: "unescape_numeric_decimal",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#151;")"#,
        expect: "—",
    },
    Case {
        name: "unescape_numeric_windows1252_hole_is_fffd",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#129;")"#,
        expect: "�",
    },
    Case {
        name: "unescape_numeric_out_of_range_is_fffd",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#x110000;")"#,
        expect: "�",
    },
    Case {
        name: "unescape_numeric_surrogate_is_fffd",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#xD800;")"#,
        expect: "�",
    },
    Case {
        name: "unescape_no_double_decoding",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#38;amp;")"#,
        expect: "&amp;",
    },
    Case {
        name: "unescape_unknown_named_entity_verbatim",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&unknownent;")"#,
        expect: "&unknownent;",
    },
    Case {
        name: "unescape_stray_semicolon_verbatim",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&;")"#,
        expect: "&;",
    },
    Case {
        name: "unescape_ampersand_alone_verbatim",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("100% & up")"#,
        expect: "100% & up",
    },
    Case {
        name: "unescape_non_string",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE([1])"#,
        expect: "!E0030 HTML_UNESCAPE: expected string, got array",
    },
    // ── roundtrip 定理(§13.1):UNESCAPE ∘ ESCAPE == 恒等 ──
    Case {
        name: "roundtrip_theorem",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE", "HTML_UNESCAPE"]); HTML_UNESCAPE(HTML_ESCAPE("a<b>&\"' &copy; ⪢̸"))"#,
        expect: "a<b>&\"' &copy; ⪢̸",
    },
];

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

fn run(src: &str) -> Result<Value, String> {
    let ast = wlwl_parser::parse(src, "t.wll").expect("case source parses");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => Ok(v),
        Err(e) => Err(format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        )),
    }
}

fn actual(case: &Case) -> String {
    match run(case.src) {
        Ok(v) => v.display(),
        Err(s) => s,
    }
}

#[test]
fn sanitize_matches_the_frozen_contract() {
    let mut bad = Vec::new();
    for case in CASES {
        let got = actual(case);
        if case.expect != got {
            bad.push(format!(
                "{name}:\n      frozen: {want}\n      actual: {got}",
                name = case.name,
                want = case.expect,
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} case(s) diverged from the frozen sanitize contract:\n  - {}",
        bad.len(),
        CASES.len(),
        bad.join("\n  - ")
    );
}

// ── 规范表对拍(四件套第 3 条:两侧不同源)─────────────────────────

fn spec_text() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md");
    std::fs::read_to_string(&p).expect("stdlib spec is readable")
}

fn section(section_no: u32, next_no: u32) -> String {
    let text = spec_text();
    let start = text
        .find(&format!("## {section_no} "))
        .unwrap_or_else(|| panic!("§{section_no} header exists"));
    let end = text[start..]
        .find(&format!("\n## {next_no} "))
        .map(|i| start + i)
        .unwrap_or_else(|| panic!("§{next_no} header follows §{section_no}"));
    text[start..end].to_string()
}

/// 从成员表首列抽成员名(与 str_math_contract 同款口径)。
fn members_of(section_body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in section_body.lines() {
        let t = line.trim();
        if !t.starts_with('|') || t.contains("---") {
            continue;
        }
        let first = t.trim_start_matches('|').split('|').next().unwrap_or("");
        if first.trim() == "签名" {
            continue;
        }
        for cell in first.split(" / ") {
            let name: String = cell
                .trim()
                .trim_matches('`')
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                continue;
            }
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

fn impl_members() -> Vec<String> {
    match wlwl_std::resolve("wlwl:std.sanitize") {
        Some(wlwl_std::StdBackend::Native(spec)) => {
            spec.functions.iter().map(|(n, _)| n.to_string()).collect()
        }
        _ => panic!("wlwl:std.sanitize must resolve to a Native SPEC"),
    }
}

#[test]
fn sanitize_member_set_matches_the_spec_table() {
    let spec = members_of(&section(13, 14));
    assert_eq!(
        spec,
        vec!["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE"],
        "§13 member table drifted"
    );
    // M1 只落地转义两成员;M3 已落地 `HTML_SANITIZE` —— 按 M1 时写在
    // 本处的交接说明,impl 断言改为三员全等(集合等值,顺序同 §13 表)。
    let impls = impl_members();
    assert_eq!(
        impls,
        vec!["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE"],
        "M3 surface: all three §13 members landed"
    );
}

/// 反向守卫:集合比较不得是空转 —— 任何一边多出 / 少掉 / 改名都会红。
#[test]
fn sanitize_member_set_is_not_trivially_satisfied() {
    let impls = impl_members();
    assert!(!impls.is_empty(), "impl surface must not be empty");
    for n in &impls {
        assert!(
            ["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE"].contains(&n.as_str()),
            "unexpected member `{n}`"
        );
    }
}

/// 规范 §13.3 的 SQL 否决记录是**契约的一部分**:「不做」与「做」同权。
/// 删掉这段规范的尝试在这里变红。
#[test]
fn sql_escape_rejection_record_stays_in_the_spec() {
    let spec = spec_text();
    for marker in [
        "为什么没有 SQL 转义成员",
        "last resort",
        "prepared statement",
        "白名单映射",
    ] {
        assert!(
            spec.contains(marker),
            "§13.3 SQL rejection record lost its marker `{marker}` — \
             the rejection is a contract, restore it or re-adjudicate"
        );
    }
}

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_sanitize_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

// ══════════════════════════════════════════════════════════════
// W-13 · `HTML_SANITIZE` 契约表:OWASP 规避向量逐条冻结
// ══════════════════════════════════════════════════════════════
//
// **期望值的来源纪律**:每条的 `expect` 是**裁定结果**,不是实现回读。
// 逐条判读「这个输出安不安全、符不符合规范 §13.2」之后才冻结;判读发现
// 实现与规范相悖的(W-13 抓出 3 条),改实现并登记 D13-008 ~ D13-010,
// 而不是把实现当下的输出冻成「契约」。
//
// 覆盖分组(名字前缀即分组,`m3_*` 测试按前缀取子集):
//   `img_*` / `script_*` / `body_*` / `meta_*` / `object_*` / `style_*` /
//   `link_*` / `iframe_*` / `bgsound` / `base_*` / `table_*` / `div_*`
//     —— OWASP XSS Filter Evasion Cheat Sheet 经典向量(逐条);
//   `a_*`  —— **唯一有非空输出的族**:白名单只留 22 个排版级标签,上面的
//     向量绝大多数整条消失,唯一能观察到属性 / scheme 判定的是 `<a href>`;
//   `mxss_*` —— mXSS 教科书样本(Sonar cheatsheet 命名空间混淆族中
//     **作用于白名单子集**的部分);
//   `malformed_*` —— 畸形输入的容错口径;
//   `policy_*` —— 非缺省 policy 与 policy 形态错;
//   `sanitize_*` —— 成员级诊断。
//
// **不承诺往返**:输出的字节与输入无对应关系(嵌套矫正的固有属性,
// 规范 §13.2 明写)。本表冻结的是「输入 → 输出」这一个方向。

/// 冻结用例:`expect` = 裁定后的输出;`why` = 裁定依据(逐条注明)。
struct SanCase {
    name: &'static str,
    html: &'static str,
    /// `""` = 缺省策略;否则是 wlwl 的 DICT 字面量文本。
    policy: &'static str,
    expect: &'static str,
    why: &'static str,
}

const SANITIZE_CASES: &[SanCase] = &[
    // ── OWASP XSS Filter Evasion Cheat Sheet:`javascript:` / `vbscript:`
    //    系向量。`img` / `link` / `bgsound` / `base` / `meta` / `object` /
    //    `body` / `input` 全部**不在**白名单 → 拆壳留内容,而这些向量
    //    自身没有可见内容,故输出恒为空串。**空串是正确结果**:危险属性
    //    随元素一起消失,没有「标签没了但属性还在」的形态。──
    SanCase { name: "img_js_quoted", html: r#"<IMG SRC="javascript:alert('XSS');">"#, policy: "", expect: "",
        why: "img 不在白名单 → 拆壳;无子内容 → 空串。SRC 随元素消失" },
    SanCase { name: "img_js_bare", html: r#"<IMG SRC=javascript:alert('XSS')>"#, policy: "", expect: "",
        why: "无引号属性值同样随 img 消失;容错解析不得因此报错" },
    SanCase { name: "img_js_case", html: r#"<IMG SRC=JaVaScRiPt:alert('XSS')>"#, policy: "", expect: "",
        why: "标签名 / 属性名小写化;大写 scheme 随元素消失" },
    SanCase { name: "img_js_entity_quote", html: r#"<IMG SRC=javascript:alert(&quot;XSS&quot;)>"#, policy: "", expect: "",
        why: "无引号属性值里的 `&quot;` 解析期解码为 `\"`;仍随 img 消失" },
    SanCase { name: "img_js_backtick", html: "<IMG SRC=`javascript:alert(\"RSnake says, 'XSS'\")`>", policy: "", expect: "",
        why: "反引号是容错怪字符,被吞进属性值;随 img 消失" },
    SanCase { name: "img_js_fromcharcode", html: r#"<IMG SRC=javascript:alert(String.fromCharCode(88,83,83))>"#, policy: "", expect: "",
        why: "无引号值到空白 / `>` 为止;随 img 消失" },
    SanCase { name: "img_hash_onmouseover", html: r#"<IMG SRC=# onmouseover="alert('xxs')">"#, policy: "", expect: "",
        why: "`#` 开头的相对 URL 本就允许,但 img 整体拆壳,属性一并消失" },
    SanCase { name: "img_empty_src_onmouseover", html: r#"<IMG SRC= onmouseover="alert('xxs')">"#, policy: "", expect: "",
        why: "布尔属性 SRC(无值)+ onmouseover;两者随 img 消失" },
    SanCase { name: "img_bare_onmouseover", html: r#"<IMG onmouseover="alert('xxs')">"#, policy: "", expect: "",
        why: "事件属性随 img 消失" },
    SanCase { name: "img_slash_onerror", html: r#"<IMG SRC=/ onerror="alert(String.fromCharCode(88,83,83))"></img>"#, policy: "", expect: "",
        why: "自闭合 `<img/>` 不入栈、后续 `</img>` 栈内找不到 → 忽略;整体空串" },
    SanCase { name: "img_u0061lert", html: r#"<img src=x onerror="alert('XSS')">"#, policy: "", expect: "",
        why: "小写同款;属性值与标签名无关地一起消失" },
    SanCase { name: "img_numeric_entity_scheme", html: r#"<IMG SRC=&#106;&#97;&#118;&#97;&#115;&#99;&#114;&#105;&#112;&#116;&#58;alert('XSS')>"#, policy: "", expect: "",
        why: "十进制实体在解析期全表解码成 `javascript:`;元素仍拆壳" },
    SanCase { name: "img_zeropad_entity", html: r#"<IMG SRC=&#0000106&#0000097&#0000118alert('XSS')>"#, policy: "", expect: "",
        why: "零填充数字引用按 WHATWG 规则解码;元素拆壳" },
    SanCase { name: "img_hex_entity", html: r#"<IMG SRC=&#x6A&#x61&#x76&#x61&#x73&#x63&#x72&#x69&#x70&#x74&#x3A alert('XSS')>"#, policy: "", expect: "",
        why: "十六进制 + 大写 X 前缀;解码后含空格截断;元素拆壳" },
    SanCase { name: "img_space_in_scheme", html: r#"<IMG SRC="jav ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "scheme 中间有空格 → 非法形态;元素拆壳" },
    SanCase { name: "img_tab_entity_scheme", html: r#"<IMG SRC="jav&#x09;ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "制表实体解码后落在 scheme 内;元素拆壳" },
    SanCase { name: "img_nl_entity_scheme", html: r#"<IMG SRC="jav&#x0A;ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "换行实体解码后落在 scheme 内;元素拆壳" },
    SanCase { name: "img_cr_entity_scheme", html: r#"<IMG SRC="jav&#x0D;ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "回车实体;元素拆壳" },
    SanCase { name: "img_ctrl14_scheme", html: "<IMG SRC=\" \u{e}  javascript:alert('XSS');\">", policy: "", expect: "",
        why: "U+000E 控制字符在值内;元素拆壳(值不参与判定)" },
    SanCase { name: "img_vbscript", html: r#"<IMG SRC='vbscript:alert("XSS")'>"#, policy: "", expect: "",
        why: "单引号属性值合法;vbscript: 不在 scheme 白名单,且 img 拆壳" },
    SanCase { name: "img_dynsrc", html: r#"<IMG DYNSRC="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "URL 类属性之一;元素拆壳" },
    SanCase { name: "img_lowsrc", html: r#"<IMG LOWSRC="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "同上" },
    SanCase { name: "bgsound", html: r#"<BGSOUND SRC="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "bgsound 不在白名单 → 拆壳" },
    SanCase { name: "link_stylesheet_js", html: r#"<LINK REL="stylesheet" HREF="javascript:alert('XSS');">"#, policy: "", expect: "",
        why: "link 是 void 元素且不在白名单 → 拆壳;href 是 URL 属性但无从判定起" },
    SanCase { name: "base_href_js", html: r#"<BASE HREF="javascript:alert('XSS');//">"#, policy: "", expect: "",
        why: "base 不在白名单 → 拆壳(规范 §13.2 未把它列入整删集,故走 unwrap)" },
    SanCase { name: "body_background_js", html: r#"<BODY BACKGROUND="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "body 不在白名单 → 拆壳;无子内容 → 空串" },
    SanCase { name: "body_onload_bare", html: r#"<BODY ONLOAD=alert('XSS')>"#, policy: "", expect: "",
        why: "事件属性无引号;随 body 消失" },
    SanCase { name: "body_onload_garbage", html: "<BODY onload!#$%&()*~+-_.,:;?@[/|\\]^`=alert(\"XSS\")>", policy: "", expect: "",
        why: "属性名里的怪字符被容错吞进名字(不当作新属性);随 body 消失" },
    SanCase { name: "meta_refresh_js", html: r#"<META HTTP-EQUIV="refresh" CONTENT="0;url=javascript:alert('XSS');">"#, policy: "", expect: "",
        why: "meta 是 void 且不在白名单 → 拆壳;`CONTENT` 里的 `url=` 语义不解析" },
    SanCase { name: "meta_refresh_data_b64", html: r#"<META HTTP-EQUIV="refresh" CONTENT="0;url=data:text/html base64,PHNjcmlwdD4=">"#, policy: "", expect: "",
        why: "同上;`data:` + base64 载荷无任何解码路径" },
    SanCase { name: "object_scriptlet", html: r#"<OBJECT TYPE="text/x-scriptlet" DATA="http://xss.rocks/scriptlet.html"></OBJECT>"#, policy: "", expect: "",
        why: "object 在整删集(危险嵌入)→ 连内容删除;此处无子内容" },
    SanCase { name: "object_classid_param", html: r#"<OBJECT classid=clsid:ae24fdae-03c6-11d1><param name=url value=javascript:alert('XSS')></OBJECT>"#, policy: "", expect: "",
        why: "object 整棵删除 → param 子节点跟着消失" },
    SanCase { name: "iframe_js", html: r#"<IFRAME SRC="javascript:alert('XSS');"></IFRAME>"#, policy: "", expect: "",
        why: "iframe 是 raw-text 元素 → 整棵删除(封掉其内文被再解析的 mXSS 面)" },
    // ── script / style 族:整删,且验证「删除不越界」──
    SanCase { name: "script_slash_src", html: r#"<SCRIPT/XSS SRC="http://xss.rocks/xss.js"></SCRIPT>"#, policy: "", expect: "",
        why: "`/XSS` 触发自闭合容错,script 仍整删;后随的 src 属性不产生输出" },
    SanCase { name: "script_query_lt", html: r#"<SCRIPT SRC=http://xss.rocks/xss.js?< B >"#, policy: "", expect: "",
        why: "无引号值遇到 `<` 提前结束标签(容错),script 整删到 `</script>`(此处不存在 → 吞到串尾)" },
    SanCase { name: "script_malformed", html: r#"<<SCRIPT>alert("XSS");//\<</SCRIPT>"#, policy: "", expect: "&lt;",
        why: "首个 `<` 后面不是字母 → 字面量;其后的 script 整删到配对 `</SCRIPT>`。**唯一幸存的是那个字面 `<`,转义成 `&lt;`**" },
    SanCase { name: "style_li_unwrap", html: r#"<STYLE>li {list-style-image: url("javascript:alert('XSS')");}</STYLE><UL><LI>XSS</br>"#, policy: "", expect: "<ul><li>XSS</li></ul>",
        why: "style 整删但**不越界**:其后的 ul/li 在白名单,文本 XSS 留下;`</br>` 按结束标签忽略" },
    // ── `<a href>` 族:白名单子集里唯一能观察到 scheme 判定的面 ──
    SanCase { name: "a_href_js_keeps_text", html: r#"<a href="javascript:alert(1)">click</a>"#, policy: "", expect: "<a>click</a>",
        why: "a 在白名单、href 在属性白名单,但 scheme 不在 URL 白名单 → **属性删除、元素与文本保留**(不整删元素)" },
    SanCase { name: "a_href_js_location", html: r#"<A HREF="javascript:document.location='http://x/';">XSS</A>"#, policy: "", expect: "<a>XSS</a>",
        why: "同上;大写标签 / 属性名小写化不影响判定" },
    SanCase { name: "a_href_tab_entity", html: r#"<a href="jav&#x09;ascript:alert(1)">click</a>"#, policy: "", expect: "<a>click</a>",
        why: "制表实体解码后落在 scheme 内 → 剥控制字符后仍是 javascript: → 拒" },
    SanCase { name: "a_href_newline_raw", html: "<a href=\"java\nscript:alert(1)\">click</a>", policy: "", expect: "<a>click</a>",
        why: "**裸换行**在引号属性值内:值跨行读完,scheme 形态非法(非字母数字)→ 拒。浏览器会剥掉换行,本成员更严" },
    SanCase { name: "a_href_leading_ctrl", html: r#"<a href="&#01;javascript:alert(1)">click</a>"#, policy: "", expect: "<a>click</a>",
        why: "值首控制字符 → 判定前剥掉首尾空白与控制字符 → javascript: → 拒" },
    SanCase { name: "a_href_data_rejected", html: r#"<a href="data:text/html,<script>alert(1)</script>">d</a>"#, policy: "", expect: "<a>d</a>",
        why: "引号内的 `<script>` 只是属性值字符,不构成标记;`data:` 不在 scheme 白名单 → 属性删除" },
    SanCase { name: "a_href_relative_ok", html: r#"<a href="/rel/path">click</a>"#, policy: "", expect: "<a href=\"/rel/path\">click</a>",
        why: "相对 URL **恒允许**(规范 §13.2);属性值恒双引号包裹后序列化" },
    SanCase { name: "a_href_mailto_ok", html: r#"<a href="mailto:a@b.c">mail</a>"#, policy: "", expect: "<a href=\"mailto:a@b.c\">mail</a>",
        why: "mailto 在缺省 scheme 白名单里" },
    SanCase { name: "a_title_kept_href_dropped", html: r#"<a href="javascript:1" title="hi">x</a>"#, policy: "", expect: "<a title=\"hi\">x</a>",
        why: "逐属性独立判定:href 拒、title 留 —— 不得因为一个属性被拒就丢掉整个元素" },
    SanCase { name: "a_dup_href_first_wins", html: r#"<a href="https://ok" href="javascript:1">x</a>"#, policy: "", expect: "<a href=\"https://ok\">x</a>",
        why: "同名属性**首个生效**,第二个不再参与判定(规范 §13.2)" },
    // ── 非 a 元素里的 style / 危险属性:拆壳后属性必须一并消失 ──
    SanCase { name: "table_background_js", html: r#"<TABLE BACKGROUND="javascript:alert('XSS')"><td>x</td></TABLE>"#, policy: "", expect: "x",
        why: "table / td 都不在白名单 → 逐层拆壳;**属性随元素消失,只留文本**" },
    SanCase { name: "div_style_bg_url", html: r#"<DIV STYLE="background-image: url(javascript:alert('XSS'))">x</DIV>"#, policy: "", expect: "x",
        why: "STYLE 不在任何标签的属性白名单里 → 删除;div 拆壳留 x" },
    SanCase { name: "div_style_expression", html: r#"<DIV STYLE="width: expression(alert('XSS'));">x</DIV>"#, policy: "", expect: "x",
        why: "IE 表达式向量;同样只靠「属性不在白名单」挡住" },
    // ── mXSS 教科书样本(Sonar cheatsheet,作用于白名单子集的部分)──
    SanCase { name: "mxss_svg_onload", html: r#"<svg/onload=alert(1)>"#, policy: "", expect: "",
        why: "svg 是外来内容 → **整棵删除**(规范 §13.2),不是拆壳" },
    SanCase { name: "mxss_svg_script", html: r#"<svg><script>alert(1)</script></svg>"#, policy: "", expect: "",
        why: "svg 整删带走了内层 script;嵌套两重删除不产生残留" },
    SanCase { name: "mxss_math_xlink", html: r#"<math><mi//xlink:href="data:x,<script>alert(1)</script>">"#, policy: "", expect: "",
        why: "命名空间混淆族:math 整删;`xlink:href` 是 URL 属性但轮不到判定" },
    SanCase { name: "mxss_noscript", html: r#"<noscript><p title="</noscript><img src=x onerror=alert(1)>">"#, policy: "", expect: "",
        why: "noscript 在整删集 → 整段消失(浏览器按「脚本已启用」与否会分叉,不复制该差分)" },
    SanCase { name: "mxss_style_comment", html: "<style><!--</style><img src=x onerror=alert(1)>--></style>", policy: "", expect: "--&gt;",
        why: "**教科书 mXSS**:浏览器把 `<!--` 当 CSS 注释、img 存活;本成员的 style 是 raw-text,扫到第一个 `</style>` 即止,img 落在其外 → 拆壳为空。幸存文本 `-->` 被转义,惰性" },
    SanCase { name: "mxss_form_button", html: r#"<form><button formaction=javascript:alert(1)>X</button></form>"#, policy: "", expect: "X",
        why: "formaction 是 URL 属性,但 form / button 都不在白名单 → 属性随元素消失,只留文本" },
    SanCase { name: "mxss_details_ontoggle", html: r#"<details open ontoggle=alert(1)>x</details>"#, policy: "", expect: "x",
        why: "事件属性随 details 拆壳消失" },
    SanCase { name: "mxss_marquee_onstart", html: r#"<marquee onstart=alert(1)>x</marquee>"#, policy: "", expect: "x",
        why: "同上" },
    SanCase { name: "mxss_video_onerror", html: r#"<video><source onerror=alert(1)></video>"#, policy: "", expect: "",
        why: "video / source 都不在白名单且无可见内容" },
    SanCase { name: "mxss_input_autofocus", html: r#"<input autofocus onfocus=alert(1)>"#, policy: "", expect: "",
        why: "input 是 void 且不在白名单 → 拆壳为空;「autofocus 触发」的面随元素消失" },
    SanCase { name: "mxss_template", html: r#"<template><script>alert(1)</script></template>"#, policy: "", expect: "",
        why: "template 在整删集(其内容惰性、会被别的解析器复活)" },
    SanCase { name: "mxss_xmp", html: r#"<xmp><script>alert(1)</script></xmp>"#, policy: "", expect: "",
        why: "xmp 是 raw-text 元素 → 内文不作为标记解析、整棵删除" },
    SanCase { name: "mxss_plaintext", html: r#"<plaintext><script>alert(1)</script>"#, policy: "", expect: "",
        why: "plaintext **按普通元素拆壳**(D13-008:规范 §13.2 未把它列入整删集)→ 内层 script 照常整删。拆壳不放宽内层判定" },
    SanCase { name: "mxss_unclosed_comment", html: r#"<b title="<!--"><script>alert(1)</script>">x</b>"#, policy: "", expect: "<b>&quot;&gt;x</b>",
        why: "**引号属性值吞掉后续标记**(`>` 不终止引号内的值)→ script 沦为属性值字符;title 不在白名单 → 属性删除,值里的 `\">` 作为文本转义输出" },
    // ── 畸形 / 容错口径 ──
    SanCase { name: "malformed_unclosed_b", html: r#"<b>unclosed"#, policy: "", expect: "<b>unclosed</b>",
        why: "未闭合元素在串尾补结束标签(**输出恒是良构的**,这是幂等的前提)" },
    SanCase { name: "malformed_lone_lt", html: r#"1 < 2 and 3<4"#, policy: "", expect: "1 &lt; 2 and 3&lt;4",
        why: "`<` 后不是字母 / `!` / `/` → 字面量;文本节点恒转义 `&` `<` `>`" },
    SanCase { name: "malformed_dup_attr", html: r#"<a href="x" href="y">z</a>"#, policy: "", expect: "<a href=\"x\">z</a>",
        why: "首个生效" },
    SanCase { name: "malformed_stray_end", html: r#"</b>orphan"#, policy: "", expect: "orphan",
        why: "栈内找不到对应开标签的结束标签 → 忽略(**不生成空元素**,规范 §13.2 已登记该差分)" },
    SanCase { name: "malformed_attr_lt", html: r#"<b a<b>x</b>"#, policy: "", expect: "<b><b>x</b></b>",
        why: "标签内出现 `<` → 标签在该处结束,`<b>` 成为嵌套的开标签;未知属性 a 随外层 b 删除" },
    SanCase { name: "malformed_eof_in_tag", html: r#"<b x="unterminated"#, policy: "", expect: "<b></b>",
        why: "串尾截断的引号值:值取到串尾,`x` 不在白名单 → 属性删除;元素良构补尾" },
    // ── 非缺省 policy:键**整体替换**缺省值,不是合并(规范 §13.2)──
    SanCase { name: "policy_custom_tags", html: r#"<b>x</b><i>y</i>"#, policy: r#"["tags": ["i"]]"#, expect: "x<i>y</i>",
        why: "tags 整体替换 → b 不再在白名单 → 拆壳留 x;i 保留。**不合并**是规范明写的行为" },
    SanCase { name: "policy_custom_attrs", html: r#"<a href="https://x" title="t" rel="r">z</a>"#, policy: r#"["attributes": ["a": ["rel"]]]"#, expect: "<a rel=\"r\">z</a>",
        why: "attributes 整体替换 → href / title 都删,只留 rel;元素仍保留(在 tags 里)" },
    SanCase { name: "policy_keep_comments", html: r#"<b>x</b><!--c-->"#, policy: r#"["strip_comments": FALSE]"#, expect: "<b>x</b><!--c-->",
        why: "strip_comments=FALSE → 注释保留;注释内容里的 `--` 在序列化时全部移除(防 `-->` 提前闭合)" },
    SanCase { name: "policy_add_scheme", html: r#"<a href="ftp://h/f">z</a>"#, policy: r#"["url_schemes": ["ftp"]]"#, expect: "<a href=\"ftp://h/f\">z</a>",
        why: "url_schemes 整体替换 → 只剩 ftp;http / https / mailto 此时会被拒(调用方自己收窄的)" },
    SanCase { name: "policy_omitted_keys_take_defaults", html: r#"<b>x</b><a href="https://k" title="t">y</a><!--c-->"#, policy: r#"["url_schemes": ["ftp", "https"]]"#, expect: "<b>x</b><a href=\"https://k\" title=\"t\">y</a>",
        why: "**省略的键取缺省值**:只改 url_schemes 不影响 tags / attributes / strip_comments(注释仍删)" },
    // ── 成员级诊断(E0030 / E0022,逐字冻结)──
    SanCase { name: "sanitize_non_string", html: "", policy: "", expect: "!E0030 HTML_SANITIZE: expected string, got integer",
        why: "data 位置类型错 —— 消息说 data 要 string(**这是 data 的错,不是 policy 的**)" },
    SanCase { name: "policy_non_dict", html: "", policy: r#"[1, 2]"#, expect: "!E0030 HTML_SANITIZE: policy must be a DICT, got array",
        why: "D13-009:原先复用 data 的类型错模板,报成「expected string, got array」—— 对着一个**合法的 string 参数**说「要 string」,指错参数。契约表当场抓到" },
    SanCase { name: "policy_tags_not_array", html: "", policy: r#"["tags": "b"]"#, expect: "!E0030 HTML_SANITIZE: policy \"tags\" must be an ARRAY",
        why: "键值类型错 → E0030 带**键名**(规范 §13.2:「policy 形态错 E0030(带名)」)" },
    SanCase { name: "policy_url_schemes_not_array", html: "", policy: r#"["url_schemes": "http"]"#, expect: "!E0030 HTML_SANITIZE: policy \"url_schemes\" must be an ARRAY",
        why: "同上" },
];

/// 成员级诊断(与数据无关,单独一组:html 字段不参与)。
const SANITIZE_DIAGS: &[SanCase] = &[
    SanCase { name: "diag_arity_zero", html: "", policy: "", expect: "!E0022 HTML_SANITIZE: function expects 1 or 2 argument(s), got 0",
        why: "D13-010:原先用共享 arity 模板,对着**合法的单参调用**说「expects 2」——`policy` 可省,元数下界是 1" },
    SanCase { name: "diag_arity_three", html: "", policy: "", expect: "!E0022 HTML_SANITIZE: function expects 1 or 2 argument(s), got 3",
        why: "上界 2" },
];

/// 把任意输入安全地嵌进 wlwl 字符串字面量(§4.2 转义集:`\" \\ \n \r \t
/// \0 \b \f $`)。`$` 必须转 —— 否则 `${` 会被当成插值表达式。
fn lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '$' => out.push_str("\\$"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// 一次 `HTML_SANITIZE` 的源程序。`policy` 空串 = 缺省策略(整个第二实参省略)。
fn program(html: &str, policy: &str) -> String {
    let l = lit(html);
    let call = if policy.is_empty() {
        l
    } else {
        format!("{l}, {}", policy)
    };
    format!(r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE({call})"#)
}

/// 幂等探针用的源程序:`SANITIZE(SANITIZE(x, p), p)`,一次 eval 拿到二次结果。
fn program_twice(html: &str, policy: &str) -> String {
    if policy.is_empty() {
        format!(
            r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE(HTML_SANITIZE({l}))"#,
            l = lit(html)
        )
    } else {
        format!(
            r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE(HTML_SANITIZE({l}, {p}), {p})"#,
            l = lit(html),
            p = policy
        )
    }
}

fn sanitize_of(html: &str, policy: &str) -> String {
    match run(&program(html, policy)) {
        Ok(v) => v.display(),
        Err(e) => e,
    }
}

fn sanitize_twice_of(html: &str, policy: &str) -> String {
    match run(&program_twice(html, policy)) {
        Ok(v) => v.display(),
        Err(e) => e,
    }
}

// ── W-13 ①:OWASP 向量 + 策略契约逐条对拍 ──────────────────────────

#[test]
fn sanitize_evidence_matches_the_frozen_contract() {
    let mut bad = Vec::new();
    for case in SANITIZE_CASES {
        let got = if case.name == "sanitize_non_string" {
            run(r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE(1)"#)
                .map(|v| v.display())
                .unwrap_or_else(|e| e)
        } else {
            sanitize_of(case.html, case.policy)
        };
        if case.expect != got {
            bad.push(format!(
                "{name} (裁定依据:{why})\n      frozen: {want}\n      actual: {got}",
                name = case.name,
                why = case.why,
                want = case.expect,
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} case(s) diverged from the frozen W-13 contract:\n  - {}",
        bad.len(),
        SANITIZE_CASES.len(),
        bad.join("\n  - ")
    );
    // 反向守卫:表不得空转,也不得被「顺手删几条」悄悄缩掉。逐组下限
    // 按实际条数取(留一点余量,但砍掉一半就红)。
    for (prefix, min) in [
        ("img_", 18),
        ("script_", 3),
        ("style_", 1),
        ("a_", 9),
        ("mxss_", 12),
        ("malformed_", 5),
        ("policy_", 7),
    ] {
        let n = SANITIZE_CASES
            .iter()
            .filter(|c| c.name.starts_with(prefix))
            .count();
        assert!(
            n >= min,
            "W-13 corpus lost its `{prefix}` group ({n} < {min})"
        );
    }
    assert!(
        SANITIZE_CASES.len() >= 75,
        "W-13 corpus shrank to {} — OWASP 向量被误删?",
        SANITIZE_CASES.len()
    );
}

#[test]
fn sanitize_diagnostics_are_frozen() {
    for case in SANITIZE_DIAGS {
        let src = match case.name {
            "diag_arity_zero" => {
                r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE()"#
            }
            _ => {
                r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE("x", ["tags": ["b"]], 3)"#
            }
        };
        let got = match run(src) {
            Ok(v) => v.display(),
            Err(e) => e,
        };
        assert_eq!(got, case.expect, "{}:{}", case.name, case.why);
    }
}

// ── W-12 ①:固定差分集 = 契约语料逐条幂等 ──────────────────────────

#[test]
fn sanitize_is_idempotent_on_the_contract_corpus() {
    let mut bad = Vec::new();
    for case in SANITIZE_CASES {
        if case.name == "sanitize_non_string" {
            continue; // 诊断用例,不是净化语料
        }
        let once = sanitize_of(case.html, case.policy);
        let twice = sanitize_twice_of(case.html, case.policy);
        if once != twice {
            bad.push(format!(
                "{name}\n      once:  {once}\n      twice: {twice}",
                name = case.name
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} corpus case(s) broke the re-parse idempotency invariant \
         (§13.2 规范性安全不变量 —— 打破即缺陷,不是文档说明事项):\n  - {}",
        bad.len(),
        SANITIZE_CASES.len(),
        bad.join("\n  - ")
    );
}

// ── W-12 ②:变异集 ──────────────────────────────────────────────
//
// 变异器入库(计划 W-12「变异脚本入库」)。**只做不改语义的变形** ——
// 同一语义的不同字节序列,正是解析差分(mXSS)的入口。覆盖面:
//
//   1. case       整串 ASCII 大写           —— 标签名 / 属性名 / scheme 大小写不敏感
//   2. nl_split   `javascript:` 里插裸换行   —— 浏览器在 URL 中剥换行,须同样不判为 js:
//   3. tab_ent    `jav&#x09;ascript:`       —— 实体在**解析期**解码,不是输出期
//   4. cr_ent     `jav&#x0D;ascript:`
//   5. dec_ent    逐字符十进制实体
//   6. hex_ent    逐字符十六进制实体
//   7. ws_prefix  属性值前加空格 + 制表      —— scheme 判定前须剥控制字符
//   8. tag_split  `="` 之间插裸换行         —— 容错解析须跨行续读
//   9. quote_swap 双引号换单引号             —— 两种引号等价
//
// 每条变形后重跑幂等断言。**期望不是「变形后无害」而是「幂等」** ——
// 变形可能合法地改变首轮输出(例如大写后属性值不同),但二次必须不动。

fn mutation_variants(html: &str) -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = Vec::new();
    let upper = html.to_ascii_uppercase();
    if upper != html {
        v.push(("case", upper));
    }
    if html.contains("javascript:") {
        v.push(("nl_split", html.replacen("javascript:", "java\nscript:", 1)));
        v.push((
            "tab_ent",
            html.replacen("javascript:", "jav&#x09;ascript:", 1),
        ));
        v.push((
            "cr_ent",
            html.replacen("javascript:", "jav&#x0D;ascript:", 1),
        ));
        v.push((
            "dec_ent",
            html.replacen("javascript:", &dec_entities("javascript"), 1),
        ));
        v.push((
            "hex_ent",
            html.replacen("javascript:", &hex_entities("javascript"), 1),
        ));
    }
    if html.contains("=\"") {
        v.push(("ws_prefix", html.replacen("=\"", "=\"  \t", 1)));
        v.push(("tag_split", html.replacen("=\"", "=\n\"", 1)));
        v.push(("quote_swap", html.replace('"', "'")));
    }
    v
}

fn dec_entities(s: &str) -> String {
    s.chars().map(|c| format!("&#{};", c as u32)).collect()
}

fn hex_entities(s: &str) -> String {
    s.chars().map(|c| format!("&#x{:X};", c as u32)).collect()
}

#[test]
fn sanitize_is_idempotent_on_mutations() {
    let mut variants = 0usize;
    let mut by_kind: std::collections::BTreeMap<&'static str, usize> = Default::default();
    let mut bad = Vec::new();
    for case in SANITIZE_CASES {
        if case.name == "sanitize_non_string" {
            continue;
        }
        for (kind, mutated) in mutation_variants(case.html) {
            variants += 1;
            *by_kind.entry(kind).or_default() += 1;
            let once = sanitize_of(&mutated, case.policy);
            let twice = sanitize_twice_of(&mutated, case.policy);
            if once != twice {
                bad.push(format!(
                    "{name} / {kind}\n      mutated: {mutated}\n      once:    {once}\n      twice:   {twice}",
                    name = case.name
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} mutated variant(s) broke idempotency:\n  - {}",
        bad.len(),
        bad.join("\n  - ")
    );
    // 变异集不得退化:每类变形都必须在场(否则「全绿」是因为没跑变异)。
    for kind in [
        "case",
        "nl_split",
        "tab_ent",
        "cr_ent",
        "dec_ent",
        "hex_ent",
        "ws_prefix",
        "tag_split",
        "quote_swap",
    ] {
        assert!(
            by_kind.get(kind).copied().unwrap_or(0) > 0,
            "mutation `{kind}` produced no variant — the suite would pass vacuously"
        );
    }
    assert!(
        variants >= 150,
        "mutation set shrank to {variants} variants (by kind: {by_kind:?})"
    );
    println!("W-12 mutation coverage: {variants} variants, by kind: {by_kind:?}");
}

// ── W-12 ③:mXSS 教科书样本(独立成组,便于审计「覆盖了哪一族」)────

#[test]
fn sanitize_is_idempotent_on_mxss_textbook_samples() {
    let m = SANITIZE_CASES
        .iter()
        .filter(|c| c.name.starts_with("mxss_"))
        .collect::<Vec<_>>();
    assert!(
        m.len() >= 12,
        "mXSS group shrank to {} — Sonar cheatsheet 的白名单子集部分被误删?",
        m.len()
    );
    let mut bad = Vec::new();
    for case in m {
        let once = sanitize_of(case.html, case.policy);
        let twice = sanitize_twice_of(case.html, case.policy);
        if once != twice {
            bad.push(format!(
                "{}\n      once: {once}\n      twice: {twice}",
                case.name
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "mXSS sample(s) broke idempotency:\n  - {}",
        bad.join("\n  - ")
    );
}

// ── W-11 病态输入契约(安全面,与功能同权)────────────────────────
//
// 「深嵌套不栈溢出」是**断言**不是基准:基准只出数字,不会让 CI 变红。
// 四段流水线(tokenize / build / filter / serialize)必须全迭代 —— 任一段
// 偷偷改成递归,这里就是栈溢出而不是绿。

const PATHOLOGICAL_DEPTH: usize = 100_000;

#[test]
fn sanitize_survives_pathological_nesting() {
    let deep = format!(
        "{}x{}",
        "<b>".repeat(PATHOLOGICAL_DEPTH),
        "</b>".repeat(PATHOLOGICAL_DEPTH)
    );
    let out = sanitize_of(&deep, "");
    // `<b>`×N + `x` + `</b>`×N = 3N + 1 + 4N
    assert_eq!(
        out.len(),
        7 * PATHOLOGICAL_DEPTH + 1,
        "{} 层嵌套的输出长度不对 —— 树构建或序列化丢节点了",
        PATHOLOGICAL_DEPTH
    );
    assert!(out.starts_with(&"<b>".repeat(PATHOLOGICAL_DEPTH)));
    assert!(out.ends_with(&"</b>".repeat(PATHOLOGICAL_DEPTH)));
    assert_eq!(out.matches('x').count(), 1, "最内层文本 `x` 丢了或重复了");
    // 未闭合的病态嵌套同样不得溢出(开标签栈一路涨,无结束标签)。
    let unclosed = "<b>".repeat(PATHOLOGICAL_DEPTH);
    let out = sanitize_of(&unclosed, "");
    // 注意:比**长度 + 首尾**,不比内容 —— `<b>×N</b>×N` 与 `<b></b>×N`
    // 等长但不同形,拿 assert_eq! 比字符串会在这里假红。
    assert_eq!(
        out.len(),
        7 * PATHOLOGICAL_DEPTH,
        "未闭合 {} 层:节点数不对",
        PATHOLOGICAL_DEPTH
    );
    assert!(out.starts_with(&"<b>".repeat(PATHOLOGICAL_DEPTH)));
    assert!(out.ends_with(&"</b>".repeat(PATHOLOGICAL_DEPTH)));
}

#[test]
fn sanitize_survives_pathological_text_and_attributes() {
    // 超长纯文本:单趟扫描,不得超线性。
    let text = "a".repeat(1_000_000);
    assert_eq!(sanitize_of(&text, ""), text);
    // 超多属性:属性数不设人为上限,但内存 O(n)。
    let many = format!(
        "<a {}>x</a>",
        (0..20_000)
            .map(|i| format!("k{i}=v"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert_eq!(sanitize_of(&many, ""), "<a>x</a>");
    // 实体风暴:全表二分不得退化成线性扫描。
    let ents = "&copy;".repeat(100_000);
    assert_eq!(sanitize_of(&ents, ""), "©".repeat(100_000));
}
