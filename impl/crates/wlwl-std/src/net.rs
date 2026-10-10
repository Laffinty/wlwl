//! [v0.11.3 M6 / ADDENDUM-05] `wlwl:std.net` —— 网络(零第三方依赖的最小面)。
//!
//! **本文件目前只含 W-02 的两个纯函数成员。** `HTTP_GET` / `HTTP_POST` /
//! `HTTP_REQUEST` 属 W-03,形态已由 [`ADR-0028`] 定死(`docs/adr/0028-std-net-shape.md`,
//! **Accepted**),但 D1「阻塞原语」是**带条件冻结**的 —— 见该 ADR §6 与守卫测试
//! `impl/crates/wlwl-eval/tests/adr_conditional_guard.rs`。
//!
//! [`ADR-0028`]: ../../../docs/adr/0028-std-net-shape.md
//!
//! ## 为什么这两个成员先落
//!
//! 它们是**纯函数**,与阻塞 / 挂起 / 超时**毫无关系**。W-03 建在 D1 之上;
//! 若 `ADR-0017` Step 3 落地导致 D1 重裁,要改的只有那一层,不是这一批。
//!
//! ## 规范与权威
//!
//! 语法与解析**逐条照 RFC 3986**:分组件走 Appendix B 的 first-match-wins
//! 正则,`URL_JOIN` 走 §5.2.2 / §5.2.3 / §5.2.4 / §5.3 的**伪代码原文**。
//! 契约表把 §5.4 的 **40 条**参考用例**逐字冻结**。
//!
//! ## 「非法 URL」的边界(规范性,2026-10-09)
//!
//! RFC 3986 的解析是**宽容**的 —— 绝大多数字符串都能解析成某种引用。所以
//! 「非法 → `E0030`」必须有一条能写进契约表的边界。本实现的判据是:
//! **不满足 `URI-reference` 的 ABNF(Appendix A)即非法**。逐条为:
//!
//! | 情形 | 结果 |
//! |---|---|
//! | 任何 ASCII 控制字符或空格(0x00–0x20 / 0x7F) | `E0030` |
//! | `:` 出现在首个 `/?#` 之前,但其前缀不是合法 `scheme` | `E0030` |
//! | 裸 `%`(后面不是两个十六进制数字) | `E0030` |
//! | 组件里有超出该组件字符集的字符 | `E0030` |
//! | `[` / `]` 不成对,或出现在 authority 之外 | `E0030` |
//! | `port` 非 `*DIGIT`,或数值 > 65535 | `E0030` |
//! | **空 host**(如 `http://`) | **接受** —— 那是合法的 authority |
//! | **无 scheme**(如 `//g`、`a/b`) | **接受** —— 那是合法的 `relative-ref` |
//!
//! 最后两行是**刻意宽松**的:把它们判成非法会让 `URL_JOIN` 无法处理
//! §5.4 里 `//g`、`""` 这类正常输入。
//!
//! ### 明确**不**校验的东西
//!
//! **IPv6 地址的完整文法。** RFC 3986 §3.2.2 自己就说 IPv6 文法「很难指定」
//! 并把它委派给 RFC 3513。本实现只做**保守**检查:`[...]` 内非空且只含
//! `[0-9A-Fa-f:.]`,或以 `v`/`V` 开头(IPvFuture)。不解析成二进制地址。
//! 这是**已知限制**,不是遗漏。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.net",
    functions: &[
        ("URL_PARSE", url_parse as StdFn),
        ("URL_JOIN", url_join as StdFn),
    ],
};

// ── 诊断助手(与 text.rs 同款)────────────────────────────────────

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

fn bad_url(host: &mut dyn StdHost, name: &str, url: &str, why: &str) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!("{name}: not a valid URI reference: {why}: {url:?}"),
    )
}

fn str_args<'a>(
    host: &mut dyn StdHost,
    name: &str,
    args: &'a [Value],
    want: usize,
) -> wlwl_error::WlwlResult<Vec<&'a str>> {
    if args.len() != want {
        return Err(arity(host, name, args.len(), want));
    }
    let mut out = Vec::with_capacity(want);
    for v in args {
        let Value::String(s) = v else {
            return Err(type_err(host, name, v));
        };
        out.push(s.as_str());
    }
    Ok(out)
}

