---
name: writing-wlwl
description: Writes correct WLWL v0.6 source code and .wl files. Covers the nine user-approved breaking decisions from v0.5 (truthy overhaul, &&/|| short-circuit, IF(ERR,...) routing, ! canonical, AT_K rename from POP, string subscript read, explicit LET MUT, integer overflow -> E0035, ${} interpolation). Use when the user asks for WLWL code, a .wl file, a wlwl script, or anything targeting the wlwl-spec-v0.6 language (SHA-1 cdb548cb5161e61d836aad2208fd33adc0917861). Always finishes by running `wlwl run` to verify the output (primary check); `wlwl fmt --check` is best-effort because v0.6 has known formatter drift. Do NOT use for WLWL v0.5 or earlier -- those use different truthy rules, POP-not-AT_K, and lack LET MUT.
---

# Writing WLWL v0.6

## TL;DR

**Always DO:**
1. One statement per line; end each non-final statement with `;` (the last is optional).
2. Use `LET MUT(name, value)` for any binding that gets `SET`-rebinding later.
3. Use `AT_K(dict, key, default)` for safe dict access; treat `POP` as a back-compat alias.
4. Use `${expr}` inside double-quoted strings; escape `\$` for a literal `${`.
5. After writing, run `wlwl run <file>` and confirm it exits 0 with expected stdout.

**Never DO:**
1. Never write `LET(x, 1)` then `SET(x, 2)` -- it must be `LET MUT(x, 1)`.
2. Never write `if (x == 0)` or `==`-style comparisons; v0.6 has no `==` -- use `<(x, 0)`, `>(x, 0)`, etc.
3. Never write `POP(d, k, default)` in new code; it still works but is the old name.
4. Never write `IF(cond, then)` with an assumed error path -- `IF(cond, then, else)` is what catches `ERR(...)`.
5. Never claim a `.wl` file works without running `wlwl run` on it. Spec drift is real; verify.

## Canonical form (per spec section A.3, with v0.6 caveats)

The formatter **intends** to be normative: spec section A.3 says `wlwl fmt --check` is the canonical-form gate. In v0.6 the formatter is **mostly** idempotent but has known drift (see Verification loop below). Source files follow these rules; whether `wlwl fmt --check` accepts every legal source is a separate question.

