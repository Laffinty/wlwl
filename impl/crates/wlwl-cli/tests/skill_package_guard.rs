//! **技能包口径守卫** —— v0.12.0 发版审计 F-11…F-22 的机器可执行形式。
//!
//! ## 纪律
//!
//! 技能包（`wlwl-skill/`）是**常驻**给 AI agent 读的：只读 `SKILL.md` 的 agent
//! 拿到的就是它 ⇒ 里面**每一句口径**都必须是**当前**的。审计里那一批
//! 「改了但 agent 读到的还是旧口径」的缺陷（F-11…F-22）全部由**过期读数**
//! 造成 —— 成员面 `214`、一致性册差异登记只到 `X-15`、`std.fs` 仍写「用
//! `E0060`–`E0062` 诊断」，以及并发「硬规则」把规范 §17.0.1 **明令禁止**的
//! 共享可变 cell 写成推荐做法。
//!
//! ## 为什么要有这条守卫
//!
//! 散文纪律会烂掉 —— 半年后没人记得，新来的人照着旧句子抄。本仓的体例是
//! 「只有会红的测试挡得住」，所以这条纪律配一条**会红**的守卫。
//!
//! ## 判据
//!
//! **硬证据**：正文里出现那串过期口径的**字面量**即违规。取字面量而不取
//! 「看起来像过期」这类代理指标 —— 代理指标会误报，而误报的守卫会被人加
//! 豁免，加了豁免就等于没有守卫。
//!
//! ⚠ `CHANGELOG.md` **不在扫描范围内**：它是**历史记录**，记的是「当时是
//! 什么」；改历史记录比留旧数字更糟。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 仓库根（`CARGO_MANIFEST_DIR` = `impl/crates/wlwl-cli`）。
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

/// 被扫的文件（仓根相对，`/` 分隔）。
///
/// ⚠ `wlwl-skill/CHANGELOG.md` **不在其中** —— 历史记录里的旧数字是**事实**，
/// 不是过期口径。
const SCANNED: &[&str] = &[
    "wlwl-skill/SKILL.md",
    "wlwl-skill/reference.md",
    "wlwl-skill/README.md",
];

/// 已确认的**过期口径**字面量 —— 每条 = 审计里一条已核实的缺陷。
///
/// ⚠ `E0060` 那条**连带**盖住两种连字符写法（`E0060`–`E0062` 与
/// `E0060`-`E0062`）：那两串里都含 `E0060`。两条区间写法仍单列出来，是为了让
/// 「这两种写法都算」这件事在守卫里**看得见**。
const FORBIDDEN: &[(&str, &str)] = &[
    (
        "214 members",
        "F-20：成员面旧读数。**当前实现成员面**是 `wlwl namespace` 实测的 \
         288 members / 25 namespaces；规范附录 A 是 v1.0 **冻结快照**（注册镜像册 \
         §0.7 规则 1：不随实现漂移）⇒ 两个口径，别混着写",
    ),
    (
        "231 members",
        "F-20：把 231 当成当前成员面读数 —— 那是 v0.12.0 时期的读数。当前实现是 \
         `wlwl namespace` 实测的 288 members / 25 namespaces",
    ),
    ("231 / 21", "F-20：同上（旧读数成对出现的写法）"),
    (
        "214 → 231",
        "F-20：把版本差量当成活读数写进正文 —— 差量属 `CHANGELOG.md`（历史记录）",
    ),
    ("214→231", "F-20：同上（无空格写法）"),
    (
        "21 命名空间",
        "F-20：命名空间旧读数（现 25 个：`wlwl:std.*` 23 + 官方包 `ai` / `agent` 2）",
    ),
    ("21 个命名空间", "F-20：同上"),
    ("21 namespaces", "F-20：同上（英文写法）"),
    (
        "命名空间仍是 21",
        "F-20：同上（「仍是 21」这种把旧读数写成现状的句式）",
    ),
    ("X-01…X-15", "F-20：一致性册 §5.4 的差异登记现到 X-25"),
    (
        "E0060",
        "F-13：`std.fs` 的 IO 失败自 v0.12.0 起是 `ERR` 值，不再产原生诊断",
    ),
    (
        "E0060–E0062",
        "F-13：`std.fs` 的 IO 失败自 v0.12.0 起是 `ERR` 值，不再产原生诊断",
    ),
    (
        "E0060-E0062",
        "F-13：`std.fs` 的 IO 失败自 v0.12.0 起是 `ERR` 值，不再产原生诊断",
    ),
    (
        "Shared state = ",
        "F-11：并发硬规则把规范 §17.0.1 明令禁止的共享可变 cell 写成推荐做法",
    ),
    // ── H-2 / H-3 口径订正：`STR` / `SPLIT` 是**全局内建**，不是 `std.str` 成员 ──
    // 旧说法是「没有数字→字符串」/「没有通用 SPLIT」。两者附录 G 早有：
    // `STR(x)`（§10.3）、`SPLIT(s, sep)`（§10.5）。
    (
        "没有数字→字符串",
        "H-2：数字 → 字符串是全局内建 `STR`（附录 G §10.3），早就有",
    ),
    ("没有数字 → 字符串", "H-2：同上（箭头两侧带空格的写法）"),
    ("没有数字到字符串", "H-2：同上（「到」字写法）"),
    ("no numeric-to-string", "H-2：同上（英文写法）"),
    (
        "没有通用 SPLIT",
        "H-3：通用切分是全局内建 `SPLIT(s, sep)`（附录 G §10.5），早就有",
    ),
    ("没有通用 `SPLIT`", "H-3：同上（成员名带反引号的写法）"),
    ("no general SPLIT", "H-3：同上（英文写法）"),
    // ── `+` **不做任何隐式强转** ──────────────────────────────────────────
    // 实测：`+(1, "a")` / `+("a", 1)` 都是 `E0030`，提示原文含
    // 「v0.3 has no implicit numeric coercion」。数字 → 字符串走全局内建 `STR`，
    // 变长拼接走 `std.str::CAT`。⇒ 反向守卫就拿第一条针做（写回 SKILL.md 必红）。
    (
        "`+` 会把数字转成字符串",
        "H-2：`+` 不做任何隐式强转（`+(1, …)` 是 E0030）；数字 → 字符串走全局内建 `STR`",
    ),
    ("`+` 会把数字转为字符串", "H-2：同上（「转为」写法）"),
    ("`+` 会把数字转字符串", "H-2：同上（省略「成」字的写法）"),
    ("`+` casts numbers to strings", "H-2：同上（英文写法）"),
    (
        "`+` coerces numbers to strings",
        "H-2：同上（英文 coerce 写法）",
    ),
    // ── H-1：`IF` 第三实参**支持**多语句组 ────────────────────────────────
    // 实测（v0.12.1 收口前夜）：`IF(c, 1, (LET(a,1); LET(b,2); +(a,b)))` 在
    // `c = FALSE` 时求值为 `3`。旧说法「第三实参不能装多语句组」是过期口径。
    (
        "cannot hold a multi-statement group",
        "H-1：`IF` 的第三实参**支持**多语句组 `(stmt1; stmt2)`（本轮已补回归守护）",
    ),
    ("不能装多语句组", "H-1：同上（中文写法）"),
];

