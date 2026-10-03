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
                    out.push_str("<!--");
                    out.push_str(&c.replace("--", ""));
                    out.push_str("-->");
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
