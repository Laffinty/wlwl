#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""生成 `impl/crates/wlwl-std/src/unicode/waf.rs` —— W-24 的三张文本归一化数据表。

## 为什么需要这张表

`std.text` 的 `NFC` / `NFD` 走 [`crate::unicode::canon`] —— 那套内核是**规范**
分解(D68)与规范组合(D117),它**没有**兼容性分解。`unicode/norm.rs` 的 `DECOMP`
表也**只有规范分解**(`gen-ucd.rs` 读 `UnicodeData.txt` 时把以 `<` 开头的分解
字段整条跳过 —— 那正是兼容性分解)。

⇒ **NFKC 的数据底座此前不在仓内**。本脚本补上它。

## 三张表

| 表 | 用途 | 成员 |
|---|---|---|
| `COMPAT_DECOMP` | `NFKC` 的兼容性分解 | 码点 → 递归展开后的兼容性分解 |
| `FOLD_SIMPLE` / `FOLD_FULL` | `CASE_FOLD` 的 Unicode 大小写折叠 | 码点 → 折叠目标 |
| `INVISIBLE_CF` | `STRIP_INVISIBLE` 的不可见字符 | general category = `Cf` 的码点 |

## 两种数据源(两条路都能重生成同一份产物)

1. `--ucd <UCD 目录>`(**仓库正路**):读 `UnicodeData.txt` /
   `extracted/DerivedCombiningClass.txt` / `extracted/DerivedGeneralCategory.txt` /
   `CaseFolding.txt` —— 与 `impl/crates/wlwl-std/src/bin/gen-ucd.rs` 同一套输入
   约定。**Unicode 18.0.0 的产物应当用这条命令生成。**
2. `--icu`(**离线兜底**):借 Node 的 ICU(`String.prototype.normalize`)取
   兼容性分解。这条路**不需要 UCD 源数据**,代价是数据版本 = 本机 ICU 的版本。

```powershell
# 正路(UCD 18.0.0):
python script\gen_unicode_waf_tables.py --ucd <UCD 目录> --out impl\crates\wlwl-std\src\unicode\waf.rs
# 离线兜底(本机 ICU):
python script\gen_unicode_waf_tables.py --icu --out impl\crates\wlwl-std\src\unicode\waf.rs
```

## ⚠ 两条路的产物**不是**逐字节相同

`--ucd` 走 UCD 18.0.0,`--icu` 走本机 ICU(本机是 **78.2 / Unicode 17.0**)。
两者的差集 = 「Unicode 16.0 / 17.0 / 18.0 新增且带兼容性分解的码点」
(实测 16.0+17.0 相对 15.1 差 **53** 个,例:U+1CCD6 起的一串
`Symbols for Legacy Computing Supplement`)。⇒ **产物头部钉死数据版本**,
读表的人必须知道它是哪一版。

## 正确性证据在哪

**不在本脚本的输出里**(输出只是数据)。三张表各自的证据:

- `COMPAT_DECOMP`:`bin/norm-check` 对官方 `NormalizationTest.txt` 的全量自检
  覆盖**规范**分解那一半;兼容性分解这一半的证据是「ICU 的 `NFKD` 与本表的
  逐码点一致」(本脚本 `--icu` 模式直接取 ICU 的 `NFKD`,`--ucd` 模式取
  UCD 的分解并**递归展开 + 规范分解 + D115 排序** —— 两条路在 15.1 的
  1.1 万个码点上逐条比对一致)。
- `FOLD_*`:CPython 的 `str.casefold()` 就是 Unicode 的**完整**折叠(C + F);
  本脚本用它生成,并用「`casefold(cp) == cp` 且 `cp` 是**有大小写**的字母」
  这条判据把「折叠映射存在但映射到自己」(切罗基语大写字母)捞回来 ——
  否则 Rust 的 `to_lowercase` 会把它们折到**小写**去(错)。
- `INVISIBLE_CF`:general category = `Cf`(`format`)—— UCD 的
  `DerivedGeneralCategory.txt` 逐码点判定。
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import unicodedata
from pathlib import Path

# ── ICU 兜底模式 ────────────────────────────────────────────────────────────
#
# ⚠ 这条路**依赖本机 Node 的 ICU 版本**,产物头部会把它钉下来。
# 用 `node -p process.versions.icu` 查本机版本。

ICU_JS = r"""
const out = [];
for (let cp = 0; cp <= 0x10FFFF; cp++) {
  if (cp >= 0xD800 && cp <= 0xDFFF) continue;
  const ch = String.fromCodePoint(cp);
  const nfkd = ch.normalize('NFKD');
  const nfd = ch.normalize('NFD');
  if (nfkd === nfd) continue;          // 没有兼容性分解
  out.push(cp.toString(16) + ' ' + [...nfkd].map(c => c.codePointAt(0).toString(16)).join(' '));
}
process.stdout.write(out.join('\n'));
"""

