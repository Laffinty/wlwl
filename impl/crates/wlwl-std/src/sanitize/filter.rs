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

#[cfg(test)]
use std::cell::Cell;

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
            url_schemes: default_url_schemes(),
            strip_comments: true,
        }
    }
}

/// 默认允许的 URL scheme 列表(单源)。
///
/// [v0.11.3 M7 / addendum-13 W-03] `HTML_SANITIZE` 的默认策略与
/// `HTML_SANITIZE_URL` 都从这里取,否则会出现「净化器放行、独立成员拒绝」的分裂。
pub(crate) const DEFAULT_URL_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// [v0.11.3 M7 / addendum-13 W-03] 默认 scheme 列表的 owned 形态(供策略用)。
pub(crate) fn default_url_schemes() -> Vec<String> {
    DEFAULT_URL_SCHEMES.iter().map(|s| s.to_string()).collect()
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
/// URL 类属性的 scheme 判定。
///
/// [v0.11.3 M7 / addendum-13 W-03] 提为 `pub(crate)`,让 `HTML_SANITIZE_URL`
/// **复用同一份逻辑** —— 另写一份就会出现「净化器放行、独立成员拒绝」的分裂,
/// 而分裂意味着其中一处是错的。契约 `sanitize_contract` 对同一组 URL 喂两个
/// 入口并断言结论一致。
///
/// [D13-023 · N-016] 上面那句注释原先是**空头支票**:实现只有
/// `find(':') == None` 一支,`/ ? #` 豁免根本没写。后果 `/wiki/Special:Search`
/// 被判成 `scheme = "/wiki/special"`(首字符非字母 → 非法 → 拒),`href` 被删 ——
/// 方向保守,但**违背自家规范 §13.2 / §13.5 的「相对 URL 恒允许」**。
/// 判定顺序按 WHATWG URL §4.2「scheme start state」:scheme 只可能在**第一个**
/// `/`、`?`、`#` **之前**结束;落在它们之后的那个 `:` 是路径段 / 查询 /
/// 片段内部的分隔符(RFC 3986 §3.3 只禁止**首段**含 `:`,§3.4 查询与
/// §3.5 片段则允许 `:` 自由出现)。
///
/// ⚠ 判定位置必须在**剥空白/控制字符之后、大小写归一之前**的位置语义上做,
/// 但本实现沿用「先小写化再找 `:`」—— 三者对 `/ ? #` 的位置判定等价
/// (这三者的大小写与空白形态都被 scheme 语法排除)。
///
/// ⚠ 不覆盖 `\`(WHATWG 对 special scheme 把 `\` 视作 `/`):`\\host` 那类值
/// 按规范 §13.2 已是「相对 URL」恒允许,豁免与否结论相同,故不扩大豁免集。
pub(crate) fn scheme_allowed(value: &str, schemes: &[String]) -> bool {
    let trimmed: &str = value.trim_matches(|c: char| c <= ' ');
    let lowered = trimmed.to_ascii_lowercase();
    let Some(pos) = lowered.find(':') else {
        return true; // 相对 URL:连 `:` 都没有
    };
    // `:` 之前有 `/` `?` `#` ⇒ 这个 `:` 不是 scheme 的结尾,是路径 / 查询 /
    // 片段里的分隔符 ⇒ 相对 URL,**恒允许**(不进 scheme 白名单比对)。
    // ⚠ 这一支必须排在「scheme 形态合法性」判定**之前**:先判形态会把
    // `/wiki/special` 当成非法 scheme 而拒掉 —— 那正是原缺陷。
    if lowered[..pos].contains(['/', '?', '#']) {
        return true;
    }
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

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Keep,
    Unwrap,
    DropSubtree,
}

// ── 同名判重的计量口径(D13-062 / N-020;判据换形见 D13-092)────────────
//
// ⚠ 计数器的**定义**是「一次『这个名字是否已见过』的判定所付出的名字
// 查找次数」。哈希集合下 = 一次「哈希 + 探测」⇒ 计数 ≈ 属性个数(线性)。
//
// ⚠⚠ **判据为什么是计数而不是墙钟比值**(D13-092):同一份实现(哈希集合)
// 在本地 debug 档量到 **4.0×**、在 macOS CI 上量到 **12.8×**(已越过旧
// 阈值 12)⇒ 墙钟比值**测的是机器**不是复杂度 —— 两档规模的**内存占位**
// 不同(2 千 vs 8 千个属性 ⇒ 哈希表与属性数组各自跨过 L2 边界),比值里
// 混着缓存命中率的成分,而那部分随机器与并发负载漂移。
// ⇒ 换成**确定性计数**:同一份代码在本地与 CI 上**同红同绿**。
//
// ⚠ **代理指标的固有上限**(同 `tree.rs` 的 `STACK_PROBES`):**任何把线性
// 扫拿回来的改法都必须在它自己的扫描点上计数**,否则下面的契约会空转。
// 本仓的写法把计量点收进 `SeenNames` 内部 ⇒ 「绕过计量点」与「退化成
// 平方」这两种改法**分别**被下面两条断言抓住(见测试)。
#[cfg(test)]
thread_local! {
    static NAME_PROBES: Cell<usize> = const { Cell::new(0) };
}

/// 取走当前线程的计量值(取走即清零)。
#[cfg(test)]
pub(crate) fn take_name_probes() -> usize {
    NAME_PROBES.with(|c| c.replace(0))
}

/// 非测试构建下编译为空(不留任何运行时开销)。
#[inline]
fn probe_name(n: usize) {
    #[cfg(test)]
    NAME_PROBES.with(|c| c.set(c.get() + n));
    #[cfg(not(test))]
    let _ = n;
}

/// 同名判重用的「已见名字」集合。
///
/// 语义:`claim` 返回 `true` = 这个名字**第一次出现** ⇒ 该属性「生效」
/// (§13.2「同名属性重复时**首个生效**」的「生效」= **占用这个名字**,
/// 不是「被保留」—— 登记与白名单判据的先后顺序即语义,见 `filter_tree`)。
struct SeenNames<'a> {
    set: std::collections::HashSet<&'a str>,
}

