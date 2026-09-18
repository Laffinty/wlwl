//! MVS — Minimal Version Selection 依赖求解 (spec v0.4 §13.9, Phase C4)。
//!
//! spec 采纳 Cargo 风格的 MVS:对每个依赖,选择**满足所有约束的最低版本**。
//! 算法骨架(§13.9.1,规范性):
//!
//! ```text
//! for each dep in [dependencies] (拓扑序):
//!     candidates = 满足 dep 约束的版本集合(从注册表 / 路径)
//!     if candidates is empty: error E0045
//!     choose min(candidates) by SemVer
//!     lock.dep = chosen
//!     if chosen has its own dependencies: append to worklist
//! ```
//!
//! v0.4 范围:仅本地 path 依赖 + lock 文件;中央仓库 registry 留 v0.5
//! (构建计划 §3 Phase C C4)。因此本模块是**纯求解库**:版本候选集由
//! 调用方注入的 provider 闭包提供,本 crate 不做任何 IO。eval 侧的
//! v0.4 provider 对版本式依赖返回空候选集 → E0045。
//!
//! 循环依赖(逻辑)→ `E0041`(spec §13.9)。

use std::collections::BTreeMap;
use std::fmt;

/// Semantic version subset: `X.Y.Z`(patch 缺省按 0)。
/// 仅支持数字段;prerelease / build 元数据不在 v0.4 范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVer {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self { major, minor, patch }
    }

    /// Parse `X.Y.Z` / `X.Y` / `X` (missing segments default to 0).
    /// Leading `v` tolerated. Returns `None` on non-numeric segments.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('v');
        let mut it = s.split('.');
        let major = it.next()?.trim().parse::<u64>().ok()?;
        let minor = it.next().map(|v| v.trim().parse::<u64>().ok()).unwrap_or(Some(0))?;
        let patch = it.next().map(|v| v.trim().parse::<u64>().ok()).unwrap_or(Some(0))?;
        Some(Self { major, minor, patch })
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 一个版本约束。支持 spec §13.8 出现过的形式:
///
/// - 精确版:`"1.2.3"`
/// - caret:`"^1.2.3"` → `>=1.2.3, <2.0.0`;`"^0.5.0"` → `>=0.5.0, <0.6.0`
///   (major=0 时 caret 锁 minor,与 Cargo 语义一致)
/// - 比较子:">=1.0" / ">1.0" / "<=2.0" / "<2.0" / "=1.2.3"
/// - 逗号并列(AND):">=1.0, <2.0"
///
/// 空串 = 任意版本。
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    /// AND 连接的比较子列表(全部满足才算满足)。
    pub comps: Vec<Comp>,
    /// 原文(错误信息 / lock 记录用)。
    pub raw: String,
}

/// 单个比较子。`Missing` 字段按 0 补齐(`>=1.0` ≡ `>=1.0.0`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comp {
    Ge(SemVer),
    Gt(SemVer),
    Le(SemVer),
    Lt(SemVer),
    Eq(SemVer),
}

