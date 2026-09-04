# `script/` — local development helpers

This directory is reserved for **reusable** one-off scripts that
help with `wlwl` development.  Anything here is intended to be
checked into the repo so future contributors (or future-you) can
re-run the same workflow.

## What belongs here

- One-off batch helpers (e.g. `regen-spec-alignment-tests.sh`).
- Dev-environment setup (e.g. `install-llvm-tools.ps1`).
- Cargo wrappers that aren't worth a custom Cargo subcommand.
- Coverage / lint report aggregators that are too small to ship
  in CI but useful locally.

## What does NOT belong here

- One-time P3-NNN scratch files used to land a single commit
  (e.g. `__fix_ast_tests.ps1` for P3-011).  Those should be
  **deleted** after the commit lands — they have no future
  value and only add noise.
- Cargo build artifacts, target dir, lock files — those are
  already in `.gitignore`.
- A copy of the spec for offline read — `docs/standard/` is
  the canonical location.

## Current contents

(empty — no reusable helpers yet)

## Naming

- Use lowercase kebab-case (`regen-coverage.sh`).
- Prefix with the workflow's domain when ambiguous
  (`unignore-spec-tests.sh`).
- For PowerShell helpers, suffix `.ps1` and use the
  `Set-StrictMode -Version Latest` prologue.

## When adding a script

1. Make it idempotent (safe to re-run).
2. Add a one-line header explaining what it does.
3. If it depends on external tools, fail fast with a clear
   "missing X" message rather than silently doing nothing.
4. Run it on a clean checkout to make sure the README
   instructions actually work.
