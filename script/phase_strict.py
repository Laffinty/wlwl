"""重建「v0.11.3 追加批次工作项达成度」的**权威口径**。

背景:`docs/plan/README.md` 写着「**15 份方案 83 个工作项**中已完成 **78 项**
(M5 6、M6 42、M7 23、M8 7;按各 addendum §7 状态列**首个标记**机械统计,口径同
`phase_strict.py` —— **该脚本才是权威**,本文档的数字会过期,以脚本为准)」。
⚠ 但 `phase_strict.py` **从未进过仓库** ⇒ 那句「权威」无处可核。本脚本就是它。

## 口径(必须与 README 那句话逐字对应)

1. **工作项** = 各 addendum 的 `## 7` 检查点表里,编号列匹配
   `W-xx` 或 `L0-A-*` 的行。
   ⚠ `addendum-00` 的编号是 `L0-A-1 … L0-A-4`(六个),**不是** `W-xx`;
   ⚠ 同一张表里还有一行 `L0-B`(可变累加器)—— 它**独立于 M5**
   (表里自己写着「已立项,独立于 M5,排在 M5 之后」)⇒ **不计入** M5。
2. **状态** = 那一行的状态列(第 4 格)。
3. **分类** = 状态列里**最左边那个**状态标记(`✅ ⚠ ⏸ ❌ 🔴 🟡 🟢 ⬜ ⏳ ▶`)。
   ⚠⚠ **不是**「第一个后面跟着粗体标题的标记」—— 那是一个更宽松的正则,
   它会在「`✅` 开头、但正文里另有一个 `⚠ **…**`」的行上取到后者
   (`addendum-14` 的 W-07 就是这样)⇒ 口径会与 README 对不上。
4. **已完成** = 首个标记是 `✅`。
5. **里程碑映射**:M5 = `addendum-00`;M6 = `addendum-01..08`;
   M7 = `addendum-09..13`;M8 = `addendum-14`。

用法:
    python script/phase_strict.py [plan 目录]
默认 plan 目录 = 本脚本所在仓库的 `docs/plan`。
"""

import os
import re
import sys
from collections import Counter

MARKS = "✅⚠⏸❌🔴🟡🟢⬜⏳▶⛔"

# 里程碑 → 该里程碑收编的 addendum 编号区间
MILESTONES = [
    ("M5", "L0", range(0, 1)),
    ("M6", "L1 八份", range(1, 9)),
    ("M7", "L2 五份", range(9, 14)),
    ("M8", "L3", range(14, 15)),
]


def plan_dir() -> str:
    if len(sys.argv) > 1:
        return sys.argv[1]
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(os.path.dirname(here), "docs", "plan")


def split_row(line: str) -> list:
    """按**未转义**的 `|` 切表格行 —— 单元格里的 `\\|` 不是分隔符。"""
    return re.split(r"(?<!\\)\|", line)


def work_item_id(cells: list):
    """取这一行的工作项编号;不是工作项行就返回 None。

    ⚠ `L0-B` 明确**排除**(它独立于 M5)—— 理由写在 `addendum-00` 自己的表里。
    """
    if len(cells) < 4:
        return None
    first = cells[1].strip()
    m = re.match(r"^(W-\d+|L0-A-\d+[a-z]?)$", first)
    return m.group(1) if m else None


def first_mark(status: str) -> str:
    """状态列里**最左边**的那个标记。"""
    for ch in status:
        if ch in MARKS:
            return ch
    return "(无标记)"


def section_7(lines: list):
    """返回 `## 7` 那一节的所有行(不含标题行本身)。"""
    start = -1
    for i, l in enumerate(lines):
        if re.match(r"^##\s*7[.、\s]", l) or re.match(r"^##\s*7\s", l):
            start = i
            break
    if start < 0:
        return None
    out = []
    for l in lines[start + 1:]:
        if l.startswith("## "):
            break
        out.append(l)
    return out


def main() -> int:
    # ⚠⚠ **stdout 必须是 UTF-8** —— 输出的状态标记含 ✅ / ⛔ / 🟡,
    #   而 Windows 控制台默认编码是 **GBK** ⇒ 直接 `print` 会抛
    #   `UnicodeEncodeError: 'gbk' codec can't encode character '\u2705'`
    #   (本机 2026-10-20 实测:脚本在 Windows 上**根本跑不出结果** ⇒
    #   「权威口径」在业主自己的机器上是不可用的)。
    #   ⇒ `reconfigure(encoding="utf-8")` 把 stdout 切到 UTF-8;
    #   `errors="replace"` 兜底:万一还有别的字符,退化成 `?` 而不是崩。
    #   ⚠ `reconfigure` 是 **Python 3.7+** 的 API,本仓 MSRV 1.75 对应
    #   Python 3.9+ ⇒ 可用;仍用 hasattr 兜一层,免得在更老的解释器上崩。
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            try:
                stream.reconfigure(encoding="utf-8", errors="replace")
            except (ValueError, OSError):
                pass

    plan = plan_dir()
    files = sorted(
        f for f in os.listdir(plan) if f.startswith("addendum-") and f.endswith(".md")
    )
    per_file = {}
    for f in files:
        with open(os.path.join(plan, f), encoding="utf-8") as fh:
            lines = fh.read().split("\n")
        sec = section_7(lines)
        if sec is None:
            per_file[f] = None
            continue
        marks = Counter()
        for l in sec:
            cells = split_row(l)
            if work_item_id(cells) is None:
                continue
            marks[first_mark(cells[3])] += 1
        per_file[f] = marks

    print("%-46s %6s  %s" % ("方案", "工作项", "首个状态标记分布"))
    print("-" * 110)

    milestone_total = Counter()
    milestone_done = Counter()
    for f in files:
        marks = per_file[f]
        if marks is None:
            print("%-46s %6s  (无 §7)" % (f, "-"))
            continue
        n = sum(marks.values())
        detail = " | ".join("%s×%d" % (k, v) for k, v in marks.most_common())
        print("%-46s %6d  %s" % (f, n, detail))

        # addendum-NN → 里程碑
        m = re.match(r"^addendum-(\d+)", f)
        if not m:
            continue
        idx = int(m.group(1))
        for name, _label, rng in MILESTONES:
            if idx in rng:
                milestone_total[name] += n
                milestone_done[name] += marks.get("✅", 0)
                break

    print("-" * 110)
    grand_total = sum(milestone_total.values())
    grand_done = sum(milestone_done.values())
    print("按里程碑:")
    for name, label, _ in MILESTONES:
        print("  %-3s %-8s %2d / %-2d" % (name, label, milestone_done[name], milestone_total[name]))
    print()
    print("合计工作项 = %d" % grand_total)
    print("已完成(首个标记为 ✅) = %d / %d" % (grand_done, grand_total))
    unfinished = grand_total - grand_done
    print("未完成 = %d" % unfinished)
    for f in files:
        marks = per_file[f]
        if marks is None:
            continue
        bad = {k: v for k, v in marks.items() if k != "✅"}
        if bad:
            print("  ⚠ %s: %s" % (f, " | ".join("%s×%d" % (k, v) for k, v in bad.items())))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
