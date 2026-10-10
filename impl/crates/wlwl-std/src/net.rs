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
        ("HTTP_GET", http_get as StdFn),
        ("HTTP_POST", http_post as StdFn),
        ("HTTP_REQUEST", http_request as StdFn),
    ],
};

// ── HTTP 的三条硬上限(ADR-0028 D3)──────────────────────────────
//
// ⚠ **越限必须是 `ERR` 而不是 OOM / 无界分配** —— 一个 `Content-Length:
// 10 GB` 的响应不该把进程打死,那是**服务端能触发的**。
/// 响应头块上限(字节)。超过即 `ERR`,不继续读到内存里。
const MAX_HEADER_BYTES: usize = 64 * 1024;
/// 响应体上限(字节)。超过即 `ERR`。
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
/// `ADR-0028` D2 的默认超时(毫秒)。调用方可覆盖,但**忘写不会永久挂住**。
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// HTTP 失败 → `ERR([kind:"HttpError", op, reason])`**值**,不是原生诊断。
///
/// 理由与 `std.encode` 的 `DecodeError` 同款:网络不通是**运行期可预期的
/// 业务分支**(会重试、会降级、会记日志),不是程序员把参数写错了。把它做成
/// 原生诊断会让整个运行**终止**,调用方连「重试」都做不到。
fn http_error(op: &str, reason: impl Into<String>) -> Value {
    Value::Err(Box::new(Value::Dict(vec![
        (
            Value::String("kind".into()),
            Value::String("HttpError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("reason".into()), Value::String(reason.into())),
    ])))
}

/// RFC 7230 `tchar` —— 头字段名与方法名只能用这些字符。
fn is_tchar(c: u8) -> bool {
    is_unreserved(c)
        || is_sub_delim(c)
        || matches!(
            c,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

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

// ── HTTP/1.1 客户端(W-03,ADR-0028 D1–D4)────────────────────────

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// 把请求实参收成 `(headers, timeout_ms)`。`args` 是**已经去掉必需部分**的尾部。
fn opt_headers_timeout(
    host: &mut dyn StdHost,
    name: &str,
    args: &[Value],
) -> wlwl_error::WlwlResult<(Vec<(String, String)>, u64)> {
    let mut headers = Vec::new();
    let mut timeout = DEFAULT_TIMEOUT_MS;
    let mut i = 0;
    while i < args.len() {
        match &args[i] {
            Value::Dict(entries) => {
                for (k, v) in entries {
                    let (Value::String(k), Value::String(v)) = (k, v) else {
                        return Err(type_err(host, name, k));
                    };
                    // 头字段名是 token;值里不许有裸 CR/LF —— 后者就是头注入。
                    let kt = k.as_bytes();
                    if kt.is_empty() || !kt.iter().copied().all(is_tchar) {
                        return Err(host.diag(
                            ErrorCode::E0030,
                            format!("{name}: header name {k:?} is not a valid token"),
                        ));
                    }
                    if v.contains('\r') || v.contains('\n') {
                        return Err(host.diag(
                            ErrorCode::E0030,
                            format!("{name}: header {k:?} value contains CR or LF"),
                        ));
                    }
                    headers.push((k.clone(), v.clone()));
                }
                i += 1;
            }
            Value::Integer(ms) => {
                if *ms < 0 {
                    return Err(host.diag(
                        ErrorCode::E0030,
                        format!("{name}: timeout must be >= 0 ms, got {ms}"),
                    ));
                }
                timeout = *ms as u64;
                i += 1;
            }
            other => return Err(type_err(host, name, other)),
        }
    }
    Ok((headers, timeout))
}

fn method_token(host: &mut dyn StdHost, name: &str, m: &str) -> wlwl_error::WlwlResult<()> {
    if m.is_empty() || !m.as_bytes().iter().copied().all(is_tchar) {
        return Err(host.diag(
            ErrorCode::E0030,
            format!("{name}: method {m:?} is not a valid HTTP token"),
        ));
    }
    Ok(())
}

/// HTTP 的全部实现。返回 `Value` —— 成功是 `DICT`,失败是 `ERR(...)` 值。
fn request(
    name: &str,
    method: &str,
    url: &str,
    body: Option<&str>,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
) -> Value {
    // ── 1. URL ──
    let p = match parse(name, url) {
        Ok(p) => p,
        Err(why) => return http_error(name, format!("invalid URL: {why}")),
    };
    match p.scheme.as_deref() {
        Some("http") => {}
        Some(other) => {
            return http_error(
                name,
                format!(
                    "scheme {other:?} is not supported — plaintext http: only (ADR-0028 \u{00a7}0)"
                ),
            )
        }
        None => return http_error(name, "URL has no scheme; `http:` is required"),
    }
    let Some(host) = p.host.clone() else {
        return http_error(name, "URL has no host");
    };
    let port = p.port.map_or(80u16, |n| n);
    // `URL_PARSE` 保留 IPv6 的方括号,这里要的是裸地址。
    let dial_host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    // path 为空时请求行必须是 `/`。
    let target = if p.path.is_empty() {
        match &p.query {
            None => "/".to_string(),
            Some(q) => format!("/?{q}"),
        }
    } else {
        match &p.query {
            None => p.path.clone(),
            Some(q) => format!("{}?{q}", p.path),
        }
    };

    // ── 2. 连接(带超时)──
    let dur = Duration::from_millis(timeout_ms);
    let addrs: Vec<_> = match (dial_host.as_str(), port).to_socket_addrs() {
        Ok(it) => it.collect(),
        Err(e) => return http_error(name, format!("resolve {dial_host}:{port}: {e}")),
    };
    if addrs.is_empty() {
        return http_error(name, format!("{dial_host}:{port} resolved to no address"));
    }
    let mut sock = None;
    let mut last = String::new();
    for a in &addrs {
        match TcpStream::connect_timeout(a, dur) {
            Ok(s) => {
                sock = Some(s);
                break;
            }
            Err(e) => last = e.to_string(),
        }
    }
    let Some(mut sock) = sock else {
        return http_error(name, format!("connect {dial_host}:{port}: {last}"));
    };
    let _ = sock.set_read_timeout(Some(dur));
    let _ = sock.set_write_timeout(Some(dur));

    // ── 3. 写请求 ──
    let host_hdr = if port == 80 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    let mut req = format!("{method} {target} HTTP/1.1\r\nHost: {host_hdr}\r\n");
    for (k, v) in &headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    // 明确关闭连接:这样「无 Content-Length」的响应可以用 EOF 定界,
    // 而不必去解 chunked(ADR-0028 D3:chunked 一律 `ERR`,不解码)。
    req.push_str("Connection: close\r\n\r\n");
    if let Some(b) = body {
        req.push_str(b);
    }
    if let Err(e) = sock.write_all(req.as_bytes()) {
        return http_error(name, format!("write: {e}"));
    }
    if let Err(e) = sock.flush() {
        return http_error(name, format!("flush: {e}"));
    }

    // ── 4. 读响应头(先读到 \r\n\r\n,带上限)──
    let mut raw: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = find_head_end(&raw) {
            break i;
        }
        if raw.len() > MAX_HEADER_BYTES {
            return http_error(
                name,
                format!("response header block exceeds {MAX_HEADER_BYTES} bytes"),
            );
        }
        match sock.read(&mut buf) {
            Ok(0) => {
                return http_error(
                    name,
                    format!(
                        "connection closed with {} byte(s) of header, no complete head",
                        raw.len()
                    ),
                )
            }
            Ok(n) => raw.extend_from_slice(&buf[..n]),
            Err(e) => return http_error(name, format!("read head: {e}")),
        }
    };

    // ── 5. 解析状态行 + 头 ──
    let head = match std::str::from_utf8(&raw[..head_end]) {
        Ok(h) => h,
        Err(e) => return http_error(name, format!("response header is not valid UTF-8: {e}")),
    };
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let mut sp = status_line.splitn(3, ' ');
    let version = sp.next().unwrap_or("");
    let status = sp.next().unwrap_or("");
    let _reason = sp.next().unwrap_or("");
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return http_error(name, format!("unexpected status line {status_line:?}"));
    }
    let Ok(status) = status.parse::<i64>() else {
        return http_error(name, format!("status code {status:?} is not an integer"));
    };

    let mut out_headers: Vec<(String, String)> = Vec::new();
    let mut content_length: Option<u64> = None;
    let mut connection_close = false;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        // obs-fold(行首空白)是**已知的响应拆分/走私向量** ⇒ 拒绝,不合并。
        if line.starts_with(' ') || line.starts_with('\t') {
            return http_error(name, format!("obs-fold in response header: {line:?}"));
        }
        let Some((k, v)) = line.split_once(':') else {
            return http_error(name, format!("malformed header line {line:?}"));
        };
        let v = v.trim();
        if v.contains('\r') || v.contains('\n') {
            return http_error(name, format!("header {k:?} value contains CR or LF"));
        }
        let lk = k.to_ascii_lowercase();
        match lk.as_str() {
            "content-length" => {
                // 多个 Content-Length —— 即便值相同也拒:「同一个头出现两次」
                // 本身就是走私信号,不同实现取第一个或最后一个会分叉。
                if content_length.is_some() {
                    return http_error(name, "duplicate Content-Length header");
                }
                let Ok(n) = v.parse::<u64>() else {
                    return http_error(name, format!("Content-Length {v:?} is not a number"));
                };
                content_length = Some(n);
            }
            "transfer-encoding" => {
                return http_error(
                    name,
                    format!("Transfer-Encoding {v:?} is not supported — chunked is a smuggling vector, refused rather than decoded (ADR-0028 D3)"),
                );
            }
            "connection" if v.to_ascii_lowercase().contains("close") => {
                connection_close = true;
            }
            _ => {}
        }
        out_headers.push((k.to_string(), v.to_string()));
    }

    // ── 6. 响应体:Content-Length 或 EOF 二者必居其一,否则拒绝 ──
    let mut body_bytes: Vec<u8> = raw[head_end + 4..].to_vec();
    let want = match content_length {
        Some(n) => {
            if n > MAX_BODY_BYTES as u64 {
                return http_error(
                    name,
                    format!("Content-Length {n} exceeds the {MAX_BODY_BYTES} byte cap"),
                );
            }
            Some(n as usize)
        }
        None if connection_close => None, // 读到 EOF
        None => {
            return http_error(
                name,
                "response has neither Content-Length nor Connection: close — refusing to guess the body length (ADR-0028 D3)",
            )
        }
    };
    loop {
        let done = match want {
            Some(w) => body_bytes.len() >= w,
            None => false,
        };
        if done {
            break;
        }
        if body_bytes.len() > MAX_BODY_BYTES {
            return http_error(
                name,
                format!("response body exceeds {MAX_BODY_BYTES} bytes"),
            );
        }
        match sock.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => body_bytes.extend_from_slice(&buf[..n]),
            Err(e) => return http_error(name, format!("read body: {e}")),
        }
    }
    if let Some(w) = want {
        if body_bytes.len() != w {
            return http_error(
                name,
                format!("body truncated: got {} of {w} byte(s)", body_bytes.len()),
            );
        }
        body_bytes.truncate(w);
    }

    let body = match String::from_utf8(body_bytes) {
        Ok(b) => b,
        Err(e) => return http_error(name, format!("response body is not valid UTF-8: {e}")),
    };

    Value::Dict(vec![
        (Value::String("status".into()), Value::Integer(status)),
        (
            Value::String("headers".into()),
            Value::Dict(
                out_headers
                    .into_iter()
                    .map(|(k, v)| (Value::String(k), Value::String(v)))
                    .collect(),
            ),
        ),
        (Value::String("body".into()), Value::String(body)),
    ])
}