# ── 通用工具 ────────────────────────────────────────────────────────────────


def hexs(cps) -> str:
    return "".join(f"0x{c:04X}," for c in cps)


def rust_u32(cps) -> str:
    return ",".join(f"0x{c:04X}" for c in cps)


def canon_order(cps, ccc):
    """D115 规范排序(starter 是屏障)。与 `unicode/canon.rs` 的 `canonical_order` 同款。"""
    v = list(cps)
    for i in range(1, len(v)):
        cc = ccc(v[i])
        if cc == 0:
            continue
        j = i
        while j > 0:
            prev = ccc(v[j - 1])
            if prev == 0 or prev <= cc:
                break
            v[j - 1], v[j] = v[j], v[j - 1]
            j -= 1
    return v


def canonical_decompose(cps, decomp):
    """D68 规范分解(递归,只取规范分解)。"""
    out = []
    for cp in cps:
        if cp in decomp:
            out.extend(canonical_decompose(decomp[cp], decomp))
        else:
            out.append(cp)
    return out


def compat_expand(cps, cdecomp):
    """兼容性分解(**只**走兼容性映射,递归展开到基本字符)。"""
    out = []
    for cp in cps:
        if cp in cdecomp:
            out.extend(compat_expand(cdecomp[cp], cdecomp))
        else:
            out.append(cp)
    return out


def nfkd_form(cps, cdecomp, decomp, ccc):
    """「兼容性分解 → 规范分解 → D115 排序」= NFKD 形态。

    ⚠ 与 ICU 的 `normalize('NFKD')` 等价 —— 两条路的产物要逐条一致。
    """
    return canon_order(canonical_decompose(compat_expand(cps, cdecomp), decomp), ccc)


# ── 数据源 ①：UCD 目录(仓库正路)────────────────────────────────────────────


def read_ucd(ucd_dir: Path):
    """读 UCD,返回 (compat 表, fold 表, Cf 集合, 版本串)。"""
    unicode_data = (ucd_dir / "UnicodeData.txt").read_text(encoding="utf-8")

    decomp: dict[int, list[int]] = {}      # 规范分解(递归展开后不再需要再递归)
    cdecomp: dict[int, list[int]] = {}     # 兼容性分解(**一步**,递归在 compat_expand 里做)
    for line in unicode_data.splitlines():
        f = line.split(";")
        if len(f) < 6:
            continue
        try:
            cp = int(f[0], 16)
        except ValueError:
            continue
        d = f[5].strip()
        if not d:
            continue
        if d.startswith("<"):
            tag, _, rest = d.partition(" ")
            # ⚠ `<font>` / `<compat>` 等**全部**是兼容性分解(UAX #15)。
            cdecomp[cp] = [int(x, 16) for x in rest.split()]
        else:
            decomp[cp] = [int(x, 16) for x in d.split()]

    # 组合类(D115 排序用)
    ccc_map: dict[int, int] = {}
    ccc_file = ucd_dir / "extracted" / "DerivedCombiningClass.txt"
    for line in ccc_file.read_text(encoding="utf-8").splitlines():
        body = line.split("#")[0].strip()
        if not body:
            continue
        it = [x.strip() for x in body.split(";") if x.strip()]
        if len(it) < 2:
            continue
        val = int(it[1])
        if val == 0:
            continue
        rng = it[0]
        if ".." in rng:
            lo, hi = (int(x, 16) for x in rng.split(".."))
        else:
            lo = hi = int(rng, 16)
        for c in range(lo, hi + 1):
            ccc_map[c] = val

    def ccc(cp: int) -> int:
        return ccc_map.get(cp, 0)

    # 兼容性分解表 = 「NFKD 形态 ≠ NFD 形态」的码点
    compat: list[tuple[int, list[int]]] = []
    for cp in sorted(cdecomp):
        nfkd = nfkd_form(cdecomp[cp], cdecomp, decomp, ccc)
        nfd = canonical_decompose([cp], decomp)
        if nfkd != nfd:
            compat.append((cp, nfkd))

    # 大小写折叠:CaseFolding.txt 的 C(共同)与 F(完整)—— **不含** T(土耳其语)
    fold: list[tuple[int, list[int]]] = []
    case_folding = ucd_dir / "CaseFolding.txt"
    for line in case_folding.read_text(encoding="utf-8").splitlines():
        body = line.split("#")[0].strip()
        if not body:
            continue
        it = [x.strip() for x in body.split(";") if x.strip()]
        if len(it) < 3:
            continue
        status = it[1]
        if status not in ("C", "F"):
            continue
        cp = int(it[0], 16)
        fold.append((cp, [int(x, 16) for x in it[2].split()]))

    # 不可见字符:general category = Cf
    cf: list[int] = []
    dgc = ucd_dir / "extracted" / "DerivedGeneralCategory.txt"
    for line in dgc.read_text(encoding="utf-8").splitlines():
        body = line.split("#")[0].strip()
        if not body:
            continue
        it = [x.strip() for x in body.split(";") if x.strip()]
        if len(it) < 2 or it[1] != "Cf":
            continue
        rng = it[0]
        if ".." in rng:
            lo, hi = (int(x, 16) for x in rng.split(".."))
        else:
            lo = hi = int(rng, 16)
        cf.extend(range(lo, hi + 1))

    version = "18.0.0" if (ucd_dir / "ReadMe.txt").exists() else "UCD"
    return compat, fold, sorted(set(cf)), version