/// `SKILL.md` 正文里必须**能**搜到的字样 —— 「无人 `AWAIT` 的任务失败」这一节
/// 是 F-18 的补齐项，删掉它就等于把这条覆盖缺口放回去。
///
/// 三种写法任一命中即算通过（中文 / 英文都接受）。
const REQUIRED_IN_SKILL: &[&str] = &["孤儿", "orphan", "unhandled task failure"];

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("`{rel}` must be readable: {e}"))
}

/// 命中行号（1 起）。同一行可能被多条针命中。
fn hits(text: &str, needle: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(i, _)| i + 1)
        .collect()
}

/// 技能包里**不得**再出现的过期口径。
///
/// 断言取**硬证据**（那串字面量），不取代理指标 —— 理由见文件头。
#[test]
fn the_skill_bundle_carries_no_retired_readings() {
    // 负向自检（先）：探测器必须对每条针**有反应** ——
    // 否则这条守卫会「因为没扫到而一直绿」，那是它最危险的失效模式。
    for (needle, why) in FORBIDDEN {
        let sample = format!("prologue\n{needle}\nepilogue\n");
        assert!(
            !hits(&sample, needle).is_empty(),
            "探测器对 `{needle}` 没有反应 —— 守卫会因「没扫到」而永远绿（{why}）",
        );
    }

    // 正向扫描：三个文件必须真的被读到（非空），否则「零命中」是假的。
    let mut offenders: Vec<String> = Vec::new();
    for rel in SCANNED {
        let text = read(rel);
        assert!(
            text.len() > 1000,
            "`{rel}` 只读到 {} 字节 —— 扫描器多半没扫到东西，「零命中」是假的",
            text.len(),
        );
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        for (needle, why) in FORBIDDEN {
            for line_no in hits(&text, needle) {
                if seen.insert(line_no) {
                    offenders.push(format!("  {rel}:{line_no}  «{needle}» — {why}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "技能包里仍有 {} 处过期口径 —— agent 读到的还是旧版本：\n{}\n\
         （`wlwl-skill/CHANGELOG.md` 是历史记录，不在扫描范围内）",
        offenders.len(),
        offenders.join("\n"),
    );

    // 正向断言：F-18 补齐的那一节必须还在 `SKILL.md` 正文里。
    let skill = read("wlwl-skill/SKILL.md");
    let skill_lower = skill.to_lowercase();
    assert!(
        REQUIRED_IN_SKILL
            .iter()
            .any(|needle| skill_lower.contains(&needle.to_lowercase())),
        "`SKILL.md` 正文里找不到「孤儿 / orphan / unhandled task failure」—— \
         「任务失败的报告义务」那一节被删掉了（F-18 的覆盖缺口回来了）",
    );
}
