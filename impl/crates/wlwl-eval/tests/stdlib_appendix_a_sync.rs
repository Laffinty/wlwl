//! [v0.11.4 / v1.0 规范 §0.7] 标准库附录 A:冻结快照 + 抄件的一致性守卫。
//!
//! v1.0 规范仓的**注册镜像册**附录 A 是**冻结快照**:单源真相是规范自身,
//! 表体**不得**由实现清单自动生成,更**不得**为追平实现的当次改动而改写
//! (注册镜像册 §0.7 规则 1)。⇒ 本仓**没有**「跑一次生成器就把规范改掉」
//! 这条路径;本仓只持有一份**抄件**(`docs/wlwl-spec-reference.md` 附录 A
//! 标记区),它由规范仓的快照**逐字抄来**。
//!
//! 本文件守两个问题(两条守卫互不替代):
//! - **守卫 A(抄件忠实度)**:抄件 == 规范仓注册镜像册的附录 A,**逐字节**;
//! - **守卫 B(实现与期望面一致)**:实现的成员集合 == **附录 A ∪ 台账**的成员
//!   集合,逐命名空间比**集合**(不比顺序、不比 `[stub]` 标记、不比「层」「册」
//!   两列 —— §0.7 规则 3 说的是「集合相等」,规则 2 说的是差异走一致性册
//!   §5.4 的**差异登记**、由实现侧收敛,**不**由快照跟随实现漂移)。
//!
//! ## 期望面 = **附录 A ∪ 台账**(一致性册 §5.5 规则 4)
//!
//! 规范一致性册 §5.5「实现侧扩展台账」给**规范未认可**的扩展一个**合法
//! 落点**:没有落点,开发者就只能去改冻结快照(注册镜像册 §0.7 规则 1 **明禁**
//! 的事)。规则 4:实现侧的门禁按「**附录 A ∪ 台账**」判集合相等 ⇒ 扩展有
//! 合法落点 ⇒ **不必**偷改冻结快照。
//!
//! ⇒ 守卫 B 的比较对象是**抄件 ∪ 台账**,不是抄件本身。
//! ⚠ 守卫 A **不**因此放松:抄件仍**逐字节**等于冻结快照 —— 台账**不**进抄件
//!   的附录 A 标记区,也**不**许让抄件去追平实现。
//! ⚠ 规则 5:台账条目**不得**被表述为「规范已支持」,**不得**被工具列进
//!   「公共面」⇒ 这里算的只是**期望面**,不是公共面清单。
//! ⚠ 本版(v1.0.2)台账**为空** ⇒ 期望面 == 抄件 ⇒ **今日的判词与「只用
//!   抄件」逐字相同**;本改动落的是**机制**。
//!
//! **规范仓找不到 ⇒ 响亮地失败**(panic),**绝不回落到仓内的任何副本**:
//! 回落会让守卫**静默减弱**(读数是绿的,实际没查)。同 `wlwl-error` 的
//! `spec_path()`。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const BEGIN: &str = "<!-- appendix-a:begin -->";
const END: &str = "<!-- appendix-a:end -->";

/// 本仓的合并册(抄件所在处)。
///
/// CARGO_MANIFEST_DIR = impl/crates/wlwl-eval;仓库根的 docs/ 在三级之上。
fn merged_spec_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/wlwl-spec-reference.md")
}

/// 规范仓里某个文件的定位(**同一个约定**:环境变量 ⇒ 同级检出)。
///
/// 查找顺序(与 `wlwl-error` 的 `spec_path()` 同款):
/// 1. `WLWL_SPEC_V1_DIR` 环境变量(CI 与多工作区开发用)—— 指向**规范仓的
///    检出目录**,不是某一册文件;
/// 2. 同级检出 `../wlwl-spec-v1`。
///
/// **没有回落。** 找不到 ⇒ 返 `None` ⇒ 由调用方**响亮地失败**(panic):落到
/// 仓内的任何副本都会让检查**静默失效**(读数绿,实际没查)—— 宁可响亮地失败。
fn spec_repo_file(relative: &str) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WLWL_SPEC_V1_DIR") {
        let p = Path::new(&dir).join(relative);
        if p.exists() {
            return Some(p);
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    let sibling = root.join("..").join("wlwl-spec-v1").join(relative);
    if sibling.exists() {
        return Some(sibling);
    }
    None
}

