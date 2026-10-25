#!/usr/bin/env python3
r"""[门禁] 一致性册 §5.4 的「状态」列 ↔ 实现仓的契约表 —— 双向核对。

## 这道门禁在防什么

规范仓的一致性册 §5.4 是**差异登记** —— 逐条写着「规范要求什么 / 实现现状是什么 /
状态是什么」。⚠ **它的「状态」列是手工维护的** ⇒ 实现的代码收敛了、契约表也写了,
但**登记的那一列没跟着翻** ⇒ 登记就会**长期落后于基线**。

2026-10-25 实测（本仓 `v0.11.4` 基线）:§5.4 的 **14 条「未实现」里 13 条早已实现** ——
收敛代码与契约表**都在 tag 内**,只有登记没翻 ⇒ 照着登记做规划的人会去重做 13 件
已经做完的事,而**没有任何机制会发现这件事**。

⇒ 本门禁把「状态」列**接上机器**:每一行差异挂一个**可运行的契约测试** ⇒
**行里写着「未实现」而契约表全绿** ⇒ 门禁红。

⚠ **现状**（v0.12.0 / 2026-10-25）:§5.4 共 **25 行** —— **24 行「已实现」**,
只剩 **X-14**（成本类:目标测试 `#[ignore]` 且仍红）⇒ 登记表已经跟上实现。

## 判据（双向）

登记的「状态」列有**三态**（一致性册 §5.1）:「已实现」/「未实现」/「部分」。
⚠ **「部分」按「未实现」处理** —— 「行为存在但**不完全**一致」⇒ 仍需可核对
⇒ 不许 `no-contract`（必须挂契约表 / 目标测试）;契约全绿 ⇒ 判「登记过期」。

| 登记状态 | expect | 契约读数 | 判定 |
|---|---|---|---|
| 已实现 | satisfied | 绿 | ✔ |
| 未实现 | satisfied | 绿 | ✘ **登记过期** —— 要求已满足,登记却写「未实现」 |
| 未实现 | satisfied | 红 | ✔ 真未实现 |
| 已实现 | satisfied | 红 | ✘ **实现回退** —— 登记说已实现,契约却红 |
| 部分 | satisfied | 绿 | ✘ **登记过期** —— 要求已满足,登记却写「部分」（按「未实现」处理） |
| 部分 | satisfied | 红 | ✔ 真未完全一致（按「未实现」处理） |
| 未实现 / 部分 | gap | 目标测试**红** | ✔ 缺口仍在（被契约钉住） |
| 未实现 / 部分 | gap | 目标测试**绿** | ✘ **缺口已闭合但状态没翻** |
| 已实现 | gap | — | ✘ 自相矛盾（gap 行的状态不可能是「已实现」） |
| 任意 | no-contract | — | ⚠ 未挂契约（必须写明 reason）;状态为「未实现」/「部分」时 ✘（真缺口必须可核对） |
| 其它词 | — | — | ✘ **状态词不认识** —— §5.1 只有三态,不许蒙混成「未实现」 |

## 接线表

`script/conformance_registry_map.json` —— **人工维护**:§5.4 的每一行都必须登记一条。
新增差异行而不接线 ⇒ 门禁红（这正是「登记与实现脱节」的第一步）。

## 规范仓的查找顺序

与 `script/build_spec_reference.py` 同款:

1. 命令行参数 `<规范仓目录>`;
2. 环境变量 `WLWL_SPEC_V1_DIR`;
3. 本仓的**同级目录** `../wlwl-spec-v1`。

⚠ 找不到规范仓时**响亮地失败**（不回落、不静默跳过）—— 回落会让这道门禁
「读数绿、实际没查」。

## 用法

```bash
python script/check_conformance_registry.py                  # 核对（跑契约表）
python script/check_conformance_registry.py --list           # 只列接线表与差异行
python script/check_conformance_registry.py --no-run         # 只查接线完整性（不跑 cargo）
python script/check_conformance_registry.py --only X-01,X-14
```

退出码:0 = 全部一致;1 = 有 ✘;2 = 环境/输入错误（规范仓找不到、接线表缺行等）。
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

# ⚠ 控制台编码 —— Windows 的 zh-CN 控制台默认 **GBK** ⇒「✓」「✘」「⇒」编码不出来
#   ⇒ 打印**读数**这一步抛 UnicodeEncodeError ⇒ 门禁在**报结果**时崩掉
#   （比「判红」还糟：读数根本看不见）。⇒ 显式走 UTF-8（与 script/package_source.py 同款）。
try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = Path(__file__).resolve().parent.parent
IMPL = ROOT / "impl"
MAP_PATH = Path(__file__).resolve().parent / "conformance_registry_map.json"

ROW_RE = re.compile(r"^\|\s*(X-\d{2,})\s*\|")
RUNNING_RE = re.compile(r"Running\s+tests[\\/]\s*([A-Za-z0-9_]+)\.rs")
RESULT_RE = re.compile(r"^test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;")

# ⚠ 章节标题 —— `### 5.4 已登记的规范 ↔ 实现差异`。
#   判据是**编号逐字相等**（`5.4`），**不是前缀**（`startswith("5.4")`）——
#   前缀会把 **§5.4.1**（最小可复现用例）也算成 §5.4 ⇒ 那里若长出一张表，
#   它的行会被当成差异行 ⇒ 接线表凭空多出要求 ⇒ 门禁误红。
HEADING_RE = re.compile(r"^(#{2,6})\s+(\d+(?:\.\d+)*)(?:\s|$)")
SECTION_54 = "5.4"

# 「已实现」/「未实现」/「部分」——登记的**状态**列取值（一致性册 §5.1 的三态）。
STATE_IMPL = "已实现"
STATE_OPEN = "未实现"
STATE_PARTIAL = "部分"
# ⚠ 「部分」=「行为存在但**不完全**一致」⇒ **按「未实现」处理**：
#   仍需可核对（不许 no-contract），契约全绿时判「登记过期」。
OPEN_STATES = (STATE_OPEN, STATE_PARTIAL)
ALL_STATES = (STATE_IMPL, STATE_OPEN, STATE_PARTIAL)


def fail(msg):
    print("✘ " + msg, file=sys.stderr)
    sys.exit(2)


def find_spec_dir(argv):
    # ⚠ 判据是「**有**位置参数」，不是「位置参数多于一个」—— 位置参数**只有一个**
    #   (规范仓目录)，多于一个说明命令行写错了。
    if len(argv) >= 1:
        return Path(argv[0]).resolve()
    env = os.environ.get("WLWL_SPEC_V1_DIR")
    if env:
        return Path(env).resolve()
    return (ROOT.parent / "wlwl-spec-v1").resolve()


def parse_registry(text):
    """从一致性册里抽出 §5.4 的差异行。

    返回 (rows, saw_54)。**状态**取**最后两列**里的最后一列
    —— 前面几列是散文,里面可能出现 `|` ⇒ 不能按固定列号切。

    ⚠ 章节判据是**编号逐字相等**（`5.4`），**不是前缀**（`startswith("5.4")`）
      —— 后者会把 **§5.4.1**（最小可复现用例）一并算进来 ⇒ 那里若长出一张表，
      它的行会被当成 §5.4 的差异行 ⇒ 接线表凭空多出要求 ⇒ 门禁误红。
    ⚠ 找不到 §5.4 时**响亮地失败**（返回的 saw_54 为 False）—— 绝不静默放行。
    """
    rows = []
    in_section = False
    saw_54 = False
    in_fence = False
    for line in text.splitlines():
        # 代码围栏里的内容**不参与**章节判定（围栏内可能有任何字符）。
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if line.startswith("#"):
            # 任何标题都是**分节** ⇒ 只有编号**恰好**是 5.4 的那节算数。
            m = HEADING_RE.match(line)
            in_section = bool(m) and m.group(2) == SECTION_54
            if in_section:
                saw_54 = True
            continue
        if not in_section:
            continue
        m = ROW_RE.match(line)
        if not m:
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 3:
            continue
        state = cells[-1].replace("**", "").strip()
        category = cells[-2].replace("**", "").strip()
        rows.append({
            "id": cells[0],
            "cells": cells,
            "state": state,
            "category": category,
        })
    return rows, saw_54


def run_cargo(package, tests, impl_dir, extra=None):
    """跑一组契约测试,返回 {测试文件名: (ok, passed, failed)}。"""
    # ⚠ `--locked` —— 依赖解析**不许漂移**：没有它,cargo 可能就地改锁文件、
    #   解析出与 CI 不同的依赖集 ⇒ 门禁读数与 CI 不一致（而门禁自己看不出）。
    #   与本仓 CI / release 的口径一致（那边也是 `cargo test --locked …`）。
    cmd = ["cargo", "test", "--locked", "-p", package]
    for t in tests:
        cmd += ["--test", t]
    cmd += ["--no-fail-fast"]
    if extra:
        cmd += ["--"] + extra
    try:
        # ⚠ **stderr 必须并进 stdout**（单流）—— cargo 把「Running tests\X.rs」
        # 写在 **stderr**，而测试框架的结果写在 **stdout** ⇒ 分开收就**配不上**
        # （结果先于「Running」行出现 ⇒ 判定全部落空）。合并成一条流后，
        # 两者按写入时序交错 ⇒ 「Running X」之后的第一个「test result」就是 X 的。
        p = subprocess.run(cmd, cwd=str(impl_dir), stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, text=True, errors="replace",
                           encoding="utf-8")
    except FileNotFoundError:
        fail("找不到 cargo —— 这道门禁需要 Rust 工具链")
    out = p.stdout or ""
    results = {}
    cur = None
    for line in out.splitlines():
        m = RUNNING_RE.search(line)
        if m:
            cur = m.group(1)
            continue
        m = RESULT_RE.match(line.strip())
        if m and cur:
            results[cur] = (m.group(1) == "ok", int(m.group(2)), int(m.group(3)))
            cur = None
    if not results:
        # 契约表一个都没跑起来 —— 响亮地失败,不静默跳过。
        print(out[-3000:], file=sys.stderr)
        fail("契约表一个都没跑起来（cargo test 无输出）—— 门禁无法判定")
    return results


def main():
    argv = sys.argv[1:]
    only = set()
    list_only = False
    no_run = False
    args = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--list":
            list_only = True
        elif a == "--no-run":
            no_run = True
        elif a == "--only":
            i += 1
            only = set(x.strip().upper() for x in argv[i].split(",") if x.strip())
        else:
            args.append(a)
        i += 1

    spec_dir = find_spec_dir(args)
    spec_file = spec_dir / "docs" / "spec" / "wlwl-v1.0-conformance.md"
    if not spec_file.is_file():
        fail("找不到一致性册：%s\n"
             "  找过的位置：命令行给的 %s ／ 环境变量 WLWL_SPEC_V1_DIR=%s ／ 本仓同级目录 %s\n"
             "  取法：① 命令行给规范仓目录 ② 环境变量 WLWL_SPEC_V1_DIR "
             "③ 本仓同级目录 ../wlwl-spec-v1"
             % (spec_file, spec_dir, os.environ.get("WLWL_SPEC_V1_DIR", "(未设)"),
                ROOT.parent / "wlwl-spec-v1"))

    if not MAP_PATH.is_file():
        fail("找不到接线表：%s" % MAP_PATH)
    mapping = json.loads(MAP_PATH.read_text(encoding="utf-8"))
    rows_cfg = mapping.get("rows", {})

    registry, saw_54 = parse_registry(spec_file.read_text(encoding="utf-8"))
    if not saw_54:
        fail("一致性册里**找不到 §5.4 这一节**（标题形如 `### 5.4 …`）—— "
             "解析口径坏了（不静默放行）")
    if not registry:
        fail("一致性册的 §5.4 里没读到差异行 —— 解析口径坏了（不静默放行）")

    if only:
        registry = [r for r in registry if r["id"].upper() in only]

    # ── 1. 接线完整性 ────────────────────────────────────────────────
    missing = [r["id"] for r in registry if r["id"] not in rows_cfg]
    extra = [k for k in rows_cfg if k not in {r["id"] for r in registry}]
    if missing:
        fail("这些 §5.4 差异行**没有接线**（新增行必须同时在 %s 登记）：%s"
             % (MAP_PATH.name, ", ".join(missing)))
    if extra:
        print("⚠ 接线表里有 §5.4 不存在的行（多余的接线）：%s" % ", ".join(sorted(extra)),
              file=sys.stderr)

    if list_only:
        for r in registry:
            cfg = rows_cfg[r["id"]]
            print("%-6s 状态=%-6s expect=%-12s %s" % (
                r["id"], r["state"], cfg.get("expect"),
                cfg.get("note") or cfg.get("reason") or ""))
        return 0

    # ── 2. 跑契约表（按包分组,一次 cargo 跑多个 target）────────────────
    by_pkg = {}
    for r in registry:
        cfg = rows_cfg[r["id"]]
        if cfg.get("expect") == "no-contract":
            continue
        by_pkg.setdefault(cfg["package"], set()).update(cfg["tests"])

    contract_green = {}
    if not no_run:
        for pkg, tests in by_pkg.items():
            res = run_cargo(pkg, sorted(tests), IMPL)
            for t, v in res.items():
                contract_green[t] = v

    # gap 行的**目标测试**（#[ignore]）——用 --ignored 单独跑一遍。
    probes = {}
    if not no_run:
        for r in registry:
            cfg = rows_cfg[r["id"]]
            if cfg.get("expect") != "gap":
                continue
            probe = cfg.get("probe")
            if not probe:
                fail("%s 是 gap 行，但没写 probe（目标测试名）" % r["id"])
            res = run_cargo(cfg["package"], [cfg["tests"][0]], IMPL,
                            extra=["--ignored", "--exact", probe])
            probes[r["id"]] = res.get(cfg["tests"][0])

    # ── 3. 判定 ─────────────────────────────────────────────────────
    if no_run:
        # ⚠ `--no-run` **只查接线完整性** —— 契约表**没跑**，所以**没有读数**、
        #   也就**没有判定**。若在这里照样打印判定列，契约读数会是空的
        #   （⇒ 每行都被算成「契约没跑起来」）⇒ 那个表会**误导**读者
        #   「这些行是不是有问题」。⇒ 这一模式只报接线结果。
        print()
        print("✓ §5.4 的 %d 行差异**全部接线**（--no-run：没跑契约表，不作判定）" % len(registry))
        return 0

    print()
    print("%-6s %-8s %-11s %-14s %s" % ("行", "登记状态", "expect", "契约读数", "判定"))
    print("-" * 100)
    problems = []
    warnings = []
    for r in registry:
        cfg = rows_cfg[r["id"]]
        expect = cfg.get("expect")
        state = r["state"]
        if state not in ALL_STATES:
            # ⚠ §5.1 只有三态 —— 不认识的词**不许**蒙混成「未实现」放行。
            problems.append("%s 的状态词不认识：%r（§5.1 的三态只有「已实现」/「未实现」/「部分」）"
                            % (r["id"], state))
            verdict = "✘ 状态词不认识"
            print("%-6s %-8s %-11s %-14s %s" % (r["id"], state, expect or "—", "—", verdict))
            continue
        if expect == "no-contract":
            if state in OPEN_STATES:
                problems.append("%s 是「%s」却没有可挂的契约（真缺口必须可核对）"
                                % (r["id"], state))
                verdict = "✘ 无契约"
            else:
                warnings.append("%s 未挂契约（%s）" % (r["id"], cfg.get("reason", "")))
                verdict = "⚠ 未挂契约"
            print("%-6s %-8s %-11s %-14s %s" % (r["id"], state, expect, "—", verdict))
            continue

        if expect == "gap":
            pr = probes.get(r["id"])
            if pr is None:
                verdict = "✘ 目标测试没跑起来"
                problems.append("%s 的目标测试没跑起来" % r["id"])
            else:
                ok, passed, failed = pr
                reading = "红(%d 失败)" % failed if not ok else "绿(%d 通过)" % passed
                if state in OPEN_STATES and not ok:
                    verdict = "✔ 缺口仍在"
                elif state in OPEN_STATES and ok:
                    verdict = "✘ 缺口已闭合但状态没翻"
                    problems.append("%s 的目标测试已转绿 ⇒ 缺口已闭合，该行应转「已实现」" % r["id"])
                elif state == STATE_IMPL and ok:
                    verdict = "✘ 自相矛盾（gap 行不应是「已实现」）"
                    problems.append("%s 的 expect 是 gap，状态却是「已实现」" % r["id"])
                else:
                    verdict = "✘ 实现回退"
                    problems.append("%s 登记「已实现」，但目标测试红了" % r["id"])
            print("%-6s %-8s %-11s %-14s %s" % (r["id"], state, expect, reading, verdict))
            continue

        # satisfied
        parts = []
        green = True
        for t in cfg["tests"]:
            v = contract_green.get(t)
            if v is None:
                green = False
                parts.append("%s:没跑" % t)
            else:
                ok, passed, failed = v
                green = green and ok
                parts.append("%s:%s(%d/%d)" % (t, "绿" if ok else "红", passed, passed + failed))
        reading = " ".join(parts)
        if state == STATE_IMPL and green:
            verdict = "✔"
        elif state in OPEN_STATES and green:
            # ⚠ 「部分」与「未实现」同判 —— 要求已满足而登记没翻 ⇒ **登记过期**。
            verdict = "✘ 登记过期（要求已满足，登记却写「%s」）" % state
            problems.append("%s 登记「%s」，但契约表全绿 ⇒ 登记过期" % (r["id"], state))
        elif state == STATE_IMPL and not green:
            verdict = "✘ 实现回退（登记「已实现」但契约红）"
            problems.append("%s 登记「已实现」，但契约表红 ⇒ 实现回退" % r["id"])
        else:
            verdict = "✔ 真未实现" if state == STATE_OPEN else "✔ 真未完全一致（「部分」）"
        print("%-6s %-8s %-11s %-14s %s" % (r["id"], state, expect, reading, verdict))

    print()
    if warnings:
        print("⚠ 未挂契约的行（文档类差异,已登记理由）：")
        for w in warnings:
            print("    " + w)
        print()
    if problems:
        print("✘ %d 项不一致：" % len(problems))
        for p in problems:
            print("    - " + p)
        return 1
    print("✓ §5.4 的状态列与契约表一致（%d 行）" % len(registry))
    return 0


if __name__ == "__main__":
    sys.exit(main())