impl<'a> SeenNames<'a> {
    fn new() -> Self {
        SeenNames {
            set: std::collections::HashSet::new(),
        }
    }

    /// 登记 `name`:`true` = 首次出现(生效),`false` = 同名(丢弃)。
    fn claim(&mut self, name: &'a str) -> bool {
        // 计量点:哈希集合的一次「查 + 插」= 一次名字查找。
        probe_name(1);
        self.set.insert(name)
    }
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
                // 🔴 [D13-062 / N-020]同名判重用 **`SeenNames`**(内部是
                // `HashSet<&str>`)而非 `Vec` + 线性扫。此前是
                // `seen.iter().any(|s| s == &a.name)` ⇒ 单个标签 k 个属性是
                // **O(k²) 次 `String` 比较**(实测 2 万属性 ≈ 2×10⁸ 次),撞
                // 规范 §13.2「任何线性输入不得触发超线性时间」的**规范性条款**。
                //
                // ⚠⚠ **语义必须逐字不变**,两处都不能动:
                //   ① **`seen` 在白名单判据之前登记** ⇒ 首个同名属性**即便因为
                //      不在白名单而被删,也仍然挡住后面的同名属性**。这与
                //      §13.2「同名属性重复时**首个生效**」一致(首个"生效"
                //      指的是「占用这个名字」,不是「被保留」)。改这一行的顺序
                //      就会让一个被拒的属性**不再挡住**后来的 —— 那是把
                //      「首个生效」悄悄读成「首个被保留」。
                //   ② 按**文档序**处理。
                //
                // ⚠ 为什么改成显式循环而不是 `retain`:原来的闭包里
                // `seen.insert(a.name.as_str())` 会让 `seen` 借着
                // `e.attrs` 的不可变引用,而 `retain` 要 `&mut e.attrs`
                // —— 借用冲突。改成「循环收集到新 `Vec`、循环结束后再整体赋值」,
                // NLL 下两个不可变借用在赋值时都已结束。
                // 顺带**零额外分配**:原实现对每个属性 `a.name.clone()` 一次,
                // 现在只有活下来的属性才被克隆。
                let mut seen = SeenNames::new();
                let mut kept = Vec::with_capacity(e.attrs.len());
                for a in e.attrs.iter() {
                    // ⚠ 见 ①:先登记,再判白名单。顺序即语义。
                    if !seen.claim(a.name.as_str()) {
                        continue;
                    }
                    if !allowed.contains(&a.name.as_str()) {
                        continue;
                    }
                    if URL_ATTRS.contains(&a.name.as_str())
                        && !scheme_allowed(&a.value, &policy.url_schemes)
                    {
                        continue;
                    }
                    kept.push(a.clone());
                }
                e.attrs = kept;
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

