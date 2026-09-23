# ADR-0019 — OOP minimal implementation w/ behavioral types (α + pioneering)

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-24 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.8.1 §12 / §13 / §14 / §15 / §16 (v0.9 fills these), spec v0.9 §11.2 (E0032 / E0050 / E0051 promotion; E0066 new), 附录 G (re-derived twice — concurrent段 + OOP段), ADR-0017 (cooperative suspension scheduler, algebraic-effect framing), ADR-0018 (E0055/E0057 retention), `wlwl-build-plan-v0.9` §4 / §9.1 Step 8-12 / §11.2 / §11.3 |

## Context and Problem Statement

WLWL v0.7 / v0.8.1 registers OOP keywords and builtins but does not
implement them. Specifically, spec 附录 G (lines 1223-1233, v0.8.1)
marks `CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` /
`CALL_METHOD` with the note *"OOP 未实现"*. The corresponding `Expr`
variants exist in the parser; the corresponding builtins exist in
`BUILTIN_REGISTRY`; but `resolve_builtin` returns a stub value for
each. Three error codes are reserved with no triggering path:

- **E0032** — "保留 / 无触发路径;原意为 immutable `LET` 上 `SET` /
  `THIS` 越界返 E0032" (deviations-v0.8.md context).
- **E0050** — "保留 / 无触发路径;原意为 `CLASS` 协议表达式不符返 E0050".
- **E0051** — "保留 / 无触发路径;原意为 `CALL_METHOD` 协议状态机
  违规返 E0051".

This is a v0.7-incurred spec debt (the keywords were registered early
to lock parser surface area, but the runtime never followed up). v0.9
is the version that resolves it: either **真实现** the OOP surface
with the v0.9 chosen design (α + pioneering per user 2026-09-24), or
**移除** the OOP keywords / builtins / codes and acknowledge OOP is
not a v0.9 feature.

The user-selected v0.9 design (2026-09-24 Memory) is **α (real
implementation) + pioneering technical route**: OOP gets a real
implementation, but the design is academic-frontier (algebraic-
effect framing + tag/payload cancellation + session-typed methods +
linear `THIS`) rather than industry-consensus (Java/Kotlin/Swift/
Python). This ADR ratifies the chosen path and locks the design.

## Decision Drivers

- **Spec debt closure.** Three reserved codes (E0032 / E0050 / E0051)
  with no triggering path is the same class of drift ADR-0018 closes
  for E0055 / E0057. v0.9 must either **启用** these codes (by
  implementing the OOP surface) or **移除** them (by removing the
  OOP keywords / builtins / codes).
- **Appendix G authority.** 附录 G (generated) must match the impl.
  Currently it says "OOP 未实现"; v0.9 must either delete the rows
  or fill them with real prose.
- **User choice (2026-09-24).** Plan §0.1 records the user's chosen
  α + pioneering path. This ADR ratifies that choice.
- **Pioneering alignment (user cross-project preference).** User
  cross-project memory records a stable preference for
  academic-frontier technical choices when offered alongside
  industry-consensus paths. OOP α + pioneering gives wlwl a
  distinctive academic identity (Koka / Links / OCaml 5 / WasmFX
  alignment) rather than a Java-clone.
- **§13-§16 章节填充.** v0.9 spec fills §13 (OOP keywords + object
  model), §14 (behavioral types + session-typed protocols — new
  chapter), §15 (`THIS` linear capability — new chapter), §16
  (OOP × concurrency interaction — new chapter). These are net-new
  chapters; v0.6 → v0.8.1 spec is §1-§17 + appendices.
- **Algebraic-effect consistency (ADR-0017).** OOP's
  `CALL_METHOD` invocation, error reporting, and protocol-violation
  reporting all use the algebraic-effect framing established in
  ADR-0017 / §4.4.1 — `perform` / `raise` shapes, `Effect { tag,
  payload }` runtime enum, no native-code leaks.
- **WasmFX-style tag dispatch (internal, §4.4.4).** The internal
  `Effect` enum is itself WasmFX-style `tag + payload`; v0.10+ Wasm
  backend migration maps each `Effect` variant to a WasmFX `suspend
  $tag_*` instruction. OOP-related `Effect` variants (`MethodCall`,
  `ProtocolViolation`) extend the enum.
