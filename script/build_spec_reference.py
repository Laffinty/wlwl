#!/usr/bin/env python3
r"""[生成器] 由 v1.0 规范仓的 11 册生成 `docs/wlwl-spec-reference.md`。

## 为什么要合并

v1.0 规范**独立成仓**(11 册:5 册语言面 + 6 册标准库面)。本仓需要**一份**
可被机器按锚点取章节的规范载体 —— 仓内 23 处守卫(13 个契约测试 + 运行时
守卫 + 生成器)都按「章节文本」定位,若让它们去读 11 个分散的文件,锚点就得
跨文件拼。

⇒ 本脚本把 11 册合并成**一份**,并做两件必要的结构改造:

1. **二级标题加册别前缀** —— 三册各自都有 `## 0` / `## 1` / `## 11` / `## 16` /
   `## 17`,合并后必须可区分,否则 `text.find("## 11 ")` 会命中错误的那一册
   ⇒ **静默读错章节,测试照样绿**。前缀形如 `## 语言规范 · 11 诊断`、
   `## 标准库规范 · 11 \`std.encode\``。
   **三级标题(`### 15.1`)不加前缀** —— 测试按它们定位。

2. **章节按章号升序重排** —— v1.0 把标准库拆成 core/official/security/
   system/text 五册,册内顺序是 3,4,5,6,7,8,22 / 9,10 / 11 / 1,2,15,… ⇒ 直接
   拼接会让「下一个 `## N`」的**结束锚点**跨章串味(例如 §8 的结束锚点会
   落到 §22)。⇒ 按章号重排成 1..22,结束锚点才成立。

## 输入

**权威源是 v1.0 规范仓**(独立仓),不是本仓内的任何副本。查找顺序:

1. 环境变量 `WLWL_SPEC_V1_DIR`;
2. 本仓的**同级目录** `../wlwl-spec-v1`(与 `wlwl-error` 的 `spec_path()`
   同款 —— 码表守卫也这么找规范)。

⚠ 刻意**不**指向本仓内的任何暂存目录:仓内的规范副本是**不可被引用**的
(见 `CONTRIBUTING.md` 的 Ground rules 与
`impl/crates/wlwl-cli/tests/spec_ref_hygiene_guard.rs`)。

## 用法

```bash
python script/build_spec_reference.py                 # 用默认查找顺序
python script/build_spec_reference.py <规范仓目录>     # 显式指定
```

⚠ **本脚本是「重建」工具,不是「同步」工具** —— 它会**整体覆盖**
`docs/wlwl-spec-reference.md`。日常维护请直接编辑那个文件;只有在需要
**从规范仓整体重做**时才跑本脚本。
"""

import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "wlwl-spec-reference.md"

H2 = re.compile(r"^## (.+)$")

# ── 册别与前缀 ──────────────────────────────────────────────────────────
# 每个来源册 → (合并后的段名, 二级标题前缀)
LANG_PREFIX = "语言规范"
STDLIB_PREFIX = "标准库规范"
CONF_PREFIX = "一致性册"