/// 规范仓的**注册镜像册**(冻结快照所在处)。
///
/// **没有回落。** 找不到 ⇒ panic(文案见下)。
fn spec_repo_registry_path() -> PathBuf {
    match spec_repo_file("docs/stdlib/wlwl-v1.0-stdlib-registry.md") {
        Some(p) => p,
        None => panic!(
            "找不到 v1.0 规范仓 —— 附录 A 是规范仓的**冻结快照**(注册镜像册 §0.7),\
             本仓只持有一份抄件,抄件的忠实度必须对着**规范仓那一份**比。\n\
             二选一:\n\
             1. 设 WLWL_SPEC_V1_DIR 指向 wlwl-spec-v1 的检出目录\n\
             (CI 里已由 spec checkout 步骤提供 —— 见 .github/workflows/ci.yml);\n\
             2. 把 wlwl-spec-v1 检出到本仓的同级目录。\n\
             ⚠ 没有回落:落到仓内的任何副本都会让这条守卫静默失效(读数绿,实际没查)。"
        ),
    }
}

/// 规范仓的**一致性册**(实现侧扩展台账所在处)。
///
/// ⚠ 定位**同款**(`WLWL_SPEC_V1_DIR` ⇒ 同级检出)⇒ 附录 A 与台账**永远**
///   来自**同一次**检出 ⇒ 两者不会来自两个不同版本的规范仓。
/// **没有回落。** 找不到 ⇒ panic(与 [`spec_repo_registry_path`] 同款:回落
///   会让这条守卫**静默减弱**)。
fn spec_repo_conformance_path() -> PathBuf {
    match spec_repo_file("docs/spec/wlwl-v1.0-conformance.md") {
        Some(p) => p,
        None => panic!(
            "找不到 v1.0 规范仓 —— 实现侧扩展台账在规范仓的**一致性册 §5.5**里\n\
             (与附录 A 是同一册的两处),本仓只持有附录 A 的抄件,\n\
             台账必须对着**规范仓那一份**读。\n\
             二选一:\n\
             1. 设 WLWL_SPEC_V1_DIR 指向 wlwl-spec-v1 的检出目录\n\
             (CI 里已由 spec checkout 步骤提供 —— 见 .github/workflows/ci.yml);\n\
             2. 把 wlwl-spec-v1 检出到本仓的同级目录。\n\
             ⚠ 没有回落:落到仓内的任何副本都会让这条守卫静默失效(读数绿,实际没查)。"
        ),
    }
}

/// 归一化行尾后再比对。
///
/// 这**不是**把断言放松:附录 A 的内容仍然是逐字比对,只是先把「本次
/// checkout 落成 CRLF 还是 LF」这个**平台产物**排除掉。
///
/// 起因(GitHub Actions run #168,2026-10-01,windows-latest):Windows runner
/// 的 git 默认 `core.autocrlf=true`,于是 `docs/**.md` 在那里被检出成
/// `\r\n`,而生成器 `gen_appendix_a_md()` 产出的是 `\n` —— 逐字节比对必然
/// 不等,同一个 commit 在 ubuntu / macos 上却全绿。本地 Windows 之所以
/// 复现不出来,是本地那份 checkout 恰好是 LF。
///
/// 同一根源还让 `wl/std/*.wll` 在仓里混了行尾(HEAD 上 `collection.wll` /
/// `math.wll` / `str.wll` 是 CRLF 而 `test.wll` 是 LF)。已由
/// `.gitattributes` 的 `eol=lf` 规则一并消除,但**测试这一层仍要归一化** ——
/// 已有 checkout 不会因为改了 `.gitattributes` 就重新落盘,而新克隆之外的
/// 开发者也可能带着自定义 `core.autocrlf`。
fn normalized(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}

