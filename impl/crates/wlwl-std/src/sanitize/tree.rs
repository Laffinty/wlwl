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

pub(crate) fn build(tokens: Vec<Token>) -> Tree {
    let mut nodes: Vec<Node> = vec![Node {
        kind: NodeKind::Text(String::new()), // root 占位;序列化时按 Element 空 children 处理
        parent: None,
        children: Vec::new(),
    }];
    // 栈里存的是**未闭合的元素**节点 id(root 不入栈)。
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
                // tokenizer 在 Start 之后紧接着发本 token,并**已吞掉配对的
                // 结束标签**(不发 End)—— 故对应的开标签此刻必在栈顶。内容
                // 挂它身上后**立即弹栈**:不弹则其后的内容会挂进这个「必被
                // 整删」的元素里跟着消失(实测 `<script>x</script>after` 的
                // `after` 全丢,D13-003)。
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
                // 隐式闭合:栈顶命中规则就弹(可连续)。
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
                // 栈内找同名开标签 → 弹到匹配处;找不到 → 忽略。
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
