//! [v0.11.3 W-07] HTML5 命名实体全表生成器。
//!
//! 数据源:<https://html.spec.whatwg.org/entities.json>(WHATWG 官方单文件
//! JSON;本批取于 2026-10-03:2231 项 = 2125 分号收尾 + 106 legacy 无分号)。
//!
//! 用法(仓库根的 impl/ 目录下):
//!
//! ```text
//! cargo run -p wlwl-std --bin gen-entities -- <entities.json 路径> \
//!     > crates/wlwl-std/src/sanitize/entities.rs
//! ```
//!
//! 纪律:**数据文件不入库,生成器入库**(沿 v0.11.2 D12 惯例 —— UCD 类数据
//! 的生成器与数据必须分开,数据文件里全是不可审阅的转义字节)。重生成后
//! `sanitize_contract` 的冻结向量必须仍然逐字通过:期望值取自规范与 WHATWG
//! 数据,不是实现输出。
//!
//! 表的语义(见 stdlib 规范 §13.1):
//! - `SEMICOLON`:分号收尾的命名实体(全量),消费时要求输入里有 `;`;
//! - `LEGACY`:HTML5 允许**不带分号**匹配的 legacy 子集(106 项);每项都有
//!   同名分号孪生且码点一致(生成时断言,不一致即炸)。

use std::collections::BTreeMap;
use std::io::Write;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: gen-entities <entities.json 路径>");
    let raw = std::fs::read_to_string(&path).expect("entities.json 可读");
    let json: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&raw).expect("entities.json 是 JSON 对象");

    let mut semi: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut legacy: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for (key, v) in &json {
        let name = key
            .strip_prefix('&')
            .unwrap_or_else(|| panic!("{key}: 实体键必须以 '&' 开头"));
        let cps: Vec<u32> = v["codepoints"]
            .as_array()
            .unwrap_or_else(|| panic!("{key}: 缺 codepoints 数组"))
            .iter()
            .map(|c| c.as_u64().expect("码点是 u64") as u32)
            .collect();
        assert!(
            !cps.is_empty() && cps.len() <= 2,
            "{key}: 码点数 {cps:?} 越界"
        );
        match name.strip_suffix(';') {
            Some(n) => semi.insert(n.to_string(), cps),
            None => legacy.insert(name.to_string(), cps),
        };
    }

    // legacy 每项必须同名分号孪生且码点一致 —— 这是 §13.1「分号表是全量、
    // legacy 是它的无分号子集」口径的数据级证明,生成时一次核实。
    for (name, cps) in &legacy {
        let twin = semi
            .get(name)
            .unwrap_or_else(|| panic!("&{name};: legacy 项没有分号孪生"));
        assert_eq!(twin, cps, "&{name};: legacy 与分号孪生码点不一致");
    }

    let mut out = String::new();
    out.push_str("//! [v0.11.3 W-07] HTML5 命名实体全表 —— **由生成器产出,勿手改**。\n");
    out.push_str("//!\n");
    out.push_str(
        "//! 生成:cargo run -p wlwl-std --bin gen-entities -- <entities.json> \\\n//!       > crates/wlwl-std/src/sanitize/entities.rs(在 impl/ 目录下)\n",
    );
    out.push_str("//! 数据源:https://html.spec.whatwg.org/entities.json(WHATWG 官方;\n");
    out.push_str("//! 本表取于 2026-10-03)。\n");
    out.push_str(&format!(
        "//! 口径:SEMICOLON 是分号收尾的全量(共 {} 项);LEGACY 是 HTML5 允许\n",
        semi.len()
    ));
    out.push_str(&format!(
        "//! 不带分号匹配的子集(共 {} 项,每项均有同名分号孪生且码点一致 ——\n",
        legacy.len()
    ));
    out.push_str("//! 生成器断言)。按名称字节序排序,查询走二分。\n\n");

    out.push_str("pub(crate) static SEMICOLON: &[(&str, &[u32])] = &[\n");
    for (name, cps) in &semi {
        out.push_str(&format!("    (\"{name}\", &[{}]),\n", fmt_cps(cps)));
    }
    out.push_str("];\n\n");
    out.push_str("pub(crate) static LEGACY: &[(&str, &[u32])] = &[\n");
    for (name, cps) in &legacy {
        out.push_str(&format!("    (\"{name}\", &[{}]),\n", fmt_cps(cps)));
    }
    out.push_str("];\n\n");
    let max_len = semi
        .keys()
        .chain(legacy.keys())
        .map(|n| n.len())
        .max()
        .unwrap_or(0);
    out.push_str(&format!(
        "/// 实体名(不含 `&` 与结尾 `;`)的最大字节长度;扫描时的游程上限。\npub(crate) const MAX_NAME_LEN: usize = {max_len};\n"
    ));

    std::io::stdout().lock().write_all(out.as_bytes()).unwrap();
}

fn fmt_cps(cps: &[u32]) -> String {
    cps.iter()
        .map(|c| format!("0x{c:04X}"))
        .collect::<Vec<_>>()
        .join(", ")
}
