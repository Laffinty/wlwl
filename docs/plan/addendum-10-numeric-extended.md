# 10 · `std.math` 扩展(任意精度 / 分数 / 统计 / 近似比较)

> **层级** L2 · **硬前置** 00(R0 治本)
> **状态** 未动工 · **规范登记** 本批**不在** §14 已登记清单里 —— 由本方案
> 首次提出,需业主立项确认

## 0. 契约摘要

- **目标**:补三块当前**完全空缺**的数值能力 —— 任意精度(十进制)、
  精确分数、描述统计;再加一组**近似比较**成员。
- **非目标**(违反即范围事故):
  - **不做复数**。`std.math` 现在是实数域;复数是**另一个域**,塞进来会
    改掉 `MIN` / `MAX` / `CLAMP` 的定义边界。
  - **不做矩阵 / 线性代数**。那是数值方法的领域,不是标准库原语。
  - **不做 `f64` 之外的新内建数值类型**。任意精度与分数都用**已有**的值
    形状承载(见 §3.1)—— 新增内建类型是语言变更,不是标准库扩张。
  - 不做任意精度的三角 / 对数 / 超越函数(库与规范都不该假装它们精确)。
  - 不改现有 32 个成员的**任何**返回值(它们是契约钉死的)。
- **为什么值得做**:`std.math` 有 32 个成员,看着很全 —— 但**全部**是
  `FLOAT` / `INTEGER` 口径。金额、比例、统计平均,这三类真实需求现在只能
  手搓 `FLOAT` 近似,而浮点误差会在对账时变成事故。

## 1. 基线(可核验)

```powershell
cd docs
Select-String -Path stdlib/wlwl-stdlib-spec-v0.11.md -Pattern '## 7 `std.math`' -Context 0,6
cd ..\impl
Select-String -Path crates/wlwl-std/src/math.rs -Pattern '^pub fn ' | Measure-Object
```

| 项 | 现值 | 出处 |
|---|---|---|
| `std.math` 成员 | **32**(全部 `FLOAT` / `INTEGER` 口径) | 附录 A |
| 整数数学 | `SIGN` / `DIV_CEIL` / `GCD` / `LCM` / `IS_SQRT` / `POW_MOD`(R1 门面) | 规范 §7.1 |
| 超越函数 | `LN` / `LOG2` / `LOG10` / `EXP` / `TRUNC` / 三角族 / 双曲族 / `ATAN2`(R2 内核) | 规范 §7.1 |
| 边界口径 | `LN(0)` = `-inf` 而非错误;域违例 → `ERR([kind: DomainError])`;`TRUNC` ≠ `INT` | 规范 §7.1 |
| 主流对照 | Python `decimal` / `fractions` / `statistics`、Java `BigDecimal` / `MathContext`、Go `math/big`、Rust **无**(在 `crates` 外) | — |
| 规模问题 | D11-012:建数组类成员平方级(数字序列的构造会撞上) | `docs/history/20261001.md` |

## 2. 成员面

**层:R2 全员。**

### 2.1 任意精度十进制(`std.math` 内)

| 成员 | 语义要点 | 失败口径 |
|---|---|---|
| `DECIMAL(str)` | 从字面量构造,`DICT` 形状(见 §3.1) | 非法字面量 → `E0030` |
| `DECIMAL_ADD` / `_SUB` / `_MUL` | 精确算术 | 越界 → `E0030` |
| `DECIMAL_DIV(a, b, scale?)` | **`b = 0` → `ERR([kind: DomainError])`**;`scale` 是除法保留位数(必须显式) | 同上 |
| `DECIMAL_CMP(a, b)` | 返 `-1` / `0` / `1` | 永不失败 |
| `DECIMAL_TO_STRING(a)` | 渲染 | 永不失败 |
| `DECIMAL_ROUND(a, scale, mode)` | `mode` 显式枚举(不设默认 —— 银行家舍入 vs 四舍五入是业务决定) | 未知 `mode` → `E0030` |

### 2.2 精确分数

