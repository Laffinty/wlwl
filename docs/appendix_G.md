# 附录 G:全局内建注册表 (impl 视角)

> 本文件由 `wlwl-eval::registry::generate_appendix_g_md()` 自动生成。
> 单源真相是 `crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY`,本 markdown 是镜像。
> 修改流程:改注册表 -> 跑本函数重写本文件 -> 跑 `cargo test` 验证 lock test。

> 对照规范:`docs/standard/wlwl-spec-v0.4*.md` 第 3663 行起 (附录 G 规范性)。

总条目数:**90** | 已实现:**42** | LexerMacro:**24** | Deferred:**24**


| 名称 | 签名 | ERR 消费者 (§12.7) | 宏函数 (§3.4) | 引入 | 状态 | 实现位置 |
|------|------|--------------------|---------------|------|------|----------|
<!-- I/O (3 条) -->
| `PRINT` | `PRINT(args...) -> NULL` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§15.1) |
| `PRINT_ERR` | `PRINT_ERR(args...) -> NULL` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§15.1) |
| `INPUT` | `INPUT(prompt?) -> STRING` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
<!-- 类型 / 转换 (7 条) -->
| `LEN` | `LEN(coll) -> INTEGER` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `STR` | `STR(x) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `INT` | `INT(s) -> OK(INTEGER) / ERR(ParseError)` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.5) |
| `FLOAT` | `FLOAT(s) -> OK(FLOAT) / ERR(ParseError)` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§9.5) |
| `TYPE` | `TYPE(x) -> STRING (RESULT -> "RESULT")` | ✔ | ✔ | v0.2 | ✓ builtin | `resolve_builtin` (§2.5) |
| `BOOL` | `BOOL(x) -> BOOLEAN` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `CALL` | `CALL(fn, args...) -> v` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
<!-- RESULT 处理 (12 条) -->
| `IS_OK` | `IS_OK(x) -> BOOLEAN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§12.2) |
| `IS_ERR` | `IS_ERR(x) -> BOOLEAN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§12.2) |
| `OR_DIE` | `OR_DIE(x, default) -> v (v0.3 alias)` | ✔ | ✔ | v0.2 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §12.2) |
| `UNWRAP_OR` | `UNWRAP_OR(x, default) -> v` | ✔ | ✔ | v0.4 | ✓ builtin | `resolve_builtin` (§12.2) |
| `UNWRAP` | `UNWRAP(x) -> v / PANIC` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§12.2) |
| `ERR_PAYLOAD` | `ERR_PAYLOAD(x) -> e / E0030` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§12.2) |
| `WRAP` | `WRAP(err, ctx) -> ERR / OK` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§12.2) |
| `TRY` | `TRY(e) -> v / early-RETURN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§12.6) |
| `PANIC` | `PANIC(msg) -> 终止` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§12.4) |
| `OK` | `OK(v) -> RESULT` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§12.1) |
| `EXPECT_ERR` | `EXPECT_ERR(expr) -> OK(payload) / ERR(E0049)` | ✔ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§15.9) |
| `ERR` | `ERR(e) -> RESULT` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§12.1) |
<!-- 控制流 / 逻辑 (10 条) -->
| `IF` | `IF(cond, t, e?) -> v` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§7.1) |
| `WHILE` | `WHILE(cond, body) -> v` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§7.2) |
| `FOR` | `FOR(var, iter, body) -> NULL` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§7.3) |
| `MATCH` | `MATCH(v, clauses, default?) -> v` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§7.4) |
| `RETURN` | `RETURN(v?) -> 早返` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§7.5) |
| `BREAK` | `BREAK() -> 跳出` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§7.5) |
| `CONTINUE` | `CONTINUE() -> 跳到下轮` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§7.5) |
| `AND` | `AND(a, b) -> BOOLEAN (short-circuit)` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§3.4) |
| `OR` | `OR(a, b) -> BOOLEAN (short-circuit)` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§3.4) |
| `NOT` | `NOT(a) -> BOOLEAN (取反)` | ❌ | ✔ | v0.2 | ✓ builtin | `resolve_builtin` (§3.4) |
<!-- 运算符 (12 条) -->
| `==` | `=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.2) |
| `!=` | `!(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.2) |
| `>` | `>(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.2) |
| `<` | `<(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.2) |
| `>=` | `>=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.2) |
| `<=` | `<=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.2) |
| `+` | `+(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.1) |
| `-` | `-(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.1) |
| `*` | `*(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.1) |
| `/` | `/(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.1) |
| `%` | `%(a, b) -> INTEGER` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§9.1) |
| `NEG` | `NEG(a) -> -a` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
<!-- ARRAY 操作 (9 条) -->
| `PUSH` | `PUSH(arr, x) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.1) |
| `POP` | `POP(arr) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.1) |
| `SHIFT` | `SHIFT(arr) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `UNSHIFT` | `UNSHIFT(arr, x) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `SLICE` | `SLICE(arr, start, end?) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `CONCAT` | `CONCAT(a, b) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `CONTAINS` | `CONTAINS(arr, x) -> BOOLEAN` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `INDEX` | `INDEX(arr, x) -> INTEGER / E0031` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `REVERSE` | `REVERSE(arr) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
<!-- DICT 操作 (6 条) -->
| `REMOVE_KEY` | `REMOVE_KEY(dict, k) -> DICT` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.2) |
| `DEL` | `DEL(dict, k) -> DICT (v0.3 alias, W0051)` | ❌ | ❌ | v0.2 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §10.2) |
| `KEYS` | `KEYS(dict) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `VALUES` | `VALUES(dict) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `HAS` | `HAS(dict, k) -> BOOLEAN` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `MERGE` | `MERGE(a, b) -> DICT` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
<!-- 下标 (3 条) -->
| `INDEX_GET` | `INDEX_GET(coll, k) -> v / E0031` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.1-§10.2) |
| `INDEX_SET` | `INDEX_SET(coll, k, v) -> NULL` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.1-§10.2) |
| `AT` | `AT(coll, i) -> v` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.1-§10.2) |
<!-- STRING 操作 (15 条) -->
| `UPPER` | `UPPER(s) -> STRING` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `LOWER` | `LOWER(s) -> STRING` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `SUB` | `SUB(s, start, end?) -> STRING` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `REPLACE` | `REPLACE(s, old, new) -> STRING` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `SPLIT` | `SPLIT(s, sep) -> ARRAY` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `TRIM` | `TRIM(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `TRIM_START` | `TRIM_START(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `TRIM_END` | `TRIM_END(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `STARTS_WITH` | `STARTS_WITH(s, pre) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `ENDS_WITH` | `ENDS_WITH(s, suf) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `REPEAT` | `REPEAT(s, n) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `PAD_START` | `PAD_START(s, n, c?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `PAD_END` | `PAD_END(s, n, c?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `CODEPOINTS` | `CODEPOINTS(s) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `FROM_CODEPOINTS` | `FROM_CODEPOINTS(arr) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
<!-- 格式化 (1 条) -->
| `FORMAT` | `FORMAT(template, args...) -> STRING` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.6) |
<!-- 模块系统 (4 条) -->
| `MODULE_REF` | `MODULE_REF(path) -> MODULE` | ❌ | ❌ | v0.4 | ⏳ deferred | — |
| `EXPORT` | `EXPORT(names) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§13.2) |
| `IMPORT` | `IMPORT(path, names, opts?) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§13.1) |
| `MODULE` | `MODULE(name?, body) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§13.1) |
<!-- OOP (3 条) -->
| `CLASS` | `CLASS(name?, parent, members) -> CLASS` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§11.1) |
| `NEW` | `NEW(cls, args...) -> INSTANCE` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§11.2) |
| `THIS` | `THIS -> 当前实例` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§11.3) |
<!-- 属性 / 方法 (3 条) -->
| `GET_PROP` | `GET_PROP(obj, k) -> v / E0037` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `SET_PROP` | `SET_PROP(obj, k, v) -> NULL` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
| `CALL_METHOD` | `CALL_METHOD(obj, m, args...) -> v` | ❌ | ❌ | v0.2 | ⏳ deferred | — |
<!-- 构造器 (2 条) -->
| `ARRAY` | `ARRAY(items...) / ARRAY()` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§10.1) |
| `DICT` | `DICT(pairs...) / DICT()` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§10.2) |

## 实现 gap (Deferred)

> spec 列了,impl 尚未接;按 spec 版本分桶:

- **v0.2** (23 项): INPUT, BOOL, CALL, NEG, SHIFT, UNSHIFT, SLICE, CONCAT, CONTAINS, INDEX, REVERSE, KEYS, VALUES, HAS, MERGE, UPPER, LOWER, SUB, REPLACE, SPLIT, GET_PROP, SET_PROP, CALL_METHOD

- **v0.4** (1 项): MODULE_REF

