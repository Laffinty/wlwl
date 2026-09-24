# 附录 G:全局内建注册表 (impl 视角)

> 本文件由 `wlwl-eval::registry::generate_appendix_g_md()` 自动生成。
> 单源真相是 `crates/wlwl-eval/src/registry.rs::BUILTIN_REGISTRY`,本 markdown 是镜像。
> 修改流程:改注册表 -> 跑本函数重写本文件 -> 跑 `cargo test` 验证 lock test。

> 对照规范:`docs/standard/wlwl-spec-v0.8.md` 附录 G (规范性)。

总条目数:**110** | 已实现:**86** | LexerMacro:**24** | Deferred:**0**


| 名称 | 签名 | ERR 消费者 (§8.3) | 宏函数 (§1.4) | 引入 | 状态 | 实现位置 |
|------|------|--------------------|---------------|------|------|----------|
<!-- I/O (3 条) -->
| `PRINT` | `PRINT(args...) -> NULL` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.2) |
| `PRINT_ERR` | `PRINT_ERR(args...) -> NULL` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.2) |
| `INPUT` | `INPUT(prompt?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.2) |
<!-- 类型 / 转换 (7 条) -->
| `LEN` | `LEN(coll) -> INTEGER` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `STR` | `STR(x) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `INT` | `INT(s) -> OK(INTEGER) / ERR(ParseError)` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.3) |
| `FLOAT` | `FLOAT(s) -> OK(FLOAT) / ERR(ParseError)` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.3) |
| `TYPE` | `TYPE(x) -> STRING (RESULT -> "RESULT")` | ✔ | ✔ | v0.2 | ✓ builtin | `resolve_builtin` (§2.5) |
| `BOOL` | `BOOL(x) -> BOOLEAN` | ✔ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§2.2) |
| `CALL` | `CALL(fn, args...) -> v` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§8.3) |
<!-- RESULT 处理 (12 条) -->
| `IS_OK` | `IS_OK(x) -> BOOLEAN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§8.3) |
| `IS_ERR` | `IS_ERR(x) -> BOOLEAN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§8.3) |
| `OR_DIE` | `OR_DIE(x, default) -> v (v0.3 alias)` | ✔ | ✔ | v0.2 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §8.3) |
| `UNWRAP_OR` | `UNWRAP_OR(x, default) -> v` | ✔ | ✔ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `UNWRAP` | `UNWRAP(x) -> v / PANIC` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `ERR_PAYLOAD` | `ERR_PAYLOAD(x) -> e / E0030` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `WRAP` | `WRAP(err, ctx) -> ERR / OK` | ✔ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§8.3) |
| `TRY` | `TRY(e) -> v / early-RETURN` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `PANIC` | `PANIC(msg) -> 终止` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§8.4) |
| `OK` | `OK(v) -> RESULT` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§8.1) |
| `EXPECT_ERR` | `EXPECT_ERR(expr) -> OK(payload) / ERR(E0049)` | ✔ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§8.3) |
| `ERR` | `ERR(e) -> RESULT` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§8.1) |
<!-- 控制流 / 逻辑 (10 条) -->
| `IF` | `IF(cond, t, e?) -> v` | ✔ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `WHILE` | `WHILE(cond, body) -> v` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `FOR` | `FOR(var, iter, body) -> NULL` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `MATCH` | `MATCH(v, clauses, default?) -> v` | ❌ | ✔ | v0.4 | ✓ macro | parser -> `Expr::*` (§6) |
| `RETURN` | `RETURN(v?) -> 早返` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `BREAK` | `BREAK() -> 跳出` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `CONTINUE` | `CONTINUE() -> 跳到下轮` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§6) |
| `AND` | `AND(a, b) -> BOOLEAN (short-circuit)` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§4.3) |
| `OR` | `OR(a, b) -> BOOLEAN (short-circuit)` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§4.3) |
| `NOT` | `NOT(a) -> BOOLEAN (取反)` | ❌ | ✔ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
<!-- 运算符 (14 条) -->
| `==` | `=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `!=` | `!(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `>` | `>(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `<` | `<(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `>=` | `>=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `<=` | `<=(a, b) -> BOOLEAN / ERR 透传` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `+` | `+(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `-` | `-(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `*` | `*(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `/` | `/(a, b) -> INTEGER / FLOAT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `%` | `%(a, b) -> INTEGER` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
| `&&` | `&&(a, b) -> BOOLEAN (v0.6 §4.3 short-circuit)` | ✔ | ❌ | v0.6 | ✓ builtin | `resolve_builtin` (§4.3) |
| `||` | `||(a, b) -> BOOLEAN (v0.6 §4.3 short-circuit)` | ✔ | ❌ | v0.6 | ✓ builtin | `resolve_builtin` (§4.3) |
| `NEG` | `NEG(a) -> -a` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§4.3) |
<!-- ARRAY 操作 (8 条) -->
| `PUSH` | `PUSH(arr, x) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- DICT 操作 (8 条) -->
| `POP` | `POP(d, k, default) -> v (v0.6 compat alias for AT_K; signature kept 3-arg)` | ❌ | ❌ | v0.6 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §10.4) |
| `AT_K` | `AT_K(d, k, default) -> v (v0.6 §10.4)` | ❌ | ❌ | v0.6 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- ARRAY 操作 (8 条) -->
| `SHIFT` | `SHIFT(arr) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `UNSHIFT` | `UNSHIFT(arr, x) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `SLICE` | `SLICE(arr, start, end?) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `CONCAT` | `CONCAT(a, b) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `CONTAINS` | `CONTAINS(arr, x) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `INDEX` | `INDEX(arr, x) -> INTEGER / -1` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `REVERSE` | `REVERSE(arr) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- DICT 操作 (8 条) -->
| `REMOVE_KEY` | `REMOVE_KEY(dict, k) -> DICT` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.4) |
| `DEL` | `DEL(dict, k) -> DICT (v0.3 alias, W0051)` | ❌ | ❌ | v0.2 | ✓ compat (W0051/W0054) | `resolve_builtin` (compat, §10.4) |
| `KEYS` | `KEYS(dict) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `VALUES` | `VALUES(dict) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `HAS` | `HAS(dict, k) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
| `MERGE` | `MERGE(a, b) -> DICT` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.4) |
<!-- 下标 (3 条) -->
| `INDEX_GET` | `INDEX_GET(coll, k) -> v / E0031` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§4.5) |
| `INDEX_SET` | `INDEX_SET(coll, k, v) -> NULL` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§4.5) |
| `AT` | `AT(coll, i) -> v` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§4.5) |
<!-- STRING 操作 (15 条) -->
| `UPPER` | `UPPER(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `LOWER` | `LOWER(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `SUB` | `SUB(s, start, end?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `REPLACE` | `REPLACE(s, old, new) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `SPLIT` | `SPLIT(s, sep) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `TRIM` | `TRIM(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `TRIM_START` | `TRIM_START(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `TRIM_END` | `TRIM_END(s) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `STARTS_WITH` | `STARTS_WITH(s, pre) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `ENDS_WITH` | `ENDS_WITH(s, suf) -> BOOLEAN` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `REPEAT` | `REPEAT(s, n) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `PAD_START` | `PAD_START(s, n, c?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `PAD_END` | `PAD_END(s, n, c?) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `CODEPOINTS` | `CODEPOINTS(s) -> ARRAY` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
| `FROM_CODEPOINTS` | `FROM_CODEPOINTS(arr) -> STRING` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§10.5) |
<!-- 格式化 (1 条) -->
| `FORMAT` | `FORMAT(template, args...) -> STRING` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§10.7) |
<!-- 模块系统 (4 条) -->
| `MODULE_REF` | `MODULE_REF(path) -> MODULE` | ❌ | ❌ | v0.4 | ✓ builtin | `resolve_builtin` (§9) |
| `EXPORT` | `EXPORT(names) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§9) |
| `IMPORT` | `IMPORT(path, names, opts?) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§9) |
| `MODULE` | `MODULE(name?, body) -> NULL` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§9) |
<!-- OOP (3 条) -->
| `CLASS` | `CLASS(name?, parent, members) -> CLASS` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§13) |
| `NEW` | `NEW(cls, args...) -> INSTANCE` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§13) |
| `THIS` | `THIS -> 当前实例` | n/a | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§15) |
<!-- 属性 / 方法 (3 条) -->
| `GET_PROP` | `GET_PROP(obj, k) -> v / E0037` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§13) |
| `SET_PROP` | `SET_PROP(obj, k, v) -> NULL` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§13) |
| `CALL_METHOD` | `CALL_METHOD(obj, m, args...) -> v` | ❌ | ❌ | v0.2 | ✓ builtin | `resolve_builtin` (§13) |
<!-- 构造器 (2 条) -->
| `ARRAY` | `ARRAY(items...) / ARRAY()` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§10.9) |
| `DICT` | `DICT(pairs...) / DICT()` | ❌ | ✔ | v0.2 | ✓ macro | parser -> `Expr::*` (§10.9) |
<!-- 并发 / 通道 (17 条) -->
| `SCOPE` | `SCOPE(fn) -> v` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `SPAWN` | `SPAWN(fn) -> TASK` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `AWAIT` | `AWAIT(task) -> v` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `YIELD` | `YIELD() -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.1) |
| `TASK_CURRENT` | `TASK_CURRENT() -> TASK` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `TASK_IS_CANCELLED` | `TASK_IS_CANCELLED() -> BOOLEAN` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `TASK_CANCEL` | `TASK_CANCEL(task) -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `TASK_CANCEL_PARENT` | `TASK_CANCEL_PARENT() -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `SHIELD` | `SHIELD(fn) -> v` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.3) |
| `CHANNEL_NEW` | `CHANNEL_NEW(buf) -> CHANNEL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_CLOSE` | `CHANNEL_CLOSE(ch) -> NULL` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_SEND` | `CHANNEL_SEND(ch, v) -> NULL / ERR(ChannelWouldBlock)` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_RECV` | `CHANNEL_RECV(ch) -> v / ERR(ChannelClosed|ChannelWouldBlock)` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_TRY_SEND` | `CHANNEL_TRY_SEND(ch, v) -> BOOLEAN` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_TRY_RECV` | `CHANNEL_TRY_RECV(ch) -> v / NULL / ERR(ChannelClosed)` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_LEN` | `CHANNEL_LEN(ch) -> INTEGER` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
| `CHANNEL_CAP` | `CHANNEL_CAP(ch) -> INTEGER` | ❌ | ❌ | v0.7 | ✓ builtin | `resolve_builtin` (§17.2) |
