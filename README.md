# WLWL

A small experimental programming language in which every syntactic form
is a function call.

## Quick example

```wlwl
LET(greeting, "Hello, ${name}!");
LET(name, "world");
PRINT(greeting);

LET MUT(counter, 0);
LET(step, FUN((), (
    SET(counter, +(counter, 1));
    counter
)));
PRINT(step());   // 1
PRINT(step());   // 2 — counter shared via closure
```

`SCOPE` / `SPAWN` / `AWAIT` / `YIELD` and channels (`CHANNEL_*`) are
first-class; see the spec §17 and `CHANGELOG.md` for the concurrency
feature set.

## Install

Build from source — requires Rust ≥ 1.75:

```bash
git clone https://github.com/Laffinty/wlwl
cd wlwl/impl
cargo build --release
./target/release/wlwl --version
./target/release/wlwl run examples/hello.wll
```

For development setup, quality gates, and the workspace layout, see
[`CONTRIBUTING.md`](./CONTRIBUTING.md).

## Docs

- Language spec: [`docs/standard/wlwl-spec-v0.8.md`](./docs/standard/wlwl-spec-v0.8.md)
- Changelog: [`CHANGELOG.md`](./CHANGELOG.md)
- Builtin registry (Appendix G mirror): [`docs/appendix_G.md`](./docs/appendix_G.md)

## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wll` programs are your own
work and are not affected by the compiler's license.