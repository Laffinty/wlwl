//! [v0.11.3 M3 W-11] 序列化 —— 过滤后的树回 HTML。
//!
//! 口径(幂等性的另一半):
//! - 文本节点:`escape_str`(转义 `& < > " '` 五字符)—— 与 `HTML_ESCAPE`
//!   同一函数,`&amp;` 系实体在解码侧全表可逆;
//! - 属性值:恒为**双引号包裹** + 同款转义(`"` 在引号内必须转义);
//! - void 元素:`<name …>` 无结束标签、无子节点;
//! - 非 void 元素:显式 `</name>`(嵌套矫正后的树,重解析即同构);
//! - 注释:`<!--` + 内容去掉所有 `--` + `-->`(防提前闭合)。

use super::escape_str;
use super::tree::{NodeKind, Tree};

/// 树 → HTML。迭代栈(Enter / Exit 两态),深嵌套不栈溢出。
pub(crate) fn serialize(tree: &Tree, out: &mut String) {
    enum Item {
        Enter(usize),
        ExitOwned(String),
    }
    // **root 是透明容器**:它的 `NodeKind` 是 `Text("")` 占位(见 tree.rs),
    // 若把 root 本身 Enter 进栈会匹配到 Text 分支只写空串、**从不遍历
    // 孩子** —— 整篇文档会静默变成空串(D13-006)。故直接以 root 的孩子
    // 为起点。
    let mut stack: Vec<Item> = tree.nodes[tree.root]
        .children
        .iter()
        .rev()
        .map(|&child| Item::Enter(child))
        .collect();
    while let Some(item) = stack.pop() {
        match item {
            Item::Enter(id) => match &tree.nodes[id].kind {
                NodeKind::Text(t) => out.push_str(&escape_str(t)),
                NodeKind::Comment(c) => {
                    // 去掉所有 `--`(防 `-->` 提前闭合)。
                    let body = c.replace("--", "");
                    if body.is_empty() {
                        // 规范写法。`<!-->` 也能被读成空注释,但那**正是
                        // abrupt closing 的字面形状** —— 输出里不留它。
                        out.push_str("<!---->");
                    } else {
                        out.push_str("<!--");
                        // ⚠🔴 **[D13-025 / N-001]第二道防线:挡 abrupt closing。**
                        // WHATWG 把 `<!-->` / `<!--->` 规定为**空注释**。body 以
                        // `>` 或 `->` 开头时,写出去的 `<!--` + body 与前缀拼成
                        // abrupt closing,浏览器重解析时注释在此结束、body 后续
                        // 内容落回 data state 成为活标记。前置**一个空格**即可拆开
                        // 该序列且不丢内容。
                        //
                        // ⚠ 与 `tokenize.rs` 那道闸的关系:分词侧已按 WHATWG 消费掉
                        //   `<!-->` / `<!--->`,所以**走分词器进来时 body 不可能以
                        //   这两种形态开头**,本分支在正常路径上够不到 —— 它守的是
                        //   「这个函数自己的输出不变量」,不依赖调用方是谁。
                        //   下方 `comment_output_never_contains_an_abrupt_closing`
                        //   直接构造树来钉它,免得它退化成一段没人验证的死代码。
                        if body.starts_with('>') || body.starts_with("->") {
                            out.push(' ');
                        }
                        out.push_str(&body);
                        out.push_str("-->");
                    }
                }
                NodeKind::RawText { .. } => {
                    // 过滤阶段必然整删;走到这里说明过滤器有洞 —— 直接吞,
                    // 并让幂等性测试抓住上游。
                }
                NodeKind::Element(el) => {
                    out.push('<');
                    out.push_str(&el.name);
                    for a in &el.attrs {
                        out.push(' ');
                        out.push_str(&a.name);
                        out.push_str("=\"");
                        out.push_str(&escape_str(&a.value));
                        out.push('"');
                    }
                    if super::tree::VOID.contains(&el.name.as_str()) {
                        continue; // void:无结束标签、无子节点
                    }
                    out.push('>');
                    stack.push(Item::ExitOwned(el.name.clone()));
                    for &child in tree.nodes[id].children.iter().rev() {
                        stack.push(Item::Enter(child));
                    }
                }
            },
            Item::ExitOwned(name) => {
                out.push_str("</");
                out.push_str(&name);
                out.push('>');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tree::Node;
    use super::*;

    /// 一棵只含一条注释的树(root 是合成根,不进输出)。
    fn tree_of_comment(body: &str) -> Tree {
        Tree {
            nodes: vec![
                Node {
                    kind: NodeKind::Text(String::new()),
                    parent: None,
                    children: vec![1],
                },
                Node {
                    kind: NodeKind::Comment(body.to_string()),
                    parent: Some(0),
                    children: vec![],
                },
            ],
            root: 0,
        }
    }

    fn serialize_comment(body: &str) -> String {
        let t = tree_of_comment(body);
        let mut out = String::new();
        serialize(&t, &mut out);
        out
    }

    /// [D13-025 / N-001]输出的**结构性**不变量:任何注释体都拼不出
    /// `<!-->` 或 `<!--->` 这两个 abrupt closing 字面形状。
    ///
    /// 这条是**直接构造树**测的,不经过分词器 —— 因为分词侧那道闸(W-11 新增)
    /// 已经让这两种 body 在正常路径上进不来。守的是「`serialize` 自己的输出
    /// 不变量」,不依赖调用方是谁;否则这段代码就是没人验证的死代码。
    #[test]
    fn comment_output_never_contains_an_abrupt_closing() {
        for body in [
            "", ">", "->", ">x", "->x", "-->", "--->", "x>y", "-", "x", "a--b", ">-->",
        ] {
            let out = serialize_comment(body);
            assert!(
                !out.contains("<!-->") && !out.contains("<!--->"),
                "comment {body:?} serialized to {out:?}, which contains an abrupt-closing \
                 sequence — a browser would end the comment there and treat the rest as markup"
            );
        }
    }

    /// `--` 仍然全部剥掉(防 `-->` 提前闭合),且前缀空格只加一次、不丢内容。
    #[test]
    fn comment_body_keeps_its_content_after_hardening() {
        assert_eq!(serialize_comment("x"), "<!--x-->");
        assert_eq!(serialize_comment("a--b"), "<!--ab-->");
        // 前置空格:`>` / `->` 开头时补一个,内容一字不少。
        assert_eq!(serialize_comment(">x"), "<!-- >x-->");
        assert_eq!(serialize_comment("->x"), "<!-- ->x-->");
        // 空注释用规范写法,不留 `<!-->` 这个字面形状。
        assert_eq!(serialize_comment(""), "<!---->");
        assert_eq!(serialize_comment("--"), "<!---->");
    }
}