/// 抄件里附录 A 的表体(两个标记之间的**全部**内容)。
///
/// 取的是**标记之间的每一个字节**,不做「只取表格行」的筛选 —— 那样一来,
/// 有人往标记区里塞一段别的正文也会被悄悄放过。
fn copy_body(md: &str) -> String {
    let start = md
        .find(BEGIN)
        .expect("appendix-a begin marker missing from the merged spec");
    let rel_end = md[start..]
        .find(END)
        .expect("appendix-a end marker missing from the merged spec");
    md[start + BEGIN.len()..start + rel_end].to_string()
}

/// 规范仓注册镜像册里附录 A 的表体:`## 附录 A` 之后的**连续表格行**
/// (表头行 + 分隔行 + 全部数据行)。
///
/// 取法与 `script/build_spec_reference.py` 的 `mark_appendix_a` 同款:表体
/// 在规范仓那边**不带**本仓的定位标记(那里是冻结快照),所以只能靠
/// 「`## 附录 A` 之后的连续表格行」把它认出来。
fn snapshot_body(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let heading = lines
        .iter()
        .position(|l| l.trim_start().starts_with("## 附录 A"))
        .expect("the v1.0 registry booklet has no `## 附录 A` section");
    let mut rows: Vec<&str> = Vec::new();
    for line in lines.iter().skip(heading + 1) {
        let t = line.trim();
        if t.starts_with('|') {
            rows.push(t);
        } else if !rows.is_empty() {
            break; // 表格结束(后面是别的正文)
        }
    }
    assert!(
        !rows.is_empty(),
        "the v1.0 registry booklet's `## 附录 A` section has no table"
    );
    rows.join("\n")
}

/// 表体 → (命名空间 → 成员集合)。
///
/// 只认**命名空间格被反引号包住**的行:表头行与分隔行的命名空间格不是
/// 反引号形式,自动被跳过(分隔行是 `|---|---|---|---|` —— 它以 `|-` 开头,
/// 不是 `| `)。
///
/// `[stub]` 记号**不是**成员名(§0.7 规则 3:它是集合内某个成员的附加
/// 属性),故排除;「层」「v1.0 册」两列不参与成员集合。
fn member_sets(body: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in body.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = t.trim_matches('|').split('|').collect();
        if cells.len() != 4 {
            continue;
        }
        let ns_raw = cells[0].trim();
        if !(ns_raw.starts_with('`') && ns_raw.ends_with('`')) {
            continue; // 表头行 / 分隔行
        }
        let namespace = ns_raw.trim_matches('`').to_string();
        let members = cells[1]
            .split_whitespace()
            .map(|tok| tok.trim_matches('`'))
            .filter(|name| !name.starts_with('['))
            .map(|name| name.to_string())
            .collect();
        out.insert(namespace, members);
    }
    out
}

// ── 实现侧扩展台账(一致性册 §5.5)──────────────────────────────────────────
//
// ⚠ 台账与附录 A **同源不同册**:附录 A 在「注册镜像册」,台账在「一致性册」
//   §5.5 ⇒ 两者**分别**取、**分别**判。
// ⚠ 为什么要读台账:一致性册 §5.5 规则 4 —— 实现侧的门禁按「**附录 A ∪
//   台账**」判集合相等 ⇒ 规范未认可的扩展有**合法落点** ⇒ **不必**偷改
//   冻结快照(注册镜像册 §0.7 规则 1 **明禁**改它)。
// ⚠ 规则 5:台账条目**不得**被表述为「规范已支持」,**不得**被工具列进
//   「公共面」⇒ 本文件算的只是**期望面**,不是公共面清单。

