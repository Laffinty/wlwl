# Install

Three install paths, all yielding the same `wlwl` binary:

## 1. Pre-built binary (recommended)

Every release on the
[Releases page](https://github.com/Laffinty/wlwl/releases) ships
`.tar.xz` (Linux / macOS) or `.zip` (Windows) archives. Each
archive carries:

- the `wlwl` (or `wlwl.exe`) binary
- a `LICENSE` and `README.md`
- the example programs under `examples/`
- a `sbom.json` CycloneDX SBOM
- a `<archive>.bundle` cosign signature bundle

Download:

```bash
# Linux
curl -LO https://github.com/Laffinty/wlwl/releases/download/v0.4.0/wlwl-v0.4.0-x86_64-unknown-linux-gnu.tar.xz
tar -xJf wlwl-v0.4.0-x86_64-unknown-linux-gnu.tar.xz
export PATH="$PWD/wlwl-v0.4.0-x86_64-unknown-linux-gnu:$PATH"
wlwl --version
```

Verify the signature with [`cosign`](https://docs.sigstore.dev/cosign/system_config/installation/):

```bash
cosign verify-blob \
  --certificate-identity-regexp 'github.com/Laffinty/wlwl' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  wlwl-v0.4.0-x86_64-unknown-linux-gnu.tar.xz
```

## 2. Build from source

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl run examples/hello.wl
```

Requires Rust ≥ 1.75 (toolchain pinning via `rust-toolchain.toml` is
planned for v0.5; today, plain `rustup default stable` works on
Linux/macOS/Windows).

## 3. Fuzz / bench (dev-only)

The fuzz harness and benchmark suite live in `impl/fuzz/` and
`impl/crates/wlwl-eval/benches/`. See
[the Contributing page](contributing.md) for how to run them.

## Next steps

- Run `wlwl run examples/showcase.wl` for a tour of stdlib
- Read the [Language tour](tour.md)
- File an issue or PR; see [Contributing](contributing.md)
