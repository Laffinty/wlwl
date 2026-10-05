//! [v0.11.3 M3 W-11] 策略解析与白名单过滤。
//!
//! 过滤两遍走通(都是显式栈,无递归 —— 病态深嵌套契约):
//! 1. **自顶向下定去留**:每个节点标 `Keep` / `Unwrap`(拆壳留孩子)/
//!    `DropSubtree`(连内容删除);unwrap 节点的孩子的有效父节点改为
//!    unwrap 节点的有效父节点;
//! 2. **按文档序重建**:Keep 节点挂到其有效父节点 —— unwrap 的孩子按
//!    原文档序自然归位。
//!
//! 属性过滤发生在 Keep 节点上:逐标签 + `"*"` 白名单、同名属性首个生效、
//! URL 类属性过 scheme 白名单(判定前剥空白 / 控制字符再取 `:` 前缀)。

use super::tree::{NodeKind, Tree, VOID};
use wlwl_error::ErrorCode;
use wlwl_value::{StdHost, Value};

/// 整棵删除(内容一并消失)的元素:raw-text / 外来内容 / 危险嵌入。
/// raw-text 的删除同时封掉「raw-text 内容再解析」的整类 mXSS 面。
///
/// **`plaintext` 不在本表**:规范 §13.2 只把 8 个 raw-text 元素列入整删集,
/// 并明写「`<plaintext>` 后的全文文本化不实现(按普通文本处理)」——
/// 即它是**普通非白名单元素**(拆壳留内容),不是整删。浏览器把其后文
/// 当文本永不再解析,本成员不复制该行为(已登记的解析差分),但也不因此
/// 把用户内容整段丢掉。D13-008。
pub(crate) const DROP_WITH_CONTENT: &[&str] = &[
    "script", "style", "iframe", "object", "embed", "noscript", "noframes", "noembed", "template",
    "textarea", "title", "svg", "math", "xmp", "head",
];

/// URL 类属性:值可能是 URL,必须过 scheme 白名单。
const URL_ATTRS: &[&str] = &[
    "href",
    "src",
    "action",
    "formaction",
    "poster",
    "cite",
    "background",
    "data",
    "srcset",
    "longdesc",
    "usemap",
    "manifest",
    "codebase",
    "archive",
    "dynsrc",
    "lowsrc",
    "icon",
    "xlink:href",
];

#[derive(Debug, Clone)]
pub(crate) struct Policy {
    pub tags: Vec<String>,
    pub attributes: Vec<(String, Vec<String>)>,
    pub url_schemes: Vec<String>,
    pub strip_comments: bool,
}

