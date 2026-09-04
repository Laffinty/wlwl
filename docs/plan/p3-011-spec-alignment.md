# P3-011 — 中期语法对齐 (spec v0.3 → parser)

**状态**: PLAN
**触发**: 用户 2026-09-04 启动, 严格按 `docs/standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba612b1ea).md` 校齐
**前提**: P3-010 已在 `bbd1521`, 13/13 crates ≥ 90% line, P3-009 系列完结

## 目标

把 parser 端到端按 spec v0.3 跑一遍, 找偏差, 修齐. 不改 spec (规范是 source of truth), 改 parser.

## 关键发现 (parser 现状审计)

**5 个 A 组偏差必须修:**

1. **链式访问 a.b / a.b(args) / a.b.c(args)** — spec §11.4 强要求, parser 完全没实现
2. **FUN 具名形式 `FUN(name(params), body)`** — spec §8.2, parser 只支持匿名 `FUN((params), body)`
3. **中文标识符** — spec §3.1 允许, lexer 只支持 ASCII alphanumeric + `_`
4. **W0020 数组/字典混用警告** — spec §4.5 末尾, parser 未检查
5. (B 组, 时间允许) **FUN 默认参数 `name = default` + 剩余参数 `*rest`** — spec §8.2, parser 只支持 `name: Type`

**其余 30+ 项** 全部支持 (16 关键字 / 块注释嵌套 / 字符串转义 / IF/WHILE/FOR/RETURN/BREAK/CONTINUE / 块表达式 / LET 类型注解 / SET/CLASS/NEW/GET_PROP/SET_PROP/CALL_METHOD 普通 Call / OK/ERR/PANIC/TRY/IS_OK/IS_ERR/OR_DIE / IMPORT 简单/重命名/命名空间路径 / EXPORT / 一元减 desugar / E0001-E0014 错误码).

**1 项延迟 (eval 端):** E0014 RETURN/BREAK/CONTINUE 非法位置 — 留给 eval 端.

## A 组修复方案

### A1. 链式访问 (spec §11.4)
- 位置: `parser/src/lib.rs` parse_call_or_ident
- 做法: parse_call_or_ident 返回后, 循环 peek `.`:
  - `.` + ident: 转为 `GET_PROP(<prev>, "<name>")` 嵌套 Call
  - `.` + `(`: 转为 `CALL_METHOD(<prev>, "<name>", args...)` 嵌套 Call
- AST 形状不变 (复用 Expr::Call), 跟 spec §11.4 "语法糖" 描述一致
- 风险: 影响主路径, 现有 54 tests 验证

### A2. FUN 具名 (spec §8.2)
- 位置: parse_fun
- 做法: `FUN(` 之后 peek: `(` → 匿名 (现有) / ident → 具名 (新增)
- AST 变更: `Expr::Fun` 加 `name: Option<String>`, serde default + skip
- 风险: AST 字段加, 序列化用 default + skip_serializing_if 兼容旧 snapshot

### A3. 中文标识符 (spec §3.1)
- 位置: lexer read_ident_or_keyword
- 做法: 替换 `is_ascii_alphanumeric() || '_'` 为 Unicode 字符属性
- 简化: 首字符 `char::is_alphabetic()`, 后续 `char::is_alphanumeric() || '_'`
- 风险: 中文字节边界, 用 `char_indices()` 不用 `bytes()`

### A4. W0020 (spec §4.5)
- 位置: parse_array_or_dict
- 做法: 收集混用 entry, emit WlwlWarning(W0020)
- 新增: `pub struct WlwlWarning` in wlwl-error 或新模块, `parse_with_warnings(input, file) -> Result<(Expr, Vec<WlwlWarning>), WlwlError>`
- `parse()` 内部调用新入口并丢 warnings (保持现有签名)

### B1. 默认参数 (spec §8.2)
- AST: `FunParam` 加 `default_expr: Option<Expr>`, serde skip
- Parser: ident 后 peek `=` → advance, parse_expr → default

