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
- `package_source.py` — **把源码树打进发布包**（缺口 G-06）。发布包原本只有
  二进制 + R1 stdlib 副本 + 例子 + 文档 ⇒ **没有编译器源码** ⇒ 离线状态下
  「读源码 / 改这个语言 / 核对某个行为怎么实现的」都不可行。
  本脚本随包发一份 `wlwl-src.tar.gz`（**排除构建产物**）＋ 一份
  `SOURCE_HASH`（**可复算**的树哈希：SHA-256 over 排序后的逐文件摘要清单）。
  用法：`python script/package_source.py --out staging/wlwl-src.tar.gz
  --hash-out staging/SOURCE_HASH`；核对：`python script/package_source.py
  --verify <src-dir> --hash-file SOURCE_HASH`。
  ⚠ 排除清单（`.git/`、`impl/target/`、`*.pyc`、
  `docs/wlwl-spec-reference.md`）**写进 `SOURCE_HASH` 的头里** ⇒ 不是暗箱。
  ⚠ **[v0.12.0] 依赖集随源码冻结**：`impl/Cargo.lock` 被 `.gitignore` 忽略、
  不入库 ⇒ **从全新 clone 打的包会静默地不含锁文件** ⇒ 用户重建时重新解析依赖 ⇒
  **不是同一个编译器**。⇒ 本脚本在打包前**保证**锁文件在树里（缺失就
  `cargo generate-lockfile` 生成；cargo 不可用 / 生成失败 ⇒ **响亮失败**，
  绝不静默发一个没有依赖集的包），并在 `SOURCE_HASH` 头里写明
  `# lock-file: impl/Cargo.lock` ⇒ 核对模式用**同一份**文件集。
  ⇒ 用户核对时若发现树里缺锁文件 ⇒ 直接报「重建出来的不是同一个编译器」。
- `check_conformance_registry.py` — **一致性册 §5.4「状态」列 ↔ 契约表的
  双向核对门禁**（缺口 G-25）。§5.4 的「状态」列是**手工维护**的 ⇒ 实现的代码
  收敛了、契约表也写了,但登记没跟着翻 ⇒ 登记会**长期落后于基线**（2026-10-25
  实测:14 条「未实现」里 13 条早已实现）。本脚本把每行差异挂到**可运行的契约
  测试**上 ⇒ **行里写着「未实现」而契约表全绿 ⇒ 门禁红**;反向也查（登记
  「已实现」而契约红 ⇒ 实现回退）。
  用法:`python script/check_conformance_registry.py`（`--list` 只列接线表、
  `--no-run` 只查接线完整性、`--only X-01,X-14` 只查指定行）。
  ⚠ 规范仓的查找顺序与 `build_spec_reference.py` 同款（命令行 → 环境变量
  `WLWL_SPEC_V1_DIR` → 本仓同级目录 `../wlwl-spec-v1`）;**找不到时响亮地
  失败**,不回落 —— 回落会让读数绿、实际没查。
  ⚠ 契约表**带 `#[ignore]` 的目标测试**用 `--ignored --exact <名字>` 单独跑
  （那是「缺口仍在」的判据:目标测试红 ⇒ 缺口仍在;转绿 ⇒ 缺口已闭合,该行
  必须翻成「已实现」）。
- `conformance_registry_map.json` — 上面那道门禁的**接线表**：§5.4 每一行差异
  ↔ 它的契约测试（包名 + target 名 + `expect`）。**人工维护**：§5.4 每新增一行，
  这里必须同时加一条，否则门禁红。`expect` 有三个取值：
  `satisfied`（契约断言规范要求 ⇒ 全绿 ⇒ 该行应「已实现」）/
  `gap`（契约里 `#[ignore]` 的目标测试断言要求**应当**成立 ⇒ 红 ⇒ 该行应「未实现」）/
  `no-contract`（文档类差异，无可挂判据，必须写 `reason`;状态为「未实现」时不允许）。
- `phase_strict.py` — **v0.11.3 追加批次工作项达成度的权威口径**。  `docs/plan/README.md` 写着「15 份方案 83 个工作项中已完成 78 项……口径同
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
