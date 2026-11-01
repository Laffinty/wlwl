# Install

Three install paths, all yielding the same `wlwl` binary:

## 1. Pre-built binary (recommended)

Every release on the
[Releases page](https://github.com/Laffinty/wlwl/releases) ships
`.tar.xz` (Linux / macOS) or `.zip` (Windows) archives. Each
archive carries:

- the `wlwl` (or `wlwl.exe`) binary
- a `LICENSE`, `README.md` and `CHANGELOG.md`
- the example programs under `examples/`
- the agent skill pack under `wlwl-skill/`
- `wlwl-src.tar.gz` plus `SOURCE_HASH` — the full compiler source tree,
  so an offline machine can read or modify the implementation
  (verify it with `python script/package_source.py --verify <src-dir>
  --hash-file SOURCE_HASH`)
- `lib/` — the **library folder**: the R1 standard-library sources under
  `lib/stdlib/`, plus the TLS extension as a **dynamic library file**
  (`libwlwl_tls.so` / `libwlwl_tls.dylib` / `wlwl_tls.dll`) together with
  its `ABI_VERSION` and `SHA256SUMS`, **only in archives cut after
  `v0.12.1_rc1`**; see the note below

> ⚠ **Check the tag before relying on the TLS library under `lib/`** — the
> packaging step was added *after* `v0.12.1_rc1`, and one tag in between
> never produced a release:
>
> | tag | release built? | TLS library inside `lib/`? |
> |---|---|---|
> | `v0.12.1_rc1` | yes | **no** |
> | `v0.12.1_rc2` | **no** — its `generate SBOM` job failed, so `github release` was skipped | (no release exists) |
> | `v0.12.1_rc3` | yes, but **withdrawn** — the tag and the release were both deleted | (withdrawn) |
> | `v0.12.1_rc4` | yes | **yes** — the current one, with `lib/stdlib/` next to the dynamic library |
>
> ⚠ **`v0.12.1_rc3` was withdrawn**: it shipped once (with the extension
> under `wlwl-tls/` instead of `lib/`) and was then deleted — both its tag
> and its release. Because that tag had already been published, the next
> release takes a **new number** rather than reusing `rc3`.
>
> Verify it yourself instead of trusting this table:
>
> ```bash
> # rc1 has no TLS step anywhere in its workflow file:
> git show v0.12.1_rc1:.github/workflows/release.yml | grep -n tls   # no output
> # the current archive really ships the library next to the stdlib sources:
> curl -sSL -o rc.tar.xz \
>   https://github.com/Laffinty/wlwl/releases/download/v0.12.1_rc4/wlwl-v0.12.1_rc4-x86_64-unknown-linux-gnu.tar.xz
> tar -tJf rc.tar.xz | grep '/lib/'
> ```
>
> When it is present, verify and use it like this:
>
> ```bash
> cd wlwl-v0.12.1_rc4-x86_64-unknown-linux-gnu/lib
> sha256sum -c SHA256SUMS          # libwlwl_tls.so: OK
> cat ABI_VERSION                  # 1
> ls stdlib/                       # the R1 sources live next to it
> cd ../.. && wlwl run --ext \
>   wlwl-v0.12.1_rc4-x86_64-unknown-linux-gnu/lib/libwlwl_tls.so \
>   your-program.wll
> ```
>
> The host **never** loads the extension on its own — you always pass it
> explicitly with `--ext`.

Download:

```bash
# Linux (swap the target triple for macOS: x86_64-apple-darwin / aarch64-apple-darwin)
curl -LO https://github.com/Laffinty/wlwl/releases/download/v0.12.1_rc4/wlwl-v0.12.1_rc4-x86_64-unknown-linux-gnu.tar.xz
tar -xJf wlwl-v0.12.1_rc4-x86_64-unknown-linux-gnu.tar.xz
export PATH="$PWD/wlwl-v0.12.1_rc4-x86_64-unknown-linux-gnu:$PATH"
wlwl --version
```

> The `v0.12.1` RCs are **release candidates** — the binaries report
> `wlwl 0.12.1`, but the GitHub release is marked *pre-release*, so
> package managers will not pick it up automatically. `v0.12.0` is the
> latest final release.

Verify the signature with [`cosign`](https://docs.sigstore.dev/cosign/system_config/installation/):

```bash
cosign verify-blob \
  --certificate-identity-regexp 'github.com/Laffinty/wlwl' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  wlwl-v0.12.1_rc4-x86_64-unknown-linux-gnu.tar.xz
```

> The `.bundle` and `.sig` files produced by the release workflow are
> **not** uploaded as release assets — `cosign verify-blob` above
> resolves the signature from the public transparency log instead.

## 2. Build from source

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl run examples/hello.wll
```

Requires Rust ≥ 1.85 (`rust-version` in `impl/Cargo.toml`). The
toolchain is pinned to 1.99.0 in the repo-root `rust-toolchain.toml`,
so `rustup` picks it up automatically.

## 3. Fuzz / bench (dev-only)

The fuzz harness and benchmark suite live in `impl/fuzz/` and
`impl/crates/wlwl-eval/benches/`. See
[the Contributing page](contributing.md) for how to run them.

## Next steps

- Run `wlwl run examples/showcase.wll` for a tour of stdlib
- Read the [Language tour](tour.md)
- File an issue or PR; see [Contributing](contributing.md)