impl Default for Policy {
    /// 缺省保守策略 —— 清单冻结在规范 §13.2(W-10)。
    fn default() -> Self {
        Policy {
            tags: [
                "a",
                "b",
                "blockquote",
                "br",
                "code",
                "em",
                "h1",
                "h2",
                "h3",
                "h4",
                "h5",
                "h6",
                "hr",
                "i",
                "li",
                "ol",
                "p",
                "pre",
                "s",
                "strong",
                "u",
                "ul",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            attributes: vec![(
                "a".to_string(),
                vec!["href".to_string(), "title".to_string()],
            )],
            url_schemes: ["http", "https", "mailto"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            strip_comments: true,
        }
    }
}

/// 从 wlwl `DICT` 解析策略。出现未知键 / 类型不符 → `E0030` 带名
/// (严格失败:策略里把 `tagz` 拼错不该静默变成「没配」)。
pub(crate) fn parse_policy(
    host: &mut dyn StdHost,
    dict: &Value,
) -> Result<Policy, wlwl_error::WlwlError> {
    let mut perr = |msg: String| -> wlwl_error::WlwlError {
        host.diag(ErrorCode::E0030, format!("HTML_SANITIZE: {msg}"))
    };
    let mut p = Policy::default();
    let Value::Dict(entries) = dict else {
        return Err(perr("policy must be a DICT".into()));
    };
    for (k, v) in entries {
        let Value::String(key) = k else {
            return Err(perr("policy keys must be STRING".into()));
        };
        match key.as_str() {
            "tags" => {
                let Value::Array(items) = v else {
                    return Err(perr("policy \"tags\" must be an ARRAY".into()));
                };
                let mut list = Vec::new();
                for t in items {
                    let Value::String(name) = t else {
                        return Err(perr("policy \"tags\" must hold STRING elements".into()));
                    };
                    let lowered = name.to_lowercase();
                    if lowered.is_empty()
                        || !lowered.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                    {
                        return Err(perr("policy \"tags\" elements must be lowercase tag names".into()));
                    }
                    list.push(lowered);
                }
                p.tags = list;
            }
            "attributes" => {
                let Value::Dict(inner) = v else {
                    return Err(perr("policy \"attributes\" must be a DICT".into()));
                };
                let mut list = Vec::new();
                for (tk, tv) in inner {
                    let Value::String(tag) = tk else {
                        return Err(perr("policy \"attributes\" keys must be STRING".into()));
                    };
                    let Value::Array(items) = tv else {
                        return Err(perr("policy \"attributes\" values must be ARRAY".into()));
                    };
                    let mut names = Vec::new();
                    for a in items {
                        let Value::String(name) = a else {
                            return Err(perr("policy \"attributes\" elements must be STRING".into()));
                        };
                        names.push(name.to_lowercase());
                    }
                    list.push((tag.to_lowercase(), names));
                }
                p.attributes = list;
            }
            "url_schemes" => {
                let Value::Array(items) = v else {
                    return Err(perr("policy \"url_schemes\" must be an ARRAY".into()));
                };
                let mut list = Vec::new();
                for s in items {
                    let Value::String(scheme) = s else {
                        return Err(perr("policy \"url_schemes\" must hold STRING elements".into()));
                    };
                    list.push(scheme.to_lowercase());
                }
                p.url_schemes = list;
            }
            "strip_comments" => match v {
                Value::Boolean(b) => p.strip_comments = *b,
                _ => return Err(perr("policy \"strip_comments\" must be BOOLEAN".into())),
            },
            other => {
                return Err(perr(format!(
                    "unknown policy key \"{other}\" — known keys: tags, attributes, url_schemes, strip_comments"
                )))
            }
        }
    }
    Ok(p)
}

fn is_dropped_with_content(tag: &str) -> bool {
    DROP_WITH_CONTENT.contains(&tag)
}

fn is_void(tag: &str) -> bool {
    VOID.contains(&tag)
}

fn allowed_attrs<'p>(policy: &'p Policy, tag: &str) -> Vec<&'p str> {
    let mut out = Vec::new();
    for (t, names) in &policy.attributes {
        if t == "*" || t == tag {
            out.extend(names.iter().map(|s| s.as_str()));
        }
    }
    out
}

