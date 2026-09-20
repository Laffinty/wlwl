# wlwl-skill

> An **Agent Skill bundle** for the WLWL programming language (v0.6).
> Loaded by Claude Code / Claude API / compatible agents to author v0.6-correct
> WLWL code on the first try.

This folder is **independent** — it does not belong to the Rust workspace
(`impl/`) or the spec/docs set (`docs/`). It is a standalone consumer of
the WLWL spec, designed to be dropped into any agent's skill loader
without rebuilding the toolchain.

---

## What is in here

```
wlwl-skill/
|-- README.md        <- you are here (human-facing overview)
|-- SKILL.md         <- agent-facing entry point, Claude Skills format
|-- reference.md     <- on-demand reference (operator/builtin catalogue,
|                      error codes, AST shapes); loaded only when
|                      SKILL.md points here
|-- interp.wll       <- golden reference program: 13 blocks (A-N) covering
|                      every v0.6 feature in one file
|-- CHANGELOG.md     <- skill-bundle changes (spec lives at docs/standard/)
|-- examples/        <- single-purpose .wll programs (run with `wlwl run`)
|   |-- truthiness.wll
|   |-- error_propagation.wll
|   |-- match.wll
|   |-- control_flow.wll
|   |-- import_stdlib.wll
|   `-- interpolation.wll
```

The top-level layout is intentionally **flat**. `examples/` is the only
subdirectory because each file in it is a standalone, copy-pasteable
reference for a specific v0.6 feature.

## How to use

### Drop-in for Claude Code

Place this directory under `~/.claude/skills/` (personal) or `.claude/skills/`
(project) so Claude Code auto-discovers it.

### Drop-in for the Claude API

Upload `SKILL.md` (with `reference.md`, `interp.wll`, and `examples/`
as supporting files) via the Skills API.

### As a standalone reference

Open `SKILL.md` in any markdown viewer; it reads top-to-bottom as a
field guide. `reference.md` is a lookup catalogue (sections §1–§18).

### Examples

Each `examples/<name>.wll` is a runnable, exit-0 program that demonstrates
one v0.6 feature in isolation:

| File | Demonstrates |
|------|--------------|
| `examples/truthiness.wll` | All 8 falsy values from §2.3 |
| `examples/error_propagation.wll` | §8.2 transparent ERR propagation; the "extract before passing" idiom |
| `examples/match.wll` | `MATCH` with literal/identifier/wildcard/array-`*rest`/dict/`OK`/`ERR` patterns |
| `examples/control_flow.wll` | `WHILE`, `FOR`, `RETURN`, `BREAK`, `CONTINUE` |
| `examples/import_stdlib.wll` | `IMPORT` plain form, `["orig":"alias"]` alias form, `wlwl:std.*` namespaces |
| `examples/interpolation.wll` | `${}` interpolation, escapes, nested expressions, `STR` quirks |

## Conventions inside this folder

- File names: lowercase + dot-separator (`SKILL.md` is uppercase because
  Claude Skills require that exact filename).
- Line endings: LF.
- `interp.wll` is **feature-augmented** relative to `impl/examples/interp.wll`
  (extra blocks K/L/M/N for `RETURN`, `MATCH`, `IMPORT`). The upstream
  `impl/examples/interp.wll` is a strict subset; sync changes both ways.
- The spec-vs-impl register lives at `../docs/plan/deviations.md`
  (211 KB). **Cite, do not load** — point agents at it for any divergence.

## Versioning

This skill targets **wlwl-spec-v0.6** (file at `../docs/standard/wlwl-spec-v0.6.md`).
When the spec ships a breaking change:

1. Bump the `description` frontmatter in `SKILL.md` (it carries the
   version marker inside the description text).
2. Refresh `interp.wll` and the affected `examples/*.wll` files.
3. Update `reference.md` only if new AST shapes, builtins, or error
   codes land.
4. Append a CHANGELOG entry here if the skill introduces new patterns.

Until then, treat v0.6 as the canonical reference and prefer fixing
examples over revising the skill.

## Why a skill at all

WLWL is a pre-release, single-implementer language. Without an explicit
agent-side guide, every code-writing session re-derives the v0.6
decisions from the spec — a 41KB text full of subtleties (truthy
overhaul, `IF(ERR,...)` semantics, the `LET MUT` requirement, the
`AT_K` rename, the §8 error model, `IMPORT` syntax, etc.). The skill
captures those decisions in ~200 lines so an agent produces
v0.6-correct output without re-reading the spec.

The verification loop in `SKILL.md` (`wlwl run` primary;
`wlwl fmt --check` advisory; `--format json|jsonl` for AI diagnostics)
is the single most valuable thing — it lets the agent self-correct
before claiming success.