impl Constraint {
    /// Parse a constraint string. Always succeeds structurally; the
    /// empty string means "any version". Unparseable segments become
    /// `None` (the caller surfaces E0045 with the raw text).
    pub fn parse(s: &str) -> Option<Self> {
        let raw = s.trim().to_string();
        if raw.is_empty() {
            return Some(Self { comps: Vec::new(), raw });
        }
        let mut comps = Vec::new();
        for part in raw.split(',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            if let Some(rest) = p.strip_prefix("^") {
                // Caret: `^X.Y.Z` — `>=X.Y.Z` 且 `<` 上界
                // (X>0 → <(X+1).0.0;X=0 → <0.(Y+1).0)。
                let base = SemVer::parse(rest)?;
                let upper = if base.major > 0 {
                    SemVer::new(base.major + 1, 0, 0)
                } else {
                    SemVer::new(0, base.minor + 1, 0)
                };
                comps.push(Comp::Ge(base));
                comps.push(Comp::Lt(upper));
            } else if let Some(rest) = p.strip_prefix(">=") {
                comps.push(Comp::Ge(SemVer::parse(rest)?));
            } else if let Some(rest) = p.strip_prefix("<=") {
                comps.push(Comp::Le(SemVer::parse(rest)?));
            } else if let Some(rest) = p.strip_prefix("==") {
                comps.push(Comp::Eq(SemVer::parse(rest)?));
            } else if let Some(rest) = p.strip_prefix('=') {
                comps.push(Comp::Eq(SemVer::parse(rest)?));
            } else if let Some(rest) = p.strip_prefix('>') {
                comps.push(Comp::Gt(SemVer::parse(rest)?));
            } else if let Some(rest) = p.strip_prefix('<') {
                comps.push(Comp::Lt(SemVer::parse(rest)?));
            } else {
                // 无操作符 = 精确版本。
                comps.push(Comp::Eq(SemVer::parse(p)?));
            }
        }
        Some(Self { comps, raw })
    }

    /// `true` = 任意版本(空约束)。
    pub fn is_any(&self) -> bool {
        self.comps.is_empty()
    }

    pub fn satisfies(&self, v: SemVer) -> bool {
        self.comps.iter().all(|c| match *c {
            Comp::Ge(b) => v >= b,
            Comp::Gt(b) => v > b,
            Comp::Le(b) => v <= b,
            Comp::Lt(b) => v < b,
            Comp::Eq(b) => v == b,
        })
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

/// 求解失败。映射:Conflict → E0045;Cycle → E0041。
#[derive(Debug, Clone, PartialEq)]
pub enum MvsError {
    /// 无候选版本满足约束(spec §13.9:`E0045: dependency conflict`)。
    Conflict {
        dep: String,
        constraint: String,
    },
    /// 循环依赖(spec §13.9:`E0041`)。携带完整环路路径。
    Cycle(Vec<String>),
}

impl fmt::Display for MvsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MvsError::Conflict { dep, constraint } => write!(
                f,
                "dependency conflict: no candidate satisfies constraint '{}' for '{}'",
                constraint, dep
            ),
            MvsError::Cycle(path) => {
                write!(f, "circular dependency detected: {}", path.join(" -> "))
            }
        }
    }
}

/// MVS 求解(spec §13.9.1 骨架)。
///
/// - `root`:根包的依赖约束表(`dep name → Constraint`)。
/// - `candidates`:版本候选集 provider。v0.4 的 eval 侧 provider 对
///   版本式依赖返回空 vec(无中央仓库)。
/// - `deps_of`:传递依赖 provider —— 给定 `(dep, chosen version)`,
///   返回它的依赖约束表;v0.4 无 registry,一般返回空。
///
/// 返回 `dep name → chosen SemVer`(满足所有约束的**最低**版本)。
/// 选择过程:每次 pop 一个未解析依赖,过滤候选 → 空 = Conflict;
/// 取 min;把该版本的传递依赖 append 进 worklist(§13.9.1)。
/// 展开过程中收集依赖边,收尾对依赖图做 DFS 找环 → `E0041`。
pub fn solve(
    root: &BTreeMap<String, Constraint>,
    candidates: &dyn Fn(&str) -> Vec<SemVer>,
    deps_of: &dyn Fn(&str, SemVer) -> BTreeMap<String, Constraint>,
) -> Result<BTreeMap<String, SemVer>, MvsError> {
    let mut chosen: BTreeMap<String, SemVer> = BTreeMap::new();
    // 依赖图(dep → 传递依赖),收尾环检测用。
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // worklist: (dep, constraint)。Vec 当栈用(§13.9.1 顺序扫描)。
    let mut worklist: Vec<(String, Constraint)> =
        root.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

    while let Some((dep, constraint)) = worklist.pop() {
        // 已选定的版本必须仍满足本约束(MVS 单版本语义:约束的交集)。
        if let Some(&v) = chosen.get(&dep) {
            if constraint.satisfies(v) {
                continue;
            }
            return Err(MvsError::Conflict { dep, constraint: constraint.raw });
        }
        let mut cands: Vec<SemVer> = candidates(&dep)
            .into_iter()
            .filter(|v| constraint.satisfies(*v))
            .collect();
        if cands.is_empty() {
            return Err(MvsError::Conflict { dep, constraint: constraint.raw });
        }
        cands.sort();
        let min = cands[0];
        chosen.insert(dep.clone(), min);
        // 传递依赖 append 到 worklist(§13.9.1),同时记录依赖边。
        let transitive = deps_of(&dep, min);
        graph.insert(
            dep.clone(),
            transitive.keys().cloned().collect(),
        );
        for (d, c) in transitive {
            worklist.push((d, c));
        }
    }

    // 循环依赖(逻辑)→ E0041(spec §13.9)。环是**依赖图**的属性,
    // 与展开顺序无关,所以收尾做一次三色 DFS。
    if let Some(cycle) = find_cycle(&graph) {
        return Err(MvsError::Cycle(cycle));
    }
    Ok(chosen)
}