/// 一致性册 §5.5 的**台账表体**:`### 5.5` 一节里紧跟「**台账表体**:」的
/// 那**一张**表。
///
/// ⚠ **窄**:① 先定位 `### 5.5` 这一节(按编号精确定位 —— `### 5.50` 不算);
///   ② 本节内找「**台账表体**:」这个**锚点**;③ 锚点之后的第一张表 = 连续的
///   表格行 ⇒ 一致性册里另外十几张表(§5.4 的差异登记表、§6.2 的待验证清单
///   表、……)**一律扫不进来**。
fn ledger_body(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    // ① `### 5.5` 这一节。
    let heading = lines
        .iter()
        .position(|l| {
            let t = l.trim_start();
            match t.strip_prefix("### 5.5") {
                // ⚠ 后面**不能**是数字或点 ⇒ `### 5.50` / `### 5.5.1` 都**不算**。
                Some(rest) => !rest.starts_with(|c: char| c.is_ascii_digit() || c == '.'),
                None => false,
            }
        })
        .expect("the v1.0 conformance booklet has no `### 5.5` section");
    // ② 本节的范围:到下一个 `### ` / `## ` 标题为止。
    let section_end = lines
        .iter()
        .skip(heading + 1)
        .position(|l| {
            let t = l.trim_start();
            t.starts_with("### ") || t.starts_with("## ")
        })
        .map(|i| heading + 1 + i)
        .unwrap_or(lines.len());
    // ③ 本节内的「**台账表体**:」锚点。
    let anchor = lines[heading + 1..section_end]
        .iter()
        .position(|l| l.trim().starts_with("**台账表体**"))
        .map(|i| heading + 1 + i)
        .expect("the v1.0 conformance booklet's `### 5.5` has no `**台账表体**:` anchor");
    // ④ 锚点之后的第一张表 = 连续的表格行。
    let mut rows: Vec<&str> = Vec::new();
    for line in lines[anchor + 1..section_end].iter() {
        let t = line.trim();
        if t.starts_with('|') {
            rows.push(t);
        } else if !rows.is_empty() {
            break; // 表格结束(后面是别的正文)
        }
    }
    assert!(
        !rows.is_empty(),
        "the v1.0 conformance booklet's `### 5.5` ledger table has no rows"
    );
    rows.join("\n")
}

/// 台账表体 → (命名空间 → 成员集合)。
///
/// ⚠ **只取「命名空间」「成员」两列**:台账还有「登记理由」「效力声明」
///   「随哪个实现仓发布」「收敛路线」四列 ⇒ **不参与**成员集合。
/// ⚠ **只认命名空间格被反引号包住的行**:表头行、分隔行、以及 §5.5 现在的
///   「(本版无条目)」占位行 ⇒ 都**不是**反引号形式 ⇒ 自动被跳过
///   ⇒ **本版台账为空 ⇒ 期望面与抄件逐字相同**。
/// ⚠ 与 [`member_sets`](附录 A,四列)**分开写**:两张表列数不同 ⇒ 共用一个
///   解析器就必然有一边要「凑」。
fn ledger_member_sets(body: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in body.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = t.trim_matches('|').split('|').collect();
        if cells.len() < 2 {
            continue;
        }
        let ns_raw = cells[0].trim();
        if !(ns_raw.starts_with('`') && ns_raw.ends_with('`')) {
            continue; // 表头行 / 分隔行 / 空占位行
        }
        let namespace = ns_raw.trim_matches('`').to_string();
        let members: BTreeSet<String> = cells[1]
            .split_whitespace()
            .map(|tok| tok.trim_matches('`'))
            .filter(|name| !name.is_empty() && !name.starts_with('[') && *name != "—")
            .map(|name| name.to_string())
            .collect();
        if members.is_empty() {
            continue;
        }
        out.entry(namespace).or_default().extend(members);
    }
    out
}