/// URL 值的 scheme 判定:剥首尾空白与控制字符(0x00–0x20)后取 `:` 前缀;
/// 没有 `:`(相对 URL)或 `:` 前出现 `/` `?` `#` → 相对,恒允许。
fn scheme_allowed(value: &str, schemes: &[String]) -> bool {
    let trimmed: &str = value.trim_matches(|c: char| c <= ' ');
    let lowered = trimmed.to_ascii_lowercase();
    match lowered.find(':') {
        None => true, // 相对 URL
        Some(pos) => {
            let scheme = &lowered[..pos];
            if scheme.is_empty()
                || !scheme
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
                || !scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            {
                // 非法 scheme 形态(如 `java script:` 已被剥空格,但残着
                // 其它怪字符)→ 不放行。
                return false;
            }
            schemes.iter().any(|s| s == scheme)
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Keep,
    Unwrap,
    DropSubtree,
}

/// 就地过滤(单源模型 `target[]`):按文档序(节点创建序)一遍定每个节点的
/// 去留与挂载点 ——
/// - `Keep`:自身输出挂到 `target[parent]`(父 Keep 即父本身,父 Unwrap /
///   Drop 则沿链上溯);
/// - `Unwrap`:自身不输出,孩子沿 `target` 链继续;
/// - `DropSubtree`:整棵无输出,且**后代强制继承**(孩子看到父动作即被传染)。
///
/// 全程显式序(文档序即构建序),无递归 —— 病态深嵌套契约。
pub(crate) fn filter_tree(tree: &mut Tree, policy: &Policy) {
    let n = tree.nodes.len();
    let mut actions = vec![Action::Keep; n];
    let mut target = (0..n).collect::<Vec<usize>>();

    for id in 1..n {
        let parent = tree.nodes[id].parent.unwrap_or(0);
        // ① 后代强制继承整删(父已定,文档序保证)。
        if actions[parent] == Action::DropSubtree {
            actions[id] = Action::DropSubtree;
            target[id] = target[parent];
            continue;
        }
        // ② 自身去留。
        actions[id] = match &tree.nodes[id].kind {
            NodeKind::RawText { .. } => Action::DropSubtree,
            NodeKind::Comment(_) => {
                if policy.strip_comments {
                    Action::DropSubtree
                } else {
                    Action::Keep
                }
            }
            NodeKind::Text(_) => Action::Keep,
            NodeKind::Element(e) => {
                if is_dropped_with_content(&e.name) {
                    Action::DropSubtree
                } else if policy.tags.iter().any(|t| t == &e.name) {
                    Action::Keep
                } else {
                    Action::Unwrap
                }
            }
        };
        // ③ 挂载点 = 父的**输出位置**:父 Keep → 就是父节点本身;父 Unwrap
        //    → 沿链上溯到最近的 Keep 祖先。判据是**父的动作**,不是自己的
        //    —— Keep 节点的孩子挂进它自己,Unwrap 节点的孩子被提升上去。
        //    (父为 DropSubtree 已在 ① 提前收敛为整删。)
        //    两处历史错法都会静默丢内容(D13-005):看自己的动作 → Keep 孩子
        //    挂到不输出的 Unwrap 父下,整棵子树无人遍历;一律用 target[parent]
        //    → Keep 的孩子被踢到父外面,`<a>x</a>` 变成 `<a></a>x`。
        target[id] = if actions[parent] == Action::Keep {
            parent
        } else {
            target[parent]
        };

        // ④ Keep 的元素就地过滤属性(同名首个生效;URL 属性过 scheme;
        //    void 元素防御性清孩子)。
        if actions[id] == Action::Keep {
            if let NodeKind::Element(e) = &mut tree.nodes[id].kind {
                let allowed = allowed_attrs(policy, &e.name);
                let mut seen: Vec<String> = Vec::new();
                e.attrs.retain(|a| {
                    if seen.iter().any(|s| s == &a.name) {
                        return false;
                    }
                    seen.push(a.name.clone());
                    if !allowed.contains(&a.name.as_str()) {
                        return false;
                    }
                    !URL_ATTRS.contains(&a.name.as_str())
                        || scheme_allowed(&a.value, &policy.url_schemes)
                });
                if is_void(&e.name) {
                    tree.nodes[id].children.clear();
                }
            }
        }
    }

    // 重建:Keep 节点按文档序挂到各自的挂载点(target 已含 unwrap 链的
    // 上溯);其余节点无输出。
    let mut new_children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for id in 1..n {
        if actions[id] == Action::Keep {
            new_children[target[id]].push(id);
        } else {
            tree.nodes[id].children.clear();
        }
    }
    for id in 0..n {
        tree.nodes[id].children = std::mem::take(&mut new_children[id]);
        if id != tree.root {
            tree.nodes[id].parent = Some(target[id]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sanitize::tokenize;

    #[test]
    fn scheme_check_normalization() {
        let p = Policy::default();
        assert!(scheme_allowed("https://x", &p.url_schemes));
        // 剥控制字符后是 javascript: → 拒。
        assert!(!scheme_allowed("  \t\n javascript:x", &p.url_schemes));
        assert!(!scheme_allowed("\u{0A}javascript:x", &p.url_schemes));
        assert!(scheme_allowed("/relative/path", &p.url_schemes));
        assert!(scheme_allowed("page.html", &p.url_schemes));
        assert!(!scheme_allowed("javascript:alert(1)", &p.url_schemes));
        assert!(!scheme_allowed("data:text/html,<x>", &p.url_schemes));
        assert!(scheme_allowed("MAILTO:a@b", &p.url_schemes));
        // 大小写不敏感:mailto 在白名单里(小写),大写输入小写化后命中。
        assert!(scheme_allowed("Mailto:a@b", &p.url_schemes));
    }

    #[test]
    fn void_elements_list_is_html5() {
        for t in ["br", "hr", "img", "input"] {
            assert!(is_void(t));
        }
        assert!(!is_void("p"));
    }

    #[test]
    fn drop_set_covers_raw_text_elements() {
        // tokenizer 的 raw-text 全集必须都在整删清单里 —— 否则 raw-text
        // 内容会被当标记解析(mXSS 面)。
        for t in tokenize::RAWTEXT {
            assert!(is_dropped_with_content(t), "{t} must drop with content");
        }
    }
}
