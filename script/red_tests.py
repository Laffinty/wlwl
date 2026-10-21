#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""红测清单:找出「因本次改动变红」的既有测试。

破坏性变更(如 `E0103`)的代价落在既有测试上,而归因必须**可复现**、不能靠眼力。
本脚本做两件事:

  1. 跑全量测试,抽出每个失败用例(目标 + 用例名 + 首条断言信息);
  2. 与一份**基线**清单做差 —— 只有**基线里没有、现在红了**的用例才是本次
     变���引入的。基线里的红是先前就有的(本仓当前就有若干历史红),不该算到
     本次头上。

用法:
    python script/red_tests.py --out .red-baseline.json      # 建基线
    python script/red_tests.py --baseline .red-baseline.json # 与基线做差

退出码:0 = 无「新增」红测;1 = 有新增红测(门禁用)。
"""

import argparse
import json
import os
import re
import subprocess
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
IMPL = os.path.join(REPO, "impl")

# `---- tests::foo stdout ----` 之后的 `panicked at ...` / 断言行
FAIL_HEAD_RE = re.compile(r"^---- (\S+) stdout ----$")
TEST_RESULT_RE = re.compile(r"^test result: .*?(\d+) failed")
FAILED_LIST_RE = re.compile(r"^    (\S+)$")


def run_tests():
    cmd = [
        "cargo", "test", "--locked", "--all-targets", "--no-fail-fast",
    ]
    p = subprocess.run(
        cmd, cwd=IMPL, capture_output=True, text=True,
        encoding="utf-8", errors="replace",
    )
    return p.stdout + "\n" + p.stderr


def parse_failures(text):
    """抽出 {目标: {用例名: 首条失败信息}}。"""
    out = {}
    target = None
    in_failures = False
    for line in text.split("\n"):
        line = line.rstrip()
        if line.startswith("     Running "):
            m = re.search(r"unittests (\S+)", line)
            target = m.group(1) if m else "unknown"
            in_failures = False
            continue
        if line.startswith("   Doc-tests "):
            m = re.search(r"Doc-tests (\S+)", line)
            target = m.group(1) if m else "doctests"
            in_failures = False
            continue
        m = FAIL_HEAD_RE.match(line)
        if m:
            in_failures = True
            out.setdefault(target, {})[m.group(1)] = ""
            continue
        if in_failures:
            if line.startswith("failures:") or line.startswith("test result:"):
                in_failures = False
                continue
            s = line.strip()
            if s and not out.get(target, {}).get(_last(out, target), ""):
                out[target][_last(out, target)] = s[:200]
    return out


def _last(out, target):
    keys = list(out.get(target, {}).keys())
    return keys[-1] if keys else ""


def diff(base, cur):
    """新增红 = 现在红了、且基线里没有。"""
    new = {}
    fixed = {}
    for target, cases in cur.items():
        b = base.get(target, {})
        for name, msg in cases.items():
            if name not in b:
                new.setdefault(target, {})[name] = msg
        for name in b:
            if name not in cases:
                fixed.setdefault(target, {})[name] = b[name]
    return new, fixed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", help="基线清单(.json)")
    ap.add_argument("--out", help="把本次清单写到这里(.json)")
    args = ap.parse_args()

    print("跑全量测试(cargo test --locked --all-targets --no-fail-fast)……", flush=True)
    text = run_tests()
    cur = parse_failures(text)
    n = sum(len(v) for v in cur.values())
    print(f"当前红测:{n} 条(目标 {len(cur)} 个)")

    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(cur, f, ensure_ascii=False, indent=2, sort_keys=True)
        print(f"已写基线:{args.out}")

    if not args.baseline:
        return 0

    with open(args.baseline, "r", encoding="utf-8") as f:
        base = json.load(f)
    new, fixed = diff(base, cur)
    nn = sum(len(v) for v in new.values())
    nf = sum(len(v) for v in fixed.values())
    print(f"\n**新增红测:{nn} 条** / 已转绿:{nf} 条")
    for target, cases in sorted(new.items()):
        print(f"\n-- {target}({len(cases)})")
        for name, msg in sorted(cases.items()):
            print(f"   {name}\n      {msg}")
    if fixed:
        print("\n-- 转绿 --")
        for target, cases in sorted(fixed.items()):
            for name in sorted(cases):
                print(f"   {target}::{name}")
    return 1 if nn else 0


if __name__ == "__main__":
    sys.exit(main())