// ── RFC 3986 字符类(Appendix A)──────────────────────────────────

fn is_alpha(c: u8) -> bool {
    c.is_ascii_alphabetic()
}
fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}
fn is_hex(c: u8) -> bool {
    c.is_ascii_hexdigit()
}
/// `unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~"`
fn is_unreserved(c: u8) -> bool {
    is_alpha(c) || is_digit(c) || matches!(c, b'-' | b'.' | b'_' | b'~')
}
/// `sub-delims = "!" / "$" / "&" / "'" / "(" / ")" / "*" / "+" / "," / ";" / "="`
fn is_sub_delim(c: u8) -> bool {
    matches!(
        c,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}
/// `pct-encoded = "%" HEXDIG HEXDIG` —— 在**非 ASCII**字节上返回 `true`(留给
/// 上层报「非法字符」而不是误判成「百分号不完整」)。
fn is_pct_encoded(s: &[u8], i: usize) -> bool {
    matches!(s.get(i), Some(b'%') if s.get(i + 1).is_some_and(|c| is_hex(*c))
        && s.get(i + 2).is_some_and(|c| is_hex(*c)))
}

/// `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`
/// `query` / `fragment` 在 `pchar` 之外还允许 `/` 与 `?`。
fn scan_component(
    s: &[u8],
    start: usize,
    end: usize,
    extra: &[u8],
    what: &str,
) -> Result<(), String> {
    let mut i = start;
    while i < end {
        let c = s[i];
        if is_pct_encoded(s, i) {
            i += 3;
            continue;
        }
        if is_unreserved(c) || is_sub_delim(c) || c == b':' || c == b'@' || extra.contains(&c) {
            i += 1;
            continue;
        }
        if c.is_ascii_control() || c == b' ' {
            return Err(format!(
                "{what} contains a control character or space (byte {c:#04x})"
            ));
        }
        if c == b'%' {
            return Err(format!("{what} contains a bare `%` (needs two hex digits)"));
        }
        return Err(format!(
            "{what} contains `{}` which is not allowed there (byte {c:#04x})",
            c as char
        ));
    }
    Ok(())
}

// ── 解析结果 ────────────────────────────────────────────────────

/// 一个 URI 引用的五个组件 + authority 的三段拆分。
///
/// ⚠ **`Option::None` 与 `Some("")` 的区别是规范性的**(RFC 3986 §5.3 末段):
/// 分隔符**没出现**是 undefined;分隔符出现了但后面立刻是下一个分隔符或结尾
/// 是 empty。§5.4.2 的 `g?y/./x` 一类用例就依赖这个区别 —— 把 undefined 和
/// empty 混同,query 与 path 就分不开了(§5.2.4 末段的 Note 明写这个坑)。
#[derive(Debug, PartialEq, Eq)]
struct Parts {
    scheme: Option<String>,
    authority: Option<String>,
    userinfo: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}

/// 按 Appendix B 的正则做 first-match-wins 切分,再按 Appendix A 校验。
fn parse(name: &str, url: &str) -> Result<Parts, String> {
    let b = url.as_bytes();
    // 整个引用不含控制字符 / 空格 —— 先扫一遍,让后面每个分支的报错都更准。
    if let Some(c) = b.iter().find(|c| c.is_ascii_control() || **c == b' ') {
        return Err(format!(
            "contains a control character or space (byte {c:#04x})"
        ));
    }

    // ── scheme:首个 `:` 之前不得含 `/?#`,且必须匹配 `scheme` 的 ABNF ──
    let mut scheme: Option<String> = None;
    let mut rest_start = 0usize;
    if let Some(colon) = b.iter().position(|c| *c == b':') {
        if !b[..colon].iter().any(|c| matches!(c, b'/' | b'?' | b'#')) {
            let cand = &b[..colon];
            // `1a:b` 这类:`:` 在首个 `/?#` 之前,却又不是合法 scheme ⇒
            // 它**既不是** `URI`(`scheme` 首位必须是 ALPHA)**也不是**
            // `relative-ref`(`path-noscheme` 首段不得含 `:`)⇒ 整体非法。
            let ok = !cand.is_empty()
                && is_alpha(cand[0])
                && cand[1..]
                    .iter()
                    .all(|c| is_alpha(*c) || is_digit(*c) || matches!(c, b'+' | b'-' | b'.'));
            if !ok {
                return Err(format!(
                    "`{}` before `:` is not a valid scheme (must be ALPHA *(ALPHA / DIGIT / + / - / .))",
                    String::from_utf8_lossy(cand)
                ));
            }
            scheme = Some(String::from_utf8_lossy(cand).into_owned());
            rest_start = colon + 1;
        }
    }

    // ── authority:紧跟 `//` 时存在 ──
    let (authority_raw, mut i) = if b[rest_start..].starts_with(b"//") {
        let s = rest_start + 2;
        let e = s + b[s..]
            .iter()
            .position(|c| matches!(c, b'/' | b'?' | b'#'))
            .unwrap_or(b.len() - s);
        (Some(&url[s..e]), e)
    } else {
        (None, rest_start)
    };

    // ── path:到首个 `?` 或 `#` 为止 ──
    let path_end = b[i..]
        .iter()
        .position(|c| matches!(c, b'?' | b'#'))
        .map(|p| i + p)
        .unwrap_or(b.len());
    let path = url[i..path_end].to_string();
    // path 里不允许 `?` / `#`(已截断)。`pchar` **不含** `/`,但 path 是
    // `segment = *pchar` 由 `/` 连接起来的(`path-abempty = *( "/" segment )`),
    // 所以 `/` 必须**显式**放进 extra —— 漏了它连 `/pub/...` 都过不了。
    scan_component(b, i, path_end, b"/", "path")?;
    i = path_end;

    // ── query / fragment ──
    let query = if b.get(i) == Some(&b'?') {
        let s = i + 1;
        let e = s + b[s..]
            .iter()
            .position(|c| *c == b'#')
            .unwrap_or(b.len() - s);
        scan_component(b, s, e, b"/?", "query")?;
        i = e;
        Some(url[s..e].to_string())
    } else {
        None
    };

    let fragment = if b.get(i) == Some(&b'#') {
        let s = i + 1;
        scan_component(b, s, b.len(), b"/?", "fragment")?;
        Some(url[s..].to_string())
    } else {
        None
    };

    // ── authority 拆分:`[userinfo "@"] host [":" port]` ──
    let (userinfo, host, port) = match authority_raw {
        None => (None, None, None),
        Some(a) => split_authority(name, a)?,
    };

    Ok(Parts {
        scheme,
        authority: authority_raw.map(str::to_string),
        userinfo,
        host,
        port,
        path,
        query,
        fragment,
    })
}

/// authority 拆成三段:`[userinfo "@"] host [":" port]`。
type AuthParts = (Option<String>, Option<String>, Option<u16>);

fn split_authority(name: &str, a: &str) -> Result<AuthParts, String> {
    let _ = name;
    // `userinfo` 不含 `@`,所以至多一个,且它可以在 `:` 之前或之后。
    let (userinfo, hostport) = match a.find('@') {
        Some(at) => (Some(a[..at].to_string()), &a[at + 1..]),
        None => (None, a),
    };
    if let Some(u) = &userinfo {
        scan_component(u.as_bytes(), 0, u.len(), b":", "userinfo")?;
    }

    // host 之后可选 `:port`。
    let (host_s, port_s) = if hostport.starts_with('[') {
        // IP-literal:`[...]` 内不得含 `:port` 分隔的歧义 ⇒ 从**闭合**括号切。
        let close = hostport
            .find(']')
            .ok_or_else(|| "IP-literal is missing its closing `]`".to_string())?;
        let host = &hostport[..=close];
        let tail = &hostport[close + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p),
            None if tail.is_empty() => None,
            None => return Err(format!("unexpected `{tail}` after the IP-literal")),
        };
        (host, port)
    } else {
        match hostport.rfind(':') {
            Some(c) => (&hostport[..c], Some(&hostport[c + 1..])),
            None => (hostport, None),
        }
    };

    // host 校验:IP-literal 或 `reg-name`。
    if host_s.starts_with('[') {
        let inner = &host_s[1..host_s.len() - 1];
        if inner.is_empty() {
            return Err("IP-literal is empty".to_string());
        }
        // 保守检查,不解析成二进制地址(RFC 3986 §3.2.2 自己把 IPv6 文法
        // 委派给了 RFC 3513,并说它「很难指定」)。
        // - IPv6address: 只含十六进制数字 / `:` / `.`
        // - IPvFuture  : `v` 1*HEXDIG `.` 1*( unreserved / sub-delims / ":" )
        let ipv6_ok = inner.bytes().all(|c| is_hex(c) || c == b':' || c == b'.');
        let future_ok = matches!(inner.as_bytes().first(), Some(b'v' | b'V'))
            && inner[1..]
                .bytes()
                .all(|c| is_unreserved(c) || is_sub_delim(c) || c == b':');
        if !ipv6_ok && !future_ok {
            return Err(format!(
                "IP-literal `{host_s}` is not a plausible IPv6 / IPvFuture"
            ));
        }
    } else {
        // `reg-name = *( unreserved / pct-encoded / sub-delims )` —— 刻意不含
        // `:` `@` `[` `]`,与 sub-delims 的边界一致(§7.6 的 userinfo 混淆面)。
        let hb = host_s.as_bytes();
        let mut k = 0;
        while k < hb.len() {
            if is_pct_encoded(hb, k) {
                k += 3;
                continue;
            }
            let c = hb[k];
            if !is_unreserved(c) && !is_sub_delim(c) {
                return Err(format!(
                    "host contains `{c}`, which is not allowed in `reg-name`"
                ));
            }
            k += 1;
        }
    }

    let port = match port_s {
        None => None,
        Some(p) => {
            if !p.bytes().all(is_digit) {
                return Err(format!("port `{p}` is not `*DIGIT`"));
            }
            // 空端口(`http://h:/`)合法且意为「默认端口」⇒ `Some("")` 记 undefined。
            if p.is_empty() {
                None
            } else {
                let n: u32 = p.parse().expect("all digits");
                if n > 65535 {
                    return Err(format!("port {n} exceeds 65535"));
                }
                Some(n as u16)
            }
        }
    };

    Ok((userinfo, Some(host_s.to_string()), port))
}