HEADER = """# WLWL 规范参考（机器可读 / AI agent 用）

> **这份文件是什么**：本仓库的**次级规范载体** —— 把 v1.0 规范仓
> ([`Laffinty/wlwl-spec-v1`](https://github.com/Laffinty/wlwl-spec-v1))
> 的 **11 册**合并为**一份**，供 **AI agent 读**、供**机器守卫按锚点取
> 章节**。
>
> **权威处在规范仓**：本文件是**次级副本** —— 人读规范以规范仓为准
> （规范侧有裁决流程、有差异登记）；本文件供本仓的守卫与 agent 使用。
>
> **来源基线**：`wlwl-spec-v1` 的 **v1.0.0**（11 册）。
>
> **标准库附录 A 是冻结快照**（规范仓注册镜像册 §0.7：单源真相是规范自身，
> 表体**不得**由实现清单自动生成）⇒ 本仓**只抄不改**，也没有从实现派生表体的
> 生成器；本仓的守卫改为问两个问题：抄件是否逐字节忠实、实现成员集合是否与
> 快照相等。
>
> **生成方式**：[`../script/build_spec_reference.py`](../script/build_spec_reference.py)
> 由规范仓的 11 册机械重建。**日常维护请直接编辑本文件**（重建是整体覆盖）。
>
> **章节编号会撞，所以标题加了册别前缀**：各册各自都有 `## 0` / `## 1` …
> 而仓内 23 处守卫是**按 `## N` 文本定位**取章节的 ⇒ 合并后必须可区分。
> 故本文件的三段标题形如「语言规范 · 11 诊断」、
> 「标准库规范 · 11 `std.encode`」、「一致性册 · 5 规范与实现的差异登记」。
> **三级标题（`### 15.1` 等）不加前缀** —— 测试按它们定位。
>
> ⚠ **头部这段说明刻意不写出完整的 `##` 标题字面量** —— 契约测试用
> `text.find(<册别前缀 + 章号>)` 定位章节，头部若出现同样的字面量，
> `find` 会命中**头部**而不是正文 ⇒ 抽出错误区段（实测：`encode_contract`
> 曾因此抽到整本语言册的表，408 个假成员）。⇒ 头部一律用「」引号描述。
>
> **标准库规范的章节按章号升序** —— 规范仓把它拆成五册（core / official /
> security / system / text），册内顺序不是 1..22 ⇒ 直接拼接会让「下一个
> `## N`」的结束锚点跨章串味。

## 目录

- **语言规范**（「语言规范 · N」）—— 词法、语法、类型与值、求值规则、错误模型、
  模块系统、面向对象与行为类型、并发、复杂度与性能保证、内存与资源模型。
  **附录 A 文法**、**附录 G 全局内建注册表**在语言面内；**附录 H 操作语义**
  在并发面内。
- **一致性册**（「一致性册 · N」）—— 一致性定义、条款类型、稳定性政策、公共面、
  **规范与实现的差异登记**、验证要求。
- **标准库规范**（「标准库规范 · N」）—— 21 个命名空间、214 个成员的名字、签名、
  语义、失败口径与边界。**附录 A 成员注册镜像**（生成器产出）在标准库面末尾。
- **代理工作说明**（「代理工作说明 · N」）—— 给 AI agent 的实现机制、踩过的坑、
  实测数据。**它不是规范**：与前两册冲突时以人读规范为准。

---

"""


def split_sections(text):
    """把一本书切成 [(标题, 正文), ...];正文含 `### ` 子节。"""
    out, head, body = [], None, []
    for ln in text.split("\n"):
        m = H2.match(ln)
        if m:
            if head is not None:
                out.append((head, "\n".join(body)))
            head, body = m.group(1), []
        elif head is not None:
            body.append(ln)
    if head is not None:
        out.append((head, "\n".join(body)))
    return out


def num_key(head):
    """`11 诊断` → 11;`附录 A 文法` → ('A',);`总纲（0 导言）` → 最前;其余 → 末尾。"""
    if head.startswith("总纲"):
        return (0, -1)
    m = re.match(r"^(\d+)\s", head)
    if m:
        return (0, int(m.group(1)))
    m = re.match(r"^附录\s+([A-Z])\b", head)
    if m:
        return (1, ord(m.group(1)))
    return (2, 999)


def find_spec_dir(argv):
    if len(argv) > 1:
        return Path(argv[1]).resolve()
    env = os.environ.get("WLWL_SPEC_V1_DIR")
    if env:
        return Path(env).resolve()
    return (ROOT.parent / "wlwl-spec-v1").resolve()


def load_book(spec_dir, rel):
    p = spec_dir / "docs" / rel
    if not p.is_file():
        raise SystemExit(f"error: missing source book {p}")
    return split_sections(p.read_text(encoding="utf-8"))


def pick(sections, pred):
    return [(h, b) for h, b in sections if pred(h)]


def render(sections, prefix, banner):
    buf = [f"<!-- ===== {banner} ===== -->\n"]
    for head, body in sections:
        buf.append(f"## {prefix} · {head}\n")
        buf.append(rewrite_book_links(body).strip("\n") + "\n")
    return "\n".join(buf)


APPENDIX_A_NOTE = (
    "**表体是 v1.0 规范仓的冻结快照,本仓只抄不改。** 依据:规范仓注册镜像册 §0.7 ——"
    "「**本册附录 A 是 v1.0 的冻结快照,不是随实现更新的活镜像。本册的单源真相是"
    "本规范自身**」,规则 1:「表体**不得**由实现清单自动生成,**不得**为追平实现的"
    "当次改动而改写」。"
    "⇒ 本仓**没有**从实现派生表体的生成器:实现侧加成员**不能**悄悄改写这个快照。"
    "下面两个标记之间是**逐字抄来**的表体(标记本身是本仓的定位锚点 —— 规范仓那边"
    "故意不带)。"
    "本仓的守卫回答两个问题:① **抄件走样了吗** —— 合并册这一段逐字节 == 规范仓的"
    "注册镜像册;② **实现与快照一致吗** —— 实现的成员集合 == 快照的成员集合。"
    "两者不一致时,按一致性册 §5 的差异登记处理,**不**由镜像跟着实现漂移。"
)


