# WLWL v0.6 Reference (on-demand)

This file is loaded by the agent only when `SKILL.md` points here. It
collects everything that does not fit the SKILL.md flow: full tables,
AST shapes, lexer traps, edge cases. Concise; the agent should skim and
return to `SKILL.md` for the actual workflow.

## Contents

- [§1. v0.6 decisions (full table)](#1-v06-decisions-full-table)
- [§2. AST shapes you must recognize](#2-ast-shapes-you-must-recognize)
- [§3. Lexer traps](#3-lexer-traps)
- [§4. Operators and builtins](#4-operators-and-builtins)
- [§5. Errors you will see](#5-errors-you-will-see)
- [§6. Anti-patterns (extended)](#6-anti-patterns-extended)

---

## §1. v0.6 decisions (full table)

| # | v0.5 -> v0.6 | Spec section | Code-level effect |
|---|--------------|--------------|---------------------|
| A | Truthy overhaul | §B.1 | `IF(0, ...)` is false; `IF("", ...)` is false; `IF([], ...)` is false; `IF(NaN, ...)` is false. Only `0`/`""`/`[]`/`NaN`/`FALSE`/`NIL`/`ERR(...)` are falsy. |
| B | `&&` / `\|\|` short-circuit | §B.2 | Right operand evaluated only when left cannot decide. Side-effects in the right operand may not run. |
| C | `IF(cond, then, else)` catches `ERR` | §B.3 | If `then` evaluates to `ERR(...)`, control jumps to `else` (no exception, no panic). Use `IS_ERR(x)` and `ERR_PAYLOAD(x)` to introspect. |
| D | `!` and `NOT` both canonical | §B.4 | `!(FALSE)` and `NOT(FALSE)` both yield `TRUE`. The formatter does not emit W0054. |
| E | `POP` -> `AT_K` rename | §B.5 | `AT_K(dict, key, default)` is the v0.6 name; `POP(dict, key, default)` is preserved as a `ResolvedCompat` alias for back-compat scripts. |
| F | String subscript | §B.6 | `s[i]` returns a single-codepoint string; `s[-1]` is the last codepoint; out-of-range yields `""`. |
| G | Explicit `LET MUT` | §B.7 | `LET MUT(x, v)` for mutable bindings. Plain `LET` is immutable forever. `SET(x, v)` requires `LET MUT`. |
| H | Integer overflow -> `ERR(E0035)` | §B.8 | Arithmetic that overflows `i64` returns `ERR(E0035)` instead of saturating. v0.5 emitted `W0015` and saturated. |
| J | `${expr}` interpolation | §B.9 | `"hello ${name}"` becomes one `Literal::Interpolated(Vec<StrPart>)`. Multi-segment works: `"${a}${b}${c}"`. Escape `\$` for a literal `$`. |

## §2. AST shapes you must recognize

The parser produces these shapes that have no v0.5 analogue:

```rust
// Expr::Let carries an optional mut_ flag (v0.6 G).
enum Expr {
    Let { name: Ident, value: Box<Expr>, mut_: bool },
    LetPattern { ... mut_: bool },
    // ... unchanged
}

// String literals split into text / expr segments (v0.6 J).
enum Literal {
    Str(String),                          // "no interpolation"
    Interpolated(Vec<StrPart>),           // "hello ${name}!"
}

enum StrPart {
    Text(String),                         // raw chars between ${...}
    Expr(Box<Expr>),                      // the ${...} payload
}
```

Implication for the formatter: `Literal::Interpolated` must re-emit with
the `StrText` token escaping rules intact. See `wlwl-formatter` for the
canonical re-emit logic.

## §3. Lexer traps

1. **`Mut` is a contextual keyword.** Lexer always lexes `Mut` as a token;
   the parser consumes it only between `LET` and `(`. Anywhere else,
   `MUT` is a regular identifier. So `LET MUT(x, 1)` parses as a mutable
   binding, while `LET(mUT, 1)` is a plain `LET` of an identifier
   named `mUT`.

2. **Interpolated strings emit exactly one `StrStart`/`StrEnd` pair** --
   one per literal, regardless of how many `${...}` segments appear
   inside. The lexer bug fixed in commit `98e124a` (P5-V06-002) used to
   emit nested pairs, which caused adjacent segments to misparse as
   expressions.

3. **Nested block comments** are supported: `/* outer /* inner */ still */`
   is one comment. Indentation does not affect the lexer.

4. **Comment-only lines are dropped** by the canonical formatter (per §A.3
   and the v0.6 `strip_comments` rule). Source may carry comments; they
   do not survive `wlwl fmt --check`. The check tolerates the dropped
   comments and one trailing-newline difference.

5. **String-literal escapes** are processed before any token-level work.
   `\"`, `\\`, `\n`, `\t`, `\${` are recognized. A backslash followed by
   anything else is an error (E0007).

## §4. Operators and builtins

Comparison (v0.6 has **no** `==`):

| Want | Use |
|------|-----|
| equality | `=(x, y)` |
| inequality | `<>(x, y)` |
| less than | `<(x, y)` |
| less or equal | `<=(x, y)` |
| greater than | `>(x, y)` |
| greater or equal | `>=(x, y)` |

Arithmetic:

| Op | Result on overflow |
|----|---------------------|
| `+(i, j)` | `ERR(E0035)` |
| `-(i, j)` | `ERR(E0035)` |
| `*(i, j)` | `ERR(E0035)` |
| `/(i, 0)` | `ERR(E0009)` (division by zero) |
| `%(i, 0)` | `ERR(E0009)` |

Boolean:

| Op | Semantics |
|----|-----------|
| `&&(a, b)` | short-circuit; a is evaluated first |
| `\|\|(a, b)` | short-circuit; a is evaluated first |
| `!(x)` / `NOT(x)` | logical negation; identical |

Collections:

| Builtin | Signature | Notes |
|---------|-----------|-------|
| `AT_K(d, k, default)` | dict, key, default | v0.6 name |
| `POP(d, k, default)` | dict, key, default | back-compat alias for AT_K |
| `LEN(x)` | collection -> int | string -> codepoint count |
| `LIST(a, b, c)` | values -> list | literal |
| `["k": v]` | dict literal | |
| `s[i]` | string subscript | single codepoint, negative ok |

String formatting:

| Builtin | Notes |
|---------|-------|
| `FORMAT(template, args)` | `{0}`, `{1}` placeholders; `args` is a list |
| `"${expr}"` | preferred; multi-segment supported |

## §5. Errors you will see

| Code | Meaning | Common cause |
|------|---------|--------------|
| E0009 | division by zero | `/(x, 0)` or `%(x, 0)` |
| E0010 | parse / type error | missing `MUT` before `SET`; mismatched delimiters |
| E0035 | integer overflow | `+(i64::MAX, 1)` and friends |
| E0042 | file IO error | `wlwl fmt` / `wlwl run` cannot read input |
| W0053 | source not canonical | `wlwl fmt --check` failed; run `wlwl fmt` to rewrite |

All `ERR(...)` values can be caught by `IF(cond, then, else)` because
`ERR` is falsy in v0.6 (decision A). To recover the payload, use
`ERR_PAYLOAD(x)` after `IS_ERR(x)` returns `TRUE`.

## §6. Anti-patterns (extended)

Beyond the top-10 in `SKILL.md`:

- **Top-level `SET`.** Allowed only after `LET MUT`. Without it, E0010.
- **Rebinding across `LET` shadowing.** `LET(x, 1); LET(x, 2)` is legal
  and creates a new immutable binding; the old `x` is gone. To mutate,
  you must `LET MUT` first.
- **`IF` without `else` for error routing.** If `then` returns `ERR`,
  v0.6 still routes to `else`. Without `else`, you get `NIL` silently
  and no diagnostic.
- **String subscript on a non-string.** `42[0]` is a runtime error.
  Convert with `STR(n)` first if needed.
- **`LIST(1, 2)[2]` out of bounds.** Returns `NIL` (no exception); check
  with `IS_NIL(x)` or just bound-check the index.
- **Misnesting block comments.** `/* /* */ */` is legal; `/* /* */ ` (no
  matching `*/`) is a parse error. The lexer reports E0011.
- **Mixing LF / CRLF in source.** The canonical formatter normalises to
  LF; CRLF source still passes `fmt --check` (strip_comments tolerates
  the difference). Avoid CRLF in new files.

## Notes on `interp.wll`

The bundled `interp.wll` is a verbatim copy of `impl/examples/interp.wll`.
Each labelled block exercises one v0.6 decision. When the agent writes
new code, the fastest sanity check is: "does this match the shape of
the corresponding block in `interp.wll`?"
