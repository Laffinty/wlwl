# ADR-0012 — `AS` keyword removed entirely; no alias for back-compat (Phase C1)

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-19 |
| **Deciders** | Li (project lead) |
| **Related** | spec v0.4 §13.4, ADR-001 (lexer/parser baseline), Phase C1 implementation report |

## Context and Problem Statement

v0.3 §4.2.1 documented the `AS` keyword as a soft-cast operator:

```wlwl
LET(x, 1 AS FLOAT);   // INTEGER 1 → FLOAT 1.0
```

v0.4 §13.4 deprecates the keyword: "AS is removed; explicit conversion
is performed by `INT(x)`, `FLOAT(x)`, `STR(x)`, `BOOL(x)` instead."
The new explicit conversions are in spec §10.3 row 9-14 and were
implemented as global builtins in Phase B8.

The question for v0.4 is what happens to a v0.3 program that uses
`AS`. Two paths:

1. Keep `AS` as a back-compat parser that translates `expr AS T` to
   `T(expr)` at AST level. Emit a deprecation warning.
2. Remove `AS` from the lexer entirely; programs that use it get
   `E0011 expected expression` (treat as syntax error).

## Decision Drivers

- §13.4 wording is firm: "AS is removed"
- v0.4 conformance suite does not include any `AS` test
- Backwards-compat is not part of v0.4 acceptance criteria (per
  plan §0.3 Decision #6 — "no v0.3 compat for keywords removed in
  v0.4 spec")
- The cost of keeping `AS` as a deprecation layer is non-zero: AST
  transformer, deprecation warning site, two parallel code paths in
  the parser that diverge after 18 months
- Decision is "small surface" — `AS` is one token

## Considered Options

### A. Keep `AS` as a back-compat deprecation alias

Lexer keeps `TokenKind::As`. Parser translates `expr AS IDENT` →
`IDENT(expr)`. Emits `W0054 deprecated AS`. Tells user "use
INT/FLOAT/STR/BOOL instead".

**Pros**: zero migration cost for existing v0.3 programs
**Cons**: contradicts §13.4 wording; complicates AST; introduces
deprecation-warning handling; if W0054 escapes CI lint, leaks
non-v0.4 syntax into v0.4 binaries

### B. Remove `AS` entirely (chosen)

Lexer drops `TokenKind::As`. Parser produces `E0011` on encountering
unrecognized keyword. Migrations are explicit code rewrites in
pre-v0.4 upgrades.

**Pros**: matches §13.4 spec; small lexer surface (1 token); no
hidden post-rule branches
**Cons**: existing v0.3 programs must rewrite `x AS T` to
`T(x)`; v0.4 cannot ingest v0.3 sources without manual edit

### C. Convert `AS` to a parser-level reject-and-explain

Same as option B at the lexer level, but the parser produces a
custom `E00XX deprecated AS; use INT/FLOAT/STR/BOOL instead` with
suggestion_code so the user gets a precise pointer.

**Pros**: better UX than bare syntax error
**Cons**: still requires parser logic, code path; arguably violates
"no v0.3 compat" if `suggestion_code` prints the migration target

## Decision Outcome

Chosen option **B** + **C** hybrid: lexer drops `TokenKind::As`, but
the parser preserves the **specific error message** for `AS` as a
named sub-token-class within E0011:

```rust
// wlwl-parser/src/lib.rs
match self.peek_kind() {
    TokenKind::As => self.diag_with_note(
        E0011,
        "`AS` was removed in v0.4. Use INT(x), FLOAT(x), STR(x), BOOL(x) instead.",
        "spec v0.4 §13.4",
    ),
    _ => self.diag(E0011, "expected expression"),
}
```

So users get a **specific message** without a deprecation layer or
hidden rewrite. Migrating code is mechanical.

The implementation lives in Phase C1's parser, with `w05*` errors NOT
emitted for `AS` (unlike options A/C). CI never sees `W0054 AS
deprecated` because the lexer doesn't emit the token.

## Consequences

Positive:

- Spec §13.4 conformance is exact; AI tools reading the lexer see no
  `As` token (deterministic build)
- Smaller lexer (one fewer keyword variant)
- Zero tokenizer / parser / runtime deprecation paths

Negative:

- Any v0.3 user code with `AS` errors out on first compile
- Migration is a one-shot rewrite per program (no automated tool
  ships in v0.4 — v0.5 may add `wlwl migrate v0.3-to-v0.4`)
- Documented in `CHANGELOG.md` "Breaking changes in v0.4" entry

## References

- spec v0.4 §13.4 (AS removal)
- docs/plan/wlwl-build-plan-v0.2.md §C1 + §0.3 Decision #6
- docs/history/20260918c1.md (Phase C1 implementation report)
- Phase E3 AST-stable node ID API verifies that E0011 surfaces for
  tools expecting E0011 (i.e., no surprise E-code added)