    /// [D13-023 · N-016] `/ ? #` 豁免 —— 注释承诺过、原文没实现的那一支。
    ///
    /// 依据:WHATWG URL §4.2 scheme start state(scheme 只在首个 `/` `?` `#`
    /// 之前结束)≡ RFC 3986 §3.3(相对引用的**首段**不得含 `:`;§3.4/§3.5
    /// 对 query / fragment 里的 `:` 无限制)。
    #[test]
    fn scheme_check_exempts_colons_after_path_query_fragment_delimiters() {
        let p = Policy::default();
        // 维基式命名空间:路径段里带冒号。
        assert!(scheme_allowed("/wiki/Special:Search", &p.url_schemes));
        assert!(scheme_allowed("p/x:y?q=1#z", &p.url_schemes));
        assert!(scheme_allowed("?q=a:b", &p.url_schemes));
        assert!(scheme_allowed("#frag:x", &p.url_schemes));
        // ⚠ 豁免是**位置**判定而非「含冒号就放过」:`special` 是形态合法的
        // scheme token,`:` 前无分隔符 ⇒ 按 scheme 判,不在白名单 ⇒ 拒。
        assert!(!scheme_allowed("Special:Search", &p.url_schemes));
        // 经典攻击串不受豁免影响。
        assert!(!scheme_allowed("javascript:alert(1)", &p.url_schemes));
        assert!(!scheme_allowed("data:text/html,x", &p.url_schemes));
        assert!(!scheme_allowed("java script:x", &p.url_schemes));
    }