/// 读规范仓一致性册 §5.5 的**实现侧扩展台账**。
///
/// ⚠ 规范仓找不到 ⇒ **响亮地失败**(panic)—— 与附录 A 快照同款:回落会让
///   守卫**静默减弱**(读数是绿的,实际没查)。
fn read_ledger() -> BTreeMap<String, BTreeSet<String>> {
    let md = fs::read_to_string(spec_repo_conformance_path())
        .expect("the v1.0 conformance booklet is readable");
    ledger_member_sets(&ledger_body(&md))
}

/// **期望面** = 抄件(附录 A)∪ 扩展台账(一致性册 §5.5)。
///
/// ⚠ §5.5 规则 4:实现侧的门禁按「**附录 A ∪ 台账**」判集合相等 ⇒ 扩展有
///   **合法落点** ⇒ **不必**偷改冻结快照。
/// ⚠ §5.5 规则 5:台账条目**不得**被表述为「规范已支持」,**不得**被工具
///   列进「公共面」⇒ 这里只是**期望面**,不是公共面清单。
fn expected_member_sets(
    copy: &str,
    ledger: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = member_sets(copy);
    for (ns, members) in ledger {
        out.entry(ns.clone())
            .or_default()
            .extend(members.iter().cloned());
    }
    out
}

/// 抄件 vs 快照的差异(归一化行尾后逐行比)。**相同 ⇒ `None`**。
fn fidelity_diff(copy: &str, snapshot: &str) -> Option<Vec<String>> {
    let a = normalized(copy);
    let b = normalized(snapshot);
    if a == b {
        return None;
    }
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut out: Vec<String> = Vec::new();
    for i in 0..al.len().max(bl.len()) {
        let x = al.get(i).copied();
        let y = bl.get(i).copied();
        if x == y {
            continue;
        }
        out.push(format!(
            "line {}:\n    copy:     {}\n    snapshot: {}",
            i + 1,
            x.unwrap_or("<missing>"),
            y.unwrap_or("<missing>")
        ));
    }
    if out.is_empty() {
        out.push(format!(
            "the two bodies differ (copy {} bytes vs snapshot {} bytes) but no line differs",
            a.len(),
            b.len()
        ));
    }
    Some(out)
}

