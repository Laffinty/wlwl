# ADR-0018 — §11.2 E0055 / E0057 retention decision for v0.9

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-09-24 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.8.1 §11.2 (E0055 / E0057 reservation rows), spec v0.9 §11.2 / §17.3 (algebraic-effect framing), `docs/history/deviations-v0.8.md` D8-003, ADR-0017 (cooperative suspension scheduler), ADR-0019 (OOP), `wlwl-build-plan-v0.9` §3.6 / §5.3 / §9.2 |

## Context and Problem Statement

spec v0.7 / v0.8.1 §11.2 registers two error codes in the "reserved"
column that have **no triggering path** in the corresponding impl:

- **E0055** — "保留 / 无触发路径;关闭后 `CHANNEL_RECV` 走
  `ERR(kind="ChannelClosed")` 载荷路径" (`deviations-v0.8.md` D8-003).
  The intent was to give a native diagnostic code for the
  channel-closed case, but in v0.7 / v0.8.1 the impl always emits the
  `ChannelClosed` `ERR` payload instead, leaving E0055 registered but
  unreachable.
- **E0057** — "保留 / 无触发路径;跨任务不可变单元格继续走 `E0024`"
  (D8-003). The intent was to give cross-task immutable-cell writes
  their own code, but in v0.7 / v0.8.1 cross-task and single-task
  immutable-cell writes both emit `E0024`, leaving E0057 registered but
  unreachable.

Two reserve codes with no triggering path are a spec / impl drift
liability:

- **Spec debt.** §11.2 is the canonical error code table; every row
  is supposed to either fire or be explicitly marked as "reserved for
  future version". Two rows that are simultaneously reserved and never
  fired dilute the table's authority.
- **Impl drift.** When readers see E0055 / E0057 in the table and try
  to handle them, they find nothing. This is the same class of bug as
  an `enum` variant that exists in code but is never constructed.
- **Algebraic-effect framing (ADR-0017).** v0.9 promotes §17 to
  algebraic-effect terminology: control-flow events are
  `Effect { tag, payload }`, not native error codes. The two
  reserve codes pre-date this framing; under the new framing, the
  `ChannelClosed` and cross-task immutable-cell cases are exactly the
  shape of a tagged effect (`Cancelled { reason: { kind: "ChannelClosed",
  channel: ... } }` or `Err { kind: "ImmutableCell", task: ... }`),
  not the shape of a native error code.

The v0.9 question is whether to **enable** the codes (give them real
triggering paths), **keep them reserved** (acknowledge the debt, defer
to v0.9.1+), or **remove them** (and rely on the `ERR` payload path
that is already in production).

## Decision Drivers

- **Algebraic-effect consistency (ADR-0017, ADR-0019 §4.4.1).** v0.9
  documents control-flow events as `Effect { tag, payload }` and runs
  them through the effect-handler loop. Two reserve codes pre-date
  this; under it, `ChannelClosed` is the canonical payload shape, not
  a native code.
- **§11.2 table authority.** Every row should either fire or be
  marked "reserved for future version with explicit dev path". Two
  rows that are registered-but-unfired for two release cycles violate
  this principle.
- **ERR transparency (spec v0.8.1 §8.2).** ERRs are values, not
  exceptions; consumers use `IS_ERR` + `ERR_PAYLOAD` to inspect
  them. `ERR(kind="ChannelClosed")` is already inspectable. Promoting
  it to a native code adds no information; demoting it to a payload
  is the natural shape.
- **Symmetry.** E0055 and E0057 are mirror cases (cross-cutting
  control-flow events on the channel / cell surface). Treating them
  symmetrically is easier to spec, easier to test, easier to teach.
- **v0.6 baseline compatibility.** v0.6 users do not import or check
  E0055 / E0057 (they have never fired). Removing them is zero-cost.
