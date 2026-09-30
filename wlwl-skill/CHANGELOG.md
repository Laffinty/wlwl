# wlwl-skill CHANGELOG

Skill-bundle changes (spec lives at `../docs/standard/wlwl-spec-v0.11.md`
and is authoritative).

## [0.10.1] — 2026-09-30

对齐 v0.10.1 实现。此前本包停在 0.10.0,而规范与实现已走了 13 个 Step。

### Fixed — 这些会让 agent 写出**跑不通或跑错**的程序

- **`YIELD` 5 条形态全部与实测相反**(最高危)。规范 §17.1 已在 R10-015 逐条改正,
  本包此前仍称「挂起后继续 / `[1, YIELD(), 3]` → `[1, NULL, 3]` / 循环恢复嵌套体
  剩余 / 间接调用也挂起」。实测:`LET(x, YIELD())` 里 **`x` 根本没绑定**(`E0020`)、
  `IF(TRUE, YIELD(), 42)` → `NULL`、数组字面量 → `NULL`、`WHILE` / `FOR`
  体内**只跑一轮**、`LET(y, YIELD); y()` → `E0020`。**全部零诊断。**
  补上可用的规律:塌掉的是**整个最外层表达式**;挂起点**之前**的绑定仍可见。
- **把中缀运算写成了合法示例**。本包两处把 `FUN((MUT), MUT + 1)` 列为
  "all valid / all parse" —— 实现**根本没有中缀解析路径**,`LET(x, a + b)` 是
  `E0011`。补上「一切运算皆前缀调用」这条根本规则(此前全包零命中)。
- **`match_exhaustiveness` 默认值说反**。此前两处说 default-off,实际是
  **缺省跟随 `gradual_typing`**。本包自己的 `examples/wlwl.toml` 只写了
  `gradual_typing = "error"`,按旧文档理解 `E0116` 是关的,实际开着。
- **`gradual_typing` 的码段说宽了**。此前说管到 `…116`,实际只管 `…112`。
- **裸 `THIS` 说成「零参引用」**。实际 `E0020: undefined name`,必须 `THIS()` ——
  且本包另一处还说它不是 `self` 的别名,自相矛盾。
- **`fmt --check` 被授权「一律忽略」**。补上可判定规则:**末句不带 `;`**(内层语句
  保留)、注释与缩进已被处理、**行尾自 v0.10.1 起无关**。并写明
  `examples/` 下 10 个 `.wll` 全部不过 `--check`,是手写教学材料。
- **`[package]` 三件套与 `W0001` 零覆盖**。R10-010 的教训只躺在示例注释里,
  `W0001` 也不在 W 码表。agent 照文档只写 `[features]` 会拿到看不懂的诊断;
  在 v0.10 上更是**静默拿到绿构建**。
- **`AND` / `OR` / `MODULE` / `EXPECT_ERR`(全局)已从注册表与附录 G 移除**
  (D10-019)—— 它们实测 `E0020`,从来就调不通。`EXPECT_ERR` 可用的是
  `wlwl:std.test` 的导出,**1 个参数**,且捕不到原生码。

### Added

- **示例接进 CI**:此前 `wlwl-skill/examples/` **不在任何测试覆盖面里**,
  与 R10-070 修掉的 `impl/examples/` 同一个洞。新增
  `crates/wlwl-cli/tests/skill_examples.rs`(3 条:全跑 / 主题守卫 / 清单三件套)。
- **五份新示例**,每份都实跑验证:`module_decl`(+依赖)、`sealed`(+svc+sig)、
  `module_signature`(+math+sig)、`match_exhaustiveness`、`object_identity`。
  覆盖的全是零示例特性:`MODULE` 形态、`SEALED` 面、`.wll.sig` 三向检查、
  `E0116`、对象身份 `==` 与 `obj.m()` 语法糖。

### Changed

- 查表补齐:`E0010`–`E0013` / `E0020` / `E0022` / `E0025` / `E0064` / `W0001`
  (此前被引用多次却查不到);§8.3 消费者 14 → **13 全局**(减去已移除的)。
- 补 `;` 规则、**没有 `{}` 块**、容器不可变与 `MERGE`(此前 `MERGE` 全包零命中)。
- 修正 v0.9 / v0.10 两处都被称作「the current standard」的自述矛盾。