- **Performance budget.** Per ADR-0016 / ADR-0017: single-task
  regression < 10%; concurrent path correctness + leak-freeness
  only. OOP's `CALL_METHOD` linear-`THIS` runtime check adds ~5%
  per call (estimate per plan §6.3 consequences).
- **Engineering cost.** 14-18 person-weeks total (per plan §11.2
  realistic estimate): concurrent rewrite ~5-7 + algebraic-effect
  runtime ~1 + OOP minimal implementation ~3 + session types with
  choice + μ ~3-4 + deadlock L1 ~1 + tag/payload ~1 + spec + tests
  ~3.5.
- **Workload-fit.** OOP is a major language-shape addition; it must
  not regress the v0.6 fidelity baseline. v0.6 has no OOP, so OOP
  implementation is **additive** — v0.6 conformance fixtures
  continue to pass unchanged.

## Considered Options

### A. Remove OOP keywords / builtins / codes (β / γ path; rejected)

Delete `CLASS` / `NEW` / `THIS` from the parser; delete
`GET_PROP` / `SET_PROP` / `CALL_METHOD` from `BUILTIN_REGISTRY`;
delete E0032 / E0050 / E0051 from §11.2; delete 附录 G rows.

**Pros:** Closes spec debt without implementation cost.

**Cons:**

- Removes a parser surface that v0.7 / v0.8.1 programs may rely on
  (even if no runtime behaviour was provided).
- Removes the planned v0.9 milestone (per plan §0.5 "OOP 真实现"
  is part of v0.9 deliverable).
- User explicitly chose α over β / γ on 2026-09-24.

### B. Industry-consensus OOP (Java / Kotlin / Swift / Python style; rejected as γ)

Real implementation with class, instance, `this`-as-implicit-receiver,
inheritance via prototype chains, methods as plain functions,
no protocol enforcement. Aligns with §13 chapter fill only.

**Pros:** Familiar; lower engineering risk; aligns with
industry-consensus OOP.

**Cons:**

- Loses the academic-frontier / pioneering identity the user
  explicitly preferred (2026-09-24 choice of α + pioneering over
  β / γ).
- Skips session types, linear `THIS`, and algebraic-effect framing.
- Skipped in 2026-09-24 plan session; the user chose α over γ.

### C. Minimal OOP skeleton only (no behavioral types; rejected as β-min)

Real implementation of `CLASS` / `NEW` / `THIS` / `GET_PROP` /
`SET_PROP` / `CALL_METHOD`; methods are plain functions; no session
types; `THIS` is a runtime reference (not linear); no protocol
enforcement.

**Pros:** Cheaper than α (no session-types machinery, no linear-
`THIS` runtime check); still removes the spec debt.

**Cons:**

- Skips the academic-frontier design points user explicitly chose.
- Doesn't justify the v0.9 OOP milestone cost (~3 person-weeks for
  the minimal version vs ~6 for α).
- User 2026-09-24 plan session chose α over β.

### D. α (real implementation) + pioneering enhancements (chosen)

Real implementation of `CLASS` / `NEW` / `THIS` / `GET_PROP` /
`SET_PROP` / `CALL_METHOD` plus four pioneering enhancements
that align with academic-frontier language design (per
`wlwl-build-plan-v0.9` §0.5 table):

1. **OOP minimal skeleton** — `CLASS(name, parent_proto, fields)`
   creates an object dictionary; `NEW(cls, args...)` instantiates;
   `THIS` is a **真线性 capability**; `GET_PROP` / `SET_PROP` /
   `CALL_METHOD` go through dict + behavioral-type state machine;
   E0032 / E0050 / E0051 启用, §11.2 normative.
2. **Algebraic-effect framing (spec + impl)** — spec §17 uses
   effect-handler terminology; new
   `wlwl-eval/src/effect.rs` (~80 lines) houses
   `enum Effect { Yield { explicit: bool }, ChannelOp { dir, channel,
   value }, Cancelled { reason: Dict }, MethodCall { obj, method,
   args }, ProtocolViolation { protocol, method, expected, got } }`;
   `Scheduler::step` is the effect handler. Indirect YIELD
   auto-suspends at every call site with zero additional
   implementation cost.
3. **Tag/payload cancellation** — `TASK_CANCEL(task, reason_dict)`
   / `TASK_CANCEL_PARENT(reason_dict)`; `SHIELD` block's cancelled
   reason propagates to `Cancelled.payload.reason`; new error code
   `E0066` (reason non-DICT type).