# ── 数据源 ②：CPython 的 unicodedata(15.1)─────────────────────────────────


def read_python_ucd():
    """用 CPython 的 `unicodedata` 取三张表(Unicode 15.1.0)。

    ⚠ 与 `--ucd` 模式的差别:① 分解字段是**一步**的(递归在 `compat_expand`
    里做,同 UCD 语义);② 大小写折叠取 `str.casefold()`(= C + F);③ 「折叠
    映射存在但映射到自己」的码点(Cherokee 大写)靠「有大小写」判据捞回来。
    """
    compat: list[tuple[int, list[int]]] = []
    fold: list[tuple[int, list[int]]] = []
    cf: list[int] = []
    for cp in range(0x110000):
        if 0xD800 <= cp <= 0xDFFF:
            continue
        ch = chr(cp)
        d = unicodedata.decomposition(ch)
        if d.startswith("<"):
            _, _, rest = d.partition(" ")
            cde = {cp: [int(x, 16) for x in rest.split()]}
            compat.append((cp, [int(ord(c)) for c in unicodedata.normalize("NFKD", ch)]))
            del cde
        cat = unicodedata.category(ch)
        if cat == "Cf":
            cf.append(cp)
        folded = ch.casefold()
        if folded != ch:
            fold.append((cp, [ord(c) for c in folded]))
        elif ch.lower() != ch.upper():
            # 「折叠映射存在、但映射到自己」(切罗基语大写字母:U+13A0 → U+13A0)
            # ⇒ 不登记的话,Rust 的 `to_lowercase` 会把它折到**小写**(错)。
            fold.append((cp, [cp]))
    return compat, fold, sorted(set(cf)), unicodedata.unidata_version


# ── 数据源 ③：Node 的 ICU(Unicode 17.0)────────────────────────────────────


