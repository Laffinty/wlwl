#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""从 `Cargo.lock` 确定性生成 CycloneDX 1.5 JSON，并可合并两份 + 校验必含组件。

## 为什么不用 `cargo cyclonedx`

⚠ **实测（release run #45，2026-11-01）**：`cargo-cyclonedx 0.5.9` 在本仓
**按 crate 各写一份** `sbom.json`（例如 `crates/wlwl-toml/sbom.json`），
**不产出 workspace 级的单一文件**；而它的落盘位置在 0.5.x 里**不写死**
（`--output-file` 已被移除）⇒ 想「找出那份总的」只能靠猜，一猜就错。
⇒ 那次发布因此**没建成**：merge 步骤拿到的「主 SBOM」其实是 `wlwl-toml` 单 crate 的。

⇒ 本脚本改成**只读 `Cargo.lock`**（依赖集**随源码冻结**，`--locked` 语义天然成立），
输出**一份**确定的 CycloneDX 文档：`components` = 所有**第三方**包
（`source` 字段非空的，即 registry / git 依赖；**本仓自己的 crate 不进组件表**）。

## 用法

```bash
# 单个 lock ⇒ 一份 SBOM
python script/gen_sbom.py --lock impl/Cargo.lock --out impl/target/sbom.json \
    --root-name wlwl --root-version 0.12.1

# 合并两份（按 purl 去重）并**要求**必须含这些 crate（缺失 ⇒ 退出码 1）
python script/gen_sbom.py \
    --merge impl/target/sbom.json impl/target/sbom-tls-ext.json \
    --out impl/target/sbom.json \
    --require rustls,ring,webpki-roots
```

## 退出码

