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

- Language spec: [`docs/standard/wlwl-spec-v0.9.md`](./docs/standard/wlwl-spec-v0.9.md)
  (v0.8 archived at
  [`docs/history/wlwl-spec-v0.8.md`](./docs/history/wlwl-spec-v0.8.md))
- Changelog: [`CHANGELOG.md`](./CHANGELOG.md)
- Builtin registry (Appendix G mirror): [`docs/appendix_G.md`](./docs/appendix_G.md)

## §0.4 Consistency table (v0.9 wip)

| 一致性项 | v0.8.1 | v0.9 (wip0.9) |
|---------|-------|----------|
| 错误码数 | 56 激活 + 11 保留 (共 67) | **61 激活 + 6 保留** (共 67) |
| 警告码 | W0010/W0011/W0014/W0020/W0030/W0040/W0051/W0053 | **+ W0065 / W0066** |
| 内建数 | 110 | **110** (OOP 从占位翻真实现) |
| spec 章节 | §1–§17 + 附录 A–G | §17 重写 + **§13–§16 新增** + §17.8 |
| Brown 9 维度 | 9 选定 | 9 不变 + Suspension 升格为真挂起 |
| 行为类型 | 无 | **顺序 + ⊕ + μ** |
| THIS 线性 | 无 | **真线性检查 → E0032** |
| Algebraic-effect | 无 | **Effect { Yield, ChannelOp, Cancelled }** |
| Tag/payload cancel | 不支持 | **`TASK_CANCEL(task, reason?)`** |
| 锁测试 | 1409 | **1517** (workspace, wip0.9) |

v0.9 release 前的 Known gaps 见 `CHANGELOG.md` v0.9.0 段。

## License

GPL v2 — see [LICENSE](./LICENSE). Your `.wll` programs are your own
work and are not affected by the compiler's license.