## [0.10.0] — 2026-09-28

### Changed

- **Target spec** bumped from `wlwl-spec-v0.9.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.10.md`** (current). v0.10 is a
  *static-contracts* release: its **runtime is identical to v0.9's**, and every
  new feature is **default-off**. A program that adds no annotation and ships
  no `.sig` behaves exactly as before.

### Added

- `SKILL.md`: new section **"Static contracts — spec §2.6, §5.2, §5.2.1, §7.4,
  §9.1, §9.6 (v0.10)"** covering type annotations, container/function types,
  bounded type variables, the `gradual_typing` switch, module signature
  sidecars + `SEALED`, and the v0.10 diagnostic table.
- `SKILL.md`: writing-flow step 7 for opting into static contracts.
- `SKILL.md`: "When to load" now lists v0.10; the frontmatter `description`
  covers the v0.10 surface.
- `reference.md`: **§24 v0.10 增量备忘 — 静态契约** with the 附录 D delta, the
  annotation-grammar traps, the two new `[features]` keys, and the five new
  CLI subcommands (`sig`, `sig-gen`, `interface`, `schema`, `lsp`).
- `reference.md` §10: `E0110`–`E0116` added to the error table; `W0110`–`W0117`
  added to the warning list, with the "there is deliberately no `E0117`" note.
- `reference.md` §2: v0.10 row in the version-decisions table.
- `README.md`: target version, scope, contents, and a new **"Static contracts
  quick rules (v0.10)"** section.
- `examples/static_contracts.wll` — runnable demo of annotations, container
  types, and `T: Comparable`. Verified output: `3 10 / 10 / 42 / TRUE / TRUE /
  3 / a`.

### Fixed