### B2. 剩余参数 (spec §8.2)
- AST: `FunParam` 加 `is_rest: bool`, serde skip
- Parser: ident 前 peek `*` → advance, 后续 ident 是 rest

## 测试集 (~64 tests)

新增 `impl/crates/wlwl-parser/tests/spec_v3_alignment.rs`:

| spec 节 | 数量 | 覆盖 |
|---|---:|---|
| §3 词法 | 5 | 16 关键字, 中文, 大小写, 嵌套注释, 多空白 |
| §4 字面量 | 8 | int/float/string/escapes/true/false/null/array/dict/中文 string |
| §5 表达式 | 6 | call, block value, empty block NULL, nested call, eval order, op call |
| §5.2 链式 (A1) | 6 | property / method / 3+ level / method after / call.property / property only |
| §6 变量 | 4 | let basic, let type ann, let complex type, SET via call |
| §7 控制流 | 8 | if 3 元 / 2 元 (default NULL), while, for array/dict, return val/non, break, continue |
| §8 函数 (A2+B) | 6 | 匿名 / 具名 / 具名 + 返回类型 / 类型注解参数 / 默认参数 / 剩余参数 |
| §9 运算符 | 4 | 算术 / 比较 / 逻辑 / 一元减 |
| §11 OOP | 4 | CLASS / NEW / GET_PROP / SET_PROP 普通 Call |
| §12 错误处理 | 4 | OK/ERR, TRY/IS_OK/IS_ERR, OR_DIE, PANIC |
| §13 模块 | 5 | simple, rename, wlwl: namespace, empty path E0043, EXPORT |
| W0020 (A4) | 4 | 数组含 dict entry, dict 含裸值, 同质 array, 同质 dict |
| **合计** | **~64** | |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| A1 链式影响主路径 | 循环只在 parse_call_or_ident 内部, 现有 54 tests 验证 |
| A2/B1/B2 AST 字段破坏反序列化 | 全部 serde default + skip_serializing_if |
| A3 中文字节边界 | 用 char_indices 不用 bytes |
| A4 警告通道改 parse() 签名 | 保留 parse(), 新增 parse_with_warnings() |

## 工作量

| 步骤 | 估时 |
|---|---:|
| 1. 写 PLAN (本文) | 已完成 |
| 2. 写 spec_v3_alignment.rs (~64 tests) | 30 min |
| 3. 跑测试, 收集 fail | 5 min |
| 4. 修 A1 链式 | 20 min |
| 5. 修 A2 FUN 具名 | 10 min |
| 6. 修 A3 中文 | 10 min |
| 7. 修 A4 W0020 | 25 min |
| 8. 修 B1 默认参数 | 15 min |
| 9. 修 B2 剩余参数 | 10 min |
| 10. cargo test + cargo llvm-cov 验证 | 10 min |
| 11. 文档 (deviations + build plan + history) | 15 min |
| 12. commit + push | 5 min |
| **总计** | **~2.5 小时** |

## 验证清单

- [ ] `cargo test -p wlwl-parser --lib` ~118 tests 通过 (现 54 + 新 ~64)
- [ ] `cargo test --workspace` ~508 tests 通过 (现 444 + 新 ~64)
- [ ] `cargo llvm-cov --workspace` 13/13 crates ≥ 90% line
- [ ] 文档: deviations.md P3-011 段 + build plan §6.2/§3 + history log 收尾六
- [ ] 1 commit, push origin/main
- [ ] 13/13 ≥ 90% 不倒退

## 不在本轮范围

- E0014 RETURN/BREAK/CONTINUE 非法位置 (eval 端)
- ERR 透明传播 parser 端覆盖 (eval 端)
- wlwl.toml / ModuleLoader (toml + module 端)
- std.ai 流式 (P3-012 议程)
- 性能 (尾调用 + hot-inline)
- 文档站 (mkdocs)
- Phase 5 (Coq)

## 命名

- commit subject: `P3-011: spec v0.3 mid-syntax alignment — chain, named FUN, CN ident, W0020, default+rest param`
- 文档: deviations.md P3-011 / build plan §6.2 + §3 / history log 收尾六
