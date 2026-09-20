# WLWL v0.6 writing skill — CHANGELOG

This file records changes to the `wlwl-skill` bundle (not the language spec;
the spec lives at `../docs/standard/wlwl-spec-v0.6.md` and is authoritative).

## 2026-09-20 — Rewrite for v0.6 audit findings

A hands-on session surfaced 8 factually wrong claims and 14 missing topics
relative to the v0.6 spec. The skill bundle was rewritten end-to-end.

### Fixed (8)

1. **`==` IS an alias for `=`** (spec §1.5/§2.4/§4.3). The prior skill
   claimed `v0.6 has no ==` and recommended `==` workarounds. Removed
   from the antipatterns list; `SKILL.md` and `reference.md` now teach
   `==` alongside `=`.
2. **`NULL` is the null literal** (spec §1.9/§2.1). Replaced every
   occurrence of the nonexistent `NIL` literal and the nonexistent
   `IS_NIL` builtin.
3. **Full 8-entry falsy list** (spec §2.3): `FALSE`, `NULL`, `0`, `0.0`,
   `""`, `[]`, `DICT()` (empty dict), `NaN`. The prior list (4 or 7)
   omitted `NULL`/`0.0`/`DICT()` and incorrectly included `ERR(...)`.
4. **`E1003` is divide-by-zero** (spec §2.2, §11.2). The prior skill
   said `E0009`.
5. **`E0061` is file-not-found** (spec §11.2). The prior skill
   mis-described `E0042` as "file IO error"; it's a manifest code.
6. **`W0054` is not in spec §11.3**. All 5 references removed.
7. **`FORMAT` follows spec §10.7 strictly** — variadic positional args.
   The prior `interp.wll` and `SKILL.md` examples used
   `FORMAT("…{0}…", [x])`, which renders the whole list at `{0}`
   (verified empirically: output was `caught: [negative]`, not
   `caught: negative`). Examples rewritten to use string interpolation
   `…${x}…` (no nested-string antipattern) or true variadic FORMAT.
8. **Out-of-range array/string index raises `E0036`** (spec §4.5), not
   `NIL` or `""`. The prior skill's `LIST(1,2)[2]` example was wrong.

### Added (12)

1. **Control flow** beyond IF: `WHILE`, `FOR` (full-body form), `RETURN`,
   `BREAK`, `CONTINUE` (spec §6). Added to both `SKILL.md` (cheat sheet)
   and `reference.md` §5 (lookup table).
2. **Pattern matching** (`MATCH`): array `*rest`, dict key patterns,
   `OK(p)`/`ERR(p)`, default clause (spec §7). Both files now cover it.
3. **`IMPORT` syntax**: plain, alias `["orig": "alias"]`, and
   side-effect forms; `wlwl:std.*` prefix list (spec §9.2/§9.3). This
   unlocks the rest of the standard library — without IMPORT, none of
   the 70+ stdlib names are reachable.
4. **Full error model** (spec §8): §8.1 RESULT variants, §8.2 transparent
   propagation with the canonical "extract-before-passing" idiom, the
   13-entry consumer registry (§8.3), §8.4 PANIC, §8.5 top-level escape.
5. **Standard library catalogue**: ~70 names across §10.4 (containers),
   §10.5 (strings), §10.6 (`wlwl:std.collection`), §10.8 (`wlwl:std.json`
   + `wlwl:std.fs`), §10.10 (`wlwl:std.test`).
6. **Full error-code catalogue** (§11.2): ~25 entries grouped by
   parse/type/runtime/IO/manifest, replacing the prior 5-entry table
   (2 of which were wrong).
7. **Warning codes** (§11.3): full 8-entry list replacing the prior
   references to nonexistent `W0054`.
8. **Reserved forms** (§12): `CLASS`/`NEW`/`THIS`/`MODULE`/`MODULE_REF`/
   `CALL`/`AND`/`OR` are listed so an agent recognizes when it has
   accidentally written one.
9. **Display / STR quirks** (§2.5): float-from-int renders with `.0`;
   functions render as `<fun(params)>`; `PRINT` joins with single space.
10. **`INDEX` vs `xs[i]` distinction** (§10.4 vs §4.5): `INDEX` returns
    `-1` for not-found; `xs[i]` raises `E0036`. Both are useful but in
    different circumstances.
11. **Container immutability** (§3.3, §10.4): all container ops return
    new containers; "update" requires `LET MUT` + `SET`.
12. **`--format json|jsonl` AI diagnostics** (§11.1): 12-field JSONL
    schema per `impl/crates/wlwl-cli/tests/conformance.rs:95–108`.

### Added: directory & files

- `examples/` directory with 6 single-purpose `.wll` files, each
  `wlwl run`-able and exiting 0:
  - `truthiness.wll` — the 8 falsy values; `BOOL(ERR(...))` demonstration.
  - `error_propagation.wll` — §8.2 transparent propagation; the
    "extract before passing" idiom.
  - `match.wll` — literal/identifier/wildcard/array-`*rest`/dict/OK/ERR
    patterns + default clause.
  - `control_flow.wll` — `WHILE`/`FOR`/`RETURN`/`BREAK`/`CONTINUE`.
  - `import_stdlib.wll` — plain IMPORT, `["orig":"alias"]` rename,
    three `wlwl:std.*` namespaces.
  - `interpolation.wll` — multi-segment `${}`, escaped `\${}`,
    nested expressions, `STR` float-`.0` quirk, function rendering.
- `interp.wll` augmented to 13 blocks (A–N). New blocks: K (`RETURN`),
  L (`MATCH`), M (`IMPORT`), N (`IMPORT` with alias). Block (C) FORMAT
  line corrected. Block (F) demonstrates OOB-avoidance via bound-check
  (since the runtime exception `E0036` is not consumable via
  `UNWRAP_OR`). Block (H) demonstrates the overflow-generation path
  through a `safe_add` bound-checking helper, with a note in the file
  explaining the current-impl deviation from spec §B.8.

### Structural (5)

1. Trimmed frontmatter description from 1100 chars to ~400 chars; kept
   the four required trigger substrings (`WLWL`, `.wll`,
   `wlwl-spec-v0.6`, `v0.5 or earlier`) verbatim at the front.
2. Added explicit `When to load / NOT to use` block to `SKILL.md`.
3. Restored the `CHANGELOG.md` file that `README.md` had promised.
4. Added `examples/` directory (new).
5. Corrected `README.md`'s "verbatim copy" claim about `interp.wll`
   (now feature-augmented) and added the `examples/` + `CHANGELOG.md`
   to its directory tree.

### Known impl/spec deviations (not skill bugs)

These are recorded here so the next agent can find them; fixes belong
to the compiler (`impl/`), not the skill.

- `E0035` integer overflow surfaces as a runtime diagnostic that
  terminates the program (exit 1) rather than a returnable `ERR`
  value consumable via `TRY`/`UNWRAP_OR`. Spec §B.8 says the latter.
  Recorded in `docs/plan/deviations.md`.
- `wlwl:std.collection` is shipped as an empty module (no exports) in
  the v0.6.0 binary. `examples/import_stdlib.wll` and `interp.wll`
  blocks M/N use `wlwl:std.json` instead, which is fully exported.
- `DICT()` is the only constructor for empty dict; `[:]` is not parseable.
  The literal-form spec table mentions `[:]` informally but the parser
  rejects it.
- `NaN` is unreachable: `0/0` raises `E1003` even on FLOAT, where IEEE
  754 would produce NaN. The 8-value falsy list mentions NaN but the
  example in `examples/truthiness.wll` notes the gap.
