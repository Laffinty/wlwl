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
- A copy of the spec for offline read — `docs/wlwl-spec-reference.md` is
  the canonical in-repo location (the merged copy of the v1.0 spec).

## Current contents

- `build_spec_reference.py` — **重建合并册 `docs/wlwl-spec-reference.md`**:
  把规范仓(`Laffinty/wlwl-spec-v1`,v1.0)的 **11 册**机械合并成**一份**,
  供机器守卫与 AI agent 按锚点取章节。规范仓目录按 `WLWL_SPEC_V1_DIR`
  环境变量 → 仓同级 `../wlwl-spec-v1` 的顺序查找。
  用法:`python script/build_spec_reference.py [规范仓目录]`。
  ⚠ **它整体覆盖目标文件** ⇒ 日常维护请直接编辑
  `docs/wlwl-spec-reference.md`;只有在需要**从规范仓整体重建**时才跑它。
- `phase_strict.py` — **v0.11.3 追加批次工作项达成度的权威口径**。
  `docs/plan/README.md` 写着「15 份方案 83 个工作项中已完成 78 项……口径同
  `phase_strict.py` —— **该脚本才是权威**」,而那个脚本**从未进过仓库**
  (`git log --all --diff-filter=A -- '*phase_strict*'` 零结果)⇒ 那句
  「权威」一直无处可核。本脚本把它补回来,并把口径写在 docstring 里。
  用法:`python script/phase_strict.py [plan 目录 ...]`(默认 = 仓内 `docs/history`
  + `docs/active`)。⚠ 2026-10-20 起方案文件**按收口状态分了两处**:已收口的
  14 份 addendum 在 `docs/history/`,唯一未收口、仍在排队的 `addendum-12`
  (`std.sync`)在 `docs/active/` ⇒ 权威口径**两处都要读**,少一处就会少算工作项;
  `docs/plan/` 现在**只剩 README.md**(索引)。
  ⚠ 口径是**状态列里最左边的那个标记** —— 不是「第一个后面跟着粗体标题的
  标记」,后者在 `addendum-14` 的 W-07 那种「`✅` 开头、正文里另有一个
  `⚠ **…**`」的行上会取错,算出来的数与 README 对不上。

## Naming

- Use lowercase kebab-case (`regen-coverage.sh`).
- Prefix with the workflow's domain when ambiguous
  (`unignore-spec-tests.sh`).
- For PowerShell helpers, suffix `.ps1` and use the
  `Set-StrictMode -Version Latest` prologue.
- **Python helpers** use `snake_case.py` (`phase_strict.py`) — the
  kebab-case rule above is for shell/PowerShell entry points.  ⚠ When a
  filename is already cited by name in `docs/`, the **cited** spelling
  wins (`phase_strict.py` is referenced from `docs/plan/README.md`).

## When adding a script

1. Make it idempotent (safe to re-run).
2. Add a one-line header explaining what it does.
3. If it depends on external tools, fail fast with a clear
   "missing X" message rather than silently doing nothing.
4. Run it on a clean checkout to make sure the README
   instructions actually work.