- Each statement on its own line, terminated by `;` (last statement's `;` is optional).
- No leading indentation in **source** (the formatter may emit indentation on long `LET(...)` blocks, but source files are written flat).
- Comments (`// line` and `/* block */`) are fine in source. The formatter drops them when canonicalising (per spec A.3), and the `--check` arm strips them on both sides before comparing, so comments do not cause `W0053`.
- Strings, escapes, and nested `/* */` are preserved verbatim.
- `LET MUT(...)` keyword is preserved by the formatter.
- Trailing newline at EOF is optional (formatter tolerates missing).

Prefer `wlwl run` as the authoritative check; treat `wlwl fmt --check` as advisory in v0.6.

## The nine v0.6 decisions (full table in `reference.md`)

| # | Decision | Rule of thumb |
|---|----------|---------------|
| A | Truthy overhaul | `0`, `""`, `[]`, `NaN` are falsy; everything else truthy |
| B | `&&` / `\|\|` short-circuit | right side not evaluated when left decides |
| C | `IF(cond, then, else)` | an `ERR(...)` on the `then` path routes to `else` |
| D | `!` and `NOT` | both canonical; v0.6 emits no W0054 |
| E | `AT_K(d, k, default)` | new name; `POP` is the back-compat alias |
| F | String subscript | `s[i]` returns single-codepoint string; negative counts from end |
| G | `LET MUT(name, value)` | explicit; plain `LET` is immutable |
| H | Integer overflow | throws `ERR(E0035)` instead of saturating + W0015 |
| J | String interpolation | `"hello ${name}"`; multi-segment supported |

For the AST shapes (`Expr::Let.mut_`, `Literal::Interpolated(Vec<StrPart>)`,
`StrPart::{Text, Expr}`) and the lexer pitfalls (`Mut` contextual keyword,
`StrStart/StrEnd` single-pair bracket), see `reference.md`.

## Writing flow

Copy this checklist and tick items as you go:

```
WLWL writing progress:
- [ ] 1. Decide the AST shape (use the 9-decision table to pick operators)
- [ ] 2. Write the .wl file (one stmt per line, `;`, no leading indent)
- [ ] 3. Run `wlwl run <file>` -- MUST exit 0 with expected stdout
- [ ] 4. Optional: run `wlwl fmt --check <file>` -- best-effort; see Verification
- [ ] 5. If 3 fails: consult `reference.md` for the failing token/operator
```

Do not skip step 3. `wlwl run` is the source of truth. `wlwl fmt --check`
in v0.6 has known idempotency drift and may fail on syntactically valid
code; treat its result as advisory, not normative.

## Top 10 antipatterns (the bugs you will write without this list)

1. **Missing `MUT`.** `LET(c, 0); SET(c, +(c, 1))` -> E0010 "cannot SET immutable". Fix: `LET MUT(c, 0);`.
2. **Using `POP` in new code.** Works but the formatter / lint surfaces a deprecation hint. Use `AT_K(d, k, default)`.
3. **Comparing with `==`.** WLWL v0.6 has no `==` operator. Use `=(x, y)` for equality, `!=` via `<>`, ordering via `<`, `>`, `<=`, `>=`.
4. **Forgetting `${}` is a segment, not a directive.** `"hello $name"` is the literal string `"hello $name"`. Always write `"hello ${name}"`.
5. **Trying to interpolate an `ERR`.** `"${risky(-1)}"` inside `PRINT` will throw if `risky` returns `ERR`. Use `IF(IS_ERR(x), "...", FORMAT("...{0}...", [x]))` for safe interpolation.
6. **Assuming `IF(cond, then)` is two-arg.** It is three-arg; missing `else` makes the false branch `NIL` silently. For error-catching you want the `else`.
7. **Calling `SET` outside a closure.** `SET` rebinds the enclosing `LET MUT`. At top-level it works only if the binding is `LET MUT`.
8. **Mutating a captured `LET`.** Functions captured by closures keep the original binding; you cannot `SET` it from inside the callee unless it was `LET MUT`.
9. **Integer literals that overflow at parse time.** `9999999999999999999999` (20+ nines) exceeds `i64` and the parser emits E0035 before the program runs.
10. **Block comments eating the closing `}`.** Nested `/* */` is supported by the lexer, but a stray `/* ... */` between `${` and `}` inside an interpolation breaks the string scanner. Keep interpolations simple.

## String interpolation cheat sheet

- Basic: `"${name}"`, `"1 + 1 = ${+(1,1)}"`.
- Adjacent segments: `"${a}${b}${c}"` -- all three resolve.
- Escaped literal: `"escaped \${literal}"` -> `escaped ${literal}`.
- Nested arithmetic: `"result = ${*(+(x,1), 2)}"`.
- Multi-line: not supported in v0.6; concatenate with `+(s, "\n", s2)`.

If the formatter produces nested `StrStart/StrEnd` pairs around your
interpolation, your source has stray `${` inside an interpolation -- fix the
source, not the formatter output.

## Verification loop (the one thing you must not skip)

After writing, in this order:

```bash
# 1. PRIMARY check: does it run and produce the expected output?
wlwl run path/to/file.wl

# 2. SECONDARY check (best-effort): is the source already canonical?
wlwl fmt --check path/to/file.wl
```

`wlwl run` is the source of truth: if it exits 0 and stdout matches
expectations, the file is correct. `wlwl fmt --check` is best-effort --
in v0.6 the formatter has known **idempotency drift** for some
constructs: it rewrites `[]` -> `ARRAY()`, `-1` -> `-(0, 1)`,
`s[i]` -> `INDEX_GET(s, i)`, and long `LET(...)` into multi-line form,
and re-formatting its own output does NOT converge. If `fmt --check`
fails on syntactically valid source, treat the failure as a known
formatter-quirk -- not as a problem with your code. The agent should
NOT run `wlwl fmt` to "fix" the source in this case; doing so can
silently rewrite working code into something that still fails
`fmt --check`.

If `wlwl run` fails, the failure is real -- parse error, runtime
exception, or wrong output. Read the diagnostic, fix the source, retry.

## Common WLWL idioms (when in doubt, do this)

- **Iterate**: `FOR(x, LIST(1, 2, 3), PRINT(x))` (range-free, uses the list directly).
- **Conditional**: `IF(<(x, 0), "neg", IF(>(x, 0), "pos", "zero"))`.
- **Default for missing key**: `LET(v, AT_K(d, k, 0));`.
- **Catching an error**: `LET(s, risky(x)); IF(IS_ERR(s), FORMAT("caught: {0}", [ERR_PAYLOAD(s)]), s)`.
- **Mutable counter in a closure**: `LET MUT(c, 0); LET(tick, FUN((), (SET(c, +(c, 1)); c)));`.

## References

- **Full 9-decision table, AST shapes, lexer traps:** see `reference.md` in this folder.
- **Gold-standard example covering every v0.6 feature in one program:** see `interp.wl`.
- **Authoritative spec:** `../docs/standard/wlwl-spec-v0.6(SHA1_cdb548cb5161e61d836aad2208fd33adc0917861).md`.

When in doubt, copy a pattern from `interp.wl` -- it is the smallest file
that exercises every v0.6 feature, and `wlwl run interp.wl` produces the
expected output. Note that `wlwl fmt --check interp.wl` currently fails due
to the formatter idempotency drift described above -- do not treat that as
a bug in your own code.