// ── RFC 3986 §5.2.3 merge / §5.2.4 remove_dot_segments ──────────

fn merge(base_path: &str, base_has_authority: bool, ref_path: &str) -> String {
    // §5.2.3 第一条:「base **有** authority 且 path 为空」⇒ `"/" + ref_path`。
    // ⚠ 「有 authority」这个前提**不能省**:base 无 authority 且 path 为空时
    // 走第二条(结果就是 `ref_path` 本身,没有那个 `/`)。两条在
    // `URL_JOIN("http://a", "b")` 上分别是 `/b` 与 `b`,差一个斜杠。
    if base_has_authority && base_path.is_empty() {
        return format!("/{ref_path}");
    }
    let cut = base_path.rfind('/').map(|i| i + 1).unwrap_or(0);
    format!("{}{}", &base_path[..cut], ref_path)
}

fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_string();
    let mut out = String::with_capacity(path.len());
    while !input.is_empty() {
        if input.starts_with("../") {
            input.drain(..3); // A: remove that prefix
        } else if input.starts_with("./") {
            input.drain(..2); // A: remove that prefix
                              // B / C: 原文是 **replace** that prefix with "/" —— 是**替换**不是删除。
                              // 写成 drain 会把 "/./x" 变成 "x"(丢了那个斜杠),RFC §5.2.4 那张缓冲表
                              // 第 2B 行立刻对不上:`/a/b/c` + `/./../../g` 应得 `/a/b/c` + `/../../g`。
        } else if input.starts_with("/./") {
            input.replace_range(..3, "/"); // B: replace with "/"
        } else if input == "/." {
            input = "/".to_string();
        } else if input.starts_with("/../") {
            input.replace_range(..4, "/"); // C: replace with "/"
            pop_segment(&mut out);
        } else if input == "/.." {
            input = "/".to_string();
            pop_segment(&mut out);
        } else if input == "." || input == ".." {
            input.clear(); // D: remove
        } else {
            // E: 搬第一个完整路径段(含它前面的 `/`)。
            let end = match input.find('/') {
                None => input.len(),
                Some(0) => input[1..].find('/').map(|i| i + 1).unwrap_or(input.len()),
                Some(i) => i,
            };
            out.push_str(&input[..end]);
            input.drain(..end);
        }
    }
    out
}