    /// [D13-062 · N-020] **同名属性去重不得随属性数平方增长。**
    ///
    /// ⚠ **语料必须是「互不相同」的名字,不是重复的名字。** 原实现是
    /// `seen.iter().any(|s| s == &a.name)`:同名时 `any` 在**下标 0** 就命中 ⇒
    /// 每属性 O(1) ⇒ **整体是线性的**;真正付出平方代价的是**互不相同**的名字 ——
    /// 每次都要扫完整个 `seen`。⚠ 用错语料这条测试就会**在缺陷形态下也绿**。
    /// 而且这些名字都不在默认白名单里,却**照样进 `seen`** —— 因为 `seen`
    /// 的登记在白名单判据**之前**(见下一条)。
    ///
    /// ⚠⚠ [D13-092] **判据从「两档规模的墙钟比值」换成「确定性计数」。**
    ///
    /// 旧判据(两档规模的耗时比 < 12 倍)**在 CI 上不可靠**,已退役:
    /// 同一份实现(哈希集合)在本地 debug 档量到 **4.0×**、在 macOS CI 上量到
    /// **12.8×**(run `39394600227` 之后那一轮,越过旧阈值 12 ⇒ 误红)。
    /// ⚠ **墙钟比值测的是机器,不是复杂度**:两档规模的**内存占位**不同
    /// (2 千 vs 8 千个属性 ⇒ 哈希表与属性数组各自跨过 L2 边界),比值里混着
    /// **缓存命中率**的成分,而那部分随机器与并发负载漂移 ⇒ 缺陷形态
    /// (27.8×)与正常形态在 CI 上**没有稳定间隔**,任何阈值都是二选一:
    /// 要么误红正常实现,要么放过缺陷实现。
    ///
    /// 新判据 = **名字查找次数**(`take_name_probes`):哈希集合下每个属性
    /// 一次「查 + 插」⇒ 计数 **== 属性数**(线性);退回线性扫 ⇒ 计数
    /// **≈ k²/2**。⇒ 同一份代码在本地与 CI 上**同红同绿**,不再随机器漂移。
    /// ⚠ 顺带比旧判据**多抓一种改法**:旧判据只看得见「慢」,看不见
    /// 「没被计量」;新判据的第一条断言要求计数 **≥ 属性数** ⇒ 把去重
    /// 整个绕开(不经过计量点)也会红。
    #[test]
    fn distinct_attribute_dedup_does_not_scale_quadratically() {
        const K: usize = 2_000;
        let mut src = String::from("<b");
        for i in 0..K {
            src.push_str(&format!(" a{i}=\"v\""));
        }
        src.push('>');
        let mut tree = crate::sanitize::tree::build(tokenize::tokenize(&src));
        let p = Policy::default();
        let _ = take_name_probes(); // 清零(与其它用例共用线程时也不串味)
        filter_tree(&mut tree, &p);
        let probes = take_name_probes();
        // ① 计量点必须被走到 —— 「没量到」与「量到很少」必须能分开,
        //    否则把去重整个绕开也能绿(空转的守卫比没有守卫更坏)。
        assert!(
            probes >= K,
            "the dedup path was not metered: {probes} name lookups for {K} attributes \
             — the complexity bound below would be vacuous"
        );
        // ② 线性上界:哈希集合下 probes == K(每个属性一次「查 + 插」);
        //    平方形态(每次扫完整个已见表)⇒ probes ≈ K²/2。
        assert!(
            probes <= 64 * K,
            "the dedup did {probes} name lookups for {K} distinct attributes \
             (linear is {K}, quadratic is ~{}) — the dedup looks super-linear again?",
            K * K / 2
        );
    }

    /// [D13-062 · N-020] **`seen` 的登记必须在白名单 / scheme 判据之前** ——
    /// 这是规范 §13.2「同名属性重复时**首个生效**」的可观察面。
    ///
    /// 首个 `href` 因 scheme 被拒,**它仍然占住了 `href` 这个名字**,于是第二个
    /// `href` 被跳过 ⇒ **一个 `href` 都不剩**。若把登记挪到判据之后,第二个
    /// (合法的)就会活下来 —— 那等于把「首个生效」悄悄读成「首个**被保留**」。
    ///
    /// 依据:规范 §13.2「同名属性重复时**首个生效**」。⚠ 注意这条**不可观察**的
    /// 情形:若首个是因为**名字不在白名单**而被拒,那么它的所有同名者本来也
    /// 会被同一条白名单拒掉,登记顺序无差别 —— 真正能区分两种实现的只有
    /// **scheme 判定**这一条,所以语料必须用 `href`。
    #[test]
    fn the_first_duplicate_claims_the_name_even_when_it_is_rejected() {
        let mut tree = crate::sanitize::tree::build(tokenize::tokenize(
            r#"<a href="javascript:alert(1)" href="https://ok.example/">x</a>"#,
        ));
        filter_tree(&mut tree, &Policy::default());
        let html = &{
            let mut s = String::new();
            crate::sanitize::serialize::serialize(&tree, &mut s);
            s
        };
        assert!(
            !html.contains("href"),
            "the first `href` was rejected by scheme but still claims the name, so NO href \
             may survive — got {html:?}"
        );
        // 对照:只有**一个** `href` 且合法时它必须留下(否则本测试会因过度收紧而绿)。
        let mut tree = crate::sanitize::tree::build(tokenize::tokenize(
            r#"<a href="https://ok.example/">x</a>"#,
        ));
        filter_tree(&mut tree, &Policy::default());
        let html = &{
            let mut s = String::new();
            crate::sanitize::serialize::serialize(&tree, &mut s);
            s
        };
        assert!(
            html.contains("href=\"https://ok.example/\""),
            "got {html:?}"
        );
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