`0` = 成功；`1` = 参数错 / lock 读不了 / 缺必含组件 / 合并输入缺失。
⚠ **响亮失败**：任何一步不成立都非零退出，绝不静默发一份「看起来有」的 SBOM。
"""

import argparse
import datetime
import hashlib
import json
import os
import sys

try:
    import tomllib
except ImportError:  # pragma: no cover - runner/local 都是 3.11+
    sys.stderr.write("::error::python >= 3.11 required (tomllib)\n")
    raise SystemExit(1)

SPEC_VERSION = "1.5"
TOOL_NAME = "wlwl-gen-sbom"


def die(msg):
    sys.stderr.write("::error::%s\n" % msg)
    raise SystemExit(1)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load_lock(path):
    """⇒ (第三方包列表 [(name, version, checksum|None)], 全部包数)。"""
    if not os.path.isfile(path):
        die("Cargo.lock not found: %s" % path)
    with open(path, "rb") as f:
        data = tomllib.load(f)
    if "package" not in data:
        # 空 lock 是合法的（没有任何依赖），但 Cargo.lock 一定有 [package] 表或 version 行
        if data.get("version") is None:
            die("%s: not a Cargo.lock (no version field)" % path)
    pkgs = data.get("package", [])
    third_party = []
    for p in pkgs:
        # ⚠ `source` 非空 ⇔ registry / git 依赖；本仓自己的 crate 没有 source
        if not p.get("source"):
            continue
        name = p.get("name")
        ver = p.get("version")
        if not name or not ver:
            die("%s: a package entry has no name/version" % path)
        third_party.append((name, ver, p.get("checksum")))
    third_party.sort(key=lambda t: (t[0], t[1]))
    return third_party, len(pkgs)


def purl(name, ver):
    return "pkg:cargo/%s@%s" % (name, ver)


def component(name, ver, checksum):
    c = {
        "type": "library",
        "name": name,
        "version": ver,
        "purl": purl(name, ver),
        "scope": "required",
    }
    # ⚠ lock 里的 `checksum` 是 sha256（registry 依赖才有；git 依赖可能没有）
    if checksum:
        c["hashes"] = [{"alg": "SHA-256", "content": checksum}]
    return c


def make_doc(components, root_name, root_version):
    return {
        "bomFormat": "CycloneDX",
        "specVersion": SPEC_VERSION,
        "version": 1,
        "metadata": {
            "timestamp": datetime.datetime.now(datetime.timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z"),
            "tools": [{"vendor": "wlwl", "name": TOOL_NAME, "version": "1"}],
            "component": {
                "type": "application",
                "name": root_name,
                "version": root_version or "0.0.0",
            },
        },
        "components": components,
    }


def read_manifest_version(path):
    """从 `Cargo.toml` 里取第一处 `version = "..."`（workspace.package 或 [package]）。

    ⚠ 放在 Python 里做，**不要**在 workflow 的 bash 里拼 `sed | head -1`：
    后者在 `set -o pipefail` 下会因 SIGPIPE 让整条管道返回 141（本仓踩过同类坑）。
    """
    import re

    if not os.path.isfile(path):
        die("Cargo.toml not found: %s" % path)
    with open(path, encoding="utf-8") as f:
        text = f.read()
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        die("cannot find `version = \"...\"` in %s" % path)
    return m.group(1)


def cmd_single(args):
    pkgs, total = load_lock(args.lock)
    comps = [component(n, v, c) for (n, v, c) in pkgs]
    ver = args.root_version
    if args.root_version_from:
        ver = read_manifest_version(args.root_version_from)
    doc = make_doc(comps, args.root_name, ver)
    os.makedirs(os.path.dirname(os.path.abspath(args.out)) or ".", exist_ok=True)
    with open(args.out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(doc, f, indent=2, sort_keys=True)
        f.write("\n")
    print(
        "%s: %d packages in lock, %d third-party components, root=%s@%s -> %s"
        % (args.lock, total, len(comps), args.root_name, ver or "0.0.0", args.out)
    )
    return 0


def cmd_merge(args):
    docs = []
    for p in args.merge:
        if not os.path.isfile(p):
            die("merge input not found: %s" % p)
        with open(p, encoding="utf-8") as f:
            docs.append((p, json.load(f)))

    by_ref = {}
    order = []
    for path, d in docs:
        for c in d.get("components", []):
            ref = c.get("purl") or ("%s@%s" % (c.get("name"), c.get("version")))
            if ref not in by_ref:
                by_ref[ref] = c
                order.append(ref)
    comps = [by_ref[r] for r in order]

    # 必含组件（缺失 ⇒ 红）—— 这是「SBOM 真的覆盖了扩展依赖」的**断言**
    have = {(c.get("name"), c.get("version")) for c in comps}
    have_names = {c.get("name") for c in comps}
    for raw in (args.require or "").split(","):
        want = raw.strip()
        if not want:
            continue
        if want not in have_names:
            die("required crate %s is MISSING from the SBOM" % want)
        vers = sorted(v for (n, v) in have if n == want)
        print("ok: %s present (%s)" % (want, ", ".join(vers)))

    # ⚠⚠ **真正的断言**：扩展 lock 里的**每一个**第三方包都必须出现在合并结果里。
    #
    # 为什么不能只查 `rustls` / `ring` / `webpki-roots`（**实测**，2026-11-01）：
    # 主 workspace 的 lock **本来就含**这三个（经 `reqwest` 的 `rustls-tls`）⇒
    # 光查名字，即便**完全不做合并**也照样通过 ⇒ **空断言**。
    # ⇒ 改成「逐包对拍扩展 lock」：只要合并这一步被漏掉/拿错文件，就一定红。
    tls_packages = []
    if args.tls_lock:
        ext_pkgs, _ = load_lock(args.tls_lock)
        tls_packages = ["%s@%s" % (n, v) for (n, v, _c) in ext_pkgs]
        missing = [t for t in tls_packages if (t.rpartition("@")[0], t.rpartition("@")[2]) not in have]
        if missing:
            die(
                "the TLS extension lock declares %d package(s) that are MISSING from the SBOM: %s"
                % (len(missing), ", ".join(missing))
            )
        print(
            "ok: all %d package(s) from the TLS extension lock are represented in the SBOM"
            % len(tls_packages)
        )

        # ⚠⚠ **非空的那条守卫**：合并的**输入里必须真的有扩展那份 SBOM**。
        #
        # **实测（2026-11-01）**：在当前依赖图下，扩展 lock 的 38 个包**全部**已经在
        # 主 workspace 的 closure 里（经 `reqwest` 的 `rustls-tls`）⇒
        # 「逐包对拍」**即便完全不合并也照样通过** ⇒ 它管不住「忘了合并」这件事。
        # ⇒ 所以再加一条**看来源**的断言：输入里必须有一份的 `metadata.component.name`
        #   等于扩展的 root 名（默认 `wlwl-tls-ext`）⇒ 漏掉合并就**必红**。
        roots = [
            d.get("metadata", {}).get("component", {}).get("name")
            for _p, d in docs
        ]
        if args.tls_root_name not in roots:
            die(
                "the TLS extension SBOM was not among the merged inputs "
                "(expected an input whose metadata.component.name == %r, got %r)"
                % (args.tls_root_name, roots)
            )
        print("ok: the TLS extension SBOM (%r) is one of the merged inputs" % args.tls_root_name)
    del have

    base = docs[0][1]
    root = base.get("metadata", {}).get("component", {})
    doc = make_doc(comps, root.get("name", "wlwl"), root.get("version"))

    # ⚠ 给**下游 verify 步骤**留的标记：它要能独立复核「覆盖了哪些 workspace」。
    from_name = []
    for path, d in docs:
        rc = d.get("metadata", {}).get("component", {})
        from_name.append(rc.get("name") or os.path.basename(path))
    pkgs_by_ws = []
    for path, d in docs:
        rc = d.get("metadata", {}).get("component", {})
        ws = rc.get("name") or "?"
        for c in d.get("components", []):
            pkgs_by_ws.append("%s@%s" % (c.get("name"), c.get("version")))
    doc["metadata"]["properties"] = [
        {"name": "wlwl:sbom:covered-workspaces", "value": ",".join(from_name)},
        {"name": "wlwl:sbom:merged-inputs", "value": ",".join(p for p, _ in docs)},
        {"name": "wlwl:sbom:all-packages", "value": ",".join(sorted(set(pkgs_by_ws)))},
        {"name": "wlwl:sbom:tls-ext-packages", "value": ",".join(sorted(tls_packages))},
        {
            "name": "wlwl:sbom:tls-ext-lock-sha256",
            "value": sha256_file(args.tls_lock) if args.tls_lock else "",
        },
    ]

    os.makedirs(os.path.dirname(os.path.abspath(args.out)) or ".", exist_ok=True)
    with open(args.out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(doc, f, indent=2, sort_keys=True)
        f.write("\n")
    print("merged %d inputs -> %d components -> %s" % (len(docs), len(comps), args.out))
    return 0


def main(argv):
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--lock", help="Cargo.lock to read")
    ap.add_argument("--out", required=True, help="output CycloneDX JSON path")
    ap.add_argument("--root-name", default="wlwl")
    ap.add_argument("--root-version", default="")
    ap.add_argument(
        "--root-version-from",
        default="",
        help="read the root version from a Cargo.toml (first `version = \"...\"`)",
    )
    ap.add_argument("--merge", nargs=2, metavar=("A", "B"), help="merge two SBOMs")
    ap.add_argument("--require", default="", help="comma-separated crate names that MUST be present")
    ap.add_argument("--tls-lock", default="", help="(merge) path to the TLS extension Cargo.lock")
    ap.add_argument(
        "--tls-root-name",
        default="wlwl-tls-ext",
        help="(merge) metadata.component.name that MUST appear among the merged inputs",
    )
    ap.add_argument(
        "--tls-packages",
        nargs="*",
        default=[],
        help="(merge) package@version list claimed to come from the TLS extension lock",
    )
    args = ap.parse_args(argv)

    if args.merge:
        return cmd_merge(args)
    if not args.lock:
        die("either --lock or --merge is required")
    return cmd_single(args)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