- **Plan-level commitment.** Plan §9.2 decision points (2026-09-24
 草稿 2 调整): §3.6 E0055 → **B(移除)** default, §3.6 E0057 → **A(移除)**
  default. Both defaulted; this ADR ratifies the defaults.
- **Opt-in preservation path.** Some users / tooling may want a
  native-code capture point for the channel-close case (strong-typed
  diagnostics, lints that flag `IS_ERR kind = "E0055"` patterns).
  v0.9 keeps an opt-in: `wlwl.toml [native_channel_close] true`
  re-enables E0055 firing. Default is `false` (B = remove).

## Considered Options

### A. Enable both codes — give them real triggering paths (rejected)

Wire E0055 and E0057 into the channel / cell handlers so they fire in
addition to the `ERR` payloads.

**Pros:** Strong-typed error code at the native-code level; useful for
static analysis tools.

**Cons:**

- Splits one logical event (channel close, immutable-cell violation)
  across two error-reporting shapes (native code + `ERR` payload).
  Consumers have to handle both; spec gets two rows for the same
  event.
- Loses the algebraic-effect framing consistency: §17 says all
  control-flow events are `Effect { tag, payload }`, but E0055 / E0057
  would be native codes.
- §11.2 row count grows (no net win for table authority).
- Plan §9.2 explicitly rejected both A's defaults (2026-09-24 草稿 2
  调整 + 草稿 3 默认).

### B. Remove both codes — rely on the existing `ERR` payload paths (chosen)

Drop E0055 and E0057 from the registry. `ChannelClosed` and
cross-task immutable-cell violations keep emitting through the
existing `ERR` payloads (`ERR(kind="ChannelClosed")` /
`ERR(kind="ImmutableCell")`).

**Pros:**

- One logical event = one reporting shape (the `ERR` payload).
- §11.2 row count drops by 2 (61 激活 / 6 保留 / 67 总注册 vs v0.8.1
  56 激活 / 11 保留 / 67 总注册; per plan §11.2 consistency table).
- Algebraic-effect framing consistency: no native codes leaking back
  into control-flow-event reporting.
- v0.6 baseline compatibility preserved (no user-visible change in
  error semantics).
- `E0057` symmetry: single-task and cross-task immutable-cell
  violations both fire `E0024`, no new code to introduce for the
  cross-task path.
- Default-rationality: plan §9.2 already defaults this way.

**Cons:**

- Loses the strong-typed capture point at the native-code level for
  channel-close diagnostics.
- Users who explicitly pattern-match `IS_ERR kind = "E0055"` would
  silently miss; mitigation: opt-in via
  `wlwl.toml [native_channel_close] true` re-enables E0055 firing
  (default `false`).

### C. Keep both codes reserved — defer the decision to v0.9.1+ (rejected)

Mark E0055 / E0057 as "reserved for v0.9.1+" with explicit dev paths.

**Pros:** No immediate work; preserves the option to enable later.

**Cons:**

- Same drift liability as today; just defers it.
- Plan §9.2 explicitly rejected (default B).
- §11.2 table authority issue unresolved.

### D. Remove E0055 + enable E0057 (split path; rejected)

Remove E0055 (option B default), but enable E0057 (option A default)
to give cross-task immutable-cell violations their own code.

**Pros:** Cross-task / single-task distinguishability.

**Cons:**

- §3.6 草稿 2 推荐理由: "E0057 维持 B: 跨任务与单任务的 immutable-cell
  错误本质相同(都是 `LET`(不可变)上 `SET`),**形式上保持统一**更符合
  §3.3 的 'flag 不再改变' 原则;`E0057` 是 §11.2 中的注册冗余,移除可
  减少规范表行数." Splitting them re-introduces the table authority
  problem E0057 was originally meant to solve.
- Asymmetric with E0055: harder to teach, harder to test.
- Plan §9.2 E0057 default is A(移除), not B(启用).

## Decision Outcome

