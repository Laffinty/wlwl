# WLWL v0.10.0 动态 REVIEW 报告

> 审计对象：`D:\Project\wlwl` @ `f5d5976`（`main`，tag `v0.10.0_rc1`，**无 `v0.10.0` 正式 tag**）
> 被测产物：`impl/target/release/wlwl.exe`（`wlwl 0.10.0`，2026-09-29 00:46 构建）
> 桌面 `C:\Users\ikrx2\Desktop\123` 为发布物副本（`wlwl.exe` + docs + examples + wlwl-skill）
> 审计方式：读全部相关文档 → 编写并运行 97 个 WLWL 程序级 probe + 1 个 23 项核心回归 + 仓库既有套件 → 逐条与规范原文比对
> 审计日期：2026-09-29

---

## 0. 结论

**v0.10 的新功能是扎实的，但「发布即可用」这一条不成立。**

- 新增的静态契约层（`E0110`–`E0116` / `W0110`–`W0117`）本身实现质量高：在我写的 40 个静态层 probe 中，**只要清单合法，38 个全部按规范行为**（三档严重级、两个开关的独立性、数值放宽、容器逐元素下推、泛型擦除、`Comparable` 成员集、模块契约三向、`SEALED`、`sig`/`sig-gen`/`interface`/`schema` 全部正确）。
- 但**按文档教你的方式打开它，它不会生效，而且一声不吭**（P0-1）。这是本次最严重的问题：旗舰功能默认关闭是设计，默认打开却静默失效是缺陷。
- 我一共查出 **13 处「实测行为与规范原文矛盾」**，其中 5 条 P0。
- 仓库测试套件 **1744 passed / 0 failed**，全绿——但静态契约的 conformance 夹具里 **8/9 个断言是死代码**（P0-5）。绿灯掩盖了旗舰功能的验证缺口。
- `wlwl-spec-v0.10.md` 的**新增部分**（§2.6 / §5.2.1 / §7.4 / §9.6 / 附录 E）是本批质量最高的文字，`E0117` 的三处互锁式负向断言优于全文任何既有条款。但**声明与事实的缺口在扩大**：上一轮 REVIEW 点名的两个可证伪的规范错误（P1-1 码号、P1-2「逐条未改」）原样留存。

| 维度 | 判定 |
|---|---|
| 新功能实现质量 | **好** — 静态契约层几乎全对 |
| 新功能可用性 | **不合格** — 开启方式在文档所述条件下无效且无诊断（P0-1） |
| 规范 ↔ 实现一致性 | **差** — 13 处矛盾，含 5 条 P0 |
| 测试体系 | **不合格** — 旗舰功能的 conformance 断言 8/9 失效（P0-5） |
| 文档自洽性 | **差** — spec / CHANGELOG / skill / 附录 G 四处互相矛盾 |
| 规范行文质量 | **6.7/10**（v0.9 为 6.9/10，轻微退步） |

---

## 1. 测试方法与产物

本次全部产物在 `C:\Users\ikrx2\Desktop\123\review-v010\`：

| 文件 | 内容 |
|---|---|
| `cases/` | 97 个 probe 用例目录，每个含真实 `.wll` 源码 + `wlwl.toml` + `expect.json` |
| `probe.py` | probe 驱动器（每个用例复制到独立临时目录跑，避免 `sig-gen` 污染夹具） |
| `author_cases.py` | 用例作者脚本（`.wll` 源码是真实交付物，此脚本只是我的录入方式） |
| `probe-report.json` | 97 个用例的完整 stdout/stderr/退出码 |
| `audit_docs.py` | 附录 G 计数头、conformance 契约解析、注册表一致性的机械审计 |
| `chan_probe.py` | 通道/调度探针（带 8s 硬超时，能把挂死记成挂死） |
| `t/T01_core.wll` | v0.6–v0.8 核心语义 23 项回归（真值 / 短路 / ERR 路由 / 作用域 / 解构 / std 容器 / 格式化 / MATCH / 消费者 / 浮点指数 / MUT 上下文关键字 …） |
| `runcase.ps1` | 单文件运行器 |
| `cargo-test.log` | `cargo test --workspace` 完整日志 |

**结果**

```
probe 套件     : 97 用例 = 84 PASS + 13 DEVIATION + 0 FAIL
T01 核心回归   : 1 PASS（23 项断言）
cargo test     : 1744 passed / 0 failed / 2 ignored
金标准并发夹具 : 11/11 通过（其中 1 个按设计 exit 1）
v0.6 保真套件  : 10 个 fixture 输出与 v0.6/v0.9 双基线逐字节一致
skill 示例     : 10/10 通过
随包 examples  : 11 个中 **2 个跑不起来**（见 P1-2）
```

`DEVIATION` 是本套件的特殊状态：**断言的是实测行为，而实测行为与 `wlwl-spec-v0.10.md` 的规范性原文矛盾**。13 条全部在 §3/§4 逐条写清。

---

## 2. v0.9 → v0.10 演进梳理

v0.10 是**静态契约版**。运行期语义声称与 v0.9 逐条一致，词法面零扩张（`SEALED` 走前缀声明，不进关键字表），并发面零新增。增量全部落在编译期。

### 2.1 语言面

| 项 | 内容 | 规范出处 | 实测 |
|---|---|---|---|
| 类型注解文法进规范 | 附录 A 的 `Type`/`TypeArg`/`BoundedVar`/`ModuleDecl` 产生式首次进入规范 | 附录 A:1717-1721 | ✅ 解析正确 |
| `SEALED([...])` | 模块顶层前缀声明公开面；运行期 no-op；不是关键字 | §9.1:646-654 | ✅ 三向检查正确；`LET(SEALED, 1)` 仍是普通变量 |
| 旁路签名文件 `<mod>.wll.sig` | 一行一声明的契约文件 | §9.6 / 附录 E.1 | ✅ 三向检查正确 |
| `T: Comparable` | 裸标识符 + 显式约束；运行期完全擦除 | §5.2.1:437-442 | ✅ 擦除确认（`TYPE(mx)` = `FUNCTION`，`Value` 未变） |

### 2.2 静态层

- 开关：`[features] gradual_typing = off|warn|error`（默认 off）、`match_exhaustiveness`（默认跟随 `gradual_typing`，可独立）— §9.4:689-690
- 码：`E0110`–`E0116` + `W0110`–`W0117`，全部仅编译期；`W0117` 恒警告且**故意没有** `E0117` — §7.4:534-549 / §11.2:927-948

实测：注解失配 `E0110`、调用失配 `E0111`、返回失配 `E0112`、导出越界 `E0113`、声明未导出 `E0114`、签名类型冲突 `E0115`、非穷尽 `E0116`、不可达子句 `W0117` — **全部按规范触发，档位切换正确，开关独立性正确**。`wlwl schema` 正确地不列 `E0117`。

### 2.3 工具面

`wlwl sig`（只读不写盘）、`sig-gen`（默认不覆盖、`--force` 覆盖、零导出不写文件）、`interface`（`sealed: null` 与 `sealed: [...]` 可区分）、`schema`、`lsp`（无 `renameProvider`/`documentFormattingProvider`）— **全部实测符合文档**。

### 2.4 变更与修复

- `wlwl fmt` 不再把省略的 `MATCH` default 臂补写成 `, NULL` ✅ 实测确认
- 签名文件残余 `:` 改报 `E0010` ✅ 实测确认
- spec §11.3 警告表补 6 个 W 码、删不存在的 `W0014` ✅ 规范侧已改（但 skill 侧未同步，见 P1-6）

---

## 3. P0 发现（5 条）

### P0-1 · 静态契约层在文档所述的开启方式下静默失效，无任何诊断

**规范/文档怎么说**

- `CHANGELOG.md:20-22`：新能力全部默认关闭（`[features] gradual_typing` 缺省 `off`）
- `SKILL.md:41-43`：*"flip `[features] gradual_typing = "warn" | "error"` in wlwl.toml"*
- `spec:689`：特性表列出 `gradual_typing`
- `SKILL.md:57`：wlwl-skill 自带的 `examples/static_contracts.wll` 就是这个特性的示例，**没有配套 `wlwl.toml`**

**实测**

```
# 只写 [features]（= 文档教的写法）
[features]
gradual_typing = "error"