/// 从输出缓冲区去掉**最后一个完整段**及其前面的 `/`(§5.2.4 步骤 2C)。
fn pop_segment(out: &mut String) {
    match out.rfind('/') {
        Some(i) => out.truncate(i),
        // 没有 `/` ⇒ 去掉整段。
        None => out.clear(),
    }
}

// ── RFC 3986 §5.2.2 transform + §5.3 recomposition ──────────────

/// 相对引用 → 绝对 URI。**strict 模式**:与 base 同 scheme 的引用**也**按
/// 有 scheme 处理(§5.2.2 里 `if ((not strict) and (R.scheme == Base.scheme))`)。
/// §5.4.2 的 `"http:g"` 一行给了两个结果,strict 侧是 RFC 自己列在**前面**的
/// 那个,故取它;WHATWG URL 取的是另一个(向后兼容),两者**不等价**。
fn resolve(base: &Parts, r: &Parts) -> Parts {
    let (t_scheme, t_authority, t_path, t_query);
    if let Some(s) = &r.scheme {
        t_scheme = Some(s.clone());
        t_authority = r.authority.clone();
        t_path = remove_dot_segments(&r.path);
        t_query = r.query.clone();
    } else if let Some(a) = &r.authority {
        t_authority = Some(a.clone());
        t_path = remove_dot_segments(&r.path);
        t_query = r.query.clone();
        t_scheme = base.scheme.clone();
    } else if r.path.is_empty() {
        t_authority = base.authority.clone();
        t_path = base.path.clone();
        t_query = r.query.clone().or_else(|| base.query.clone());
        t_scheme = base.scheme.clone();
    } else {
        t_authority = base.authority.clone();
        t_path = if r.path.starts_with('/') {
            remove_dot_segments(&r.path)
        } else {
            remove_dot_segments(&merge(&base.path, base.authority.is_some(), &r.path))
        };
        t_query = r.query.clone();
        t_scheme = base.scheme.clone();
    }
    Parts {
        scheme: t_scheme,
        authority: t_authority,
        userinfo: None,
        host: None,
        port: None,
        path: t_path,
        query: t_query,
        fragment: r.fragment.clone(),
    }
}

