#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""把**源码树**打进发布包，并给出一份**可复算**的树哈希。

用法:
    # 打包（发布流程用）：
    python script/package_source.py --out staging/wlwl-src.tar.gz --hash-out staging/SOURCE_HASH
    # 核对（拿到发布包的人用）：
    python script/package_source.py --verify <解包后的源码目录> --hash-file staging/SOURCE_HASH

为什么要有它（缺口 G-06）
------------------------
发布包原本只有 `wlwl.exe` + R1 stdlib 副本 + 例子 + 文档 ⇒ **没有编译器源码**
⇒ 离线状态下「读源码 / 改这个语言 / 核对某个行为到底怎么实现的」都不可行
⇒ 上游用户只能去 GitHub 拉 tag 源码（本次实践实测：`github.com` 直连超时，
  但 `codeload.github.com` 通 ⇒ 绕行成本 = 一次手工下载 + 一次 `cargo build`）。

⇒ 本脚本把源码树（**排除构建产物**）打进包，并写一份 `SOURCE_HASH`
⇒ 「只有发布包的机器」也能**离线**取到源码 ⇒ 上面那条绕行变成「解包即得」。

哈希口径（**可复算**：同样的源码树 ⇒ 同样的哈希，跨机器 / 跨 OS 一样）
--------------------------------------------------------------------
对**纳入打包的每个文件**算 sha256，再把「相对路径 + 两个空格 + 该 sha256」按
路径排序后逐行拼起来，最后对这份清单取 sha256。
⇒ 用户解开 tarball 后用 `--verify` 就能核对「我手里的源码是不是包里那一份」。
⚠ 排序是**按相对路径**（`/` 分隔，跨 OS 一致）⇒ 树哈希与**目录遍历顺序无关**。

排除清单（**写进 `SOURCE_HASH` 的头里** ⇒ 不是暗箱，而且**核对模式从那里读回**
⇒ 打包与核对的文件集**逐字一致**）
--------------------------------------------------------------------
* 固定项：`.git` / `impl/target` / `target` / `__pycache__`（构建产物与版本库数据）、
  `docs/wlwl-spec-reference.md`（合并规范册的**次级件**，上游已决定「**不随包分发**」
  ⇒ 这里尊重同一决定）、`*.pyc`；
* **动态项**：输出文件与其**所在目录**（⚠ 不排掉的话，脚本会把它**正在写的
  tarball** 也打进包 ⇒ 哈希不可复算、且包会自我包含）。

锁文件（**依赖集随源码冻结**）
--------------------------------------------------------------------
`impl/Cargo.lock` 被 `.gitignore` 忽略、**不入库** ⇒ 解包后 cargo 会**重新解析依赖**
⇒ 重建出来的**不是同一个编译器** ⇒ 「源码能重建出同一个编译器」这句话是假的。
⇒ 打包前**保证**它在树上：缺了就 `cargo generate-lockfile` 生成，生成不了就
**响亮地失败**（不静默发一个「没有依赖集」的包）⇒ 锁文件进 tarball ⇒ 依赖集
随源码一起冻结。⚠ 它**不在**排除清单里（排除清单会写进 `SOURCE_HASH` 的头里
⇒ 不是暗箱）;`--verify` 会专门检查它在不在 ⇒ 缺了会**点名**报出来。