# ── 跨册链接改写 ──────────────────────────────────────────────────────
# 规范仓的正文里有指向**同仓其它册**的相对链接(`../stdlib/xxx.md`、
# `wlwl-v1.0-language.md`)。合并成一份之后,那些路径在本仓**解析不了**
# ⇒ 全部成了死链。
#
# ⇒ 改写成「链接文字(本文件「<段名>」)」:文字保留(读者知道指向什么),
# 目标换成本文件内的定位。**章号未移**(v1.0 的章号与 v0.11 对齐)⇒ 链接
# 文字里带的 §N 仍然有效。
BOOK_TO_SECTION = {
    "wlwl-v1.0-overview.md": "语言规范 · 总纲",
    "wlwl-v1.0-language.md": "语言规范",
    "wlwl-v1.0-objects.md": "语言规范",
    "wlwl-v1.0-concurrency.md": "语言规范",
    "wlwl-v1.0-conformance.md": "一致性册",
    "wlwl-v1.0-stdlib-core.md": "标准库规范",
    "wlwl-v1.0-stdlib-text.md": "标准库规范",
    "wlwl-v1.0-stdlib-system.md": "标准库规范",
    "wlwl-v1.0-stdlib-security.md": "标准库规范",
    "wlwl-v1.0-stdlib-official.md": "标准库规范",
    "wlwl-v1.0-stdlib-registry.md": "标准库规范 · 附录 A",
}

# 册文件 → 册的中文名(当链接文字就是文件名时用它替换)
BOOK_TO_NAME = {
    "wlwl-v1.0-overview.md": "总纲",
    "wlwl-v1.0-language.md": "语言核心册",
    "wlwl-v1.0-objects.md": "对象册",
    "wlwl-v1.0-concurrency.md": "并发册",
    "wlwl-v1.0-conformance.md": "一致性册",
    "wlwl-v1.0-stdlib-core.md": "标准库·核心册",
    "wlwl-v1.0-stdlib-text.md": "标准库·文本册",
    "wlwl-v1.0-stdlib-system.md": "标准库·系统册",
    "wlwl-v1.0-stdlib-security.md": "标准库·安全册",
    "wlwl-v1.0-stdlib-official.md": "标准库·官方包册",
    "wlwl-v1.0-stdlib-registry.md": "标准库·注册镜像册",
}

BOOK_LINK = re.compile(
    r"\[([^\]]*)\]\((?:\.\./(?:spec|stdlib)/)?(wlwl-v1\.0-[a-z-]+\.md)\)"
)


def rewrite_book_links(text):
    """把指向规范仓册文件的相对链接改写成合并册内的定位。"""

    def sub(m):
        label, book = m.group(1), m.group(2)
        if label.strip("`") == book:
            # 链接文字就是文件名 ⇒ 换成册的中文名
            label = BOOK_TO_NAME.get(book, label)
        return f"{label}(本文件「{BOOK_TO_SECTION.get(book, book)}」)"

    return BOOK_LINK.sub(sub, text)


def mark_appendix_a(body):
    """给注册镜像册的附录 A 表体补上本仓的定位标记与说明。

    规范仓那边**故意不带**这两个标记 —— 那里是**冻结快照**(规范仓 §0.7
    规则 1:表体**不得**由实现清单自动生成)。本仓的生成器要按标记定位并
    替换表体 ⇒ 标记必须在本仓这一侧。

    ⚠ **只加标记与说明,不改表体一个字** —— 表体逐字来自规范仓。
    """
    lines = body.split("\n")
    # 找表格块:连续的以 `|` 开头的行
    start = None
    for i, ln in enumerate(lines):
        if ln.startswith("|"):
            start = i
            break
    if start is None:
        return body
    end = start
    for i in range(start, len(lines)):
        if lines[i].startswith("|"):
            end = i
        elif lines[i].strip() == "":
            continue
        else:
            break
    out = lines[:start] + [
        APPENDIX_A_NOTE,
        "",
        "<!-- appendix-a:begin -->",
    ] + lines[start:end + 1] + [
        "<!-- appendix-a:end -->",
    ] + lines[end + 1:]
    return "\n".join(out)