- **Annotation-grammar traps now state measured behaviour, not the spec's stale
  claim.** Spec §5.2.1's "三条实测事实" facts #2 and #3 no longer match the
  reference implementation; the bundle documents what the compiler actually
  does and flags the spec text as stale:
  - `DICT<STRING, INTEGER>` → **`E0011` expected `')'`, got `Gt`** — a parse
    error. The spec says it parses silently into an opaque type name.
  - `FUN(INTEGER) -> STRING` → **`E0012` expected `','`, got `Minus`** — a
    parse error. Same stale claim.
  - `ARRAY[INTEGER]: Comparable` → **`E0010`** ("a type constraint may only
    follow a bare type variable"). The spec says `E0012`, which is the
    return-type-mismatch code and unrelated.
  - Still accurate, and kept: bare `ARRAY` is `E0010`; bare `DICT` / `OPTION` /
    `RESULT` parse as opaque named types with no error.

## [0.9.0] — 2026-09-25

### Changed

- **Target spec** bumped from `wlwl-spec-v0.8.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.9.md`** (current). v0.9 is a
  *language upgrade* over v0.8: true-suspension concurrency, structured
  cancellation, deadlock L1, and OOP with session-type protocols.
- **SKILL.md description + title** rewritten: "Writing WLWL (v0.9)"
  covers v0.9 concurrency semantics, OOP (§13–§16), linear THIS, and
  the error-code delta. Frontmatter description lists the v0.9 surface.
- **`wlwl-spec-v0.8.md` references** updated throughout SKILL.md /
  reference.md / README.md to point at v0.9; `docs/history/` archive
  paths preserved.
- **Concurrency section rewritten** (SKILL.md + reference.md §21):
  - `YIELD()` legal at **any** expression position inside a task body
    (v0.7/v0.8 "Block direct child" restriction removed).
  - Blocking `CHANNEL_SEND` / `RECV` **suspend** when full/empty with
    no peer; `ERR(ChannelWouldBlock)` path **removed**.
  - `TASK_CANCEL(task, reason?)` / `TASK_CANCEL_PARENT(reason?)` carry
    an optional DICT reason (default `{}`); non-DICT → `E0066`.
  - `AWAIT` of a cancelled task returns
    `ERR(kind="Cancelled", reason: <dict>)`.
  - Close wakes parked receivers with `ERR(ChannelClosed)` and parked
    senders with `E0054`.
- **OOP section added** (SKILL.md + reference.md §14–§16):
  `CLASS` / `NEW` / `GET_PROP` / `SET_PROP` / `CALL_METHOD` / `THIS`;
  session-type protocols (sequence + ⊕ + μ); linear `THIS` discipline.
- **Error-code table** (reference.md §10): `E0055` / `E0057` marked
  removed; `E0065` / `E0066` / `W0065` / `W0066` added; `E0014` /
  `E0032` / `E0050` / `E0051` redefined per v0.9 §11.2.
- **`wlwl.toml` features** (reference.md §19): added
  `strict_deadlock_detect`, `native_channel_close`,
  `channel_large_buf_threshold`.
- **Anti-patterns expanded to 24 rows**: new rows cover linear `THIS`
  escapes, external `SET_PROP`, protocol-order violations, non-DICT
  cancel reason, `ChannelWouldBlock` expectation, and deadlocks.

### Added

- **`examples/oop.wll`** — CLASS / NEW / methods with `self` injection,
  session-protocol sequence, and linear-`THIS` usage.
- **`reference.md` §23 v0.9 增量备忘** — compact index of every v0.9
  item affecting `.wll` writing (concurrency, OOP, error codes, types,
  features).
- **Types `CLASS` / `INSTANCE`** in the type/display/equality tables
  (reference.md §1, §11, §21).
- **`MODULE_REF(path)`** documented (reference.md §7) — loads module
  dict without binding names.

### Verified

- All `examples/*.wll` and `interp.wll` run with exit 0 against the
  v0.9 reference implementation.

---

## [0.8.1] — 2026-09-23

### Changed

- **Target spec** bumped from `wlwl-spec-v0.7.md` (archived at
  `docs/history/`) to **`wlwl-spec-v0.8.md`** (current). v0.8 is a
  *clarification / documentation / alignment* patch over v0.7 — v0.7
  core + §17 concurrency still required knowledge.
- **SKILL.md description + title** rewritten: "Writing WLWL (v0.8.1)"
  covers v0.8 字面增改 + v0.8.1 patch 修复. Frontmatter description
  lists every v0.8 字面增改 (12 items) and v0.8.1 patch 修复 (5 items).
- **`wlwl-spec-v0.7.md` references** updated throughout SKILL.md /
  reference.md / README.md to point at v0.8; `docs/history/` archive path
  preserved.
- **§12 reserved forms** rewritten in SKILL.md and reference.md:
  `CLASS` / `NEW` / `THIS` / `MODULE_REF` / `CALL` /
  (`AND` / `OR` / `MODULE` have since been **removed** as unreachable)
  `BuiltinSpec` records in `BUILTIN_REGISTRY`. The registry
  (`docs/appendix_G.md`) is the single source of truth; section §12 now
  points to it. Audit-driven (see `docs/history/audit-report-v0.8.1.md`
  §3.2 + spec §12 rewrite D8-005).

### Added

- **4 new anti-patterns (rows 16–19)**, audit §8.2 reproducer-driven:
  - **#16** Builtin name as value (`LET(f, +)` → E0020; `LET(f, PRINT)` → E0020; spec §4.3 op-tokens + §5.4 user-defined function only)
  - **#17** Float integer overflow / divide-by-zero NOT catchable (these are native codes E0035 / E1003 per §11.2 — not ERR; `EXPECT_ERR(/(1, 0))` does NOT save; spec §11.2 + §2.2)
  - **#18** Style guide rebalance: SUB 3rd arg is length, not end-index (v0.8.1 D8-010 observable change; spec §10.5; `SUB("Hello", 7, 5)` now returns `"world"`)
  - **#19** Float exponent literals work in v0.8.1 (`1.5e2`, `1e-3`, `1E3` per spec §1.7 EBNF `digits exponent`; pre-D8-01 raised E0011).
- **`YIELD()` placement note** softened: v0.7 / v0.8 spec §17.1 frames this
  as "理想语义 + 实现路径" (static task-body segmentation in
  `yield_split.rs`), not a hard rule. Future suspension-based schedulers
  may lift the limitation without breaking change (D8-007).
- **`NOT(ERR(x))` note**: spec §4.3 prose corrected (D8-006) — all
  operators including NOT propagate ERR transparently. Safe idiom:
  `NOT(BOOL(ERR(x)))` for safe coercion.
- **String literal subscript** (§4.5 prose + §A.2 grammar, v0.8.1
  D8-012): `"hi"[0]` parses and evaluates to `"h"`. INT / FLOAT /
  Boolean / NULL literal postfix remains rejected at parser (no
  `INDEX_GET` semantics).
- **`MUT` as binding name** (§1.4, v0.8.1 D8-011): `LET(MUT, "x")` is
  valid; MUT is a context keyword only after LET-modifier slot.
- **`=` triple-identity** (§1.5, D8-008): `=` is `==` in call position
  AND a separator for INDEX_SET sugar AND for default-param — three
  roles, no lookahead needed.
- **`%` float E0030** (§2.2 + §11.2, D8-008): `%` with any FLOAT arg
  raises E0030 (was previously listed in only spec prose; E0030 row
  updated).
- **`EXPECT_ERR`** (§8.3, D8-008): added to the §8.3 consumer table
  (was in prose only — explicit table entry for lookup).
- **`SHIELD` / `SCOPE(ERR)` / `AWAIT` host diagnostic clarifications**
  (§17.1 / §17.5, D8-008): user ERR vs host diagnostic now distinct;
  `SCOPE(ERR("x"))` propagates transparently (E0052 not fired).
- **`TASK` name-collision** (§2.1 / §10.11, D8-008): type name `TASK`
  vs `wlwl:std.agent.TASK` / `wlwl:std.ai.TASK` — same name, no
  conflict; recommended full-qualified write style.

### Verified

- All `examples/*.wll` in the bundle still parse under v0.8.1 spec
  (mechanical re-check; no source change to examples required).
- `interp.wll` (gold) extends with a `SUB` length example
  (`SUB("Hello, world", 7, 5)` → `"world"`) demonstrating D8-010 fix.

---

## [0.7.0] — 2026-09-22

### Added

- **Concurrency (spec §17)** section in `SKILL.md`: SCOPE / SPAWN /
  AWAIT / YIELD / TASK_* / SHIELD / CHANNEL_* with hard rules
  (explicit SCOPE, YIELD placement, ChannelClosed kind, TRY_* preference).
- **5 new antipatterns** (rows 11–15): top-level SPAWN, bare-branch
  YIELD, NULL-as-close, non-suspending SEND/RECV, unhandled Cancelled
  AWAIT.
- `examples/concurrency.wll` — SCOPE + two SPAWN + YIELD + channel
  fan-in (TRY_*), self-printing PASS lines.
- `examples/concurrency_cancel.wll` — TASK_CANCEL_PARENT inside SHIELD,
  deferred `TASK_IS_CANCELLED`, ChannelClosed kind check.
- `reference.md` §12 concurrency reference: builtin table, ERR kinds,
  `E0052`–`E0058`, type names `TASK`/`CHANNEL`, equality/display notes.

### Changed

- Target spec **wlwl-spec-v0.7** (additive over v0.6). Authoritative
  path: `../docs/history/wlwl-spec-v0.7.md`; v0.6 archived at
  `../docs/history/wlwl-spec-v0.6.md`.
- Deviations pointer → `../docs/history/deviations-v0.7.md`
  (former `docs/history/deviations-v0.10.md`).
- `SKILL.md` description + title cover v0.7 concurrency; writing flow
  checklist gains a concurrency step.
- Equality note: `==(f, f)` / `==(h, h)` are TRUE (handle identity).

## [0.6.1] — 2026-09-20

### Changed

- Extension notes for `.wll` (was `.wl`).
- Authoritative spec path → `../docs/history/wlwl-spec-v0.6.md`.

### Known impl/spec deviations (not skill bugs)

- Integer overflow: spec says `ERR(E0035)` consumable via `UNWRAP_OR`;
  current impl surfaces a runtime diagnostic (exit 1). Recorded in
  `docs/history/deviations-v0.7.md`.
- Captured-`LET` upgrade (legacy E-CloCap) vs §3.3 strict reading —
  prefer `LET MUT` for shared mutation (skill antipattern #1).

## [0.6.0] — 2026-09-20

### Added

- Nine v0.6 decisions documented (truthy overhaul, short-circuit,
  IF-ERR routing, `!` canonical, `AT_K`, string subscript, `LET MUT`,
  overflow `E0035`, `${}` interpolation).
- Gold examples through `N` blocks in `interp.wll`.
