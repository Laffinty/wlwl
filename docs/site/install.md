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
- the R1 standard-library sources under `stdlib/`
- the agent skill pack under `wlwl-skill/`
- the TLS extension under `wlwl-tls/` — the native dynamic library
  (`wlwl_tls.dll` / `libwlwl_tls.dylib` / `libwlwl_tls.so`) plus
  `ABI_VERSION` (the extension ABI it was built against) and
  `SHA256SUMS` (verify with `sha256sum -c SHA256SUMS`). Load it
  explicitly — the host never loads it on its own:
  `wlwl run --ext wlwl-tls/libwlwl_tls.so your-program.wll`
- `wlwl-src.tar.gz` plus `SOURCE_HASH` — the full compiler source tree,
  so an offline machine can read or modify the implementation
  (verify it with `python script/package_source.py --verify <src-dir>
  --hash-file SOURCE_HASH`)

Download:

```bash
# Linux
curl -LO https://github.com/Laffinty/wlwl/releases/download/v0.12.1_rc1/wlwl-v0.12.1_rc1-x86_64-unknown-linux-gnu.tar.xz
tar -xJf wlwl-v0.12.1_rc1-x86_64-unknown-linux-gnu.tar.xz
export PATH="$PWD/wlwl-v0.12.1_rc1-x86_64-unknown-linux-gnu:$PATH"
wlwl --version
```

> `v0.12.1_rc1` is a **release candidate** — the binaries report
> `wlwl 0.12.1`, but the GitHub release is marked *pre-release*, so
> package managers will not pick it up automatically. `v0.12.0` is the
> latest final release.

Verify the signature with [`cosign`](https://docs.sigstore.dev/cosign/system_config/installation/):

```bash
cosign verify-blob \
  --certificate-identity-regexp 'github.com/Laffinty/wlwl' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  wlwl-v0.12.1_rc1-x86_64-unknown-linux-gnu.tar.xz
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