退出码：0 = 成功；1 = 失败（文件缺失 / 哈希不符 / 参数错）。
"""

import argparse
import hashlib
import os
import subprocess
import sys
import tarfile

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# 固定的排除项：目录 / 文件用**相对路径前缀**匹配，后缀用 fnmatch 匹配。
BASE_EXCLUDE_PREFIXES = (
    ".git",
    "target",
    "impl/target",
    "__pycache__",
    "docs/wlwl-spec-reference.md",
)
BASE_EXCLUDE_SUFFIXES = (".pyc",)

# 依赖集锁文件（相对仓根）—— **必须**随包走，见文件头的「锁文件」一节。
LOCK_RELPATH = "impl/Cargo.lock"


def rel_posix(root: str, path: str) -> str:
    return os.path.relpath(path, root).replace("\\", "/")


def excluded(rel: str, prefixes, suffixes) -> bool:
    """这个相对路径是否被排除（前缀按路径段匹配，`.git` 不会误伤 `a/.gitignore`）。"""
    rel = rel.replace("\\", "/")
    for p in prefixes:
        p = p.replace("\\", "/").rstrip("/")
        if rel == p or rel.startswith(p + "/"):
            return True
    return rel.endswith(tuple(suffixes))


def collect(root: str, prefixes, suffixes):
    """列出纳入打包的文件 ⇒ [(相对路径, 绝对路径)]，按相对路径排序。"""
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        rel_dir = rel_posix(root, dirpath)
        if rel_dir == ".":
            rel_dir = ""
        # 原地裁掉被排除的目录（os.walk 的 topdown=True 允许这样剪枝）
        dirnames[:] = sorted(
            d for d in dirnames
            if not excluded(
                os.path.join(rel_dir, d) if rel_dir else d, prefixes, suffixes
            )
        )
        for name in sorted(filenames):
            rel = os.path.join(rel_dir, name) if rel_dir else name
            if excluded(rel, prefixes, suffixes):
                continue
            out.append((rel.replace("\\", "/"), os.path.join(dirpath, name)))
    out.sort(key=lambda p: p[0])
    return out


def file_sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def tree_hash(root: str, prefixes, suffixes, files=None):
    """⇒ (树哈希, 清单文本, 文件数)。

    ⚠ 清单**按相对路径排序**（`collect` 已排好）⇒ 树哈希与**目录遍历顺序无关**。
    `files` 可传入**已收集好的**清单 ⇒ 打包时「哈希」与「tar 条目」取**同一份**
    文件集（不会因为两次遍历之间树变了而对不上）。
    """
    if files is None:
        files = collect(root, prefixes, suffixes)
    lines = []
    for rel, abs_path in files:
        lines.append(f"{rel}  {file_sha256(abs_path)}\n")
    manifest = "".join(lines)
    return hashlib.sha256(manifest.encode("utf-8")).hexdigest(), manifest, len(files)


def hash_file_text(tree: str, count: int, prefixes, suffixes,
                   lock_file: str = LOCK_RELPATH) -> str:
    return (
        "# wlwl source tree hash\n"
        "# algorithm: sha256 over sorted \"<relpath>  <sha256(file)>\\n\" lines\n"
        f"# files: {count}\n"
        "# exclude-prefixes: " + ",".join(prefixes) + "\n"
        "# exclude-suffixes: " + ",".join(suffixes) + "\n"
        f"# lock-file: {lock_file}\n"
        "# verify: python script/package_source.py --verify <src-dir> --hash-file SOURCE_HASH\n"
        f"tree-sha256: {tree}\n"
    )


def read_hash_file(path: str):
    """⇒ (树哈希, 前缀排除表, 后缀排除表, 锁文件路径)。

    ⚠ 排除清单与锁文件路径**从这份文件里读回** ⇒ 核对模式用的文件集与打包时
    **逐字一致**（不是「重跑一次排除逻辑」—— 那两边不同步就会对不上）。
    """
    tree = None
    prefixes = list(BASE_EXCLUDE_PREFIXES)
    suffixes = list(BASE_EXCLUDE_SUFFIXES)
    lock_file = LOCK_RELPATH
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if line.startswith("tree-sha256:"):
            tree = line.split(":", 1)[1].strip()
        elif line.startswith("# exclude-prefixes:"):
            prefixes = [
                p for p in line.split(":", 1)[1].strip().split(",") if p
            ]
        elif line.startswith("# exclude-suffixes:"):
            suffixes = [
                s for s in line.split(":", 1)[1].strip().split(",") if s
            ]
        elif line.startswith("# lock-file:"):
            lock_file = line.split(":", 1)[1].strip() or LOCK_RELPATH
    if tree is None:
        raise SystemExit(f"{path}: 找不到 'tree-sha256:' 行")
    return tree, prefixes, suffixes, lock_file


def dynamic_excludes(root: str, out_path: str, hash_path: str):
    """输出文件与其所在目录 ⇒ 相对路径前缀（⚠ 不排掉就会自我包含）。"""
    extras = []
    for p in (out_path, hash_path):
        ap = os.path.abspath(p)
        parent = os.path.dirname(ap)
        for cand in (ap, parent):
            try:
                rel = rel_posix(root, cand)
            except ValueError:
                continue
            if rel.startswith(".."):
                continue  # 在根之外 ⇒ 不影响
            rel = rel.replace("\\", "/").rstrip("/")
            if rel and rel != ".":
                extras.append(rel)
    # 短的先放（长的是它的子路径 ⇒ 前缀匹配会先命中长的）
    return sorted(set(extras), key=len, reverse=True)


def ensure_lock_file(root: str) -> str:
    """保证 `<root>/impl/Cargo.lock` 在树上 ⇒ 缺了就生成，生成不了就**响亮地失败**。

    为什么要它：`impl/Cargo.lock` 被 `.gitignore` 忽略、**不入库** ⇒ 解包后
    cargo **重新解析依赖** ⇒ 重建出来的**不是同一个编译器** ⇒「源码能重建出
    同一个编译器」这句话是假的。⇒ 打包前把锁文件**钉进 tarball** ⇒ 依赖集
    随源码一起冻结。

    ⚠ 生成不出来时**不静默继续**（不静默发一个「没有依赖集」的包）—— 响亮地失败，
      并把「怎么办」写在错误里。
    """
    path = os.path.join(root, *LOCK_RELPATH.split("/"))
    if os.path.isfile(path):
        return path
    impl_dir = os.path.join(root, "impl")
    if not os.path.isfile(os.path.join(impl_dir, "Cargo.toml")):
        raise SystemExit(
            f"找不到 {LOCK_RELPATH}，也找不到 impl/Cargo.toml —— 源码树不完整（{root}）\n"
            "  ⇒ 无法生成锁文件 ⇒ 拒绝打一个「没有依赖集」的包")
    print(f"{LOCK_RELPATH} 不存在 ⇒ 生成锁文件（cargo generate-lockfile）……")
    try:
        p = subprocess.run(
            ["cargo", "generate-lockfile"],
            cwd=impl_dir, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, errors="replace", encoding="utf-8")
    except FileNotFoundError:
        raise SystemExit(
            f"{LOCK_RELPATH} 不存在，且找不到 cargo ⇒ 无法生成锁文件。\n"
            f"  请在 impl/ 下跑一次 `cargo generate-lockfile`，再重跑本脚本。")
    if p.returncode != 0 or not os.path.isfile(path):
        sys.stderr.write((p.stdout or "")[-2000:] + "\n")
        raise SystemExit(
            f"生成 {LOCK_RELPATH} 失败 ⇒ 拒绝打包。\n"
            f"  请在 impl/ 下跑一次 `cargo generate-lockfile`（需要能访问 crates.io 索引），再重跑本脚本。")
    print(f"已生成 {LOCK_RELPATH}")
    return path


def do_package(root: str, out: str, hash_out: str) -> int:
    # ⚠ 锁文件**必须**在包里（依赖集随源码冻结）⇒ 先把它备齐。
    ensure_lock_file(root)
    prefixes = list(BASE_EXCLUDE_PREFIXES) + dynamic_excludes(root, out, hash_out)
    suffixes = list(BASE_EXCLUDE_SUFFIXES)
    # ⚠ 两道断言（都是**响亮地失败**，不是悄悄发一个坏包）：
    #   ① 排除清单不许把锁文件排掉（将来谁加一条 `Cargo.lock` ⇒ 这里就红）；
    #   ② 收集完的文件集里**确实**有它。
    if excluded(LOCK_RELPATH, prefixes, suffixes):
        raise SystemExit(
            f"排除清单把 {LOCK_RELPATH} 排掉了 ⇒ 依赖集不会随源码冻结 ⇒ 拒绝打包")
    files = collect(root, prefixes, suffixes)
    if LOCK_RELPATH not in {rel for rel, _ in files}:
        raise SystemExit(
            f"打包文件集里没有 {LOCK_RELPATH} ⇒ 依赖集没随源码冻结 ⇒ 拒绝打包")
    tree, _manifest, count = tree_hash(root, prefixes, suffixes, files)
    os.makedirs(os.path.dirname(os.path.abspath(out)) or ".", exist_ok=True)
    os.makedirs(os.path.dirname(os.path.abspath(hash_out)) or ".", exist_ok=True)

    # 确定性 tar：固定 mtime / uid / gid / 排序 ⇒ 同样的源码树 ⇒ 同样的字节。
    def norm(ti):
        ti.uid = 0
        ti.gid = 0
        ti.uname = ""
        ti.gname = ""
        ti.mtime = 0
        return ti

    with tarfile.open(out, "w:gz", compresslevel=9) as tar:
        # ⚠ 用**同一份** files（与算哈希的那份一致）⇒ 哈希与 tar 条目严格同集。
        for rel, abs_path in files:
            tar.add(abs_path, arcname=os.path.join("wlwl-src", rel), filter=norm)
    with open(hash_out, "w", encoding="utf-8", newline="\n") as f:
        f.write(hash_file_text(tree, count, prefixes, suffixes, LOCK_RELPATH))
    print(f"packaged {count} file(s) -> {out}")
    print(f"tree-sha256: {tree}")
    return 0


def do_verify(root: str, hash_file: str) -> int:
    want, prefixes, suffixes, lock_file = read_hash_file(hash_file)
    got, _manifest, count = tree_hash(root, prefixes, suffixes)
    if lock_file and not os.path.isfile(os.path.join(root, *lock_file.split("/"))):
        print(f"MISMATCH: 缺 {lock_file}")
        print("  ⇒ 依赖集没随源码冻结 ⇒ 重建出来的**不是同一个编译器**")
        print(f"  expected: {want}\n  actual:   {got}")
        return 1
    if got != want:
        print(f"MISMATCH\n  expected: {want}\n  actual:   {got}")
        return 1
    print(f"ok: tree-sha256 {got} ({count} file(s))")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="把源码树打进发布包，并给出可复算的树哈希")
    ap.add_argument("--root", default=REPO, help="源码树根目录（默认 = 仓根）")
    ap.add_argument("--out", help="输出的源码 tarball 路径")
    ap.add_argument("--hash-out", help="输出的 SOURCE_HASH 路径")
    ap.add_argument("--verify", help="核对模式：解包后的源码目录")
    ap.add_argument("--hash-file", help="核对模式：SOURCE_HASH 路径")
    args = ap.parse_args()

    if args.verify:
        if not args.hash_file:
            raise SystemExit("--verify 需要 --hash-file")
        if not os.path.isdir(args.verify):
            raise SystemExit(f"目录不存在: {args.verify}")
        return do_verify(args.verify, args.hash_file)

    if not args.out or not args.hash_out:
        raise SystemExit("打包模式需要 --out 与 --hash-out")
    root = os.path.abspath(args.root)
    if not os.path.isdir(root):
        raise SystemExit(f"源码树不存在: {root}")
    return do_package(root, args.out, args.hash_out)


if __name__ == "__main__":
    sys.exit(main())