/// §5.3 Component Recomposition。
fn recompose(p: &Parts) -> String {
    let mut s = String::new();
    if let Some(sc) = &p.scheme {
        s.push_str(sc);
        s.push(':');
    }
    if let Some(a) = &p.authority {
        s.push_str("//");
        s.push_str(a);
    }
    s.push_str(&p.path);
    if let Some(q) = &p.query {
        s.push('?');
        s.push_str(q);
    }
    if let Some(f) = &p.fragment {
        s.push('#');
        s.push_str(f);
    }
    s
}

// ── 成员 ────────────────────────────────────────────────────────

/// `URL_PARSE(url) -> DICT`
///
/// 七个键:`scheme` / `userinfo` / `host` / `port` / `path` / `query` /
/// `fragment`。组件**未出现**是 `NULL`,**出现但为空**是 `""` —— §5.3 末段
/// 明写这个区别是规范性的。
///
/// ⚠ **`userinfo` 是计划外加的一个键。** 计划 §2 列的是六个键,但 RFC 3986
/// §7.6 用 `ftp://cnn.example.com&story=breaking_news@10.0.0.1/` 证明:
/// 悄悄丢掉 userinfo 会让「人类以为 host 是 cnn.example.com」而实际是
/// `10.0.0.1`。解析器把这一段吃掉,等于替调用方**藏起一个已知攻击面**。
///
/// `host` 保留 IPv6 literal 的方括号(`"[::1]"`),好让调用方能无损拼回。
pub fn url_parse(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let url = str_args(host, "URL_PARSE", &args, 1)?[0];
    let p = parse("URL_PARSE", url).map_err(|why| bad_url(host, "URL_PARSE", url, &why))?;
    let opt = |s: &Option<String>| match s {
        None => Value::Null,
        Some(v) => Value::String(v.clone()),
    };
    let dict = vec![
        (Value::String("scheme".into()), opt(&p.scheme)),
        (Value::String("userinfo".into()), opt(&p.userinfo)),
        (Value::String("host".into()), opt(&p.host)),
        (
            Value::String("port".into()),
            match p.port {
                None => Value::Null,
                Some(n) => Value::Integer(i64::from(n)),
            },
        ),
        (Value::String("path".into()), Value::String(p.path.clone())),
        (Value::String("query".into()), opt(&p.query)),
        (Value::String("fragment".into()), opt(&p.fragment)),
    ];
    Ok(wlwl_value::Outcome::normal(Value::Dict(dict)))
}