Chosen option **B** for both E0055 and E0057. Concretely, v0.9 commits
to:

- **E0055 removed.** §11.2 row deleted. `ChannelClose` event keeps
  flowing through the existing `ERR(kind="ChannelClosed")` payload.
  spec §11.2 prose update (Step 11) explicitly notes: "关闭后
  `CHANNEL_RECV` 触发 `ERR(kind="ChannelClosed")` 载荷(原 E0055
  注册移除,与 algebraic-effect framing 一致)."
- **E0057 removed.** §11.2 row deleted. Cross-task immutable-cell
  violations keep firing `E0024`, identical to single-task path.
  spec §11.2 prose update (Step 11) explicitly notes: "跨任务不可变
  单元格继续走 `E0024`(与单任务路径统一;原 E0057 注册移除)."
- **Opt-in preservation path for E0055.** `wlwl.toml
  [native_channel_close] true` re-enables E0055 firing in the
  channel-close path. Default `false`. This is **explicitly
  opt-in** and is **not** documented in §11.2 as a default mode;
  it lives in `wlwl.toml` reference (per plan §11.4 留 Step 11/12
  spec 派生时定, 草稿 4 三次审计标 "可以不动"). Implementation is
  in `wlwl-cli`'s config loader; lock test
  `wlwl_toml_native_channel_close_enables_e0055` covers it.
- **No opt-in for E0057.** The symmetry argument (§3.6 草稿 2 修
  正) is final; E0057 stays removed even in opt-in mode. There is no
  `[native_cross_task_immutable_cell]` flag.
- **§11.2 row count.** v0.9 row count is **61 activated + 6 reserved
  = 67 total registered** (per plan §11.2 consistency table).
  Activated: v0.8.1's 56 + E0032 + E0050 + E0051 (promoted from
  reserve, ADR-0019 §4.3) + E0065 + E0066 (new, ADR-0017 deadlock +
  ADR-0019 tag/payload) = 61. Reserved: v0.8.1's 11 - 3 (promoted)
  - 2 (E0055/E0057 removed) = 6.
- **Spec / impl sync.** `gen-appendix-g` re-runs after both
  Step 11 (concurrent段) and Step 12 (OOP段) to ensure §11.2 rows
  and `b11_*` lock tests stay aligned (per ADR-0017 migration
  guidance).
- **`deviations-v0.8.md` D8-003 closed.** With E0055 / E0057
  removed in v0.9, D8-003 (the reservation rationale) is closed
  by the decision itself. Step 14 (deviations-v0.9.md 启动) records
  the closure.

### Explicit non-rules

- No new error code for the channel-close event (the `ERR` payload
  is the canonical shape).
- No new error code for the cross-task immutable-cell violation
  (E0024 is the canonical shape).
- No opt-in flag for E0057 (asymmetry rejection).
- No re-numbering of §11.2 rows (the table format is unchanged;
  only two rows are deleted).
- The opt-in `[native_channel_close]` flag is **not** a §11.2
  normative item; it is a `wlwl.toml` configuration switch
  (analogous to `[strict_deadlock_detect]` from ADR-0017 §3.4).

### Migration guidance

- **No `.wll` source change required.** Users who were not
  matching on E0055 / E0057 (they could not — the codes never fired)
  are unaffected.
- **Tooling impact.** Linters / static analyzers that pattern-match
  E0055 / E0057 as known codes will report "unknown code"; mitigation
  is one-line config update (drop E0055 / E0057 from their whitelist).
- **v0.6 baseline.** v0.6 has no concurrency runtime, so E0055 /
  E0057 were never reachable in v0.6 either; removing them is
  zero-impact on v0.6 conformance fixtures.
- **Lock tests.** Step 6 (plan §9.1) adds
  `registry_no_e0055_no_e0057_after_v09` +
  `channel_close_returns_channelclosed_payload_not_native`. Both run
  against the new state of `wlwl-eval/src/registry.rs` and the
  updated §11.2 prose.