4. **Session-typed methods** — each `CLASS` may declare a session-
   typed protocol; v0.9.0 ships **顺序 + 选择 (`⊕`) + μ 递归骨架**
   (external selection `&` / named protocols / parallel branches
   `par` deferred to v0.9.1+ per plan §4.4.3 2026-09-24 audit
   finding 4); `CALL_METHOD` walks the protocol state machine and
   fires `E0051` on violation.
5. **WasmFX-style tag dispatch (internal, merged with #2)** — the
   `Effect` enum is itself WasmFX-style `tag + payload`;
   `Scheduler::step` corresponds to WasmFX `resume`; v0.10+ Wasm
   backend migration maps each variant to a `suspend $tag_*`
   instruction.

**Pros:**

- Closes the v0.7 / v0.8.1 spec debt (E0032 / E0050 / E0051 fire
  in real impl).
- Fills §13 / §14 / §15 / §16 in spec v0.9 (net-new chapters).
- Gives wlwl a distinctive academic identity (Koka / Links /
  OCaml 5 / WasmFX alignment) rather than a Java-clone.
- Algebraic-effect framing (ADR-0017) extends cleanly to OOP
  (`MethodCall` / `ProtocolViolation` are new `Effect` variants
  shaped exactly like the existing `Yield` / `ChannelOp` /
  `Cancelled`).
- Linear `THIS` (Wadler 1990 style) is honest: not "越界报错" but
  a real linear capability tracked at runtime.
- Session types (Honda-Yoshida-Carbone 2008; Lindley-Morris Links)
  provide static guarantees on method invocation order — a level of
  safety not available in industry-consensus OOP.
- WasmFX-style tag dispatch opens a clean v0.10+ path to a Wasm
  backend (one-to-one `Effect ↔ WasmFX suspend` mapping).
- 附录 G re-derived twice (Step 11 concurrent段, Step 12 OOP段)
  keeps generator in sync with the new chapter numbers; lock
  tests `appendix_g_*_anchors_match_v09_section_numbers` guard.

**Cons:**

- Engineering cost is bounded but non-trivial: OOP minimal ~3 +
  algebraic-effect runtime ~1 + session types with choice + μ
  ~3-4 + tag/payload ~1 = ~8 person-weeks for this ADR alone
  (~half of v0.9's 14-18 person-weeks budget).
- Session types `⊕` / `μ` parser + state machine adds spec
  complexity (testing matrix grows; `b11_*` lock tests must cover
  protocol-violation paths).
- Linear `THIS` runtime check adds ~5% per `CALL_METHOD` (estimate
  per plan §6.3 consequences). Within budget but visible in
  benchmarks.
- `E0051` triggers via `ProtocolViolation` effect — multiple
  violation paths (state-machine mismatch / wrong-choice in `⊕` /
  μ-recursion error / missing-method) require ~10 lock tests
  (per plan §9.3 estimate).
- Algebraic-effect runtime naming churn (already a side effect of
  ADR-0017) extends to `MethodCall` / `ProtocolViolation` variants.
- New chapter numbers (§13-§16) require spec §1-§17 cross-
  reference updates and `gen-appendix-g` re-runs (twice).
- Pioneering design might invite spec controversy at v0.9.0
  release; fallback per plan §10: if any sub-item (behavioral
  types / linear `THIS` / algebraic-effect docs) introduces
  ≥ 2 spec disputes in Step 9 / 12, that sub-item falls back to
  conservative wording.

## Decision Outcome

Chosen option **D** (α + pioneering). Concretely, v0.9 commits to:

### OOP minimal skeleton

- **`CLASS(name, parent_proto, fields)`** — parser produces
  `Expr::Class { name, parent_proto, fields }`; `eval_class`
  constructs an object dictionary `{ __class__: ClassObj { name,
  parent_proto, fields }, ...fields }`; `parent_proto` is a
  session-type expression (see §14). Fields are stored in the
  object's dict; access through `GET_PROP` / `SET_PROP` is
  dict-shaped.
- **`NEW(cls, args...)`** — `eval_new` instantiates by copying the
  class fields into a new instance dict, then walks the
  session-typed protocol's `init` branch (if any), passing `args`
  in order; protocol-violation returns `E0050` (init protocol
  mismatch) or `E0051` (state-machine error after init).
- **`THIS`** — runtime tracks a **线性 capability** through a
  thread-local "this stack"; operations:
  - Storing `THIS` in a dict / array / closure environment → `E0032`.
  - Capturing `THIS` in a closure that outlives the call frame →
    `E0032`.
  - Passing `THIS` across `AWAIT` boundary (inside an awaited
    closure) → `E0032`.
  - Using `THIS` outside `CALL_METHOD` / `INIT` / `SET_PROP` /
    direct `THIS` reference → `E0032` (越界返 E0032).
  The capability is a runtime reference (similar to Roc / Austral
  linear types; not a static type-system feature in v0.9).
- **`GET_PROP(obj, key)`** — dict read; `obj` must be an instance
  type; missing key → `E0037`.
- **`SET_PROP(obj, key, value)`** — `obj` must hold a field named
  `key`; field must be marked `mutable`; otherwise → `E0032`. The
  linear `THIS` capability is consulted: setting via external
  reference without `THIS` → `E0032`.
- **`CALL_METHOD(obj, method, args...)`** — walks the session-typed
  protocol state machine (see §14 below). On entry, the linear
  `THIS` capability is set to `obj`. Method body runs. On exit,
  `THIS` capability is consumed. State machine advances; on
  protocol-violation → `E0051`.

### Algebraic-effect framing (spec + impl)

- spec §17 gains a normative 段: "本章并发原语建模为 algebraic
  effects. `YIELD()` = `perform Yield`, `TASK_CANCEL(t, reason)` =
  `raise Cancelled(reason)`, `CHANNEL_SEND/RECV` = `perform
  ChannelOp`. 运行时调度器是 effect handler,捕获 `Suspended` 状态
  作为 continuation,把控制流切到下一个 `perform` 现场."
- spec §13 (OOP keywords + object model) gains a parallel normative
  段: "OOP 调用建模为 algebraic effects. `CALL_METHOD(obj, m,
  args...)` = `perform MethodCall { obj, m, args }`. `SET_PROP`
  / `GET_PROP` = `perform PropAccess { object, key }` (read) /
  `perform PropAccess { object, key, value }` (write). 协议校验
  与线性 `THIS` 由 effect handler 完成."
- impl: new file `wlwl-eval/src/effect.rs` (~80 lines) houses
  `enum Effect`; `wlwl-eval/src/runtime.rs::Scheduler::step` is the
  canonical effect handler. The `Effect` enum is WasmFX-style
  `tag + payload`:
  ```rust
  enum Effect {
      Yield { explicit: bool },
      ChannelOp { dir: Send | Recv, channel: ChannelId, value: Option<Value> },
      Cancelled { reason: Dict },
      MethodCall { obj: Value, method: String, args: Vec<Value> },
      PropAccess { object: Value, key: String, value: Option<Value> },
      ProtocolViolation { protocol: ProtocolId, method: String, expected: ProtocolState, got: ProtocolState },
  }
  ```
- Indirect YIELD (`LET(y, YIELD); y()`) auto-suspends at every
  call site — effect-handler model means every `YIELD`-bound
  function call performs the same `Yield` effect. Zero additional
  implementation cost (per plan §4.4.1 audit finding 3).
- Users do **not** get custom effects in v0.9 (deferred to
  v0.10+); the framing is internal naming + spec wording only.

### Tag/payload cancellation

- spec §17.3: `TASK_CANCEL(task, reason_dict)` /
  `TASK_CANCEL_PARENT(reason_dict)` — `reason` is a DICT.
- `SHIELD` block exit: if `Cancelled` was raised but not consumed
  inside `SHIELD`, the `reason` dict propagates to
  `Cancelled.payload.reason`.
- impl: `task.rs` (`Task::cancel(reason)`) +
  `channel.rs` (`Cancelled { reason }`) ~200 lines.
- New error code **E0066** — "保留位 error: `TASK_CANCEL` /
  `TASK_CANCEL_PARENT` 的 `reason` 参数不是 DICT 类型". Replaces
  the spec-impl drift on cancel-reason typing.

### Session-typed methods (per plan §4.4.3)

- spec §14 (new chapter) — "Behavior Types and Session-Typed
  Protocols": each `CLASS` may declare a session-type protocol that
  constrains method-call ordering. v0.9.0 ships:
  - **顺序** (sequence): `{a: T1, b: T2, c: end}` — a → b → c.
  - **选择 (⊕ internal choice)**: `{a: T1 ⊕ b: T2}` — caller
    chooses a or b.
  - **递归 (μ skeleton)**: `{rec: μX. {get: ?int.X, inc: end,
    close: end}}` — protocol may self-reference.
- v0.9.1+ deferred items (per plan §4.4.3 2026-09-24 audit finding
  4): external selection `&`, named protocols (`typealias`),
  parallel branches (`par`).
- impl: new files
  `wlwl-eval/src/class.rs` (~500 lines) +
  `wlwl-eval/src/protocol.rs` (~300 lines, μ + ⊕ parser + state
  machine). `BUILTIN_REGISTRY` routes `CALL_METHOD` through the
  protocol state machine.
- `CALL_METHOD` violations:
  - Wrong choice in `⊕` → `E0051`.
  - State-machine mismatch (called method not in current state's
    options) → `E0051`.
  - `μ` recursion exhausted → `E0051`.
  - Protocol expression syntax error in `CLASS` second arg → `E0050`.

### WasmFX-style tag dispatch (internal, merged with algebraic-effect framing)

- The `Effect` enum is itself WasmFX-style `tag + payload`.
- v0.10+ Wasm backend migration map (informative, not v0.9
  deliverable):
  | wlwl `Effect`         | WasmFX `tag`              | WasmFX `suspend`         |
  |------------------------|---------------------------|---------------------------|
  | `Yield { explicit }`   | `$tag_yield`              | `suspend $tag_yield`      |
  | `ChannelOp { ... }`    | `$tag_channel_send/recv`  | `suspend $tag_channel_*`  |
  | `Cancelled { reason }` | `$tag_cancelled`          | `suspend $tag_cancelled`  |
  | `MethodCall { ... }`   | `$tag_method_call`        | `suspend $tag_method_call`|
  | `PropAccess { ... }`   | `$tag_prop_access`        | `suspend $tag_prop_access`|
  | `ProtocolViolation`    | `$tag_protocol_violation` | `suspend $tag_protocol_*` |

### Spec §13-§16 章节填充 + 章节号冻结规则

- §13 **OOP keywords and object model** — keyword syntax,
  dictionary shape, `CLASS` / `NEW` / `THIS` / `GET_PROP` /
  `SET_PROP` / `CALL_METHOD` semantics.
- §14 **Behavior types and session-typed protocols** (new
  chapter) — Honda-Yoshida-Carbone 2008 / Lindley-Morris Links
  style; 顺序 + ⊕ + μ skeleton coverage; external selection /
  named protocols / parallel branches deferred to v0.9.1+.
- §15 **`THIS` and linear capability** (new chapter) — Wadler
  1990 linear type theory; runtime check pattern;越界 E0032;
  capability lifecycle.
- §16 **OOP × concurrency interaction** (new chapter) —
  `CALL_METHOD` 内 `YIELD` / `CHANNEL_*` 的合法位置与并发原语
  交互；effect handler 在 `CALL_METHOD` 内的状态。
- 章节号冻结规则 (plan §6.3 audit finding 5): §13 / §14 / §15 /
  §16 **章节号在 v0.9.0 release tag 上冻结**; v0.9.1 只能向已
  存在章节追加 sub-section (§14.1 / §14.2 / ...),不重新编号;
  v0.10+ 若章节饱和,新增 §18+ 但不得回填进 §13-§16. 附录 G
  锚点与 spec 章节号一一对应 (`b11_*` 锁测试守住).

### Error code impact

- **E0032 promoted** from reserve to active — "保留 → 激活:
  immutable `LET` 上 `SET`; `THIS` 越界 / 跨 call 边界 / 跨
  AWAIT 边界 / 存入容器".
- **E0050 promoted** from reserve to active — "保留 → 激活:
  `CLASS` 协议表达式不符 (syntax / type error in second arg)".
- **E0051 promoted** from reserve to active — "保留 → 激活:
  `CALL_METHOD` 协议状态机违规 (wrong choice / state-machine
  mismatch / μ-recursion exhausted)".
- **E0066 new** — "`TASK_CANCEL` / `TASK_CANCEL_PARENT` 的
  `reason` 参数不是 DICT 类型".
- Total activated: v0.8.1's 56 + 3 (promoted) + 2 (E0065 from
  ADR-0017 deadlock L1 + E0066 from this ADR) = **61**.
- Total reserved: v0.8.1's 11 - 3 (promoted) - 2 (E0055/E0057
  removed per ADR-0018) = **6**. Total registered unchanged at
  **67**.

### Explicit non-rules

- No user-defined `perform` / `handle` in v0.9 (deferred to
  v0.10+).
- No external selection (`&`) / named protocols / parallel
  branches (`par`) in v0.9 session types (deferred to v0.9.1+).
- No static type system for linear `THIS` (v0.9 has dynamic
  linear capability tracking; full linear type system is a
  v0.10+ item per plan §2.5 "议程外" entry).
- No v0.9 re-numbering of existing §1-§12 chapters; new
  chapters are §13-§16 appended (after §12 `形式表`).
- No inheritance via prototype chains in v0.9 (single-level
  `parent_proto` for session-type inheritance only; multi-level
  inheritance deferred to v0.9.1+ if requested).
- No operator overloading (`+` on objects, etc.) — `GET_PROP` /
  `CALL_METHOD` are the only dispatch surface.
- No `super` / `parent` keyword in v0.9 (session-type inheritance
  covers the only v0.9 use case).

### Migration guidance

- **No `.wll` source change required** for v0.6 / v0.7 / v0.8.1
  programs (none used OOP because it was stub).
- **v0.6 fidelity gate** unchanged: OOP is additive; v0.6
  conformance fixtures pass unchanged.
- **Lock-test count growth:** v0.8.1 = 1409 passed → v0.9 target
  ~1465 passed (per plan §9.3; +56 incremental lock tests across
  §3.1-§3.6, §4.4.1-§4.4.3, §5; this ADR contributes ~24 of
  those: ~10 OOP minimal skeleton + ~10 session types 顺序+⊕+μ +
  ~3 tag/payload cancel + ~1 WasmFX-style tag dispatch).
- **Spec §1 cross-reference updates:** existing §1-§12 prose
  mentioning `CLASS` / `NEW` / `THIS` / `GET_PROP` / `SET_PROP` /
  `CALL_METHOD` must update cross-references to point to §13-§16
  (Step 11 spec derivation).
- **`gen-appendix-g` re-runs twice:** once after Step 11 (并发段
  附录 G 重生成) and once after Step 12 (OOP段 附录 G 重生成).
  Both runs are guarded by `appendix_g_*_anchors_match_v09_section_numbers`
  lock tests.

## Consequences

**Positive:**

- Spec debt closed: E0032 / E0050 / E0051 fire in real impl; no
  more "保留 / 无触发路径" rows in §11.2.
- §13-§16 chapter fill delivers four new chapters with distinctive
  academic content (Koka / Links / OCaml 5 / WasmFX alignment) —
  not a Java-clone.
- Algebraic-effect framing (ADR-0017 / this ADR §4.4.1) is
  consistent end-to-end: OOP control-flow events (`MethodCall`,
  `PropAccess`, `ProtocolViolation`) are new `Effect` variants
  shaped exactly like the existing `Yield` / `ChannelOp` /
  `Cancelled` ones.
- Linear `THIS` (Wadler 1990) is honest: runtime-tracked
  capability, not just "越界报错".
- Session types (Honda-Yoshida-Carbone 2008; Lindley-Morris
  Links) give wlwl a level of OOP safety not available in
  industry-consensus OOP.
- Tag/payload cancellation extends the algebraic-effect framing
  to the cancellation surface; `E0066` closes the spec-impl
  drift on cancel-reason typing.
- WasmFX-style tag dispatch (merged with algebraic-effect) opens
  a clean v0.10+ path to a Wasm backend.
- 章节号冻结规则 (§13-§16 frozen at v0.9.0 release tag) prevents
  v0.9.1 / v0.10+ from re-numbering and breaking 附录 G anchors.
- No v0.6 / v0.7 / v0.8.1 source change required.

**Negative:**

- Engineering cost: OOP minimal ~3 + algebraic-effect runtime
  ~1 + session types with choice + μ ~3-4 + tag/payload ~1 =
  ~8 person-weeks for this ADR alone.
- Session types `⊕` / `μ` parser + state machine adds spec
  complexity; testing matrix grows; lock tests must cover
  protocol-violation paths (estimate ~10 lock tests for E0051).
- Linear `THIS` runtime check adds ~5% per `CALL_METHOD`
  (estimate; within budget but visible in benchmarks).
- New chapter numbers (§13-§16) require spec §1-§12 cross-
  reference updates and `gen-appendix-g` re-runs (twice in
  Step 11 and Step 12).
- Pioneering design might invite spec controversy at v0.9.0
  release; fallback per plan §10: if any sub-item (behavioral
  types / linear `THIS` / algebraic-effect docs) introduces
  ≥ 2 spec disputes in Step 9 / 12, that sub-item falls back to
  conservative wording (per plan §0.5 "保守路径与之并存的
  可能性").
- Spec v0.9 line count grows by ~200-300 lines (§13-§16
  chapter fill + algebraic-effect normative paragraphs).
- Dev docs churn: every section that mentions OOP (parser docs,
  builtin reference, dev notes) needs an update.

### Fallback paths (per plan §10)

- If session-types μ implementation exceeds ~2 person-weeks,
  μ deferred to v0.9.1 (v0.9.0 ships 顺序 + ⊕ only); total work
  drops to ~12-15 person-weeks.
- If linear `THIS` runtime check adds > 10% per `CALL_METHOD`
  (over-running the budget), fall back to "越界报错" simple
  check (E0032 fires but no lifetime tracking).
- If algebraic-effect framing regresses eval hot-path > 5%,
  keep effect framing as spec-only (drop `effect.rs` file),
  fall back to internal `Signal::Yield` naming (per ADR-0017
  fallback path).
- If pioneering design invites ≥ 2 spec disputes during Step
  9 / Step 12 review, the disputed sub-item falls back to
  conservative wording (Java/Kotlin/Python style) per plan §0.5
  "保守路径与之并存的可能性".

## References

- `docs/plan/wlwl-build-plan-v0.9.md` §0.5 (pioneering table) /
  §4 (P1 OOP) / §4.3 (real impl path) / §4.4.1 (algebraic-effect
  framing) / §4.4.2 (tag/payload cancellation) / §4.4.3
  (session-typed methods) / §4.4.4 (WasmFX-style tag dispatch) /
  §9.1 Step 8-12 / §10 (risk table) / §11.2 (consistency table) /
  §11.3 (doc acceptance)
- `docs/standard/wlwl-spec-v0.8.md` §12 (形式表 with OOP rows) /
  §13-§16 (currently empty; v0.9 fills) / 附录 G (rows 1223-1233
  marked "OOP 未实现" in v0.8.1)
- `docs/adr/0017-cooperative-suspension-scheduler.md` (algebraic-
  effect framing context, `Effect` enum foundation)
- `docs/adr/0018-e0055-e0057-retention-decision.md` (sibling
  spec-debt-closure ADR)
- `docs/history/deviations-v0.8.md` (E0032 / E0050 / E0051
  reservation rationale)
- Lindley, S. *Lightweight Functional Session Types* (Links,
  Edinburgh) — session-typed-methods reference.
- Lindley, S.; Morris, J. G. *Links: Web Programming Without
  Tiers* — session-typed protocol syntax reference.
- Honda, K.; Yoshida, N.; Carbone, M. *Multiparty Asynchronous
  Session Types* (POPL 2008; JACM 2016) — session types
  foundational paper.
- Wadler, P. *Linear Types can Change the World!* (IFIP TC2 1990)
  — linear type theory foundational paper.
- Roc / Austral linear capability references.
- Koka: Leijen, D. *Koka: Functional Programming with Algebraic
  Effects and Effect Handlers* (2014+; v3.x 2024-2026).
- McCabe, F.; Lindley, S. *WebAssembly Stack Switching Proposal*
  (WasmFX, Phase 3, 2026-05) — `tag + payload` shape and v0.10+
  Wasm backend migration target.
- Pretnar & Bauer 2015 *An Effect System for Algebraic Effects and
  Effect Handlers* — algebraic-effects foundational paper.
- Caires, L.; Pfenning, F. *Session Types as Intuitionistic Linear
  Propositions* (CONCUR 2010) — Curry-Howard map for channel
  protocols (informative, deferred to v0.10+).
- v0.6 evaluator baseline (`Rc<RefCell<Env>>`) — unchanged; OOP is
  additive.