//! [v0.11.3 M3 W-11] 树构建 —— 开标签栈 + 白名单子集的隐式闭合规则。
//!
//! **节点池 + 索引**(`Vec<Node>` + `usize` 句柄),不用 `Rc` / `Box` 链:
//! 分配一次、缓存友好,且过滤阶段的 unwrap / 重挂不需要改写子指针。
//! 构建循环为**迭代栈**,深嵌套不栈溢出(病态输入契约)。
//!
//! 规则子集(与浏览器解析器的差异已在规范 §13.2 显式登记):
//! - `<p>` 在新块级标签开始时隐式闭合;`<li>` 闭合 `<li>`;`<dt>`/`<dd>`
//!   闭合彼此;`<tr>`/`<td>`/`<th>`/`<thead>`/`<tbody>`/`<tfoot>` 按 HTML5
//!   位次互相闭合;`<option>` 闭合 `<option>`;
//! - 结束标签:栈内找得到 → 弹到匹配处;找不到 → 忽略;
//! - void 元素不入栈、无子节点;
//! - raw-text 的内容是整块 [`NodeKind::RawText`],过滤阶段整体删除。
//!
//! **foster parenting 不实现**(表格外文本不搬移)—— 规范已登记该差异。
//!
//! [N-006 / D13-058]开标签栈**带一张按名字索引的位置表**(`open_pos`):
//! 结束标签的「栈内有没有同名开标签」从**扫全栈**(`rposition`)变成**一次
//! 哈希查表**。规范 §13.2「任何线性输入不得触发超线性时间」是**规范性条款**,
//! 而旧写法在 `"<b>"×5万 + "</i>"×5万` 上是 O(n²)(判负时栈原样保留 ⇒ 可以
//! 无限长)。⚠ 代价是**索引必须与栈同步**:`truncate` 让 `>= pos` 的下标整体
//! 失效,所有改栈的路径(显式 `truncate` / 隐式闭合 / RawText 弹栈)都必须
//! 走 [`pop_open`] —— 漏一处就是「关错元素」而不是「慢一点」。

use std::collections::HashMap;

#[cfg(test)]
use std::cell::Cell;