- **Deviation closure.** `deviations-v0.9.md` records
  `D9-001` = "E0055/E0057 保留位决议" with status "已修复 (ADR-0018
  Accepted)".

## Consequences

**Positive:**

- §11.2 table authority restored: every row either fires or is
  explicitly marked "reserved for future version with explicit dev
  path".
- Algebraic-effect framing (ADR-0017 / ADR-0019 §4.4.1) is
  consistent end-to-end: no native codes leaking into control-flow
  event reporting.
- Symmetry restored: cross-task / single-task immutable-cell
  violations share one error code (E0024); channel-close is one
  reporting shape (`ERR(kind="ChannelClosed")` payload).
- §11.2 row count drops by 2 reserved rows (67 total registered,
  unchanged; 6 reserved, down from 11).
- No v0.6 baseline impact; no v0.7 / v0.8.1 source change required.
- Opt-in preservation path keeps the door open for strong-typed
  diagnostics users without making it the default.

**Negative:**

- Loses default strong-typed capture point at the native-code level
  for the channel-close case. Mitigation: `[native_channel_close]
  true` opt-in (default `false`).
- Two E-codes have a different shape than their v0.8.1 reservation
  text suggested. Mitigation: spec §11.2 prose update is explicit
  about the change (Step 11).
- The asymmetry between E0055 (opt-in preserved) and E0057 (no
  opt-in) might confuse readers who expect symmetry. Mitigation:
  this ADR + plan §3.6 rationale are documented; spec §11.2 prose
  explicitly says "E0057 has no opt-in counterpart by symmetry
  decision".
- Lock-test count is reduced by 0 (E0055 / E0057 reservation tests
  are removed, but their replacement tests cover the same surface;
  net change = 0 in lock-test count).

### Fallback paths (per plan §10)

- If users strongly prefer E0055 to fire by default → flip
  `[native_channel_close]` to `true` by default in v0.9.1 (one-line
  config change; no spec change required).
- If users strongly prefer E0057 to fire for cross-task cases →
  re-introduce E0057 in v0.9.1 spec §11.2 row + impl firing path.
  This requires a new ADR (ADR-0020 or similar) but is bounded.
- If algebraic-effect framing is dropped (per ADR-0017 fallback
  path) → re-evaluate whether E0055 / E0057 reservation makes sense
  in a non-effect-framed world. Most likely they'd be re-enabled.

## References

- `docs/plan/wlwl-build-plan-v0.9.md` §3.6 (E0055/E0057 决议) /
  §5.3 / §9.2 (decision points) / §10 (risk table) / §11.2
  (consistency table) / §11.4 (3 small problems deferred to
  Step 11/12 spec derivation)
- `docs/standard/wlwl-spec-v0.8.md` §11.2 (E0055 / E0057 reservation
  rows) / §8.2 (ERR transparency)
- `docs/history/deviations-v0.8.md` D8-003 (E0055 / E0057 reservation
  rationale)
- `docs/adr/0017-cooperative-suspension-scheduler.md` (algebraic-
  effect framing; §17 → Effect { tag, payload })
- `docs/adr/0019-oop-minimal-implementation-with-behavioral-types.md`
  (algebraic-effect runtime naming; spec §11.2 E0065 / E0066
  additions)
- `docs/standard/wlwl-spec-v0.8.md` §17.2 (channel close protocol) /
  §17.3 (cancellation protocol)
- spec v0.9 §17 (algebraic-effect framing normative 段, added in
  Step 11) — the framing context under which this decision is made.
- Pretnar & Bauer 2015 *An Effect System for Algebraic Effects and
  Effect Handlers* — foundational paper for the framing.
- McCabe, F.; Lindley, S. *WebAssembly Stack Switching Proposal*
  (WasmFX, Phase 3, 2026-05) — `tag + payload` shape consistency.