/// 期望面(附录 A ∪ 台账)vs 实现的成员集合差异。**相同 ⇒ `None`**。
fn member_set_diff(
    expected: &BTreeMap<String, BTreeSet<String>>,
    impl_sets: &BTreeMap<String, BTreeSet<String>>,
) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for ns in impl_sets.keys() {
        if !expected.contains_key(ns) {
            out.push(format!(
                "namespace `{ns}` is in the implementation but missing from the expected set \
                 (Appendix A ∪ the §5.5 extension ledger)"
            ));
        }
    }
    for ns in expected.keys() {
        if !impl_sets.contains_key(ns) {
            out.push(format!(
                "namespace `{ns}` is in the expected set (Appendix A ∪ the §5.5 extension \
                 ledger) but not in the implementation"
            ));
        }
    }
    for (ns, members) in impl_sets {
        let Some(expected_members) = expected.get(ns) else {
            continue;
        };
        let missing: Vec<&str> = members
            .difference(expected_members)
            .map(String::as_str)
            .collect();
        let extra: Vec<&str> = expected_members
            .difference(members)
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            out.push(format!(
                "namespace `{ns}`: the implementation has member(s) the expected set lacks: {missing:?}"
            ));
        }
        if !extra.is_empty() {
            out.push(format!(
                "namespace `{ns}`: the expected set has member(s) the implementation lacks: {extra:?}"
            ));
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// 删掉表体里的**第一行数据行**(负向自检专用的改坏手段)。
///
/// 用它是因为它**一定**改动了表体 —— 负向自检最怕的就是「因为没改到
/// 东西」而空转,读数是红的,其实什么也没验。
fn without_first_data_row(body: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut dropped = false;
    for line in body.lines() {
        let t = line.trim();
        let is_data_row = t.starts_with("| `") && t.trim_matches('|').split('|').count() == 4;
        if is_data_row && !dropped {
            dropped = true;
            continue;
        }
        out.push(line);
    }
    assert!(dropped, "the probe found no data row to drop");
    out.join("\n")
}

/// 读合并册(抄件)。
fn read_copy() -> String {
    copy_body(
        &fs::read_to_string(merged_spec_path()).expect("the merged spec is readable from the repo"),
    )
}

/// 读规范仓的冻结快照。
fn read_snapshot() -> String {
    snapshot_body(
        &fs::read_to_string(spec_repo_registry_path())
            .expect("the v1.0 registry booklet is readable"),
    )
}

/// 守卫 A(抄件忠实度):合并册的附录 A 表体 == 规范仓注册镜像册的附录 A 表体,
/// **逐字节**。
#[test]
fn the_copy_is_a_faithful_transcript_of_the_frozen_snapshot() {
    let copy = read_copy();
    let snapshot = read_snapshot();
    if let Some(diff) = fidelity_diff(&copy, &snapshot) {
        panic!(
            "the appendix A copy in the merged spec is NOT byte-identical to the frozen \
             snapshot in the v1.0 spec registry booklet:\n{}\n\
             The snapshot is the truth (§0.7) — do NOT regenerate it from the \
             implementation; re-copy it from the spec repo.",
            diff.join("\n")
        );
    }
}

/// 守卫 B(实现与期望面一致):实现的成员集合 == **附录 A ∪ 台账**的成员集合
/// (逐命名空间比集合)。
///
/// ⚠ 比较对象 = **抄件 ∪ 台账**:抄件是附录 A 的逐字节副本(守卫 A 已钉死),
///   台账是规范仓一致性册 §5.5 的实现侧扩展登记 ⇒ 两者**合起来**才是期望面
///   (一致性册 §5.5 规则 4:「实现侧的门禁按『附录 A ∪ 台账』判集合相等」)。
/// ⚠ 拿**抄件**当比较对象的一半,是为了让「抄件走样」与「实现漂移」两种
///   失败**不会互相掩盖**。
/// ⚠ §5.5 规则 5:台账条目**不得**被表述为「规范已支持」,**不得**被工具
///   列进「公共面」⇒ 这里算的只是**期望面**,不是公共面清单。
#[test]
fn the_implementation_member_sets_match_the_frozen_snapshot() {
    let copy = read_copy();
    let ledger = read_ledger();
    let expected = expected_member_sets(&copy, &ledger);
    let impl_sets: BTreeMap<String, BTreeSet<String>> =
        wlwl_eval::stdlib_mirror::implementation_member_sets()
            .into_iter()
            .collect();
    if let Some(diff) = member_set_diff(&expected, &impl_sets) {
        panic!(
            "the implementation's member sets differ from the expected set \
             (Appendix A ∪ the §5.5 extension ledger):\n{}\n\
             The snapshot is the truth (§0.7 rule 2): register the difference in the \
             v1.0 conformance booklet §5.4 and converge the implementation side — \
             do NOT let the snapshot follow the implementation.",
            diff.join("\n")
        );
    }
}

/// 守卫 A 的**负向自检**:比对逻辑能抓住差异(不是恒真断言)。
///
/// 为什么值得守:一条「逐字节相等」的断言最常见的失效形态是**两边取的根
/// 本不是同一段东西**(提取器退化成「取不到就返回空」,而空 == 空恒成立)。
/// 这条自检把提取器喂一个**改坏**的表体,要求比对**报出**差异;再喂一份
/// **没改**的,要求**不**报 —— 顺带证明它不是「一律报」。
#[test]
fn the_copy_fidelity_check_actually_rejects_a_difference() {
    let snapshot = read_snapshot();
    let broken = without_first_data_row(&snapshot);
    assert_ne!(
        normalized(&broken),
        normalized(&snapshot),
        "the probe must actually change the body — otherwise this self-check checks nothing"
    );
    assert!(
        fidelity_diff(&broken, &snapshot).is_some(),
        "the fidelity comparison must REJECT a body that differs from the snapshot"
    );
    assert!(
        fidelity_diff(&snapshot, &snapshot).is_none(),
        "the fidelity comparison must ACCEPT an identical body (otherwise it rejects everything)"
    );
}

/// 守卫 B 的**负向自检**:成员集合比对能抓住差异(不是恒真断言)。
///
/// 同上:「集合相等」这种断言最容易退化成「两边都空 ⇒ 相等」。这里喂一个
/// **少了一个命名空间**的抄件,要求比对**报出**差异。
///
/// ⚠ 这里**刻意用空台账**:这条自检要证的是「比对逻辑有鉴别力」,不是
///   「台账的效力」⇒ 台账非空时它不该被台账**救回来**。
#[test]
fn the_member_set_check_actually_rejects_a_difference() {
    let copy = read_copy();
    let no_ledger: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let impl_sets: BTreeMap<String, BTreeSet<String>> =
        wlwl_eval::stdlib_mirror::implementation_member_sets()
            .into_iter()
            .collect();
    let broken = without_first_data_row(&copy);
    assert!(
        member_set_diff(&expected_member_sets(&broken, &no_ledger), &impl_sets).is_some(),
        "the member-set comparison must REJECT a copy that lost a namespace"
    );
    assert!(
        member_set_diff(&expected_member_sets(&copy, &no_ledger), &impl_sets).is_none(),
        "the member-set comparison must ACCEPT an identical copy (otherwise it rejects everything)"
    );
}

/// 一段**假的**一致性册:§5.5 前后各有一张别的表 ⇒ `ledger_body` 必须只取
/// §5.5 里紧跟「**台账表体**:」的那一张。
const FAKE_CONFORMANCE: &str = "\
### 5.4 已登记的规范 ↔ 实现差异

| 编号 | 差异 | 状态 |
|---|---|---|
| X-14 | 别的一张表 | 已实现 |

### 5.5 实现侧扩展台账

**台账的规则**:

1. 台账**不属**公共面;

**台账表体**:

| 命名空间 | 成员 | 登记理由 | 效力声明 | 随哪个实现仓发布 | 收敛路线 |
|---|---|---|---|---|---|
| `std.experimental` | `EXPERIMENTAL` | 理由 | 效力 | 仓 | 路线 |

**说明**:台账表体后面还有别的表。

## 6 验证要求

| 项 | 需要验证的命题 |
|---|---|
| V-02 | 别的表 |
";

/// 守卫 B 的**台账自检**:「附录 A ∪ 台账」这句话**不是空话**。
///
/// ⚠ 为什么值得守:若台账解析器恒返回空集,守卫照样全绿 ⇒ 将来第一个台账
///   条目落进规范时,实现侧会发现**门禁突然红了**,而没人知道机制一直没
///   生效 ⇒ 这里用一段**假的**台账证明:① 解析器**认得出**台账行;② 台账
///   里的成员**真的**进了期望面;③ 台账里的成员**不在**实现里时,比对
///   **报出**差异。
/// ⚠ 另用一段**假的一致性册**证明台账表体**只**取 §5.5 自己那一张表。
#[test]
fn the_extension_ledger_really_reaches_the_expected_member_set() {
    // ① 解析器认得出台账行(命名空间格 + 成员格都是反引号形式)。
    let fake = ledger_member_sets(
        "| `std.experimental` | `EXPERIMENTAL` `EXPERIMENTAL2` | 理由 | 效力 | 仓 | 路线 |",
    );
    assert_eq!(
        fake.get("std.experimental"),
        Some(&BTreeSet::from([
            "EXPERIMENTAL".to_string(),
            "EXPERIMENTAL2".to_string(),
        ])),
        "the ledger parser must read the namespace/member columns — got {fake:?}"
    );
    // ② 台账**并进**期望面。
    let copy = read_copy();
    let expected = expected_member_sets(&copy, &fake);
    assert!(
        expected.contains_key("std.experimental"),
        "the ledger's namespace must reach the expected set (Appendix A ∪ ledger)"
    );
    // ③ 鉴别力:实现里没有 `std.experimental` ⇒ 比对**必须报出**差异。
    let impl_sets: BTreeMap<String, BTreeSet<String>> =
        wlwl_eval::stdlib_mirror::implementation_member_sets()
            .into_iter()
            .collect();
    assert!(
        member_set_diff(&expected, &impl_sets).is_some(),
        "a ledger entry for a namespace the implementation lacks must be reported — \
         otherwise the ledger has no effect on the verdict"
    );
    // ④ 窄:台账表体**只**取 §5.5 里紧跟「**台账表体**:」的那一张表。
    let body = ledger_body(FAKE_CONFORMANCE);
    assert!(
        body.contains("`std.experimental`"),
        "the ledger body must be the §5.5 ledger table — got {body:?}"
    );
    assert!(
        !body.contains("X-14"),
        "the ledger body must not absorb §5.4's difference registry — got {body:?}"
    );
    assert!(
        !body.contains("V-02"),
        "the ledger body must not absorb a later section's table — got {body:?}"
    );
    // ⑤ 今日台账为空 ⇒ 期望面 == 抄件 ⇒ **今日的判词不变**。
    //    ⚠ 这里把「今日为空」写成**前提**而不是**断言**:台账将来有内容时它
    //    自动让位(不假红)⇒ 「台账确实进了期望面」由 ①②③ 钉住。
    let real = read_ledger();
    if real.is_empty() {
        assert_eq!(
            expected_member_sets(&copy, &real),
            member_sets(&copy),
            "an empty ledger must leave the expected set identical to the copy's member sets"
        );
    }
}

#[test]
fn every_namespace_in_the_spec_resolves_in_the_implementation() {
    let md = fs::read_to_string(merged_spec_path()).unwrap();
    for ns in [
        "std.io",
        "std.fs",
        "std.json",
        "std.format",
        "std.collection",
        "std.str",
        "std.math",
        "std.test",
        // [v0.11.3 M8 / addendum-14 / ADR-0034] `std.ai` / `std.agent` 已移出,
        // 不再走 `wlwl:std.*` 前缀 —— 单独断在下方,不跟这个循环回步。
        // ⚠ 怕本来一只只删掉名字、留下一个**空壳**行,那就是「假搬」
        // (`addendum-14` §4 点名的分界判据)。
        // [v0.11.3 M6 / addendum-01 A1] 时间能力第一片。
        "std.time",
        // [v0.11.3 M6 / addendum-02] 正则。
        "std.regex",
    ] {
        assert!(
            md.contains(&format!("`{ns}`")),
            "stdlib spec lost its `{ns}` namespace"
        );
        assert!(
            wlwl_std::resolve(&format!("wlwl:{ns}")).is_some(),
            "`{ns}` is documented in the spec but does not resolve"
        );
    }

    // 官方包行:前缀不是 `wlwl:`(那是 std 的保留前缀),也不在 `wlwl:std.` 下。
    for pkg in ["ai", "agent"] {
        assert!(
            md.contains(&format!("`{pkg}`")),
            "stdlib spec lost its `{pkg}` official-package row"
        );
        assert!(
            wlwl_std::resolve(pkg).is_some(),
            "`{pkg}` is documented as an official package but does not resolve"
        );
        assert!(
            wlwl_std::resolve(&format!("wlwl:std.{pkg}")).is_none(),
            "`wlwl:std.{pkg}` must NOT resolve — an empty SPEC would still resolve, \
             which is exactly the \"fake demotion\" this addendum has to rule out"
        );
    }
}