def main(argv):
    spec_dir = find_spec_dir(argv)
    if not spec_dir.is_dir():
        raise SystemExit(
            f"error: v1.0 spec repo not found at {spec_dir}\n"
            f"       设 WLWL_SPEC_V1_DIR 指向规范仓检出,或把规范仓检出到本仓同级目录。"
        )

    spec = spec_dir / "docs" / "spec"
    stdlib = spec_dir / "docs" / "stdlib"
    del spec, stdlib  # 路径由 load_book() 内部拼,这两个局部变量不再需要

    # ── 语言面 ─────────────────────────────────────────────────────────
    lang = []
    # 总纲(overview 册)给**独立标题** —— 它与语言核心册各有一份「## 0 导言」,
    # 合并后若都叫「语言规范 · 0 导言」,读者分不清哪份是哪册的。
    lang += [(f"总纲（{h}）", b) for h, b in
             pick(load_book(spec_dir, "spec/wlwl-v1.0-overview.md"),
                  lambda h: h.startswith("0 "))]
    lang += pick(load_book(spec_dir, "spec/wlwl-v1.0-language.md"),
                 lambda h: re.match(r"^(0|[1-9]|1[0-2]|18|19)\s", h)
                 or re.match(r"^附录 [ACEFG]\b", h))
    lang += pick(load_book(spec_dir, "spec/wlwl-v1.0-objects.md"),
                 lambda h: re.match(r"^1[3-6]\s", h))
    lang += pick(load_book(spec_dir, "spec/wlwl-v1.0-concurrency.md"),
                 lambda h: h.startswith("17 ") or re.match(r"^附录 H\b", h))
    lang.sort(key=lambda s: num_key(s[0]))

    # ── 一致性册 ───────────────────────────────────────────────────────
    conf = load_book(spec_dir, "spec/wlwl-v1.0-conformance.md")

    # ── 标准库面 ───────────────────────────────────────────────────────
    # 册首(每册一份「导言与总则」,含该册的定位说明)在前,章节按章号升序。
    stdlib_books = [
        ("stdlib/wlwl-v1.0-stdlib-core.md", "核心册"),
        ("stdlib/wlwl-v1.0-stdlib-official.md", "官方包册"),
        ("stdlib/wlwl-v1.0-stdlib-security.md", "安全册"),
        ("stdlib/wlwl-v1.0-stdlib-system.md", "系统册"),
        ("stdlib/wlwl-v1.0-stdlib-text.md", "文本册"),
    ]
    prefaces, chapters = [], []
    for rel, label in stdlib_books:
        secs = load_book(spec_dir, rel)
        prefaces += [(f"{h}（{label}）", b) for h, b in pick(secs, lambda h: h.startswith("0 "))]
        chapters += pick(secs, lambda h: re.match(r"^([1-9][0-9]*)\s", h))
    chapters.sort(key=lambda s: num_key(s[0]))
    appendix_a = pick(load_book(spec_dir, "stdlib/wlwl-v1.0-stdlib-registry.md"),
                      lambda h: h.startswith("附录 A"))
    # ⚠ 规范仓的注册镜像册**不带** `appendix-a` 标记(那儿是**冻结快照**,
    # 不是活镜像)⇒ 本仓必须自己补上这两个标记,否则:
    #   ① `stdlib_appendix_a_sync` 与 `result_contract` 两条锁测试找不到标记区 ⇒ 红;
    #   ② `gen-appendix-a` 无从下手(它按标记定位并替换表体)。
    # 标记只是**本仓的定位锚点**,不改表体一个字。
    appendix_a = [(h, mark_appendix_a(b)) for h, b in appendix_a]

    stdlib_all = prefaces + chapters + appendix_a

    # ── 代理工作说明 ───────────────────────────────────────────────────
    # ⚠ v1.0 规范包里**没有**代理册 —— 它是本仓自己的工作说明,不是规范内容。
    # ⇒ 沿用现行合并册里的那一段,原样保留。
    cur = OUT.read_text(encoding="utf-8")
    m = re.search(r"^## 代理工作说明 · 0 .*$", cur, re.M)
    agent = cur[m.start():] if m else "<!-- 代理工作说明:尚未写入 -->\n"

    parts = [
        HEADER,
        render(lang, LANG_PREFIX, "语言规范（v1.0:overview/language/objects/concurrency）"),
        render(conf, CONF_PREFIX, "一致性册（v1.0 新增）"),
        render(stdlib_all, STDLIB_PREFIX,
               "标准库规范（v1.0:core/official/security/system/text/registry）"),
        "<!-- ===== 代理工作说明（沿用本仓自有内容,不在 v1.0 规范包内）===== -->\n",
        agent,
    ]
    OUT.write_text("\n\n".join(parts), encoding="utf-8", newline="\n")
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")
    print(f"  语言面章: {len(lang)}  一致性册章: {len(conf)}  标准库面章: {len(stdlib_all)}")
    print(f"  语言章序: {', '.join(h.split()[0] for h, _ in lang)}")
    print(f"  标准库章序: {', '.join(h.split()[0] for h, _ in stdlib_all)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