$ wlwl check main.wll      # main.wll: LET(flag: BOOLEAN, "yes"); PRINT(flag);
OK: parsed main.wll (42 bytes)      exit=0     ← 静默，什么都没有

# 加上 [package] 三件套
[package]
name = "probe"
version = "0.0.1"
entry = "main.wll"

[features]
gradual_typing = "error"

$ wlwl check main.wll
error[E0110]: annotation mismatch: expected `BOOLEAN`, found `STRING`
                              exit=1     ← 正常
```

**根因**（`impl/crates/wlwl-cli/src/main.rs:611-624` → `wlwl-toml/src/manifest.rs:184-199`）

`load_manifest_gradual_typing` 调 `wlwl_toml::manifest::parse`，而 `validate()` 要求 `package.name` / `package.version` / `package.entry` 齐备。缺任一项 → `parse` 返回 `Err` → **静默回落 `Off`**。

对比：清单**值**写错是有诊断的 ——
```
gradual_typing = "sideways"
→ warning[W0001]: invalid [features] gradual_typing value `sideways`; ... falling back to "off"
```
即「写错值」被诊断，「写对值但清单不完整」完全无声。这正是 ADR-0020 自己在注释里写的纪律「**不静默吞笔误**」的反例。

**影响**

1. 用户严格按 CHANGELOG / SKILL / spec 写清单 → 静态层永不执行 → 退出码 0 → 误以为「我的类型都对」
2. `wlwl-skill` 的旗舰特性示例 `static_contracts.wll` 随包发布却**从不触发任何静态检查**
3. CI 里只要项目根目录没有完整 `[package]` 清单，整个静态层就是装饰

**最小复现**：`cases/M1_features_only_manifest` vs `cases/M2_full_manifest`

**建议**：清单解析失败时发一条诊断（`W0001` 已有现成通道），或让 `[features]` 可独立于 `[package]` 生效；文档侧把「必须带完整 `[package]`」写进 SKILL.md 与 spec §9.4；给 `wlwl-skill/examples/static_contracts.wll` 配一份 `wlwl.toml`。

---

### P0-2 · §17 的「真挂起对程序文本透明」承诺未实现：挂起点的结果一旦被消费就丢失

**规范怎么说**（§17.1 表 1450-1456 + §17.4:1602，全部是**规范性**表述）

> 1452 `LET(x, YIELD())` → 「挂起后继续；`x` 取 `YIELD` 的值 `NULL`」
> 1453 `IF(cond, YIELD(), 42)` → 「在选中分支的 `YIELD` 处挂起后继续」
> 1454 `[1, YIELD(), 3]` → 「字面量为 `[1, NULL, 3]`」
> 1455 `WHILE`/`FOR`/`IF` 体内的 `YIELD` → 「恢复**嵌套体的剩余部分**」
> 1602 「挂起对程序文本是**透明**的… 恢复后语义上等价于『该表达式刚完成』。`LET(x, YIELD())` 恢复后 `x` **已绑定**」
> 1456 `LET(y, YIELD); y()` → 「间接调用与直接 `YIELD()` 等价」

**实测**（每一行都是 §17.1 表的一行；宿主 `SCOPE(FUN(() , LET(h, SPAWN(FUN(() , <body>))); AWAIT(h)))`）

| 规范行 | 任务体 | 规范期望 | 实测 | probe |
|---|---|---|---|---|
| 1452 | `LET(x, YIELD()); x` | `NULL` | **`error[E0020]: undefined name 'x'`** | `D3` |
| 1456 | `LET(y, YIELD); y()` | 等价于 `YIELD()` | **`error[E0020]: undefined name 'YIELD'`** | `D8` |
| 1453 | `IF(TRUE, YIELD(), 42)` | `42` | **`NULL`** | `D4` |
| 1454 | `[1, YIELD(), 3]` | `[1, NULL, 3]` | **`NULL`** | `D5` |
| 实参位 | `+(YIELD(), 100)` | `100` | **`NULL`** | `D2` |
| 1455 | `WHILE(<(i,3), SET(i,+(i,1)); YIELD()); i` | `3` | **`1`**（循环只跑一轮） | `D6` |

对照组（**不违反**）：`LET(x,1); YIELD(); x` → `1` ✅（`D9`）、`LET(x,1); IF(TRUE,YIELD(),0); x` → `1` ✅、`LET(a,10); YIELD(); +(a,5)` → `15` ✅、两个互让任务 → `1 2` ✅。

**可归纳的规律**：**挂起点之前建立的绑定在恢复后可见；挂起点所在的那条语句，如果它的值被消费，值就消失。** `YIELD` 在语句位置（结果被丢弃）是对的；一旦结果被外层表达式消费，整个表达式塌成 `NULL`。

**影响**

- §17.1 表 6 行 + §17.4 1 行，共 7 行规范性文本，**5 行不符**（另 2 行只覆盖了「挂起前绑定」这一半）
- 循环体里放 `YIELD` 会让循环**只执行一轮**，且返回值看起来「合理」（`1` 而不是 `NULL`），极难察觉
- 这是 v0.9 引入的「真挂起」特性，v0.10 原样继承

**定性**：不是 v0.10 的回归（v0.10 声称运行期与 v0.9 一致），而是**v0.10 把「运行期逐条一致」这句话写进了 CHANGELOG 首段和 spec §0.1/附录 D，等于给这个未兑现的承诺盖了章**。

**建议**：要么实现续体（continuation）保存，要么把 §17.1 表改成如实描述（并从「规范性」降级）。当前状态是最坏的——规范说 A，实现做 B，两者都在正式文档里。

---

### P0-3 · 无缓冲通道：生产者先 `SPAWN` 必然假死锁，与 §17.7 明文冲突

**规范怎么说**（§17.2.1:1491 + §17.7:1661）

> 1491 「挂起的任务离开运行队列，登记在通道的等待表上；**对端到达**或通道关闭时被唤醒」
> 1661 「同一通道上互为对端的挂起收发双方**不**构成死锁（见 §17.2.1 对端配对）」

**实测**（同一个程序，只改 spawn 顺序）

```
LET(ch, CHANNEL_NEW(0));
LET(p, SPAWN(FUN(() , CHANNEL_SEND(ch, 1); CHANNEL_CLOSE(ch); 0)));
LET(c, SPAWN(FUN(() , CHANNEL_RECV(ch))));
PRINT(AWAIT(c));  AWAIT(p)
```
| 顺序 | 结果 |
|---|---|
| 生产者先 spawn | **`E0065: deadlock ... 2 task(s) parked on channel op(s) with no peer to wake them: [#0, #1]`** |
| 消费者先 spawn | ✅ `1` |
| `CHANNEL_NEW(1)` 或 `2` | ✅ 正常 |

`strict_deadlock_detect = false` 时降级为 `W0065` + `E0053: AWAIT task 1 is suspended on a channel op; no peer task available to wake it` — 两种档位都认为「没有对端」。

**机制**：`p` 先跑，`CHANNEL_SEND` 在无缓冲通道上无等待接收方 → 挂起；`c` 后跑，`CHANNEL_RECV` 有等待发送方（`p`）→ 按 §17.2.1 应**立即唤醒 `p`**，但实现没有唤醒，而是先做了死锁判定。

**影响**

- 阻塞型 `CHANNEL_SEND`/`CHANNEL_RECV` 实际上是**顺序敏感且不可预测**的：文档里没有任何地方提示这一点
- `wlwl-skill/SKILL.md:382-401` 的「Canonical shapes / Producer-consumer」示例本身就跑不通：它在 `SCOPE` 主体里直接 `CHANNEL_RECV` → `E0053`（`cases/I5b_recv_outside_task`）；搬进任务里又撞上 `E0065`（`cases/I5`）
- 只有缓冲通道（`CHANNEL_NEW(n>0)`）上的生产者-消费者是可靠的（`cases/I5d_buffered_pipeline` ✅）

**建议**：唤醒等待对端后再做 L1 死锁判定；补一条金标准夹具钉住「生产者先 spawn」；修正 SKILL.md 的 canonical shape（改用缓冲通道，或把消费侧也 `SPAWN` 且保证接收方先就位）。

---

### P0-4 · `CLASS` / `INSTANCE` 的 `==` 恒为 `FALSE`，违反 §2.4 明文的对象身份规则

**规范怎么说**（§2.4:206，**规范性**，v0.9 起）

> 「`CLASS` / `INSTANCE` 按**对象身份**恒等：同一类对象与自身相等，两次 `CLASS(...)` 调用产生的类对象不相等，即使成员相同；**同一实例与其别名相等**，两次 `NEW(...)` 产生的实例不相等，即使字段值相同。」

**实测**（`cases/D1b_instance_identity` / `D1c_class_identity`）

```
LET(C, CLASS("C", NULL, []));  LET(a, NEW(C));  LET(b, a);
PRINT(==(a, b), ==(a, a), ==(NEW(C), NEW(C)));   →  FALSE FALSE FALSE
PRINT(==(C, C));                                  →  FALSE
```

自反律 `a == a` 都不成立。同一段代码里 `==(f, f)`（函数）和 `==(h, h)`（句柄）都正确返回 `TRUE`（T01 T16 通过），说明身份比较机制存在，只是 `CLASS` / `INSTANCE` 两个类型没接进去。

**旁证**：`wlwl-skill/SKILL.md:61` 写的是对的（"`==(cls, cls)` 和 `==(obj, obj)` 是 `TRUE`"），实现与规范、与 skill 三方不一致，实现是唯一的异类。

**影响**：任何依赖 `INSTANCE` / `CLASS` 去重的代码（例如把实例放进 `DICT` / `ARRAY` 做键）会静默拿到「每个实例都不同」的结果，不报错。

**建议**：把 `Class` / `Instance` 接入已有的身份比较路径；补 conformance 夹具（目前 `impl/tests/conformance/` 下没有任何 OOP 身份夹具）。

---

### P0-5 · 静态契约 conformance 夹具的 8/9 个断言是死代码

**夹具怎么声明契约**（`impl/tests/conformance/static_types/README.md` + `static_types.wll` 头部）

每份夹具文件开头写 `// expects: <码…>`，测试读出来当断言。`static_types.wll` 里有 **9 个 `// expects:` 块**（分别期望 `E0110` / `E0110 E0111` / `E0111` / `E0112` / `E0111` / `E0110` / `E0116` / `W0117`）和 **3 个 `// mode: both` 控制组**。

**测试怎么读**（`impl/crates/wlwl-cli/tests/conformance_static.rs:54-77`）

```rust
for line in source.lines() {
    let line = line.trim();
    if !line.starts_with("//") {
        if !line.is_empty() { break; }        // ← 遇到第一行代码就停
        continue;
    }
    if let Some(rest) = line.strip_prefix("// expects:") { expects = ...; }
    if line.trim_end() == "// mode: both"   { also_clean_when_off = true; }
}
```

**实测**（`audit_docs.py` 复刻同一逻辑跑真实夹具）

```
`// expects:` blocks actually parsed : ['E0110']
`// expects:` blocks present in file  : ['E0110`, 'E0110', 'E0110', 'E0111', 'E0112', 'E0111', 'E0110', 'E0116', 'W0117']
`// mode: both` markers in file        : 3
=> dead `expects:` blocks              : 8
=> dead `mode: both` control groups    : 3
=> codes never asserted at all         : ['E0111', 'E0112', 'E0116', 'W0117']
```

第 17 行 `LET(flag: BOOLEAN, "yes");` 一出现就 `break`，所以：**8 个断言块从未被读取**；3 个 `mode: both` 控制组（注释里反复强调「证明这套夹具与这套断言不是摆设」）**一个都没执行**；`E0111` / `E0112` / `E0116` / `W0117` 在夹具层面**零断言**。

守卫也没救回来：`contract_of` 的注释写「**不猜**：解析不出契约就让测试失败，免得夹具悄悄退化成『什么都不断言』」，但第一个块解析成功，守卫就通过了。**这个守卫防的是「没有契约」，防不住「契约只读到第一段」。**

整个静态契约 conformance 套件只有 **3 个 `#[test]`**（`conformance_static.rs:230 / 236 / 256`），其中 2 个是 `module_sig`，1 个是上面这个。加上 `SEALED` **完全没有 conformance 夹具**（只有 `sig.rs` 单测），`match_exhaustiveness` 开关本身也没有夹具。

**影响**：v0.10 旗舰功能的验证面，实际只有「`gradual_typing = "error"` 时出现 `E0110`」这一条断言在跑。`cargo test` 全绿（1744）掩盖了这一点。

**建议**：把 `contract_of` 改成扫描全文件的 `// expects:` 块并按块切分虚拟源，或者把 9 个断言拆成 9 个独立夹具文件（README 本来就是这个约定：「每份只带一个静态层能抓出的问题」——夹具文件自己违反了自己的 README）。

---

## 4. P1 发现（6 条）

### P1-1 · spec §2.6 的「三条实测事实」有 2 条与实测不符（上轮 REVIEW 的 P1-1 未修）

`spec:231-236` 把这三条标为「**规范性，来自参考实现的实际行为**」：

| # | 规范原文 | 实测 | 判定 |
|---|---|---|---|
| 1 | 裸 `ARRAY` 报 `E0010`；裸 `DICT`/`OPTION`/`RESULT` 能解析成不透明具名类型 | ✅ 完全一致（`A4` / `A5`） | 正确 |
| 2 | 「尖括号形式与箭头形式都不可达…`DICT<STRING, INTEGER>` 与 `FUN(INTEGER) -> STRING` **都不会报错**，但会被解析成一个名字里含括号的单个不透明类型」 | `FUN(INTEGER) -> STRING` → 静默通过 ✅；`DICT<STRING, INTEGER>` → **`E0011: expected ')', got Gt`** ❌ | **一半错** |
| 3 | 「`ARRAY[INTEGER]: Comparable` 报 **`E0012`**」 | **`E0010`**：`a type constraint may only follow a bare type variable (e.g. \`T: Comparable\`), not a type like ARRAY[…] or DICT[…]` | **错**（`E0012` 是返回类型失配码，无关） |

这一条同时是上上轮静态 REVIEW 的 **P1-1**，`c0d8652` 修了实现侧，**规范文本原样未动**。实测证据在 `cases/A2` / `cases/A3`。

另外 `wlwl-skill` 对这两条的描述**又和实现不一致**：`reference.md §24.1` 与 `SKILL.md:175-186, 514` 声称「箭头形式和尖括号形式**都是**解析错误」，但箭头形式实测是静默通过的。**四方（spec / skill / build-plan / 实现）在同一事实上有三个不同说法。**

### P1-2 · 随包发布的 examples 有 2 个跑不起来

| 文件 | 失败 | 性质 |
|---|---|---|
| `impl/examples/showcase.wll:19` | `E0020: undefined name 'true'` | 用了小写 `true`，WLWL 布尔字面量是 `TRUE`。文件头自称是 v0.4「Phase 4 showcase」，**从 v0.6 起就没再更新过** |
| `impl/examples/std_test.wll:10` | `E1003: /: division by zero` | 注释写「division by zero → caught as ERR」，但除零是**原生码不是 ERR**，`EXPECT_ERR` 捕不到 —— 这正是 skill 反模式表第 17 条自己警告的写法 |

两个文件同时出现在桌面发布副本 `123/examples/` 和仓库 `impl/examples/`。CI 不跑 examples，所以没人发现。

**建议**：修掉或删除；`showcase.wll` 至少把 `true` 改成 `TRUE`；`std_test.wll` 的期望应改成「除零是原生码，捕不到」。

### P1-3 · 三个公布的锁测试数与实测全部不符

| 出处 | 数字 | 位置 |
|---|---|---|
| `CHANGELOG.md:125` §0.4 一致性表 | **1656** | 表格里作为 v0.10 的正式一致性承诺 |
| `CHANGELOG.md:131-132` | **1731** | 正文注明「Step 12 使全量达到 1731」 |
| `docs/history/wlwl-build-plan-v0.10-COMPLETED.md:1218-1219` | **1736** | 计划书的最终值 |
| **实测 `cargo test --workspace`** | **1744 passed / 0 failed / 2 ignored** | 2026-09-29 |

§0.4 是一张**兼容性承诺表**，其中的数字一项都没对上。基线 1517 → 实测 1744 是 **+227**，表里写的是 **+139**。

### P1-4 · `MODULE` 在附录 G 注册为内建，但无法解析

```
$ wlwl run main.wll     # LET(m, MODULE("n", 1)); PRINT(m);
error[E0020]: undefined name `MODULE`
```

`impl/crates/wlwl-eval/src/registry.rs:1246-1256` 确实注册了 `MODULE`，`dispatch: LexerMacro`，`docs/appendix_G.md` 也列了它（`MODULE(name?, body) -> NULL`）—— 但附录 G 表里**没有任何列区分「已注册」和「可调用」**，读者会以为它能用。附录 G 自称是「具名构造的单一真相源」，实际上混着不可调用的条目。

（顺带：v0.10 新增的 `SEALED` 也不在附录 G 里。这不算违规——spec §12 已澄清它不是保留形式——但「单一真相源」的覆盖面有缺口。）

### P1-5 · `MODULE_REF` 的返回类型，文档内部三处不一致

| 出处 | 写法 |
|---|---|
| `spec:670` §9.2 | `MODULE_REF(path) -> MODULE` |
| `spec:2039` 附录 G | `MODULE_REF(path) -> DICT` |
| `docs/appendix_G.md:108` | `MODULE_REF(path) -> MODULE` |
| **实测** | **`DICT`** |

同一份规范文档的正文和自己的附录 G 就对不上。实测支持附录 G。构建计划书 `§4.2:547-551` 另有一说「实测不存在，`Value` 没有 `Module` 变体」——这一说被实现否定了（`MODULE_REF` 存在，返回 DICT）。

### P1-6 · `wlwl-skill` 与 spec v0.10 不同步（两处）

1. **幻码 `W0014`**：`reference.md §10` 的警告码清单仍列 `W0014`（non-ASCII case）。`CHANGELOG.md:82-86` 明确说这个码**注册表里根本不存在**，v0.10 已把它从 spec §11.3 删掉。skill 记的是已删除的码。
2. **指向已不存在的 spec 文本**：`reference.md §24.1` 与 `SKILL.md:175-186 / 514` 反复强调「Spec §5.2.1 fact #2/#3 是 stale，以编译器为准」。但当前 `spec:420-444` 的 §5.2.1 **已经被重写成与实现一致**，那两条「stale 事实」在规范里**根本不存在了**。skill 现在在警告一个已经被修掉的问题。

我的实测：非 ASCII `UPPER` 不发任何诊断（`cases/H6`），`ARRAY[INTEGER]: Comparable` 报 `E0010` —— 两条都符合 v0.10 规范原文，**是 skill 落后了**。

---

## 5. P2 发现（5 条）

| # | 问题 | 位置 |
|---|---|---|
| P2-1 | 附录 G 计数头与表体矛盾：声明「函数类内建 86 / 词法宏构造 24」，逐行统计实为 **82 / 28**（总数 110 对得上，分栏错了 4 条）。v0.9 → v0.10 附录 G **逐字节未变**，即此矛盾原样继承 | `spec:1944` vs `:1946-2071` |
| P2-2 | 4 处失效/指错交叉引用：`§5.5`（`:167`）、`§13.2.4`（`:418`）指向不存在的小节（v0.9 遗留未修）；`§7.3 的静态诊断`（`:6`）、`§7.3 的语义小节`（`:1882`）指错——静态诊断在 `§7.4` | spec 导言 + 附录 D |
| P2-3 | 「运行期语义与 v0.9 逐条一致（§1–§17 **只有 §5.2 的注解解释与 §7.3 的静态诊断被扩写**）」**不实**：实际有 **7 处**就地改动（§1.4 / §10.5 / §11.2 / §11.3 / §12 / §16.4 / §17.4），其中 **§16.4 与 §17.4 新增了 `[规范性]` 段落，落在 §0.1 声明的冻结范围（§13–§16）内** | `spec:6`、`spec:1882` vs `spec:18` |
| P2-4 | 附录 D 未申报正文新增：§16.4「`CALL_METHOD` 的控制流形态（规范性）」、§17.4「保留的 tag 面（规范性）」、§10.5 `UPPER`/`LOWER` 行整行重写、§11.2 新增「已注册但无触发路径」表 | `spec:1870-1888` |
| P2-5 | §17.7 的量化性能承诺「单任务路径相对 v0.6 退化 < 10%」无基准程序、无测量环境、无统计口径，**无任何情态动词**，无法写成验收测试 | `spec:1641` |

另有一处**规范内部矛盾**值得记录：`spec:1387` §16.1 写「`CALL_METHOD` 执行期间，方法体**可以**触发挂起」，而 `spec:1405-1409` §16.4 新增的「规范性」段落把 `MethodCall` tag 描述为「同步直落」。v0.10 新增文本与既有文本对同一件事给了不同答案。

---

## 6. 做对了的部分（应当保留）

不想只列问题，所以明确记录 v0.10 确实达标的部分：

1. **静态契约层的实现质量高**。40 个静态层 probe 里 38 个一次通过：三档严重级切换、`match_exhaustiveness` 默认跟随 + 可独立关闭、数值安全放宽（`INTEGER→FLOAT` 通 / `FLOAT→INTEGER` 拒）、容器字面量逐元素下推、`OPTION[T]` 的 `NULL`/`RESULT` 双见证且裸 `T` 不通过、未知类型名仍报 `E0110`、`Comparable` 成员集与运行期比较算子逐项一致、泛型运行期完全擦除、`DYNAMIC` 双向可赋值不产生诊断。
2. **`E0117` 的处理是全文范本**。spec `:939` / `:988` / `:1012` 三处互锁，明确「故意不存在，不是保留名」，`wlwl schema` 也不列它。这让「缺失的码」本身成为可断言的事实。
3. **模块契约三向检查全对**：`E0113`（导出越界）/ `E0114`（声明未导出）/ `E0115`（类型冲突），`.sig` 残余 `:` → `E0010`，`sig` 只读不写盘、`sig-gen` 默认不覆盖 / `--force` 覆盖 / 零导出不写文件。
4. **`SEALED` 设计克制**：不进关键字表、运行期 no-op、可被解析为普通变量（`LET(SEALED, 1)` 正常），§12 保留集合保持为空。
5. **v0.6 运行期保真度有金标准兜底**：`v07_fidelity.rs` 拿 10 个 fixture 的输出与 v0.6/v0.9 双基线逐字节比对，通过。§0.4 的「运行时码逐个不变 / 内建数 110 零新增 / 附录 G 表体逐字节不变 / 关键词表 14 不变」我逐条核对，**全部属实**。
6. **`wlwl interface` / `schema` 的 JSON 契约可用**：`sealed: null` 与 `sealed: [...]` 可区分，静态码表完整且正确排除 `E0117`。
7. **`fmt` 不补写 `, NULL` 的改动是对的**，且理由写在 CHANGELOG 里（不让「作者兜底」与「作者漏分支」在源码里同形）。

---

## 7. 规范行文质量：v0.10 vs v0.9

### 7.1 结构

两版 §0–§17 **同名同序，无新增、无重排**（`§13`/`§14`/`§15`/`§17` 在 v0.9 就已存在，v0.10 未新增这些章节）。真正的结构增量只有一处：

**附录 E** 从 v0.9 的零内容占位符（`## 附录 E / F(保留)`，正文一句「为将来预留」）变成 v0.10 的 **规范性工具契约**（44 行、3 个子节、1 段 EBNF、2 张 JSON 字段表）。这是实质新增的规范面，**不是标题改写**。

其余增量全部是既有章节的**追加式子节**：§2.6（22 行）/ §5.2.1（26 行）/ §7.4（17 行）/ §9.6（18 行），加上 §9.1 的 `SEALED` 段、§11.2 编译期码段、附录 A 的 5 个新产生式、附录 D 的 v0.10 段。落位方式符合 §0.1 的冻结条款（除了 P2-3 指出的两处）。

桌面副本 `123/docs/wlwl-spec-v0.10.md`（123852 B）与仓库 `docs/standard/`（121773 B）**内容逐行零差异**，差额 100% 来自 2079 个 CR。发布物与权威源一致。

### 7.2 规范性用语

| 用语 | v0.9 | v0.10 |
|---|---|---|
| 必须 / 不得 / 应当 / 可以 | 35 / 19 / 8 / 22 | 41 / 22 / 8 / 24 |
| 合计 / 千行 | 46.5 | 46.7 |

密度基本持平。**问题在于 §0.3 定义了四档情态，但 `MUST` / `SHOULD` 英文原文全文只各出现 1 次（即定义句自身）**，正文一律中文——术语表与正文用词不闭环。

v0.10 新增小节的规范密度（每百行）：附录 E 6.8、§16 6.7、§13 4.7、§15 4.4、§17 3.4、**§14 1.6**（124 行仅 2 处情态动词，是全文最低）。

**规范性缺口**：

1. `spec:1646` §17.7「通道关闭唤醒次序 | **未定义**（实现可任选）」——明示放弃，但表格本身是规范性章节，读者无法区分「未定义」与「遗漏」
2. `spec:1641` 量化性能承诺无任何情态动词（第 5 条 P2）
3. `spec:1193` §14 用「可以」描述可选特性，`:1208` 同章用「必须」描述硬约束，层级落差 100%
4. `spec:1645/1647` 用加粗「不」而非「不得」，与 §0.3 情态体系脱钩

### 7.3 可验证性

**写得好的**：`E0117`（三处互锁，可写负向断言）、§7.4 三条 MATCH 语义（可枚举构造子空间明确限定为 `BOOLEAN`/`NULL`/`RESULT` 三种，并说明整数/浮点/字符串/数组/字典**不报**——这一条写得极好，直接避免了「报了就是误报」）、§5.2.1 的安全数值放宽与 `OPTION[T]` 见证规则、`DYNAMIC` 双向可赋值。

**写不好的（无法支撑测试）**：

| 位置 | 条款 | 为什么测不了 |
|---|---|---|
| `spec:444` | 「内建调用的返回类型只在注册表**已结构化**的条目上可知」 | 「已结构化」是自指谓词，规范没给名单或判定规则。实测 60/110 在规范里不存在，测试只能照抄实现 |
| `spec:1641` | 「单任务路径相对 v0.6 退化 < 10%」 | 无基准、无环境、无口径；且「v0.6 运行时」是规范未定义的对象 |
| `spec:442` | 「`Comparable` 成员集合**对着运行期比较算子量出来**」 | 「量出来」是推导程序不是规则，成员表在规范里没有 |
| `spec:549` | 数组/字典穷尽性不判定，括号内是反例说明 | 「可盖住」无形式定义；`[_, 2]` 的覆盖语义在 §7.3 未定义 |
| `spec:1617-1622` | 保留 tag 面「本版不由运行期产生」 | tag 名不在任何文法或注册表里，「不产生」无可观测锚点。唯一代理是 `:1620` 的「同步直落」，而 §16.1:1387 说方法体**可以**挂起 —— 两处答案不同 |

### 7.4 交叉引用

穷举 `§N(.N)*` 与「附录 X」全部形态，对照 138 个标题：

| 失效引用 | 位置 | 状态 |
|---|---|---|
| `§5.5` | `:167` | v0.9 遗留，**未修** |
| `§13.2.4` | `:418` | v0.9 遗留，**未修** |
| `§附录 A.3` | `:981` | `§` 与「附录」混用，与全文 24 处裸引法不一致 |
| `§7.3 的静态诊断` | `:6` | **v0.10 新增，指错**（应在 §7.4） |
| `§7.3 的语义小节` | `:1882` | **v0.10 新增，指错**（同上） |

附录 A / E / G 引用全部有效。全文无编号表格，故无表引用可失效。

**净效果：旧的 2 处断链一条没修，新增 2 处指错。**

### 7.5 附录 G

- 条目层面：spec 附录 G 110 行 = `docs/appendix_G.md` 110 行，名称集合一致（唯一差异是 `||` vs `\|\|` 转义）
- **v0.9 → v0.10 附录 G 逐字节未变**，CHANGELOG §0.4 的「零新增、表体逐字节不变」承诺**属实**
- 但 mirror 与 spec 有 8 处实质冲突：`MODULE_REF` 返回类型、`GET_PROP`/`INDEX_GET` 漏诊断码、`UNWRAP` 失败态命名、`AT` 形参个数、`IMPORT` 可选参、OOP 六条的「引入」版本列（spec 写 v0.9 / mirror 写 v0.2）、`TASK_CURRENT`/`TASK_IS_CANCELLED` 的节号
- `appendix_G.md` 自身的分类注释错乱：`:65` 标「ARRAY 操作 (8 条)」但下一行只有 `PUSH`，`:67` 又标「DICT 操作 (8 条)」紧跟 `POP`，`:70` 再次出现「ARRAY 操作 (8 条)」

### 7.6 附录 D 完整性

**声称有、正文无**：没有。附录 D 的 7 条 v0.10 增量逐条能在正文定位（附录 A:1717-1721 + §2.6:216 / §5.2.1:420-444 + §9.4:689 + §11.2:931-933 / §9.1:646-654 + §9.6:698-714 + 附录 E:1890-1932 / §7.4:534-549 / §5.2.1:437-442）。

**正文有、附录 D 没申报**：4 处（P2-4）。

### 7.7 上轮静态 REVIEW 的落实情况

| # | 问题 | 状态 | 证据 |
|---|---|---|---|
| P0-1 | `fmt` 物化 `NULL` default 臂抑制 `E0116` | ✅ 实现已修 | 实测 `cases/G1` |
| P0-2 | 默认形参 / `*rest` 误报 `E0111` | ✅ 实现已修 | 实测正常 |
| P0-3 | `SEALED` 吞掉用户函数 | ⚠️ **部分** | 实现已修（实测 `E4` 通过），**规范未记录该歧义**；`:646-654` 仍只写「前缀声明」，未声明它同时可被解析为普通调用 |
| **P1-1** | §2.6 事实③码号错 | ❌ **未落实** | `spec:236` 仍写 `E0012`；本次实测 `E0010` |
| **P1-2** | 「§1–§17 逐条未改」自述不实 | ❌ **未落实** | 实际 7 处，其中 2 处在冻结章内 |
| P1-3 | `fmt` 可产出不可解析源码 | ❌ 未落实 | 规范无往返要求；实测 `G3`/`G4` 通过，说明已不再产出不可解析源码，但**规范仍无约束** |
| P1-4 | `SEALED` 位置不校验 | ⚠️ 部分 | 规范 `:1760` 有约束，实现行为已变（回退普通调用而非硬错），但 §9.1 未描述回退语义 |
| P1-5 | 密封分支 `E0023` 绕过 `self.diag()` | ❌ 未落实 | 纯实现缺陷，规范侧正确 |
| P2-7 | 附录 A 文法零锁测试 | ❌ 未落实 | 附录 A 本版大幅扩写（新增 `ModuleDecl` + `Type`/`TypePos`/`TypeArg`/`TypeHead`/`BoundedVar`），仍无机器校验 |

**汇总：3 个 P0 全在实现侧落实并经我实测复验；5 个 P1 中 3 个（P1-1 / P1-2 / P1-3）规范文本完全未动，2 个部分；P2 全部未动。**

### 7.8 评分

| 维度 | v0.9 | v0.10 | 依据 |
|---|---|---|---|
| 结构组织 | 7.5 | **8.0** | 附录 E 从占位变实质规范面；新增子节追加式落位 |
| 规范性用语 | 6.0 | **6.0** | 密度持平；`MUST`/`SHOULD` 与正文仍不闭环 |
| 可验证性 | 6.5 | **6.5** | `E0117`/`§7.4` 写得好，但 4 条不可测 + 1 处跨节矛盾 |
| 交叉引用 | 6.5 | **6.5** | 旧 2 处未修，新增 2 处指错，净持平 |
| 内部一致性 | 7.0 | **6.5** | 附录 G 计数矛盾原样继承；兼容承诺漏报 7 处；mirror 8 处冲突 |
| 规范完整性 | 7.0 | **7.0** | 附录 D 声称的 7 条全落地；正文 4 处未申报 |
| **加权总分** | **6.9** | **6.7** | |

**结论：v0.10 相对 v0.9 轻微退步（−0.2）。**

退步不在新增内容本身——静态契约层是本批质量最高的文字，`E0117` 的负向断言设计优于全文任何既有条款。退步来自**声明与事实的缺口在扩大**：v0.9 有 2 处失效引用 + 1 处悬空 W 码；v0.10 在此之上新增 2 处指错小节、4 处未申报改动、1 处跨节语义矛盾，而旧问题一条未修。两个可证伪的规范错误（P1-1 码号、P1-2 兼容声明）原样留存，而 `:6` 与 `:1882` 那句「§1–§17 逐条未改」现在描述的是一个改了 7 处的章节集合——它比在 v0.9 下更不成立。

**本次动态测试额外揭示的一点**：规范行文质量的评分和实现质量可以完全脱钩。v0.10 的静态层实现接近满分，但它作为「静态契约版」的**规范性门面**有两处可被证伪的错误（§2.6 事实②③）和一句不成立的兼容声明。

---

## 8. 测试体系质量

| 项 | 状况 |
|---|---|
| 套件规模 | `cargo test --workspace` = **1744 passed / 0 failed / 2 ignored**（2 个 ignored 是 `v07_fidelity_bless_golden` 之类，非 v0.10 引入） |
| 运行期保真 | `v07_fidelity.rs` 拿 10 个 fixture 与 v0.6/v0.9 双基线逐字节比对 ✅ 通过。基线文件自 2026-09-23 未再生成（v09 是 v06 的字节副本） |
| 静态契约夹具 | ❌ **9 个断言块只读进 1 个，3 个控制组全死**（P0-5） |
| 目录规划 vs 实际 | 计划书 §10.4 规划 4 个 conformance 目录，实际只有 2 个（`static_types/` / `module_sig/`）；`match_exh/` 与 `generics/` 不存在 |
| `SEALED` | 无 conformance 夹具，只有 `sig.rs` 单测 |
| `match_exhaustiveness` 开关本身 | 无夹具（`static_types.wll` 靠默认跟随） |
| 并发金标准 | `impl/tests/concurrency/` 11 个夹具全部通过，但**没有一个覆盖生产者/消费者模式**——P0-3 因此逃过 CI |
| OOP 身份 | 无夹具——P0-4 因此逃过 CI |
| examples | **不跑**——P1-2 因此逃过 CI |
| 内建签名覆盖 | 110 条中首批结构化 60 条，其余落 `DYNAMIC`（CHANGELOG 已如实声明，是范围裁剪不是缺陷） |
| 死代码式守卫 | `contract_of` 的「解析不出契约就让测试失败」防住了「没有契约」，防不住「契约只读到第一段」 |

**一句话**：CI 是绿的，但绿灯覆盖不到 v0.10 的四个主要风险面（静态契约断言、并发生产者-消费者、OOP 身份、examples）。

---

## 9. 建议的修复顺序

按「用户能不能用上 / 会不会被骗」的优先级：

### 第一批：阻断性（建议 v0.10.1）

1. **P0-1** 清单解析失败发 `W0001`；文档补「需完整 `[package]`」；给 `wlwl-skill/examples/static_contracts.wll` 配 `wlwl.toml`
2. **P0-5** `contract_of` 改为按 `// expects:` 块切分（或把 9 个断言拆成 9 个夹具文件），补 `SEALED` 夹具
3. **P1-2** 修/删 `showcase.wll` 与 `std_test.wll`；把 `impl/examples/` 接进 CI
4. **P0-4** `CLASS`/`INSTANCE` 接入身份比较；补 OOP 身份 conformance 夹具

### 第二批：功能正确性（建议 v0.11）

5. **P0-2** 实现续体保存，或把 §17.1 表改成如实描述并降级为「已知限制」
6. **P0-3** 唤醒等待对端后再做 L1 判定；补生产者先 spawn 的金标准夹具；修正 SKILL.md 的 canonical shape
7. **P1-1** 改 `spec:236` 的 `E0012` → `E0010`；改 `spec:235` 关于尖括号形式的表述；同步修 `reference.md §24.1` 与 `SKILL.md:175-186/514`（skill 落后于 spec）

### 第三批：文档一致性

8. **P1-3** §0.4 表的锁测试数改成实测 1744（或改成「以 CI 为准，不在表里写死」）
9. **P1-5** `spec:670` 的 `-> MODULE` 改成 `-> DICT`，与自己的附录 G 对齐
10. **P1-4** 附录 G 增列区分「已注册 / 可调用」，或把 `MODULE` 标成保留未实现
11. **P1-6** `reference.md` 删掉 `W0014`；`SKILL.md` 删掉已失效的「§5.2.1 stale」警告段
12. **P2-1** 附录 G 计数头改成实测 82/28；**P2-2** 修 4 处失效引用；**P2-3** `spec:6` / `:1882` 的兼容声明按实际 7 处改写；**P2-4** 附录 D 补 4 处申报

---

## 10. 复现方式

```powershell
cd C:\Users\ikrx2\Desktop\123\review-v010

# 全部 97 个 probe（84 PASS + 13 DEVIATION + 0 FAIL）
python probe.py
python probe.py A3          # 只跑 A 组（§2.6 三条实测事实）

# v0.6–v0.8 核心语义 23 项回归
powershell -NoProfile -File .\runcase.ps1 -Files .\t\T01_core.wll

# 通道/调度探针（带超时，能把挂死记成挂死）
python chan_probe.py

# 文档机械审计（附录 G 计数头 / conformance 契约解析 / 注册表一致性）
python audit_docs.py

# 仓库既有套件
cd D:\Project\wlwl\impl ; cargo test --workspace
```

被测二进制可用 `python probe.py <path-to-wlwl.exe>` 换成桌面发布副本
`C:\Users\ikrx2\Desktop\123\wlwl.exe`（同为 `0.10.0`，构建早于仓库最后一次修复提交，
SHA256 不同）复跑，以区分「实现问题」与「构建时间差」。
