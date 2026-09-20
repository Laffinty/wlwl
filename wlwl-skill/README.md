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
|-- README.md     <- you are here (human-facing overview)
|-- SKILL.md      <- agent-facing entry point, Claude Skills format
|-- reference.md  <- on-demand reference (9-decision table, AST shapes,
|                   anti-patterns); agent reads only when SKILL.md points to it
|-- interp.wll     <- golden reference program: every v0.6 feature in one file
```

The directory is intentionally **flat** (no nested subdirectories).
Claude Skills progressive disclosure works off flat references too -- the
agent loads `SKILL.md` when WLWL is in scope, and follows the inline
references to `reference.md` / `interp.wll` only when needed.

## How to use

### Drop-in for Claude Code

Place this directory under `~/.claude/skills/` (personal) or `.claude/skills/`
(project) so Claude Code auto-discovers it.

### Drop-in for the Claude API

Upload `SKILL.md` (with `reference.md` and `interp.wll` as supporting
files) via the Skills API.

### As a standalone reference

Open `SKILL.md` in any markdown viewer; it reads top-to-bottom as a
field guide.

## Conventions inside this folder

- File names: lowercase + dot-separator (`SKILL.md` is uppercase because
  Claude Skills require that exact filename).
- Line endings: LF.
- `interp.wll` is a verbatim copy of `impl/examples/interp.wll` for
  reference parity; update both if you change the language.

## Versioning

This skill targets **wlwl-spec-v0.6** (SHA-1 `wlwl-spec-v0.6.md`).
When the spec ships a breaking change:

1. Bump the `description` frontmatter in `SKILL.md` (it carries the
   version marker inside the description text).
2. Refresh `interp.wll` to mirror the spec's `examples/interp.wll`.
3. Update `reference.md` only if new AST shapes or builtins land.
4. Add a CHANGELOG entry here if the skill introduces new patterns.

Until then, treat v0.6 as the canonical reference and prefer fixing
examples over revising the skill.

## Why a skill at all

WLWL is a pre-release, single-implementer language. Without an explicit
agent-side guide, every code-writing session re-derives the v0.6
decisions from the spec -- a 41KB text full of subtleties (truthy
overhaul, `IF(ERR,...)` semantics, the `LET MUT` requirement, the
`AT_K` rename, etc.). The skill captures those decisions in ~200 lines
so an agent produces v0.6-correct output without re-reading the spec.

The "always run `wlwl fmt --check && wlwl run`" verification loop is the
single most valuable thing in `SKILL.md` -- it lets the agent self-correct
before claiming success.