/// `URL_JOIN(base, rel) -> STRING`
///
/// 把 `rel` 按 RFC 3986 §5 解析成以 `base` 为基的绝对引用,**strict 模式**
/// (见本文件 `resolve` 的说明)。不做百分号解码、不做 host 小写化、不做 scheme
/// 规范化 —— 那些是 §6.2 的**语法 / scheme 规范化**,是**另一个语义**。
pub fn url_join(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    let v = str_args(host, "URL_JOIN", &args, 2)?;
    let b = parse("URL_JOIN", v[0]).map_err(|why| bad_url(host, "URL_JOIN", v[0], &why))?;
    let r = parse("URL_JOIN", v[1]).map_err(|why| bad_url(host, "URL_JOIN", v[1], &why))?;
    let out = recompose(&resolve(&b, &r));
    Ok(wlwl_value::Outcome::normal(Value::String(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Parts {
        parse("t", s).expect("parses")
    }

    /// 机制钉子。**期望值取自 RFC 3986 原文**,不是从本实现输出抄的 ——
    /// 全量证据在 `net_contract.rs`(§5.4 全部 40 条逐字冻结)。
    #[test]
    fn appendix_b_regex_splits_five_components() {
        // RFC 3986 Appendix B 正文给出的那一个例子。
        let u = p("http://www.ics.uci.edu/pub/ietf/uri/#Related");
        assert_eq!(u.scheme.as_deref(), Some("http"));
        assert_eq!(u.host.as_deref(), Some("www.ics.uci.edu"));
        assert_eq!(u.path, "/pub/ietf/uri/");
        assert_eq!(u.query, None, "RFC 明写 query 是 <undefined>");
        assert_eq!(u.fragment.as_deref(), Some("Related"));
    }

    /// §5.3 末段的 undefined / empty 之分 —— 混同就分不开 query 与 path。
    #[test]
    fn undefined_and_empty_components_differ() {
        assert_eq!(p("http://a/").query, None, "no `?` ⇒ undefined");
        assert_eq!(
            p("http://a/?").query.as_deref(),
            Some(""),
            "`?` 后面立刻结束 ⇒ empty"
        );
        assert_eq!(p("http://a/#").fragment.as_deref(), Some(""));
    }

    #[test]
    fn remove_dot_segments_matches_rfc() {
        // §5.2.4 正文那两张缓冲表的第一行。
        assert_eq!(remove_dot_segments("/a/b/c/./../../g"), "/a/g");
        assert_eq!(remove_dot_segments("mid/content=5/../6"), "mid/6");
        // `..` 不能改 authority ⇒ 回退到根后不再往上删。
        assert_eq!(remove_dot_segments("/../g"), "/g");
    }

    #[test]
    fn strict_mode_keeps_same_scheme_prefix() {
        let base = p("http://a/b/c/d;p?q");
        assert_eq!(recompose(&resolve(&base, &p("http:g"))), "http:g");
    }
}