use super::tokenize::{self, Attr, Token};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NodeKind {
    Text(String),
    Element(ElementData),
    Comment(String),
    RawText { name: String, body: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ElementData {
    pub name: String,
    pub attrs: Vec<Attr>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    pub kind: NodeKind,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

#[derive(Debug)]
pub(crate) struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize, // 合成的文档根(`#document`),不出现在序列化输出
}

/// void 元素:无子节点、无结束标签(HTML5 全集 14 个)。
pub(crate) const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// 隐式闭合规则:`incoming` 开始时应弹掉的**开标签**集合。
/// 表只覆盖白名单相关子集 —— 差异已在规范 §13.2 登记。
fn implied_closes(incoming: &str) -> &'static [&'static str] {
    match incoming {
        "li" => &["li", "p"],
        "dt" | "dd" => &["dt", "dd", "p"],
        "td" | "th" => &["td", "th", "p"],
        "tr" => &["tr", "td", "th", "p"],
        "thead" | "tbody" | "tfoot" => {
            &["thead", "tbody", "tfoot", "tr", "td", "th", "caption", "p"]
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "blockquote" | "pre" | "ul" | "ol"
        | "hr" | "table" | "div" => &["p"],
        "option" => &["option", "p"],
        _ => &[],
    }
}

// [N-006 / D13-058]开标签栈元素被**逐个查看**的次数(测试计量口径)。
//
// ⚠ 这个计数器的**定义**就是「一个栈元素被拿去比对过一次」:任何把线性扫
// 拿回来的改法都**必须**在那个扫描点上计数,否则下面的复杂度契约会空转
// (这是「代理指标」这类守卫的固有上限,已在契约注释里登记)。
#[cfg(test)]
thread_local! {
    static STACK_PROBES: Cell<usize> = const { Cell::new(0) };
}

/// 取走当前线程的计量值(取走即清零)。
#[cfg(test)]
pub(crate) fn take_stack_probes() -> usize {
    STACK_PROBES.with(|c| c.replace(0))
}

/// 非测试构建下编译为空(不留任何运行时开销)。
#[inline]
fn probe(n: usize) {
    #[cfg(test)]
    STACK_PROBES.with(|c| c.set(c.get() + n));
    #[cfg(not(test))]
    let _ = n;
}

/// 开标签入栈:登记它在栈内的位置。
fn push_open(
    stack: &mut Vec<usize>,
    open_pos: &mut HashMap<String, Vec<usize>>,
    id: usize,
    name: &str,
) {
    let pos = stack.len();
    match open_pos.get_mut(name) {
        // ⚠ 同名嵌套(`<b><b>`)只 push 不新建键 —— 否则病态嵌套会退化成
        // 每层一次小分配。
        Some(v) => v.push(pos),
        None => {
            open_pos.insert(name.to_string(), vec![pos]);
        }
    }
    stack.push(id);
}

/// 把开标签栈顶的 `n` 个元素弹掉,并**同步**清掉位置索引里的对应条目。
///
/// ⚠ [N-006]这是**唯一**允许改栈的出口 —— 索引与栈的同步性全靠它。
/// 漏一处 ⇒ 索引里留下一个已经不存在的元素的下标 ⇒ 后续 `</name` 会
/// 「弹到」错误的位置(静默关错元素),比旧的慢更糟。
fn pop_open(
    stack: &mut Vec<usize>,
    open_pos: &mut HashMap<String, Vec<usize>>,
    nodes: &[Node],
    n: usize,
) {
    for _ in 0..n {
        let id = match stack.pop() {
            Some(id) => id,
            None => return,
        };
        if let NodeKind::Element(e) = &nodes[id].kind {
            // ⚠ 自栈顶**向下**弹 ⇒ 该名的位置序列正好从末尾逐个摘掉,一一
            // 对应;若正序弹则同名的多条会摘错序(集合仍对但过程不可读)。
            probe(1);
            let mut exhausted = false;
            match open_pos.get_mut(e.name.as_str()) {
                Some(v) => {
                    v.pop();
                    exhausted = v.is_empty();
                }
                // 栈里只可能有 Element(入栈口已过滤 void / 自闭合),
                // 所以「索引里查不到」只可能是索引失步。
                None => debug_assert!(false, "位置索引缺 {} —— 索引与栈失步", e.name),
            }
            if exhausted {
                open_pos.remove(e.name.as_str());
            }
        }
    }
}

/// 测试构建下的收尾不变式:位置索引必须与栈**逐项**一致。
#[cfg(test)]
fn assert_index_matches_stack(
    stack: &[usize],
    open_pos: &HashMap<String, Vec<usize>>,
    nodes: &[Node],
) {
    let mut want: HashMap<&str, Vec<usize>> = HashMap::new();
    for (pos, &id) in stack.iter().enumerate() {
        if let NodeKind::Element(e) = &nodes[id].kind {
            want.entry(e.name.as_str()).or_default().push(pos);
        }
    }
    let got: HashMap<&str, Vec<usize>> = open_pos
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();
    assert_eq!(
        got, want,
        "位置索引与开标签栈失步(N-006 的索引必须有唯一出口)"
    );
}

pub(crate) fn build(tokens: Vec<Token>) -> Tree {
    let mut nodes: Vec<Node> = vec![Node {
        kind: NodeKind::Text(String::new()), // root 占位;序列化时按 Element 空 children 处理
        parent: None,
        children: Vec::new(),
    }];
    // 栈里存的是**未闭合的元素**节点 id(root 不入栈)。
    let mut stack: Vec<usize> = Vec::new();
    // [N-006]标签名 → 该名在栈内的位置(由外到内;`last()` 即最内层)。
    let mut open_pos: HashMap<String, Vec<usize>> = HashMap::new();

    let append = |nodes: &mut Vec<Node>, parent: usize, kind: NodeKind| -> usize {
        let id = nodes.len();
        nodes.push(Node {
            kind,
            parent: Some(parent),
            children: Vec::new(),
        });
        nodes[parent].children.push(id);
        id
    };

    for token in tokens {
        match token {
            Token::Text(t) => {
                let parent = stack.last().copied().unwrap_or(0);
                append(&mut nodes, parent, NodeKind::Text(t));
            }
            Token::Comment(c) => {
                let parent = stack.last().copied().unwrap_or(0);
                append(&mut nodes, parent, NodeKind::Comment(c));
            }
            Token::RawText { name, body } => {
                // tokenizer 在 Start 之后紧接着发本 token,并**已吞掉配对的
                // 结束标签**(不发 End)—— 故对应的开标签此刻必在栈顶。内容
                // 挂它身上后**立即弹栈**:不弹则其后的内容会挂进这个「必被
                // 整删」的元素里跟着消失(实测 `<script>x</script>after` 的
                // `after` 全丢,D13-003)。
                let parent = stack.last().copied().unwrap_or(0);
                append(&mut nodes, parent, NodeKind::RawText { name, body });
                // ⚠ [N-006]判「栈顶是不是这个 raw-text 元素」要在**弹之前**
                // 问 —— 判据与修复前逐字相同(栈顶是元素且名字在 RAWTEXT 里)。
                let top_is_rawtext = match stack.last() {
                    Some(&top) => {
                        matches!(&nodes[top].kind, NodeKind::Element(e) if tokenize::RAWTEXT.contains(&e.name.as_str()))
                    }
                    None => false,
                };
                if top_is_rawtext {
                    pop_open(&mut stack, &mut open_pos, &nodes, 1);
                }
            }
            Token::Start {
                name,
                attrs,
                self_closing,
            } => {
                // 隐式闭合:栈顶命中规则就弹(可连续)。
                while let Some(&top) = stack.last() {
                    let open = match &nodes[top].kind {
                        NodeKind::Element(e) => e.name.as_str(),
                        _ => break,
                    };
                    if implied_closes(&name).contains(&open) {
                        // ⚠ [N-006]这也是一次弹栈 ⇒ 走同一个出口。
                        pop_open(&mut stack, &mut open_pos, &nodes, 1);
                    } else {
                        break;
                    }
                }
                let parent = stack.last().copied().unwrap_or(0);
                let id = append(
                    &mut nodes,
                    parent,
                    NodeKind::Element(ElementData {
                        name: name.clone(),
                        attrs,
                    }),
                );
                if !self_closing && !VOID.contains(&name.as_str()) {
                    push_open(&mut stack, &mut open_pos, id, &name);
                }
            }
            Token::End(name) => {
                // 栈内找同名开标签 → 弹到匹配处(连带它上面的全部开标签);
                // 找不到 → 忽略(栈原样保留)。
                //
                // ⚠ [N-006 / D13-058]此处此前是
                // `stack.iter().rposition(|&id| … e.name == name)`:**判负时它
                // 扫完整个栈**,而判负不缩短栈 ⇒ 栈长无上界 ⇒
                // `"<b>"×N + "</i>"×N` 是 Θ(N²) 次比较(规范 §13.2 的规范性
                // 线性条款)。现在「最内层同名位置」是一次查表。
                // `rposition`(最内层优先)与 `Vec::last()` 同义。
                //
                // ⚠ 取值先落到局部量再进 `if let`:2021 版里 `if let` 的
                // 临时量活到分支结束,会与分支内的 `&mut open_pos` 撞借用。
                let hit = open_pos.get(name.as_str()).and_then(|v| v.last()).copied();
                if let Some(pos) = hit {
                    let depth = stack.len() - pos; // 含被匹配的那一个
                    pop_open(&mut stack, &mut open_pos, &nodes, depth);
                }
            }
        }
    }

    #[cfg(test)]
    assert_index_matches_stack(&stack, &open_pos, &nodes);

    Tree { nodes, root: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sanitize::tokenize;

    /// N-006 **修复前**的建树(`rposition` 逐项扫栈版),作为语义参照。
    ///
    /// ⚠ 它留在测试里**不是**「期望值抄实现」:缺陷在**复杂度**而不在语义,
    /// 所以修复前的输出正是修复后必须**逐字段相等**的那个参照物。有它在,
    /// 「加索引时漏清一处」这种**静默关错元素**的改法在任何一条语料上立刻
    /// 可判,不必靠人肉推演栈 —— [`assert_index_matches_stack`] 只查**收尾**
    /// 一瞬间的形态,中间态的错位它照不到。
    fn build_reference(tokens: Vec<Token>) -> Tree {
        let mut nodes: Vec<Node> = vec![Node {
            kind: NodeKind::Text(String::new()),
            parent: None,
            children: Vec::new(),
        }];
        let mut stack: Vec<usize> = Vec::new();
        let append = |nodes: &mut Vec<Node>, parent: usize, kind: NodeKind| -> usize {
            let id = nodes.len();
            nodes.push(Node {
                kind,
                parent: Some(parent),
                children: Vec::new(),
            });
            nodes[parent].children.push(id);
            id
        };
        for token in tokens {
            match token {
                Token::Text(t) => {
                    let parent = stack.last().copied().unwrap_or(0);
                    append(&mut nodes, parent, NodeKind::Text(t));
                }
                Token::Comment(c) => {
                    let parent = stack.last().copied().unwrap_or(0);
                    append(&mut nodes, parent, NodeKind::Comment(c));
                }
                Token::RawText { name, body } => {
                    let parent = stack.last().copied().unwrap_or(0);
                    append(&mut nodes, parent, NodeKind::RawText { name, body });
                    if let Some(&top) = stack.last() {
                        if let NodeKind::Element(e) = &nodes[top].kind {
                            if tokenize::RAWTEXT.contains(&e.name.as_str()) {
                                stack.pop();
                            }
                        }
                    }
                }
                Token::Start {
                    name,
                    attrs,
                    self_closing,
                } => {
                    while let Some(&top) = stack.last() {
                        let open = match &nodes[top].kind {
                            NodeKind::Element(e) => e.name.as_str(),
                            _ => break,
                        };
                        if implied_closes(&name).contains(&open) {
                            stack.pop();
                        } else {
                            break;
                        }
                    }
                    let parent = stack.last().copied().unwrap_or(0);
                    let id = append(
                        &mut nodes,
                        parent,
                        NodeKind::Element(ElementData {
                            name: name.clone(),
                            attrs,
                        }),
                    );
                    if !self_closing && !VOID.contains(&name.as_str()) {
                        stack.push(id);
                    }
                }
                Token::End(name) => {
                    if let Some(pos) = stack.iter().rposition(
                        |&id| matches!(&nodes[id].kind, NodeKind::Element(e) if e.name == name),
                    ) {
                        stack.truncate(pos);
                    }
                }
            }
        }
        Tree { nodes, root: 0 }
    }

    /// 语料覆盖**每一条改栈的路径**:显式 `truncate`(结束标签)、隐式闭合
    /// (`implied_closes` 的 li/p/td/tr/option/dt/dd 各一族)、RawText 弹栈、
    /// void / 自闭合不入栈、错位闭合(`<b><i></b>` 这类)、未匹配结束标签。
    #[rustfmt::skip]
    const CORPUS: &[&str] = &[
        "", "a", "a<b>c</b>d",
        "<b><i></b></i>", "<b><i></b>", "<b></i></b>", "<b><i></i></b>x",
        "<b><b><b></b></b></b>", "<b><i></i><u></u></b>",
        "<p>a<p>b", "<p><b><p>", "<div><p>a</div>b",
        "<ul><li>a<li>b</ul>", "<ol><li>a</ol>",
        "<dl><dt>a<dd>b<dt>c</dl>",
        "<select><option>a<option>b</select>",
        "<table><tr><td>1<td>2</table>",
        "<table><thead><tr><th>h<tbody><tr><td>d<tfoot><tr><td>f</table>",
        "<script>x</script>after", "<title>t</title>u", "<style>a{}</style>b",
        "<textarea></textarea>c", "<script>a</SCRIPT>b", "<script>a</scriptx>b",
        "<script>a<script>b</script>", "<b><script>x</script></b>y",
        "<br><img>", "<br/><hr/>", "<p>a<br>b",
        "<b><i></b><u></u></i>",
        "<blockquote><p>q</blockquote>",
    ];

    /// [N-006 / D13-058]语义不变量:换算法不得改一棵树。
    ///
    /// 期望值来源 = [`build_reference`],即**修复前**的建树(缺陷在复杂度,
    /// 不在语义)。反向守卫:语料不得缩到不再覆盖上述路径。
    #[test]
    fn the_position_index_changes_no_tree() {
        assert!(
            CORPUS.len() >= 25,
            "语料缩到 {} 条 —— 覆盖丢了?",
            CORPUS.len()
        );
        for src in CORPUS {
            let got = build(tokenize::tokenize(src));
            let want = build_reference(tokenize::tokenize(src));
            assert_eq!(got.root, want.root, "root 不同: {src:?}");
            assert_eq!(got.nodes, want.nodes, "树与修复前不同: {src:?}");
        }
        // 病态形状也走一遍(小规模即可 —— 参照实现本身是平方的)。
        for src in [
            format!("{}x{}", "<b>".repeat(64), "</i>".repeat(64)),
            format!("{}x{}", "<b>".repeat(64), "</b>".repeat(64)),
            format!("{}x{}", "<ul><li>".repeat(32), "</b>".repeat(32)),
        ] {
            let got = build(tokenize::tokenize(&src));
            let want = build_reference(tokenize::tokenize(&src));
            assert_eq!(got.nodes, want.nodes, "病态形状下树与修复前不同");
        }
    }

    /// [N-006 / D13-058]复杂度契约:**未匹配**的结束标签必须 O(1) 判负。
    ///
    /// 规范依据:§13.2 性能契约「**任何线性输入不得触发超线性时间**」
    /// (规范性条款)。输入形状取审查报告点名的 `"<b>"×N + "</i>"×N` ——
    /// 5 万个 `</i>` 在栈里**永远找不到**同名开标签,旧写法每个都扫完整个栈。
    ///
    /// ⚠ 判据是**计量值**而非耗时(耗时会被机器噪声打脸):`probes` = 被逐个
    /// 查看的栈元素数。线性承诺 ⇔ 该值对 N **线性**;旧写法是 Θ(N²)
    /// (N=50_000 ⇒ 2.5×10⁹,与 `probes ≤ N` 差 5 个数量级 —— 转红不靠
    /// 余量,靠量级)。⚠ 这条只覆盖**埋了计数**的扫描点:谁把 `rposition`
    /// 拿回来却不计数,契约就空转(代理指标的固有上限)。
    #[test]
    fn unmatched_end_tags_do_not_scan_the_open_tag_stack() {
        const N: usize = 50_000;
        let src = format!("{}x{}", "<b>".repeat(N), "</i>".repeat(N));
        let _ = take_stack_probes(); // 清零
        let tree = build(tokenize::tokenize(&src));
        let probes = take_stack_probes();
        // 语义:`</i>` 全部忽略(规范 §13.2「不在栈内则忽略」)⇒ 5 万个 `<b>`
        // 仍在栈上 ⇒ 节点 = root + 5 万元素 + 1 个文本。
        assert_eq!(tree.nodes.len(), N + 2, "树形状变了(判负时不得动栈)");
        assert_eq!(
            tree.nodes[0].children.len(),
            1,
            "最外层 `<b>` 应是根的唯一子节点"
        );
        assert!(
            probes <= N,
            "未匹配结束标签的栈查看次数 {probes} 超过线性上限 {N} —— \
             「找不到就扫全栈」回来了?"
        );
        // 命中路径的计量**必须非零**:否则上一条可能是因为「计数器压根没被
        // 走过」而绿(这就是本仓反复踩的「守卫生效 = 退回 bug 形态真的转红」)。
        let matched = format!("{}x{}", "<b>".repeat(N), "</b>".repeat(N));
        let tree = build(tokenize::tokenize(&matched));
        let hit_probes = take_stack_probes();
        assert_eq!(tree.nodes.len(), N + 2, "配对嵌套的树形状变了");
        assert_eq!(
            hit_probes, N,
            "配对嵌套应恰好每个结束标签弹一个元素({N}) —— \
             计量口径或索引同步被改动了"
        );
    }
}
