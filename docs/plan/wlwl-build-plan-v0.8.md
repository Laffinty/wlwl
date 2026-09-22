# WLWL v0.8 实施构建计划

> **状态**:WIP — v0.7.0 已发布(2026-09-22,tag v0.7.0)。计划阶段(`wip0.8` 分支);尚未启动 Phase A。
> **前置**:v0.7.0(规范 `docs/standard/wlwl-spec-v0.7.md`,实现 commit `7d64889`)
> **目标**:清偿 v0.7 自洽性债;补正规范文字错漏;把实现限制从语义规则中分离。**不加新运行时原语**。
> **本阶段**:plan → 勘误/澄清 → 锁测试覆盖 → 收口发布。

---

## 0. 文档导读

### 0.1 调研背景

v0.8 不引入新能力。v0.7 的设计主轴(一切皆调用、错误即值、默认不可变、结构化并发 + 通道)**全部不动**。v0.8 的全部任务是**让规范内部不再互相打架**,并把"v0.7.0 参考实现的限制"与"WLWL 语言语义"在规范文本上明确分离。

外部审计报告(2026-09-22,`WLWL_v0.8_开发目标报告.md`)识别 10 项 P0/P1 缺陷。本计划的吸收策略见 §0.3,逐项处置见附录 A。

### 0.2 关键决策(v0.8 启动时锁定)

| # | 决策项 | 决策 | 备注 |
|---|--------|------|------|
| 1 | 技术栈 | **沿用 Rust + tree-walking** | 不引入字节码 |
| 2 | 运行时 | **协作式单线程调度器保留** | v0.7.0 的路径 B 静态切段策略**不**在 v0.8 重写 |
| 3 | 语义模型 | **结构化并发 + 通道保留** | §17 章节结构不变 |
| 4 | Brown 9 维度 | **沿用 v0.7.0 选择** | Lazy/Dynamic/Dynamic/Strong/Awaited/Destructive/Aware/Top-Down/Transient |
| 5 | 取消传播 | **复用 §8.2 ERR transparent propagation** | 不引入独立 cancellation 异常 |
| 6 | 版本号约定 | **保留 major.minor.patch,patch=0 时省略** | 不引入 v0.7.1 erratum 概念;v0.8 是次版本修订 |
| 7 | "只追加"承诺 | **沿用 v0.7 措辞,补"非矛盾子集"限定** | 见 §10 D21 |
| 8 | 兼容性 | **v0.7 程序在 v0.8 下求值结果相同**,除非该程序依赖 v0.7 规范内部矛盾点 | 矛盾点列表见 v0.8 release notes |
| 9 | 许可证 | **GPL-2.0-or-later,FSF 老版 verbatim** | 沿用 v0.7 |
| 10 | spec 文件改动时机 | **Phase A 末尾起即可改动** | 与 v0.7 的"Phase H1 才改"不同;v0.8 多数修订本身就是规范文字 |
| 11 | v0.8 范围 | **规范文字勘误 + 语义对称性澄清 + 锁测试覆盖** | 见 §1.2 |
| 12 | 目标用户 | **公开** | 不变 |

### 0.3 v0.7 → v0.8 关键变化

| 类别 | 变化 | 来源 |
|------|------|------|
| 文档勘误 | §8.2 示例重写 | 报告 F-01 |
| 文档勘误 | §12 全表重写,与附录 G 对齐 | 报告 F-02 |
| 文档勘误 | 附录 G `POP` 条目更正 | 报告 F-03 |
| 文档勘误 | 附录 G 交叉引用全面重写 | 报告 F-04 |
| 文档勘误 | `E0055`/`E0057` 标注"无触发路径" | 报告 F-05 |
| 文档勘误 | 统一版本号记法(v0.7.0 → v0.7 或 v0.7.0 二选一) | 报告 F-08 |
| 语义澄清 | `NOT` 入 §8.3 消费者表 | 报告 F-06 |
| 语义澄清 | 字面量下标允许(若实现已允许) | 报告 F-07 |
| 语义澄清 | `FORMAT` `{N}` 索引基准 | 报告 F-09 |
| 语义澄清 | 负数与 `-(...)` 词法消歧 | 报告 F-10 |
| 语义澄清 | `EXPECT_ERR` 入 §8.3 消费者表 | 报告 F-11 |
| 语义对称 | `FUN`/`LET` 返回值不对称 normative 说明 | 报告 G-01 |
| 语义对称 | `=` 三重身份消歧 | 报告 G-02 |
| 实现限制剥离 | `YIELD()` 位置限制从语义降级为实现限制 | 报告 G-03 |
| 文档澄清 | 类型名 `TASK` 不构成标识符绑定 | 报告 G-04 |
| 文档澄清 | `SHIELD` 与 `SCOPE` 关系 | 报告 G-05 |
| 文档澄清 | `AWAIT` "宿主诊断"定义 | 报告 G-06 |
| 文档澄清 | `%` 对浮点实参产生 `E0030` | 报告 G-07 |
| 文档澄清 | `SCOPE(ERR)` 行为 | 报告 G-08 |
| 锁测试覆盖 | BUILTIN_REGISTRY 覆盖 v0.7 新增 17 个并发 builtin | §6.0 |
| 锁测试覆盖 | `deviations.md` 中 v0.7 闭包可升级行为锁定为 v0.7 偏差 | §6.0 |
| 不变项 | `wlwl-spec-v0.7.md` §17(并发)、§8(错误模型)、§4.3(运算符即函数) | — |