fn find_head_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn http_get(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.is_empty() || args.len() > 3 {
        return Err(arity(host, "HTTP_GET", args.len(), 1));
    }
    let Value::String(url) = &args[0] else {
        return Err(type_err(host, "HTTP_GET", &args[0]));
    };
    let (headers, timeout) = opt_headers_timeout(host, "HTTP_GET", &args[1..])?;
    Ok(wlwl_value::Outcome::normal(request(
        "HTTP_GET", "GET", url, None, headers, timeout,
    )))
}

pub fn http_post(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() < 2 || args.len() > 4 {
        return Err(arity(host, "HTTP_POST", args.len(), 2));
    }
    let Value::String(url) = &args[0] else {
        return Err(type_err(host, "HTTP_POST", &args[0]));
    };
    let Value::String(body) = &args[1] else {
        return Err(type_err(host, "HTTP_POST", &args[1]));
    };
    let (headers, timeout) = opt_headers_timeout(host, "HTTP_POST", &args[2..])?;
    Ok(wlwl_value::Outcome::normal(request(
        "HTTP_POST",
        "POST",
        url,
        Some(body),
        headers,
        timeout,
    )))
}

pub fn http_request(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<wlwl_value::Outcome> {
    if args.len() < 2 || args.len() > 5 {
        return Err(arity(host, "HTTP_REQUEST", args.len(), 2));
    }
    let Value::String(method) = &args[0] else {
        return Err(type_err(host, "HTTP_REQUEST", &args[0]));
    };
    let Value::String(url) = &args[1] else {
        return Err(type_err(host, "HTTP_REQUEST", &args[1]));
    };
    method_token(host, "HTTP_REQUEST", method)?;
    let mut body: Option<&str> = None;
    let mut i = 2;
    if let Some(Value::String(b)) = args.get(2) {
        body = Some(b);
        i = 3;
    }
    let (headers, timeout) = opt_headers_timeout(host, "HTTP_REQUEST", &args[i..])?;
    Ok(wlwl_value::Outcome::normal(request(
        "HTTP_REQUEST",
        method,
        url,
        body,
        headers,
        timeout,
    )))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// 仓内本地 HTTP 夹具:绑 `127.0.0.1:0`、回一句**原样**的字节。
    ///
    /// ⚠ **绝不访问外网** —— CI 不稳定,且违反本仓「可复现」卖点。
    /// 夹具给的是**原始响应字节**,所以 `chunked` / 无 `Content-Length` /
    /// 超大 `Content-Length` 这些形状都能精确造出来。
    fn fixture(raw: &'static str) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf); // 读到头即可,本夹具不校验请求
                let _ = s.write_all(raw.as_bytes());
                let _ = s.flush();
                // `Connection: close` 的形状靠直接关连接来表达。
            }
        });
        port
    }

    fn err_reason(v: &Value) -> Option<&str> {
        let Value::Err(p) = v else { return None };
        let Value::Dict(entries) = &**p else {
            return None;
        };
        entries
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == "reason"))
            .and_then(|(_, v)| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
    }

    fn dict_get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
        let Value::Dict(entries) = v else { return None };
        entries
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == key))
            .map(|(_, v)| v)
    }

    #[test]
    fn get_returns_status_headers_and_body() {
        let p = fixture("HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-T: v\r\n\r\nhello");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/x"),
            None,
            vec![],
            5000,
        );
        assert_eq!(dict_get(&v, "status"), Some(&Value::Integer(200)));
        assert_eq!(dict_get(&v, "body"), Some(&Value::String("hello".into())));
        assert_eq!(
            dict_get(dict_get(&v, "headers").unwrap(), "X-T"),
            Some(&Value::String("v".into()))
        );
    }

    #[test]
    fn post_sends_body_and_content_length() {
        // 夹具把请求原样回显,好让测试看到我们**发出去了**什么。
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let sink = seen.clone();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                *sink.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).into_owned();
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });
        let v = request(
            "HTTP_POST",
            "POST",
            &format!("http://127.0.0.1:{port}/p"),
            Some("hi"),
            vec![("X-A".into(), "b".into())],
            5000,
        );
        assert_eq!(dict_get(&v, "status"), Some(&Value::Integer(200)));
        let got = seen.lock().unwrap().clone();
        assert!(
            got.starts_with("POST /p HTTP/1.1\r\n"),
            "request line: {got:?}"
        );
        assert!(
            got.contains("Content-Length: 2\r\n"),
            "body length: {got:?}"
        );
        assert!(got.contains("X-A: b\r\n"), "caller header: {got:?}");
        assert!(
            got.ends_with("\r\n\r\nhi"),
            "body follows the blank line: {got:?}"
        );
    }

    #[test]
    fn chunked_is_refused_not_decoded() {
        let p =
            fixture("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/"),
            None,
            vec![],
            5000,
        );
        let r = err_reason(&v).expect("must be an ERR value");
        assert!(r.contains("chunked"), "reason should name the vector: {r}");
    }

    #[test]
    fn no_length_and_no_close_is_refused() {
        let p = fixture("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nhello");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/"),
            None,
            vec![],
            5000,
        );
        let r = err_reason(&v).expect("must be an ERR value");
        assert!(r.contains("neither Content-Length"), "reason: {r}");
    }

    #[test]
    fn redirect_is_returned_not_followed() {
        let p =
            fixture("HTTP/1.1 302 Found\r\nLocation: /elsewhere\r\nContent-Length: 4\r\n\r\ngoin");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/a"),
            None,
            vec![],
            5000,
        );
        assert_eq!(
            dict_get(&v, "status"),
            Some(&Value::Integer(302)),
            "3xx 必须原样返回(ADR-0028 D4),不能跟过去"
        );
        assert_eq!(
            dict_get(dict_get(&v, "headers").unwrap(), "Location"),
            Some(&Value::String("/elsewhere".into()))
        );
    }

    #[test]
    fn https_and_connection_refused_are_err_values() {
        let v = request(
            "HTTP_GET",
            "GET",
            "https://example.com/",
            None,
            vec![],
            1000,
        );
        assert!(err_reason(&v)
            .expect("https must be an ERR value")
            .contains("plaintext"));

        // 绑一个端口拿到号随即关闭 —— 连接必被拒绝。
        let dead = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{dead}/"),
            None,
            vec![],
            2000,
        );
        assert!(
            err_reason(&v).is_some(),
            "连接拒绝必须是 ERR 值而不是 panic"
        );
    }

    #[test]
    fn body_over_the_cap_is_err_not_oom() {
        // 声明一个超过 8 MiB 上限的 Content-Length,但一个字节都不发。
        let p = fixture("HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n");
        let v = request(
            "HTTP_GET",
            "GET",
            &format!("http://127.0.0.1:{p}/"),
            None,
            vec![],
            5000,
        );
        let r = err_reason(&v).expect("must be an ERR value");
        assert!(r.contains("cap"), "reason should name the cap: {r}");
    }

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
