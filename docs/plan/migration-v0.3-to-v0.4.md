# WLWL v0.3 → v0.4 迁移指南

> 对应 spec:`docs/standard/wlwl-spec-v0.4(SHA1_97524ced037b5ef0a5820a2ebd5bafb4ba4e239b).md`
> 钉死章节:§13.4(AS 删除)/ §10.2(DEL)/ §12.2(OR_DIE)/ §3.4 + §14.5(NOT 与 `!`)
> 本文档是 Phase C1 交付物的一部分;构建计划对应项见 `docs/plan/wlwl-build-plan-v0.2.md` §3 Phase C C1。

v0.4 删除 / 改名了 4 个 v0.3 形式。下表给出机械改写规则:

| v0.3 写法 | v0.4 写法 | 违规后果 | 警告码 |
|-----------|-----------|----------|--------|
| `AS("y", "x")` | `LET(y, x)` | `E0020`(undefined name `AS`) | — |
| `DEL(d, k)` | `REMOVE_KEY(d, k)` | 可运行(兼容别名) | `W0051` |
| `OR_DIE(x, d)` | `UNWRAP_OR(x, d)` | 可运行(兼容别名) | `W0051` |
| `!(a)` / `!(a, b, ...)` | `NOT(a)` / `NOT(a, b, ...)` | 可运行(兼容别名) | `W0054` |

## 1. `AS` — 完全删除(§13.4)

v0.3 的 `AS("y", "x")` 把已绑定的名字 `x` 挂上别名 `y`。v0.4 中 `AS`
**不再是宏函数 / 全局内建**,任何引用都是未定义名:

```wlwl
// v0.3
LET(x, 1);
AS("y", "x");

// v0.4 —— 直接 LET 一个新绑定
LET(x, 1);
LET(y, x);
```

> 实现备注:WLWL v0.1→v0.3 的 Rust 实现(本仓库)从未实现过 `AS`,
> 因此"删除"在实现层是 no-op;`AS("y","x")` 一直走 `undefined_name`
> → `E0020`。锁测试 `c1_as_function_is_deleted_e0020` /
> `c1_as_is_not_a_builtin_or_macro` 守住这一状态。
> 偏差记录:deviations P4-C1-001。

## 2. `DEL` → `REMOVE_KEY`(§10.2)

`DEL(d, k)` 保留为兼容别名,每次使用发 `W0051`;v0.5 删除。

```wlwl
LET(d, ["a": 1, "b": 2]);
LET(d2, REMOVE_KEY(d, "a"));   // v0.4 主名
```

## 3. `OR_DIE` → `UNWRAP_OR`(§12.2)

```wlwl
LET(v, TRY(maybe_fail()));
LET(x, UNWRAP_OR(v, 0));       // v0.4 主名(OR_DIE 仍可用,发 W0051)
```

## 4. `!` → `NOT`(§3.4 / §14.5 W0054)

`!` 单字符形式与 §3.3 标点冲突,v0.4 主名是宏函数 `NOT`;
`!` 保留为兼容别名(发 `W0054`)。空参 `NOT()` 返回 `TRUE`(vacuous truth)。

## 5. `wlwl.toml` 迁移注意

v0.4 新增 / 收紧的字段(Phase C3-C6):

- `[package] language_version`:声明本包依赖的 WLWL 语义版本。
  实现加载时校验,不匹配 → `E0044`。
- `wlwl.lock` 与 `wlwl.toml` 不一致 → `E0042`(lock 禁止手动编辑;
  删除 lock 后重跑即可重新生成)。
- 版本式依赖(`"ns:name" = "^0.5.0"`)在 v0.4 没有中央仓库可解析,
  MVS 候选集为空 → `E0045`(中央仓库 v0.5 启用;本地开发用
  `{ path = "..." }` 路径依赖)。