| 成员 | 语义要点 |
|---|---|
| `FRACTION(n, d)` | 约分到最简;`d = 0` → `ERR` |
| `FRACTION_ADD` / `_SUB` / `_MUL` / `_DIV` | 精确算术 |
| `FRACTION_TO_DECIMAL(f, scale?)` | 转十进制(可能不精确 ⇒ `scale` 必填) |
| `FRACTION_TO_STRING(f)` | 渲染成 `"3/4"` |

### 2.3 描述统计(输入 `ARRAY[INTEGER|FLOAT]`)

| 成员 | 语义要点 |
|---|---|
| `MEAN` / `MEDIAN` / `MODE` | 空数组 → `NULL`(同 §5「空数组得 NULL」口径) |
| `STDDEV` / `VARIANCE` | 样本 vs 总体要**显式区分**(`STDDEV_S` / `STDDEV_P`) |
| `PERCENTILE(arr, p)` | `p ∈ [0, 100]`;插值方式写明 |
| `CORRELATION` / `LINEAR_FIT` | 两个等长数组;不等长 → `ERR` |

### 2.4 近似比较(最该加、也最常被漏的一块)

| 成员 | 语义要点 |
|---|---|
| `APPROX_EQ(a, b, rel_tol, abs_tol)` | **显式相对 + 绝对容差**。这是 `ASSERT_EQ` 在浮点上真正能用的形态 |
| `IS_NAN(x)` / `IS_INF(x)` | `NaN` / `±inf` 判定(现在只能靠 `==(x, x)` 绕) |
| `SIGNIFICANT_DIGITS(x, n)` | 有效位数 —— 比容差更直觉的判定 |

## 3. 语义裁决点(开写前必须逐条回答)

1. **数据形状(硬裁决)**:wlwl 没有任意精度类型。三个选项:
   (a) 用 `DICT` 承载(`{sign, digits, scale}`)—— 无语言变更,但每次运算
   一次字典构造,且**用户能手工构造出非法状态**;
   (b) 用 `STRING` 承载(十进制字面量)—— 运算就是字符串算术,慢;
   (c) **新增内建类型** —— 干净,但那是**语言变更**(要过分册升版流程)。
   → 本份**强烈建议 (a)**,并在规范里写明「`DICT` 形状是公开契约,字段名
   变更即 breaking」;把 (c) 留给「1.0 之后新增内建类型不受限」那条路。
2. **`DECIMAL_DIV` 的除不尽怎么办?** 静默截断 = 精度谎言。→ 必须 `scale`
   必填,且在结果里保留「是否精确」的标记。
3. **`DECIMAL_ROUND` 给不给默认模式?** 银行家舍入(C# `decimal` / Python
   `ROUND_HALF_EVEN`)vs 四舍五入,业务含义不同 ⇒ 建议**不给默认**,强制显式。
4. **统计的输入类型**:`ARRAY[INTEGER]` 与 `ARRAY[FLOAT]` 混装怎么办?
   → 建议:混装即 `E0030`(静默提升会丢精度)。
5. **`MEDIAN` 偶数长度取哪个?** 两个中间值平均 vs 取左 —— 写死。
6. **统计与 `std.collection` 的边界**:`MEAN` 是 `REDUCE(arr, 0, ADD)` 的糖
   吗?若语义完全相同,加 `MEAN` 就是冗余。→ 裁决:若完全等价,**不加**,
   在 skill 里教 `REDUCE`。**这一条可能砍掉整块需求。**
7. **空数组口径**:`std.collection` §5 已有「空数组得 `NULL`」,统计成员必须
   沿用,不能自己发明。

## 4. 验收门禁

```powershell
cd impl
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --all-targets
cargo deny --locked --all-features check
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --locked --no-deps
cargo check --locked -p wlwl-std --features real-ai
```

契约表扩 `tests/str_math_contract.rs`(现 156 条),**期望值取自
Python `decimal` / `fractions` / `statistics` 与 Java `BigDecimal` 的官方
语义,不是实现输出**:

| 组 | 用例 | 交叉核对源 |
|---|---|---|
| `DECIMAL` | 加 / 减 / 乘 / 除 / 比较 / 舍入,含**大数**(> `f64` 精度边界) | Python `decimal`(默认 28 位精度,须对齐) |
| `DECIMAL_DIV` | 除不尽 + `scale` 必填 + `b = 0` → `DomainError` | 同上 |
| `FRACTION` | 约分 / 四则 / 转十进制 | Python `fractions` |
| 统计 | `MEAN` / `MEDIAN` / `STDDEV` / `PERCENTILE`,含**空数组** / **单元素** / **偶数长度** | Python `statistics` |
| **空数组** | 每个统计成员对 `[]` → `NULL` | `std.collection` §5 口径 |
| 混装 | `[1, 2.5]` → `E0030` | 裁决 §3.4 |
| `APPROX_EQ` | `0.1 + 0.2 == 0.3` 的**经典浮点陷阱**必须过;`1e300` vs `1e300` 也过 | IEEE 754 |
| `IS_NAN` | `0.0 / 0.0` 路径的 `NaN` 判定(现有 `==` 绕法要钉住为**反模式**) | IEEE 754 |
| **回归** | 现有 32 个成员的 `str_math_contract` 用例**逐条不动** | 防顺手改坏 |
| 性能 | `MEAN` 10 万元素 ≥ 100 MB/s(**依赖 00**) | 新基准 |

## 5. 九处表落点

九处见 `docs/history/20261002.md` §5.0。**扩已有命名空间**:

| 落点 | 本份动作 |
|---|---|
| 1 规范成员表 | §7 补 ≈ 25 行 + **`DICT` 形状是公开契约**条款 + 统计空数组条款 |
| 2 附录 A | 跑 `gen-appendix-a`(`std.math` 32 → ≈ 57 成员) |
| 3 `SPEC.functions` | `math.rs` 追加 |
| 4 契约测试硬编码 | `str_math_contract` 成员数与点名清单 |
| 5 模块登记 | 无新模块(建议**新开 `math_ext.rs`** 以免 `math.rs` 超长 —— 注意 `src/` 文件数守卫) |
| 6 probe | ≥ 2 条(金额对账 / 统计),`EXPECTED_CASE_COUNT` 同步 |
| 7 附录 G | **不动** |
| 8 skill 指针 | `SKILL.md` + `reference.md` §9.5;**反模式**加「浮点用 `==` 比」 |
| 9 CHANGELOG | 成员面 +≈25 |

## 6. 风险与已知代价

- ⚠️ **裁决点 §3.6 可能砍掉整块统计需求**。若 `MEAN` 等价于 `REDUCE`,
  加成员就是冗余。**先算清楚再写**,不要用成员数衡量工作量。
- ⚠️ **`DICT` 承载任意精度有「用户能造非法状态」的固有缺陷**(手写一个
  `{sign: 0, digits: "abc"}`)。规范要写明「非法 `DICT` → `E0030`,不修复
  不报错」。这是可接受的代价,但必须**明说**,不能装作它是干净的。
- ⚠️ **统计成员的规模性能依赖 00**。在平方级成本上,`MEAN` 10 万元素会
  慢到不能用于生产 —— 那会让这一整块价值归零。**硬前置 00。**
- **明确不做**:复数 / 矩阵与线性代数 / 新内建数值类型 / 任意精度超越函数 /
  改现有 32 个成员的返回值。

## 7. 检查点

| 子项 | 内容 | 状态 | commit |
|---|---|---|---|
| W-01 | 数据形状裁决(§3.1)+ 业主立项确认(本份**不在** §14 清单内) | 未动工 | — |
| W-02 | **先算 `MEAN` 是否冗余**(§3.6),据此裁剪成员表 | 未动工 | — |
| W-03 | `DECIMAL` 族 + 契约(Python 交叉核对) | 未动工 | — |
| W-04 | `FRACTION` 族 + 契约 | 未动工 | — |
| W-05 | 统计族 + 空数组 / 混装契约 | 未动工 | — |
| W-06 | `APPROX_EQ` / `IS_NAN` / `IS_INF` / `SIGNIFICANT_DIGITS` + 反模式 | 未动工 | — |
| W-07 | 性能(依赖 00)/ 收口 | 未动工 | — |