### 0.4 v0.7 baseline(本计划不重写)

v0.7.0 发布于 2026-09-22(tag `v0.7.0`)。包含:

- 17 个并发/通道 builtin(SCOPE/SPAWN/AWAIT/YIELD/TASK_*/SHIELD/CHANNEL_*)
- 6 个新错误码 E0052-E0057(E0058 在 v0.7 §11.2 列名,实际注册 E0052-E0057)
- 110 builtin 总条目
- §17 章节 7 个子节
- 3 个新 ADR(0014/0015/0016)
- 实现侧 Phase G 质量门禁全绿

v0.8 任何 commit **不得**降低 v0.7 baseline 的可观察行为(见 D21)。

---

## 1. 实施总览

### 1.1 目标

让 v0.8 成为 v0.7 之后第一份**自洽可审计**的规范,并把实现限制从语言语义中剥离,使 v0.7.0 的"只追加"承诺在 v0.8 修订后仍可被严格验证。

### 1.2 任务范围(必须做)

- **规范文字勘误**:F-01/F-02/F-03/F-04/F-05/F-08(报告 P0/P1 A 类)
- **语义对称性裁定**:F-06/F-07/F-09/F-10/F-11/G-01/G-02(报告 P0/P1)
- **实现限制剥离**:G-03(报告 P1,最深刻的洞察)
- **文档澄清**:G-04/G-05/G-06/G-07/G-08(报告 P2)
- **锁测试覆盖**:BUILTIN_REGISTRY 与规范 §8.3 / §17 的对齐验证(报告未覆盖,本计划自加)
- **deviations 收口**:v0.7 闭包可升级行为锁定为 v0.7 偏差(报告 F-05 / G-04 注记相关)
- **release.yml 校验**:确认 v0.8 不改变打包资产清单(staging 包含 `wlwl-skill`,不含 `docs/plan`)

### 1.3 范围之外(明确不做)

- **新运行时原语**:不引入新 builtin、新内建类型、新错误码、新章节
- **新错误码触发路径**:`E0057` 继续保留;不在 v0.8 引入触发条件
- **破坏性别名变更**:`AND`/`OR` 不在 v0.8 改为 `&&`/`||` 别名(留 v0.9+ 评估)
- **结构性重写**:不引入字节码、不替换调度器路径 B
- **新 ADR**:v0.8 不新增 ADR(本计划范围属"修补"非"决策")
- **format / docs/site 更新**:站点页面在 v0.7.0 时已同步,v0.8 不动;除非规范文字改动要求站点同步

---

## 2. 实施阶段

### Phase A — 读 impl 验证报告假设 + 文档勘误(0.5 人·周)

**目标**:把报告里所有"需参考实现确认"的悬而未决项落地为确定结论;完成全部纯文档性勘误。

**任务**:

- [ ] A1:阅读 `impl/crates/wlwl-eval/src/registry.rs` 与 §8.3 消费者表,核对 v0.7 17 个新并发 builtin 是否全部 NOT_IN_ERR_CONSUMER(报告 G-09 假设)
- [ ] A2:阅读 `impl/crates/wlwl-eval/src/runtime.rs`、`task.rs`、`channel.rs`、`yield_split.rs`,确认:
  - `YIELD` 位置检查是否真的只在静态切段路径 B 下生效(报告 G-03)
  - `CHANNEL_NEW(buf)` 是否有 `buf` 上界,数值多少(报告附录未覆盖项 #4)
- [ ] A3:阅读 `impl/crates/wlwl-eval/src/lib.rs` 中 `%` 对浮点实参的实现行为,确认是否产生 `E0030` 还是自动截断(报告 G-07 假设)
- [ ] A4:阅读 `impl/crates/wlwl-parser/src/` lexer/parser,确认字面量下标 `[1,2,3][0]` 当前是否允许(报告 F-07 假设"若 v0.7 实现禁止")
- [ ] A5:阅读 `impl/crates/wlwl-eval/src/lib.rs` 中 `TYPE` 实现,确认 `TYPE` 是普通函数调用还是特殊形式(报告 G-04 假设)
- [ ] A6:基于 A1-A5 结果,在 plan §0.3 表格里把每行的"假设"换成"已确认/已修订"
- [ ] A7:F-01 重写 §8.2 示例(3 个示例替换原 4 行)+ §8.5 补"顶层丢弃表达式算逃逸"
- [ ] A8:F-03 附录 G `POP` 条目更正,语义在 A2 确认后填写
- [ ] A9:F-04 附录 G 交叉引用全面重写(按 §0.3 映射表)
- [ ] A10:F-05 §11.2 `E0055`/`E0057` 标注"v0.7/v0.8 无触发路径"
- [ ] A11:F-08 统一版本号记法(选定 v0.7,不改 v0.7.0)
- [ ] A12:对 A1-A5 的阅读做一份简短的"impl 现状摘要"附录,作为 B/C/D 阶段的工作基础

**产出**:规范正文修订 + impl 现状摘要附录 + 报告悬而未决项收口。

**门禁**:
- 报告附录未覆盖项 #4(`CHANNEL_NEW(buf)` 上界)有定论
- 报告 G-09 / G-04 / G-07 / F-07 的"假设"标记全部清除
- 规范正文变更全部在 git diff 中体现

### Phase B — 错误模型消费者表补齐(0.3 人·周)

**目标**:§8.3 消费者表与正文语义对齐。

**任务**:

- [ ] B1:F-06 `NOT` / `!` 入 §8.3 消费者表(建议措辞见报告);`BOOL` 已在表中,与 `NOT` 同行并列
- [ ] B2:F-11 `EXPECT_ERR` 入 §8.3 消费者表;§8.3 正文末已有独立段,移到表中或保留段并在表里给条目
- [ ] B3:锁测试:在 `impl/crates/wlwl-eval/src/lib.rs` `b11_*` 系列测试中补:
  - `b11_not_is_consumer`:`NOT(ERR("x"))` 不透传,返回 `FALSE`
  - `b11_expect_err_is_consumer`:`EXPECT_ERR(42)` panic → `E0100`;`EXPECT_ERR(ERR("x"))` 返回 `ERR("x")`
- [ ] B4:同步更新 `docs/appendix_G.md`(若 `BOOL`/`NOT`/`EXPECT_ERR` 状态列已存在,核对;否则补)
- [ ] B5:同步更新 `impl/crates/wlwl-eval/src/registry.rs` 的 `ErrConsumerStatus` 字段,确认 `NOT`/`EXPECT_ERR` 标记为 `Yes`

**产出**:§8.3 表对齐 + 2 个新锁测试 + registry.rs 一致性更新。

**门禁**:`cargo test --workspace` 全绿(本阶段新增 2 个锁测试),无现有测试回归。

### Phase C — 字面量下标 + `=` 三重身份消歧(0.5 人·周)

**目标**:消除 §4.5 / §1.5 / §5.2 三处文档自相矛盾。

**任务**:

- [ ] C1:F-07 字面量下标:基于 A4 结果,二选一:
  - 选项 A(推荐):允许,删除 §4.5 末句,补 §A.2 文法注释
  - 选项 B:禁止,修改 §A.2 文法
  - 默认 A;若 A4 显示实现不允许,改 B
- [ ] C2:F-10 负数与 `-(...)` 词法消歧,补 §1.6 一句 normative 文字
- [ ] C3:G-02 `=` 三重身份消歧,补 §1.5 一段 normative 文字(报告措辞);§A.2 文法 `Postfix` 的 `"[" Expression "]" "=" Expression` 加注释"`=` 在此处是终结符,非 `OpToken`"
- [ ] C4:lexer 测试:补 `lex_neg_paren.wll` 与 `lex_negative_int.wll` 两个 conformance fixture,验证 `-1` 与 `-(1)` 解析不同
- [ ] C5:若 C1 选 A,在 `impl/crates/wlwl-parser/src/` 补"字面量下标"测试,确认 `[1,2,3][0] == 1`
- [ ] C6:更新 §4.1 操作数列表:`LET` 表达式可作 `ExprStmt` 或 `Block` 直接子项,**不得**作调用实参或运算符操作数(报告 G-01 附带要求)
- [ ] C7:同步 §A.2 文法 `Expression` 的 `LetExpr` 子句加注释

**产出**:3 处文档对齐 + lexer 测试覆盖。

**门禁**:lexer 锁测试全绿;规范与实现一致。

### Phase D — §17 实现限制剥离(0.3 人·周)

**目标**:把 §17.1 的 `YIELD` 位置限制从"语言硬规则"降级为"v0.7 参考实现限制"。

**任务**:

- [ ] D1:G-03 §17.1 `YIELD()` 位置限制改为"v0.7/v0.8 参考实现采用静态切段调度,因此仅支持多语句 Block 的直接子项位置;其他位置产生 `E0014`。该限制是实现限制,不是语言语义;未来版本可放宽而不视为破坏性修订。"
- [ ] D2:§17.7 补一行"v0.7/v0.8 不承诺 `YIELD` 在任意表达式位置的让出语义"
- [ ] D3:G-09 同步消化(报告 G-09 与 G-03 关联,G-03 落地后 G-09 自动解决)
- [ ] D4:锁测试:补 `b17_yield_position_*` 三个测试:
  - `b17_yield_in_block` 合法位置不报错
  - `b17_yield_in_if_cond` 非法位置产生 `E0014`
  - `b17_yield_in_let_rhs` 非法位置产生 `E0014`
- [ ] D5:更新 §17.0 设计摘要明确"路径 B 是 v0.7/v0.8 实现选择,不是语言规范"

**产出**:§17 章节实现/语义分离 + 3 个锁测试。

**门禁**:§17.1 / §17.7 / §17.0 三处措辞一致;锁测试全绿。

### Phase E — 文档澄清(0.5 人·周)

**目标**:完成 G-04 / G-05 / G-06 / G-07 / G-08 五处澄清,全部为 normative 文字,不涉及实现改动。

**任务**:

- [ ] E1:G-04 §2.1 补"类型名不构成标识符绑定"normative 段;§10.11 文档里 `TASK` 写作 `wlwl:std.ai.TASK` 全限定形式
- [ ] E2:G-05 §17.3 补"SHIELD 不创建作用域,可在任何位置;若在 SCOPE 外调用,屏蔽当前任务的取消(顶层任务无取消源,故 no-op)"
- [ ] E3:G-06 §17.1 `AWAIT` "宿主诊断"定义:`AWAIT` 目标任务若因实现内部错误(如 `E0101` 栈溢出)终止,该诊断在 `AWAIT` 处重新抛出;用户 `ERR` 值作为普通结果返回(已在 §17.5 描述,此处补措辞交叉引用)
- [ ] E4:G-07 §2.2 `%(a, b)` 对浮点实参产生 `E0030`;§11.2 `E0030` 触发列表补"非整数 `%` 实参"
- [ ] E5:G-08 §17.5 补"实参求值为 ERR 时按 8.2 透明传播;§17.1 表中 `E0052`(fn 不是函数)等参数类型检查在实参 ERR 时不触发"
- [ ] E6:F-02 §12 全表重写,逐条对齐附录 G 的注册状态(报告 §2 F-02 措辞);`AND`/`OR` 维持"reserved,求值产生 `E0020"`(不引入弃用别名,v0.8 不做)
- [ ] E7:F-09 §10.7 `FORMAT` 索引基准:`{N}` 的 `N` 自 0 起,含首实参;模板混用 `{name}` 与 `{0}` 时首实参既作为字典也作为 `{0}` 渲染源;首个实参非字典时 `{name}` 形式占位符按未匹配原样保留
- [ ] E8:G-01 §5.1 补 normative 段说明 `FUN`/`LET` 返回值不对称是有意的;§3.1 补 `SET` 表达式值是 `NULL`(已存在但措辞强化)
- [ ] E9:附录 G 交叉引用同步更新(与 F-04 在 Phase A 完成后一并落地)

**产出**:8 处规范文字澄清 + 附录 G 与 §12 同步对齐。

**门禁**:规范正文 grep 自检无未覆盖的"P0/P1 项"措辞残留。

### Phase F — 质量门禁(0.3 人·周)

**目标**:确保 v0.8 修订不引入回归。

**任务**:

- [ ] F1:`cargo test --workspace` 全绿;新增锁测试(b11_* ×2 + b17_* ×3 + lex_* ×2 + b12_* 视 Phase E6 需要)全部 PASS
- [ ] F2:`cargo clippy --workspace --all-targets -D warnings` 0 error
- [ ] F3:`cargo doc --workspace --no-deps` 0 warning
- [ ] F4:`cargo deny check` 0 violation
- [ ] F5:spec conformance suite(`impl/tests/conformance/*.wll` + 新增字面量下标 fixture `impl/tests/parser/literal_index.wll`)全 pass
- [ ] F6:cross-task concurrency regression:Phase B/C/D 新增测试在 `cargo test --workspace` 下绿;`ec_*` 跨任务 ERR consumer 系列无回归
- [ ] F7:Phase E 措辞自检脚本:对规范 `docs/standard/wlwl-spec-v0.8.md`(届时新建)grep 报告 P0/P1 项,确认全部覆盖
- [ ] F8:`appendix_G.md` 与 `registry.rs::BUILTIN_REGISTRY` 的锁测试(已有 `b11_generated_md_matches_registry`)复跑绿

**门禁**:8 项全绿;无 warning。

### Phase G — deviations 收口(0.2 人·周)

**目标**:把 v0.7 的实现偏差显式登记为"v0.7 偏差,不在 v0.8 修复"。

**任务**:

- [ ] G1:把 `deviations.md`(v0.7) 中的"闭包可升级被捕获单元格"从"实施偏差"升级为"v0.7 永久行为,与 §3.3 字面不完全一致;v0.8 不修复,显式登记"
- [ ] G2:补 `deviations-v0.8.md`(若 v0.8 启动时新建):记录所有 v0.8 阶段的实现限制(YIELD 路径 B、字面量下标选 A/B 后是否需要锁、等等)
- [ ] G3:`E0057` 的"v0.7/v0.8 无触发路径"与 deviations-v0.8.md 关联

**产出**:`docs/history/deviations-v0.8.md`(新建,归档 v0.8 阶段的所有偏差)。

**门禁**:deviations-v0.8.md 与规范正文措辞一致。

### Phase H — 收口发布(0.3 人·周)

**目标**:打 tag `v0.8.0`,发布 release notes。

**任务**:

- [ ] H1:`CHANGELOG.md` 新增 `[v0.8.0]` 条目,逐项列出 Phase A-G 改动
- [ ] H2:`wlwl-skill/CHANGELOG.md` 同步(若需要)
- [ ] H3:`README.md` / `docs/site/index.md` 的"Spec: v0.7" 更新为"Spec: v0.8"(仅 spec 版本号引用)
- [ ] H4:`docs/standard/wlwl-spec-v0.8.md` 创建(由 v0.7 spec 复制 + 所有 Phase A-E 修订落地)
- [ ] H5:`docs/history/wlwl-spec-v0.7.md` 已在 v0.7.0 归档;确认归档完整
- [ ] H6:`wlwl-build-plan-v0.8.md` 顶部加 ARCHIVE NOTE 横幅,文件重命名为 `wlwl-build-plan-v0.8-COMPLETED.md`
- [ ] H7:deviations-v0.8.md 加 ARCHIVE NOTE
- [ ] H8:`release.yml` 校对:staging 包含 `wlwl-skill`,不含 `docs/plan`
- [ ] H9:打 annotated tag `v0.8.0` 指向 Phase F 全绿的 commit;message 引用 §0.3 改动清单
- [ ] H10:`git push origin v0.8.0` 触发 release pipeline(沿用 v0.7.0 release.yml)
- [ ] H11:人工确认 GitHub Actions run 全绿、GitHub Release 资产正确

**门禁**:tag 推送后 24h 内 GitHub Actions 跑通且资产完整。

---

## 3. 模块划分

### 3.1 Rust crate 边界

```
crates/wlwl-std       — 不需要改
crates/wlwl-eval      — 锁测试追加(b11_not_is_consumer, b17_yield_position_*)
                        registry.rs :: BUILTIN_REGISTRY 状态字段对齐
crates/wlwl-parser    — 字面量下标(若选 A);lexer 负数消歧测试
crates/wlwl-lexer     — 负数消歧无实现改动(仅补 normative 文字)
crates/wlwl-ast       — 不需要改
crates/wlwl-error     — 不需要改
crates/wlwl-toml      — 不需要改
crates/wlwl-formatter — 不需要改
crates/wlwl-cli       — 不需要改
```

### 3.2 文件改动清单

| 路径 | 阶段 | 类型 |
|------|------|------|
| `docs/standard/wlwl-spec-v0.7.md` | A, B, C, D, E | 修订(就地) |
| `docs/standard/wlwl-spec-v0.8.md` | H4 | 新建 |
| `docs/history/wlwl-spec-v0.7.md` | H5 | 已在 v0.7.0 归档,本阶段不动 |
| `docs/appendix_G.md` | A, E9 | 修订 |
| `docs/plan/wlwl-build-plan-v0.8.md` | 本文件 | 新建(本阶段) |
| `docs/plan/wlwl-build-plan-v0.8-COMPLETED.md` | H6 | 重命名 |
| `docs/history/deviations-v0.8.md` | G2, H7 | 新建 + 收口 |
| `CHANGELOG.md` | H1 | 新增条目 |
| `wlwl-skill/CHANGELOG.md` | H2 | 视需要 |
| `README.md` / `docs/site/index.md` | H3 | 版本号引用 |
| `impl/crates/wlwl-eval/src/lib.rs` | B3, D4 | 加测试 |
| `impl/crates/wlwl-eval/src/registry.rs` | B5 | 字段对齐 |
| `impl/crates/wlwl-parser/src/` | C5 | 字面量下标测试(若选 A) |
| `impl/tests/parser/literal_index.wll` | C5 | 新建 |
| `impl/tests/parser/lex_neg_paren.wll` | C4 | 新建 |
| `impl/tests/parser/lex_negative_int.wll` | C4 | 新建 |

---

## 4. 测试

### 4.1 锁测试新增

| 测试名 | 阶段 | 目的 |
|--------|------|------|
| `b11_not_is_consumer` | B3 | `NOT` 不透传 ERR |
| `b11_expect_err_is_consumer` | B3 | `EXPECT_ERR` 消费者语义 |
| `b17_yield_in_block` | D4 | `YIELD` 在 Block 直接子项合法 |
| `b17_yield_in_if_cond` | D4 | `YIELD` 在 IF 条件位产生 `E0014` |
| `b17_yield_in_let_rhs` | D4 | `YIELD` 在 `LET` 右侧产生 `E0014` |
| `lex_neg_paren` | C4 | `-(1)` 解析为减法调用 |
| `lex_negative_int` | C4 | `-1` 解析为字面量 |
| `literal_index` | C5 | `[1,2,3][0] == 1`(若选 A) |

### 4.2 并发安全测试

v0.8 不引入新并发原语;现有 `ec_*` 跨任务 ERR consumer 系列、`scope_*` SCOPE 行为系列、`yield_midbody` 中段让出系列全部复跑,不引入回归。

### 4.3 spec conformance

`impl/tests/conformance/*.wll` 全 pass(无新增 fixture,但现有 fixture 与 v0.8 spec 措辞须一致;若不一致,在 Phase E 同步修订 fixture)。

### 4.4 性能承诺

v0.8 不涉及运行时原语;无 benchmark 承诺变化。`cargo bench` 复跑确认无退化(实际不可能退化,作为安全网)。

---

## 5. 风险评估

| 编号 | 风险 | 等级 | 概率 | 应对 |
|------|------|------|------|------|
| R1 | A1-A5 读 impl 后发现报告假设错了(F-07 字面量下标实现已禁止 / G-04 `TYPE` 是特殊形式) | 低 | 30% | Phase C/E 已给 A/B 二选一 |
| R2 | `b11_not_is_consumer` 测试 FAIL,说明实现已是 NOT 消费者(只是规范没写) | 中 | 25% | 测试直接通过即可,规范补登记 |
| R3 | v0.8 规范改动触发"只追加"承诺争议(用户程序依赖报告 F-01 旧示例) | 低 | 5% | F-01 示例重写不改变语义,仅规范文字 |
| R4 | 报告 v0.9+ 附录项(§10.10 RUN_TESTS / §10.8 STRINGIFY / §17.2 buf 上界)在 v0.8 实施期间被发现是真实问题 | 低 | 10% | 升级到 Phase A 早期 read impl 时主动检查 |
| R5 | F-04 附录 G 引用重写引入新错误 | 低 | 15% | F8 锁测试 `b11_generated_md_matches_registry` 复跑 |
| R6 | §17.1 YIELD 降级措辞被误解为"v0.8 不承诺 YIELD" | 中 | 20% | D1 措辞明确"v0.7/v0.8 参考实现"而非"v0.8 整体" |

---

## 6. 决策清单

| # | 决策项 | 选项 | 决策 | 备注 |
|---|--------|------|------|------|
| D1 | 字节码 | 引入 / 不引入 | 不引入 | 沿用 v0.7 |
| D2 | 调度器路径 B | 替换 / 保留 | 保留 | v0.8 不做运行时改动 |
| D3 | OS 线程 | v0.8 / v0.9+ / 不做 | 不做 | 沿用 v0.7 D19 |
| D4 | 新错误码触发路径 | v0.8 / v0.9+ | v0.9+ | `E0057` 继续保留 |
| D5 | 字面量下标 | 允许 / 禁止 | 待 Phase A4 | 默认 A |
| D6 | `AND`/`OR` | 弃用别名 / 维持 reserved / 移除 | 维持 reserved | v0.8 不做 |
| D7 | `=` 三重身份 | 收敛 / 保留 | 保留(消歧) | 见 §0.3 G-02 |
| D8 | `YIELD` 位置限制 | 语义规则 / 实现限制 | 实现限制 | 报告 G-03 |
| D9 | 版本号 | 引入 patch / 维持 minor-only | 维持 minor-only | v0.8 直接发布,不发布 v0.8.1 |
| D10 | erratum 拆出 | 拆出 v0.7.1 / 并入 v0.8 | 并入 v0.8 | 报告 v0.7.1 提议扬弃 |
| D11 | 性能承诺 | 提速 / 不退化 / 不承诺 | 不退化 | v0.8 无运行时改动,作为安全网 |
| D12 | spec 文件改动时机 | Phase A 起 / Phase H 起 | Phase A 起 | v0.8 多数修订是规范文字 |
| D13 | 锁测试覆盖 | 仅追加 / 全量覆盖 | 仅追加 | v0.8 不重写锁测试 |
| D14 | OS 平台矩阵 | 沿用 v0.7 | linux x86_64/aarch64 + macOS x86_64/aarch64 + windows x86_64 | 沿用 |
| D15 | SHIELD 与 SCOPE 关系 | 必须嵌套 / 独立 | 独立 | 见 E2 |
| D16 | AWAIT 宿主诊断 | 抛 ERR / 抛诊断 / 文档化 | 文档化 | 见 E3 |
| D17 | % 对浮点 | E0030 / 自动截断 | E0030 | 见 E4(待 A3 确认) |
| D18 | SCOPE(ERR) 行为 | 透传 / E0052 | 透传 | 见 E5 |
| D19 | TYPE 与命名空间 | 特殊形式 / 普通调用 | 待 A5 | 默认普通调用 |
| D20 | 跨任务 cell 升级 | 保留 v0.7 / 修正 | 保留 v0.7 | 见 G1 |
| D21 | "只追加"承诺 | 沿用 v0.7 措辞 / 加"非矛盾子集"限定 | 加限定 | 见 §10 D21 |
| D22 | v0.8 release notes 矛盾点列表 | 单独附 / 并入 CHANGELOG | 并入 CHANGELOG | 见 H1 |

---

## 7. spec 文件改动时机

- **v0.7 spec**(`docs/standard/wlwl-spec-v0.7.md`):Phase A 末尾起允许就地修订;修订后保留作为 v0.7 最终版
- **v0.8 spec**(`docs/standard/wlwl-spec-v0.8.md`):H4 阶段新建,内容 = v0.7 spec + 所有 Phase A-E 修订 + Phase H1 release notes 摘录
- **v0.7 spec 归档**:`docs/history/wlwl-spec-v0.7.md` 在 v0.7.0 发布时已归档,本阶段不动

---

## 8. 依赖与基础设施

- Rust toolchain:沿用 v0.7 stable
- CI:`.github/workflows/ci.yml` 沿用;`release.yml` 在 v0.7.0 已更新(包含 wlwl-skill / 不含 docs/plan),v0.8 不改
- 锁测试:`b11_*` 系列已有;Phase B3/D4 新增 5 个
- 文档生成:`docs/appendix_G.md` 由 `cargo run --bin gen-appendix-g` 生成;v0.8 阶段需重新生成

---

## 9. 与 v0.6 → v0.7 策略对比

| 维度 | v0.6 → v0.7 | v0.7 → v0.8 |
|------|--------------|--------------|
| 策略 | **只追加** | **只追加,补"非矛盾子集"限定** |
| spec 文件改动时机 | Phase H1 | Phase A 起 |
| 新章节 | §17 并发 | 无 |
| 新 builtin | 17 | 0 |
| 新错误码 | 6 | 0(但补全登记) |
| 文档勘误 | 随 Phase 落地 | **专门 Phase A 集中** |
| 实现限制剥离 | 无显式步骤 | **专门 Phase D** |
| 锁测试覆盖 | 随 Phase 落地 | **专门 Phase F** |
| deviations 收口 | 随 Phase 落地 | **专门 Phase G** |

---

## 10. 决策清单(细节展开)

### D21 "只追加"承诺措辞

v0.7 spec §0.4:

> 任何程序若在 v0.6 下有定义,在 v0.7 下求值结果相同。

v0.8 改为:

> 任何程序若在 v0.7 下有定义、且不依赖 v0.7 规范内部矛盾点的程序,在 v0.8 下求值结果相同。

"内部矛盾点"清单(v0.8 release notes 列出):

1. §8.2 示例自相矛盾(报告 F-01)— 不影响可观察行为,纯文字
2. 附录 G `POP` 与 §10.4 不一致(报告 F-03)— 若用户依赖 `POP(arr)` 等同 `AT_K(d,k)`,需迁移
3. 附录 G 引用错位(报告 F-04)— 不影响可观察行为
4. `E0055`/`E0057` 措辞模糊(报告 F-05)— v0.7/v0.8 无触发路径,纯文字
5. §17.1 `YIELD` 位置从语义降级为实现限制(报告 G-03)— 若用户程序在非法位置放 `YIELD` 且期待合法,需迁移
6. §2.2 `%` 对浮点实参(报告 G-07)— 若用户程序 `%(7.5, 2)` 期待自动截断,会从无错变 `E0030`,需迁移

其余 P0/P1 项(F-02/F-06/F-07/F-09/F-10/F-11/G-01/G-02/G-04/G-05/G-06/G-08)**不改变可观察行为**。

---

## 11. 任务范围依赖图

```
Phase A  ──┬──► Phase B
           ├──► Phase C
           ├──► Phase D
           ├──► Phase E
           └──► (impl 现状摘要附录)

Phase B ──┐
Phase C ──┤
Phase D ──┼──► Phase F
Phase E ──┤
          └──► Phase G

Phase F ──┐
Phase G ──┴──► Phase H
```

Phase A 是所有后续阶段的前置(impl 假设验证)。Phase B/C/D/E 可并行(改动 disjoint 文件)。Phase G 依赖 Phase E(决定 deviations 内容)。Phase H 在 F/G 之后。

---

## 12. 暂搁项(留 v0.9+)

| 编号 | 来源 | 简述 |
|------|------|------|
| V9-01 | 报告 v0.9+ 附录 #4 | `CHANNEL_NEW(buf)` 上界定义 |
| V9-02 | 报告 v0.9+ 附录 #3 | `STRINGIFY` 字典键插入序 vs 字典序 |
| V9-03 | 报告 v0.9+ 附录 #2 | `RUN_TESTS` 返回结构字段缺失行为 |
| V9-04 | 报告 v0.9+ 附录 #5 | 规范格式化器与文档注释(///)的存在意义冲突 |
| V9-05 | 报告 v0.9+ 附录 #1 | `concurrency` 特性开关(禁用 §17) |
| V9-06 | 报告 v0.9+ 候选 | `AND`/`OR` 弃用别名评估 |
| V9-07 | 报告 v0.9+ 候选 | `E0057` 触发路径设计 |
| V9-08 | 报告 v0.9+ 候选 | `FORMAT` 占位符扩展 |

这些条目**不进入 v0.8 范围**;在 v0.8 收口时归档到 `docs/history/v0.9-candidates.md`。

---

## 附录 A · 报告条目处置总表

| 报告编号 | 阶段 | 处置 | 备注 |
|---------|------|------|------|
| F-01 §8.2 示例重写 | A7 | ✅ 采纳 | 文字勘误 |
| F-02 §12 vs 附录 G 对齐 | E6 | ✅ 采纳 | `AND`/`OR` 维持 reserved |
| F-03 POP 条目更正 | A8 | ✅ 采纳 | 语义需 A2 确认 |
| F-04 附录 G 引用重写 | A9, E9 | ✅ 采纳 | — |
| F-05 E0055/E0057 标注 | A10 | ✅ 采纳 | — |
| F-06 NOT 消费者地位 | B1 | ✅ 采纳 | — |
| F-07 字面量下标 | C1 | ✅ 采纳(待 A4) | 默认 A |
| F-08 版本号 | A11 | ✅ 采纳 | 不引入 patch |
| F-09 FORMAT 索引 | E7 | ✅ 采纳 | — |
| F-10 负数消歧 | C2 | ✅ 采纳 | — |
| F-11 EXPECT_ERR 入表 | B2 | ✅ 采纳 | — |
| G-01 LET/FUN 不对称 | E8 | ✅ 采纳 | 保留不对称 |
| G-02 = 三重身份 | C3 | ✅ 采纳 | — |
| G-03 YIELD 降级 | D1 | ✅ 采纳 | — |
| G-04 类型名撞名 | E1 | ✅ 采纳(待 A5) | — |
| G-05 SHIELD 与 SCOPE | E2 | ✅ 采纳 | — |
| G-06 AWAIT 宿主诊断 | E3 | ✅ 采纳 | — |
| G-07 % 对浮点 | E4 | ✅ 采纳(待 A3) | — |
| G-08 SCOPE(ERR) 行为 | E5 | ✅ 采纳 | — |
| G-09 YIELD 尾部行为 | D3 | ✅ 采纳(并入 G-03) | — |
| v0.7.1 erratum | — | ⛔ 扬弃 | 引入 patch level |
| AND/OR 弃用别名 | — | ⛔ 扬弃 | v0.8 不做破坏性 |
| §10.10 RUN_TESTS | — | ⛔ 扬弃 | 无矛盾证据 |
| §10.8 STRINGIFY | — | ⛔ 扬弃 | 无矛盾证据 |
| §17.2 buf 上界 | V9-01 | ⛔ 移 v0.9+ | 报告自标 v0.9+ |

---

## 附录 B · 与 v0.7 build plan 的对应关系

| v0.7 build plan 章节 | v0.8 build plan 对应 |
|----------------------|----------------------|
| §0 文档导读 | §0 文档导读 |
| §1 实施总览 | §1 实施总览 |
| §2 实施阶段(Phase A-H) | §2 实施阶段(Phase A-H) |
| §3 模块划分 | §3 模块划分 |
| §4 锁测试 | §4 测试 |
| §5 测试 | §4 测试 |
| §6 风险评估 | §5 风险评估 |
| §10 决策清单 | §6 / §10 决策清单 |
| §附录 D-1 交接摘要 | (v0.8 收口时新建) |

结构对齐,方便审计对照。

---

**审批**:本计划需 Li 批准后启动 Phase A。