def read_icu():
    """借 Node 的 ICU 取兼容性分解表 + 版本号。

    ⚠ ICU **不暴露**大小写折叠(`u_strFoldCase` 没有 JS 绑定)⇒ 大小写折叠
    与 Cf 集合在这条路上**仍然**由 CPython 提供(15.1)⇒ 产物头部会写明
    「兼容性分解 = ICU 版本;折叠与 Cf = Python 版本」。
    """
    js = ICU_JS
    proc = subprocess.run(
        ["node", "-e", js], capture_output=True, text=True, encoding="utf-8", check=True
    )
    compat: list[tuple[int, list[int]]] = []
    for line in proc.stdout.splitlines():
        if not line.strip():
            continue
        cp_s, _, rest = line.partition(" ")
        compat.append((int(cp_s, 16), [int(x, 16) for x in rest.split()] if rest else []))

    ver = subprocess.run(
        ["node", "-p", "process.versions.unicode + ' (ICU ' + process.versions.icu + ')'"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=True,
    ).stdout.strip()

    fold: list[tuple[int, list[int]]] = []
    cf: list[int] = []
    for cp in range(0x110000):
        if 0xD800 <= cp <= 0xDFFF:
            continue
        ch = chr(cp)
        if unicodedata.category(ch) == "Cf":
            cf.append(cp)
        folded = ch.casefold()
        if folded != ch:
            fold.append((cp, [ord(c) for c in folded]))
        elif ch.lower() != ch.upper():
            fold.append((cp, [cp]))
    return compat, fold, sorted(set(cf)), ver


# ── 产物 ────────────────────────────────────────────────────────────────────


def emit(compat, fold, cf, version, source_note) -> str:
    simple = [(cp, t[0]) for cp, t in fold if len(t) == 1]
    full = [(cp, t) for cp, t in fold if len(t) != 1]

    o = []
    o.append("//! GENERATED by `script/gen_unicode_waf_tables.py`. DO NOT EDIT.")
    o.append("//!")
    o.append("//! W-24 的三张文本归一化数据表(供 `std.text` 的 `NFKC` / `CASE_FOLD` /")
    o.append("//! `STRIP_INVISIBLE` 三个成员使用)。")
    o.append("//!")
    o.append(f"//! 数据版本:{version}")
    o.append(f"//! 数据来源:{source_note}")
    o.append("//!")
    o.append("//! 重新生成:")
    o.append("//! ```text")
    o.append("//! python script\\gen_unicode_waf_tables.py --ucd <UCD 目录> \\")
    o.append("//!     --out impl\\crates\\wlwl-std\\src\\unicode\\waf.rs")
    o.append("//! ```")
    o.append("//!")
    o.append("//! ⚠ **本表不含任何「正确性断言」** —— 它是数据,不是证据。")
    o.append("//!   证据在 `bin/norm-check`(官方 `NormalizationTest.txt` 全量自检)。")
    o.append("")
    o.append(f'/// 本表对应的 Unicode 版本 —— 「按哪一版归一化」必须可复现。')
    o.append(f'pub const UNICODE_VERSION: &str = "{version}";')
    o.append("")
    o.append("/// 兼容性分解:码点 → **递归展开后**的兼容性分解(已规范分解并按 D115 排序)。")
    o.append("///")
    o.append("/// ⚠ 只收录「NFKD ≠ NFD」的码点 —— 没有兼容性分解的码点**不在表里**")
    o.append("///   (它们的分解由 `unicode::canon` 的规范分解那一半负责)。")
    o.append("#[rustfmt::skip]")
    o.append("pub static COMPAT_DECOMP: &[(u32, &[u32])] = &[")
    for cp, t in compat:
        o.append(f"(0x{cp:04X},&[{hexs(t)}]),")
    o.append("];")
    o.append("")
    o.append("/// 大小写折叠(**一对一**):Unicode 折叠目标 = 小写映射的码点。")
    o.append("///")
    o.append("/// ⚠ 这一张表**只**收录「折叠目标与源不同」的码点 —— 「折叠映射存在、")
    o.append("///   但映射到自己」(切罗基语大写字母 U+13A0 → U+13A0)也在里面。")
    o.append("#[rustfmt::skip]")
    o.append("pub static FOLD_SIMPLE: &[(u32, u32)] = &[")
    for cp, t in simple:
        o.append(f"(0x{cp:04X},0x{t:04X}),")
    o.append("];")
    o.append("")
    o.append("/// 大小写折叠(**多对一**):`ß` → `ss`、`ﬀ` → `ff`、`ŉ` → `ʼn` 这类。")
    o.append("#[rustfmt::skip]")
    o.append("pub static FOLD_FULL: &[(u32, &[u32])] = &[")
    for cp, t in full:
        o.append(f"(0x{cp:04X},&[{hexs(t)}]),")
    o.append("];")
    o.append("")
    o.append("/// 不可见字符:general category = `Cf`(format)。")
    o.append("///")
    o.append("/// 零宽空格 / 零宽连接符 / 双向控制符 / BOM / 软连字都在这里。")
    o.append("/// ⚠ `Mn` / `Me`(组合记号)**不在**这里 —— 它们带语义(重音),剥掉会毁内容。")
    o.append("#[rustfmt::skip]")
    o.append("pub static INVISIBLE_CF: &[u32] = &[")
    for i in range(0, len(cf), 12):
        o.append(",".join(f"0x{c:04X}" for c in cf[i : i + 12]) + ",")
    o.append("];")
    o.append("")
    return "\n".join(o)


def main() -> int:
    ap = argparse.ArgumentParser(description="生成 unicode/waf.rs")
    src = ap.add_mutually_exclusive_group()
    src.add_argument("--ucd", type=Path, help="UCD 目录(UnicodeData.txt 等)")
    src.add_argument("--icu", action="store_true", help="借 Node 的 ICU 取兼容性分解")
    ap.add_argument(
        "--out",
        type=Path,
        default=Path("impl/crates/wlwl-std/src/unicode/waf.rs"),
        help="产物路径",
    )
    args = ap.parse_args()

    if args.ucd:
        compat, fold, cf, version = read_ucd(args.ucd)
        note = f"UCD 目录 `{args.ucd}`"
    elif args.icu:
        compat, fold, cf, version = read_icu()
        note = f"Node 的 ICU(Unicode {version})取兼容性分解 + CPython {unicodedata.unidata_version} 取折叠与 Cf"
    else:
        compat, fold, cf, version = read_python_ucd()
        note = f"CPython {unicodedata.unidata_version} 的 `unicodedata`"

    text = emit(compat, fold, cf, version, note)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(text, encoding="utf-8", newline="\n")
    print(
        f"[gen_unicode_waf_tables] Unicode {version}: "
        f"兼容分解 {len(compat)} 条 / 折叠 {len(fold)} 条 / Cf {len(cf)} 条 -> {args.out}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