/// 三色 DFS 找环。返回环路路径(`a -> b -> a` 的 Vec,首尾同名)。
fn find_cycle(graph: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    // state: 0 = 未访问,1 = 在当前 DFS 栈上,2 = 完成。
    fn dfs(
        node: &str,
        graph: &BTreeMap<String, Vec<String>>,
        stack: &mut Vec<String>,
        state: &mut BTreeMap<String, u8>,
    ) -> Option<Vec<String>> {
        match state.get(node) {
            Some(2) => return None,
            Some(1) => {
                // 回边:从栈上第一次出现 node 的位置截出环。
                let pos = stack.iter().position(|n| n == node).unwrap_or(0);
                let mut cyc: Vec<String> = stack[pos..].to_vec();
                cyc.push(node.to_string());
                return Some(cyc);
            }
            _ => {}
        }
        state.insert(node.to_string(), 1);
        stack.push(node.to_string());
        if let Some(deps) = graph.get(node) {
            for d in deps.clone() {
                if let Some(c) = dfs(&d, graph, stack, state) {
                    return Some(c);
                }
            }
        }
        stack.pop();
        state.insert(node.to_string(), 2);
        None
    }

    let mut state: BTreeMap<String, u8> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    for node in graph.keys() {
        if !state.contains_key(node) {
            if let Some(c) = dfs(node, graph, &mut stack, &mut state) {
                return Some(c);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, Constraint> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Constraint::parse(v).unwrap()))
            .collect()
    }

    fn versions(vs: &[&str]) -> Vec<SemVer> {
        vs.iter().map(|v| SemVer::parse(v).unwrap()).collect()
    }

    #[test]
    fn semver_parse_and_order() {
        assert_eq!(SemVer::parse("1.2.3"), Some(SemVer::new(1, 2, 3)));
        assert_eq!(SemVer::parse("0.5"), Some(SemVer::new(0, 5, 0)));
        assert_eq!(SemVer::parse("2"), Some(SemVer::new(2, 0, 0)));
        assert_eq!(SemVer::parse("v1.0.0"), Some(SemVer::new(1, 0, 0)));
        assert_eq!(SemVer::parse("banana"), None);
        assert!(SemVer::new(1, 2, 3) < SemVer::new(1, 3, 0));
        assert!(SemVer::new(1, 9, 9) < SemVer::new(2, 0, 0));
    }

    #[test]
    fn constraint_exact_and_caret() {
        let c = Constraint::parse("1.2.3").unwrap();
        assert!(c.satisfies(SemVer::new(1, 2, 3)));
        assert!(!c.satisfies(SemVer::new(1, 2, 4)));

        // ^1.2.3 → >=1.2.3, <2.0.0
        let c = Constraint::parse("^1.2.3").unwrap();
        assert!(c.satisfies(SemVer::new(1, 2, 3)));
        assert!(c.satisfies(SemVer::new(1, 9, 0)));
        assert!(!c.satisfies(SemVer::new(2, 0, 0)));
        assert!(!c.satisfies(SemVer::new(1, 2, 2)));

        // ^0.5.0 → >=0.5.0, <0.6.0 (major=0 锁 minor,Cargo 语义)
        let c = Constraint::parse("^0.5.0").unwrap();
        assert!(c.satisfies(SemVer::new(0, 5, 7)));
        assert!(!c.satisfies(SemVer::new(0, 6, 0)));
    }

    #[test]
    fn constraint_ranges_and_empty() {
        let c = Constraint::parse(">=1.0, <2.0").unwrap();
        assert!(c.satisfies(SemVer::new(1, 5, 0)));
        assert!(!c.satisfies(SemVer::new(2, 0, 0)));
        assert!(!c.satisfies(SemVer::new(0, 9, 0)));

        let any = Constraint::parse("").unwrap();
        assert!(any.is_any());
        assert!(any.satisfies(SemVer::new(99, 0, 0)));
    }

    #[test]
    fn constraint_unparseable_is_none() {
        assert!(Constraint::parse("banana").is_none());
    }

    #[test]
    fn mvs_picks_minimal_satisfying_version() {
        let root = map(&[("a", "^1.0.0")]);
        let cands = versions(&["1.0.0", "1.2.0", "1.9.9", "2.0.0"]);
        let chosen = solve(&root, &|_| cands.clone(), &|_, _| BTreeMap::new()).unwrap();
        // MVS:最低满足版本,不是最高。
        assert_eq!(chosen["a"], SemVer::new(1, 0, 0));
    }

    #[test]
    fn mvs_conflict_when_no_candidate() {
        let root = map(&[("a", "^3.0.0")]);
        let cands = versions(&["1.0.0", "2.0.0"]);
        let err = solve(&root, &|_| cands.clone(), &|_, _| BTreeMap::new()).unwrap_err();
        assert_eq!(
            err,
            MvsError::Conflict { dep: "a".into(), constraint: "^3.0.0".into() }
        );
        assert!(err.to_string().starts_with("dependency conflict:"));
    }

    #[test]
    fn mvs_conflict_when_constraint_intersection_empty() {
        // 两个约束的交集为空:a 要求 ^1.0(同一 dep 出现两次)
        let root = map(&[("a", "^1.0.0"), ("b", "2.0.0")]);
        // b 的传递依赖也要求 a ^1.0,而 root 里 a 还被 ^2.0 约束……
        // 简化:直接让 a 的候选只满足其一。
        let cands = versions(&["1.5.0"]);
        let mut root2 = map(&[("a", "^1.0.0"), ("a", ">=2.0.0")]);
        // BTreeMap 同 key 后写覆盖前写;改用两层约束模拟交集:
        root2.insert("a".into(), Constraint::parse(">=2.0.0, <2.0.0").unwrap());
        let err = solve(&root2, &|_| cands.clone(), &|_, _| BTreeMap::new()).unwrap_err();
        assert!(matches!(err, MvsError::Conflict { .. }));
        let _ = root;
    }

    #[test]
    fn mvs_expands_transitive_deps() {
        // root → a ^1.0;a@1.0 → b ^0.5;b 候选 0.5.1 → 应选出 a=1.0.0, b=0.5.1
        let root = map(&[("a", "^1.0.0")]);
        let a_cands = versions(&["1.0.0", "1.1.0"]);
        let b_cands = versions(&["0.5.1", "0.9.0"]);
        let chosen = solve(
            &root,
            &|name| {
                if name == "a" { a_cands.clone() } else { b_cands.clone() }
            },
            &|name, v| {
                if name == "a" && v == SemVer::new(1, 0, 0) {
                    map(&[("b", "^0.5.0")])
                } else {
                    BTreeMap::new()
                }
            },
        )
        .unwrap();
        assert_eq!(chosen["a"], SemVer::new(1, 0, 0));
        assert_eq!(chosen["b"], SemVer::new(0, 5, 1));
    }

    #[test]
    fn mvs_cycle_is_e0041() {
        // a → b → a 的逻辑环
        let root = map(&[("a", "1.0.0")]);
        let err = solve(
            &root,
            &|_| versions(&["1.0.0"]),
            &|name, _| {
                if name == "a" {
                    map(&[("b", "1.0.0")])
                } else {
                    map(&[("a", "1.0.0")])
                }
            },
        )
        .unwrap_err();
        assert!(matches!(err, MvsError::Cycle(_)));
        assert!(err.to_string().starts_with("circular dependency detected:"));
    }

    #[test]
    fn mvs_empty_root_ok() {
        let chosen = solve(&BTreeMap::new(), &|_| Vec::new(), &|_, _| BTreeMap::new()).unwrap();
        assert!(chosen.is_empty());
    }
}
