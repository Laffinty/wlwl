//! [v0.11.4 / v1.0 规范 §0.7] stdlib 规范附录 A:冻结快照 + 抄件。
//!
//! **附录 A 是 v1.0 规范的冻结快照,不是随实现更新的活镜像。** v1.0 规范仓
//! 的注册镜像册 §0.7 明写:「本册的单源真相是本规范自身」「表体**不得**
//! 由实现清单自动生成,**不得**为追平实现的当次改动而改写」。
//!
//! ⇒ 本仓**不再**拥有「跑一次生成器就把规范文本改写掉」这条路径。本文件
//! 产出的是**从实现派生的表体**,**只用于一致性确认**,**不是真相源**;
//! 真相源永远在规范仓的冻结快照里。
//!
//! 治理模型(两条守卫,见 `tests/stdlib_appendix_a_sync.rs`):
//! - **抄件忠实度**:合并册(`docs/wlwl-spec-reference.md`)附录 A 标记区
//!   与规范仓注册镜像册附录 A **逐字节相同**;
//! - **实现与快照一致**:实现的成员集合 == 快照的成员集合(不一致时走
//!   v1.0 一致性册 §5 的**差异登记**,由实现侧收敛,**不**由快照漂移)。
//!
//! 实现侧的成员清单(「成员是谁」这一点上实现说了算):
//! - R2 原生层成员来自 `wlwl_std` 的 `ModuleSpec` 绑定表;
//! - R1 语言层成员来自嵌入 `.wll` 源码的 `EXPORT([...])` 声明(经
//!   `wlwl_std::lang_exports` 提取;运行期加载由 AST 的
//!   `collect_exports` 权威提取,两者不一致会被锁测试抓出来)。
//!
//! **表体格式 = v1.0 规范的四列表**:`| 命名空间 | 成员 | 层 | v1.0 册 |`。
//! 第 4 列不再是 v0.11 的「引入版本」,而是**该命名空间的成员契约归
//! v1.0 标准库的哪一册**(核心 / 文本 / 系统 / 安全 / 官方包)。
//!
//! 「成员」列里跟在成员名后面的 `[stub]` 标记表示**该成员的返回值形状
//! 不在公共面内**(v1.0 一致性册 §4.4):实现侧**可以**把桩回包替换为
//! 真实实现,替换**不**构成破坏性变更;标记**不**改变「成员集合相等」
//! 这条规则 —— 它是集合内某个成员的附加属性。
//!
//! 层归属、「v1.0 册」与 stub 成员登记在 `NAMESPACE_META`(私有 const ——
//! 公开的模块级文档链不到私有项,故写代码而非 intra-doc 链接),以 v1.0
//! 规范的**注册镜像册**为权威(§0.5 记法、§0.8 命名空间到册的映射、
//! 附录 A 的 `[stub]` 标记位置)。
//!
//! 本地确认工具(在 `impl/` 目录下,**只读**,不写任何文件):
//! ```bash
//! cargo run --bin check-appendix-a
//! ```

use std::collections::BTreeSet;

use wlwl_std::StdBackend;

/// 一个命名空间在附录 A 里的登记项。
///
/// 字段与 v1.0 附录 A 的四列表一一对应:`path` + `layer` + `v1_book`
/// 撑起表里的三列,`stubs` 决定「成员」格里哪些成员名后面跟 `[stub]`。
///
/// ⚠ **这些字段是「本仓用来确认『实现与快照一致』的元数据,不是快照的
/// 来源」。** 快照只存在于 v1.0 规范仓的注册镜像册(§0.7 冻结快照);这里
/// 登记「层」「v1.0 册」「`[stub]`」三列,是为了让本仓能拿**实现派生的
/// 表体**与那份快照**逐字比对**。改这里的取值**不会**改写快照 —— 只会
/// 让比对报出差异,由人按 v1.0 一致性册 §5.4 走差异登记。
struct NsMeta {
    /// 实现侧的完整路径(`IMPORT` 用的路径)。
    path: &'static str,
    /// 实现层:R1 / R2 / 混合(与规范「层」列同口径)。
    layer: &'static str,
    /// 该命名空间的成员契约归 v1.0 标准库的哪一册(核心 / 文本 / 系统 /
    /// 安全 / 官方包)。
    ///
    /// ⚠ 取值以**快照**(规范仓注册镜像册 §0.8)为准;这里是**用于比对**的
    /// 登记项,不是快照的来源。
    v1_book: &'static str,
    /// 该命名空间里带 `[stub]` 标记的成员(返回值形状不在公共面内)。
    ///
    /// ⚠ 同上:标记位置以**快照**为准,这里是**用于比对**的登记项。
    stubs: &'static [&'static str],
}

/// 「本命名空间没有 stub 成员」的简写。
const NO_STUBS: &[&str] = &[];

/// 附录 A 的行登记:21 个命名空间。
///
/// ⚠ **这不是快照的来源。** 快照是 v1.0 规范仓注册镜像册里的**冻结
/// 快照**(§0.7);本表是**本仓用来确认「实现与快照一致」的元数据**。
///
/// 「v1.0 册」的取值以 v1.0 规范的**注册镜像册 §0.8**(命名空间到册的
/// 映射)为准;「层」的取值以同册附录 A 的表体为准;`stubs` 以同册附录 A
/// 的 `[stub]` 标记位置为准,并以本仓实现的桩事实为依据。
const NAMESPACE_META: &[NsMeta] = &[
    NsMeta {
        path: "wlwl:std.io",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "wlwl:std.fs",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "wlwl:std.json",
        layer: "R2",
        v1_book: "核心",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "wlwl:std.format",
        layer: "R2",
        v1_book: "核心",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M2] SHA256 / HMAC_SHA256;[v0.11.3 M6 / W-03] PBKDF2_ITER /
    // ARGON2ID;[v0.11.3 M6 / W-04] RANDOM_BYTES / RANDOM_HEX / TIMING_SAFE_EQ。
    // 编码与哈希在 v1.0 归**安全册**。
    NsMeta {
        path: "wlwl:std.encode",
        layer: "R2",
        v1_book: "安全",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M6 / addendum-01 A1] 时间能力第一片:两个纯读成员。
    // 全员 R2 —— 成员在 `wlwl-std` 内直读系统时钟(与 `RANDOM_BYTES` 直读
    // `getrandom` 同款:`std.time` 是时间的源,不经宿主)。B 片的虚拟时钟
    // 才需要宿主开口,见 ADR-0024 §6.3-2。
    NsMeta {
        path: "wlwl:std.time",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M6 / addendum-02] 正则(RE2 式线性时间,零第三方依赖)。
    NsMeta {
        path: "wlwl:std.regex",
        layer: "R2",
        v1_book: "文本",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M6 / addendum-08] 显式播种的 RNG。R2 全员(算法在
    // `rand.rs`,零第三方依赖:哈希 / SplitMix64 / xoshiro256++ 手写)。
    NsMeta {
        path: "wlwl:std.rand",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M6 / addendum-06] 进程环境与命令行参数。R2 全员。
    NsMeta {
        path: "wlwl:std.env",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M6 / addendum-06] 不经 shell 跑子进程。R2 全员。⚠ 它是**阻塞
    // 的**(与 `std.time::SLEEP` 同源):单线程协作式调度下,子进程运行期间
    // **不调度任何其他任务**(规范 §19)。
    NsMeta {
        path: "wlwl:std.process",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M1] Unicode 大小写与规范规范化(`NFC` / `NFD` / `NFC_QC`)。
    // R2 全员:规范化要走 Unicode 表,语言层做不到。
    NsMeta {
        path: "wlwl:std.text",
        layer: "R2",
        v1_book: "文本",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M1] 输出安全(旗舰特色功能)。**全员 R2**:转义 / 解码 / 净化
    // 都要处理文档级输入,而解释器侧字符串与数组构建超线性(D11-012 /
    // D12-009);归层基准见 v0.11.3 计划 W-08 与 baseline.txt M1 段。
    NsMeta {
        path: "wlwl:std.sanitize",
        layer: "R2",
        v1_book: "文本",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "wlwl:std.collection",
        // [v0.11 M5] 16 成员 R1 + `RANGE` 归 R2。层归属变更不算破坏性变更
        // (ADR-0021 §0.2)—— 变的是实现语言,不是成员面。依据见
        // baseline.txt 的 M5 段与偏差 D11-012。
        // [v0.11.3 M5 / L0-A-2] 再加 `MAP` / `FILTER` / `CHUNK` / `WINDOW`
        // 归 R2(四档基准实测每元素成本随尺寸上涨,平方级坐实)。
        // [v0.11.3 M5 / L0-A-3a] 再加 `ENUMERATE` / `ZIP` / `UNIQ` / `FLAT` /
        // `JOIN` 归 R2(同样形态、同样理由)。十个成员**全部留在 `EXPORT` 里**
        // (门面改名导出),故成员面与本行的成员名册都不变,只有「层」这一列变。
        // [v0.11.3 M5 / L0-A-3b] 再加 `GROUP_BY` / `DEDUP_BY` / `KEY_BY`(D 类,
        // 键是 `STR(k)` 渲染串 ⇒ 内部 `HashMap<String, usize>` 索引 + 末次物化,
        // `Value::Dict` 表示不动)。**`SORT` / `SORT_BY` 仍留 R1** —— 它们要换
        // `Vec::sort_by` 就会改**比较器调用次数**(R1 实测恰好 C(n,2)),那是
        // 可观察行为,待业主裁;见 `docs/history/addendum-00-r0-foundations.md` §7。
        // 十三个成员**全部留在 `EXPORT` 里**(门面改名导出),故成员面与本行的
        // 成员名册都不变,只有「层」这一列变。
        layer: "混合(R1 门面 + R2 `RANGE`/`MAP`/`FILTER`/`CHUNK`/`WINDOW`/`ENUMERATE`/`ZIP`/`UNIQ`/`FLAT`/`JOIN`/`GROUP_BY`/`DEDUP_BY`/`KEY_BY`)",
        v1_book: "核心",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "wlwl:std.str",
        // [v0.11.2 M2] `INDEX_OF` / `CONTAINS_SUB` 追加。与 `std.collection`
        // 同款口径:层与成员面不变(那句只对 M2 当次成立)。
        // [v0.12.0 / F-01 + S-1 甲案] `CAT` 归 **R2 原生内核** ⇒ 本模块成
        // **混合**层(R1 七员 + R2 `CAT`)。
        // ⚠ 这一列是**元数据**(登记「层」这个事实),**不是**成员实现:内核本体
        //   的落点在 `wlwl-std`(`kernels.rs` 的 R2 注册表 + `wl/std/str.wll`),
        //   **不在**本文件 ⇒ 本文件改这一格只让「快照说的」与「实现做的」对得上。
        layer: "混合(R1 `JOIN`/`SPLIT_LINES`/`CHAR_AT`/`COUNT`/`QUOTE`/`INDEX_OF`/`CONTAINS_SUB` + R2 `CAT`)",
        v1_book: "核心",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "wlwl:std.math",
        layer: "混合",
        v1_book: "核心",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "wlwl:std.test",
        layer: "混合",
        v1_book: "核心",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M8 / addendum-14 / ADR-0034] 两行从 `wlwl:std.*` 换到官方包路径。
    // ⚠ **成员一个未动**,且 `wlwl:std.ai` / `wlwl:std.agent` 现在返回 `None`。
    // ⚠ 形态上的差距(如实记下):包仍以 `wlwl-std` 的 `real-ai` feature 编译,
    // **没有独立的 crate 边界与分发产物** ⇒ `addendum-14` §6 要求记为**部分完成**。
    NsMeta {
        path: "ai",
        layer: "R2",
        v1_book: "官方包",
        stubs: NO_STUBS,
    },
    NsMeta {
        path: "agent",
        layer: "R2",
        v1_book: "官方包",
        // ⚠ `CALL_TOOL` 是**桩**:工具执行器未接线,回包固定为
        // `{ok: false, mocked: true, error_code: NULL, message, params}`
        // (实现见 `wlwl-std` 的 `agent.rs`;契约测试
        // `ai_agent_contract.rs` 的 `call_tool_mock_*` 四条钉住这个形状)。
        // ⇒ 它的**返回值形状不在公共面内**(v1.0 一致性册 §4.4):实现侧
        // 换成真实执行器**不**构成破坏性变更 ⇒ 标 `[stub]`。
        // ⚠ `ai` 包**不带**这个标记:它的推理成员要么走真实 HTTP(带
        // `real-ai` feature + 端点环境变量),要么走**确定性**回包;两种
        // 形态都是公共面内的既定行为,不在「形状可替换」的豁免里。
        stubs: &["CALL_TOOL"],
    },
    // [v0.11.3 M6 / addendum-07] Zstandard + DEFLATE 家族。R2 全员;本工作区
    // 第一次引入压缩依赖(`zstd` + `flate2`)。
    NsMeta {
        path: "wlwl:std.compress",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M6 / addendum-05 W-02..W-04] 网络。R2 全员,**零新增第三方
    // 依赖**(明文 HTTP/1.1 走 `std::net::TcpStream` + 系统解析器)。
    // ⚠ `HTTP_*` 是**阻塞原语**,且该裁决(`ADR-0028` D1)是**带条件冻结**的 ——
    // 冻结前提是「`ADR-0017` Step 3 真 state-machine 续跑仍未落地」。
    NsMeta {
        path: "wlwl:std.net",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.12.1 / W-26] `std.log` 结构化日志 + 运行时指标（需求清单 F-1 / F-2）。
    // ⚠ 本命名空间**尚未进公共面**（附录 A 要等规范仓 v1.0.4 落表）⇒
    // 成员集合对拍靠一致性册 §5.5「实现侧扩展台账」那一行兜住。
    NsMeta {
        path: "wlwl:std.log",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.12.1 / W-13] `std.sync` 原子共享容器（需求清单 B-4 的候选 (a)）。
    // ⚠ 本命名空间**尚未进公共面**（附录 A 要等规范仓 v1.0.4 落表）⇒
    // 成员集合对拍靠一致性册 §5.5「实现侧扩展台账」那一行兜住。
    NsMeta {
        path: "wlwl:std.sync",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.12.1 / W-15] `std.signal` OS 信号 + 优雅停机（需求清单 B-7）。
    // ⚠ 本命名空间**尚未进公共面**（附录 A 要等规范仓 v1.0.4 落表）⇒
    // 成员集合对拍靠一致性册 §5.5「实现侧扩展台账」那一行兜住。
    NsMeta {
        path: "wlwl:std.signal",
        layer: "R2",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.12.1 / W-16] `std.http` HTTP 请求解析 + 响应构造（C-1 / C-2）。
    // ⚠ 同上：**尚未进公共面** ⇒ 靠 §5.5 台账兜住。
    NsMeta {
        path: "wlwl:std.http",
        layer: "R1",
        v1_book: "系统",
        stubs: NO_STUBS,
    },
    // [v0.11.3 M7 / addendum-11 W-02] `RESULT` 组合子。
    // ⚠ 三条成员的 `ERR` 分支**不在成员体内执行** —— 语言规范 §8.2 规定实参为
    // `ERR` 时调用不求函数体(§8.3 注册表明文「不新增条目」),而「原样返还」恰好
    // 就是这三条在 `ERR` 上应有的行为。`OR_ELSE` / `ERR_MAP` / `ERR_OR_ELSE`
    // 因此**不可实现**,惯用法见规范 §22.3。依据见 `ADR-0029` D1 / D6。
    // [v0.11.3 M7 / addendum-11 **W-04**] `RESULT_ALL` / `RESULT_ANY` 归 R2:
    // 两者都要把 n 个载荷收成新数组,而 R1 累加链实测每翻一倍约 ×3
    // (n=4000 时 765 ms)。`NULL_OR` 只二选一、不建数组,留 R1。
    // [v0.12.0 / D-13] `TAKE_ERR` 是 **R2 原生内核**(见 `wlwl-std` 的
    // `src/result.rs` 的 `kernel_take_err`)⇒ 同样是 R2。
    // ⇒ 本模块是**混合**模块(4 R1 + 3 R2)。
    NsMeta {
        path: "wlwl:std.result",
        layer: "混合(R1 `RESULT_MAP`/`AND_THEN`/`FLATTEN`/`NULL_OR` + R2 `RESULT_ALL`/`RESULT_ANY`/`TAKE_ERR`)",
        v1_book: "核心",
        stubs: NO_STUBS,
    },
];

/// 一个命名空间的当前成员名册(实现真相)。
fn members(path: &str) -> Vec<String> {
    match wlwl_std::resolve(path) {
        Some(StdBackend::Lang(src)) => wlwl_std::lang_exports(src.source),
        Some(StdBackend::Native(spec)) => {
            // [v0.11 M2 / ADR-0022] 名录特判废止,成员直接来自 SPEC。
            spec.functions.iter().map(|(n, _)| n.to_string()).collect()
        }
        None => Vec::new(),
    }
}

/// 从实现派生附录 A 的表体(仅表格本身;标记与前置说明留在规范文件里)。
///
/// **v1.0 格式**:四列 `| 命名空间 | 成员 | 层 | v1.0 册 |`。
///
/// ⚠ **这是用于一致性确认的派生表体,不是真相源。** 真相源是 v1.0 规范仓
/// 注册镜像册里的**冻结快照**(§0.7)。本函数只回答「本仓实现现在长什么样」,
/// 用来跟快照比对;它**不**、也**不得**被用来改写快照或规范文本 ——
/// v0.11 那条「表体由生成器产出,手改无效;单源真相是实现」的规则**不再
/// 适用**(§0.7 规则 1)。
///
/// 「成员」格里跟在成员名后面的 `[stub]` 标记(注册镜像册附录 A 的写法:
/// `` `CALL_TOOL` `[stub]` ``)标的是**返回值形状不在公共面内**的成员 ——
/// 标记**不是**成员名本身,「成员集合相等」这条规则看的仍只是成员名。
pub fn generate_appendix_a_md() -> String {
    let mut out = String::new();
    out.push_str("| 命名空间 | 成员 | 层 | v1.0 册 |\n");
    out.push_str("|---|---|---|---|\n");
    for meta in NAMESPACE_META {
        // [v0.11.3 M8 / addendum-14 / ADR-0034] 显示名不再硬编码 `std.` 前缀。
        //
        // 旧写法 `format!("| \`std.{short}\` | ...")` 假定每个路径都在
        // `wlwl:std.` 下 —— 一时真的(全部命名空间都是 std),但**官方包**
        // (`ai` / `agent`)不在。不改的话它会把 `ai` 打印成 **`std.ai`** ——
        // 附录 A 在对一个已不存在的命名空间打字,而所有对账仍然自洽。
        let short = meta.path.trim_start_matches("wlwl:");
        let names = members(meta.path);
        let mut cells: Vec<String> = Vec::new();
        for name in &names {
            cells.push(format!("`{name}`"));
            if meta.stubs.contains(&name.as_str()) {
                cells.push("`[stub]`".to_string());
            }
        }
        let members = if cells.is_empty() {
            "—".to_string()
        } else {
            cells.join(" ")
        };
        out.push_str(&format!(
            "| `{short}` | {members} | {} | {} |\n",
            meta.layer, meta.v1_book
        ));
    }
    out
}

/// 实现侧每个命名空间的**成员集合**(实现真相)。
///
/// 与 `generate_appendix_a_md()` **同源**(同一个 `members()` 取法),但
/// **不经** `NAMESPACE_META`:命名空间名单取 `wlwl_std::namespace_paths()`
/// (实现真相,与 META 无共源),成员名单取 `wlwl_std::resolve` 的绑定表 /
/// `EXPORT` 声明。
///
/// 用途:**一致性确认** —— 「实现的成员集合 == 冻结快照的成员集合」
/// (v1.0 规范 §0.7 规则 2:不一致时走一致性册 §5.4 的差异登记,由实现侧
/// 收敛;**不**由快照跟随实现漂移)。
///
/// 返回的命名空间名是**表体里的显示名**(去掉 `wlwl:` 前缀),以便与快照
/// 的表体逐行对齐。
pub fn implementation_member_sets() -> Vec<(String, BTreeSet<String>)> {
    wlwl_std::namespace_paths()
        .into_iter()
        .map(|path| {
            let namespace = path.trim_start_matches("wlwl:").to_string();
            let members = members(path).into_iter().collect::<BTreeSet<String>>();
            (namespace, members)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_namespace_resolves_with_members() {
        for meta in NAMESPACE_META {
            let path = meta.path;
            let names = members(path);
            assert!(
                !names.is_empty(),
                "{path} resolves but its member list came out empty — \
                 the mirror would silently hide a whole namespace"
            );
        }
    }

    #[test]
    fn layer_meta_covers_exactly_the_resolvable_namespaces() {
        for meta in NAMESPACE_META {
            let path = meta.path;
            assert!(
                wlwl_std::resolve(path).is_some(),
                "{path} is listed in NAMESPACE_META but does not resolve"
            );
        }
        // 反向:**实现真相到的命名空间**而 META 缺登记 = 附录 A 撒谎。
        //
        // ⚠ 这一半边原先写的是**一份写死的 10 条路径清单**,而那份清单从没跟着
        // 实现走 —— `std.rand` / `std.env` / `std.process` /
        // `std.compress` 落地时它就过时了,因此附录 A 镜像整整消了四行 / 18 个成员,而
        // `stdlib_appendix_a_sync` 照旧全绿 ——它对账的两侧(镜像 / 生成器输出)读的是
        // **同一份 META**,同源对账只能防「实现动了而镜像没动」,防不住「登记表本身漏了」。
        // ↳ 现在直接向 `wlwl_std::namespace_paths()`(实现真相,与 META **无共源**)要求,不再写死清单。
        let impls = wlwl_std::namespace_paths();
        for path in &impls {
            assert!(
                NAMESPACE_META.iter().any(|m| m.path == *path),
                "{path} resolves in the implementation but is missing from NAMESPACE_META — \
                 appendix A would silently omit the whole namespace"
            );
        }
        assert_eq!(
            NAMESPACE_META.len(),
            impls.len(),
            "NAMESPACE_META has {} row(s) but the implementation has {} namespace(s) — \
             a stale row or a missing registration",
            NAMESPACE_META.len(),
            impls.len()
        );
    }

    /// 「v1.0 册」列只能取 v1.0 标准的五个册名。
    ///
    /// 为什么值得守:这一列是**给人读的路由**(读者照它去找成员契约在哪一册),
    /// 写错一个字的代价是「照着导航找不到规范」,而机器对账**看不出来** ——
    /// 它只比字节,不判断这个字节是不是册名。
    #[test]
    fn the_v1_book_column_only_uses_the_five_book_names() {
        const BOOKS: &[&str] = &["核心", "文本", "系统", "安全", "官方包"];
        for meta in NAMESPACE_META {
            assert!(
                BOOKS.contains(&meta.v1_book),
                "{}: `{}` is not one of the five v1.0 stdlib books {BOOKS:?}",
                meta.path,
                meta.v1_book
            );
        }
    }

    /// 每个 `[stub]` 标记都必须指向**真实存在的成员**。
    ///
    /// 为什么值得守:`stubs` 是**手写**的登记项,而成员名册是**实现**给的 ——
    /// 两者无共源。写错一个名字(成员改名、成员删除、登记项拼错)时,表里
    /// 会出现一个**指向虚空**的 `[stub]`:它既不解释任何东西,也不会被
    /// 任何别的守卫抓到(机器只比字节)。⇒ 拼错 = 静默撒谎。
    #[test]
    fn every_stub_marker_names_a_real_member() {
        for meta in NAMESPACE_META {
            let names = members(meta.path);
            for stub in meta.stubs {
                assert!(
                    names.iter().any(|n| n == stub),
                    "{}: `[stub]` is registered for `{stub}`, which is not a member of \
                     that namespace ({names:?}) — the marker points at nothing",
                    meta.path
                );
            }
        }
    }

    /// `[stub]` 标记**必须**紧跟它标注的成员名后面。
    ///
    /// 写法取自 v1.0 注册镜像册附录 A:`` `CALL_TOOL` `[stub]` `` ——
    /// 标记是**独立的一个反引号记号**,不是成员名的一部分。语义是
    /// 「**它前面那个**成员」被标注(规范原话:「成员名后的 `[stub]` 标记」)。
    ///
    /// 为什么值得守:标记的位置就是它的**语义**。写成 `` `[stub]` `CALL_TOOL` ``
    /// 会把标记挂到**上一个**成员(或挂空)身上,而表体的字节比对**照样通过**
    /// —— 只有这条守卫能抓住。
    #[test]
    fn the_stub_marker_follows_its_member_name() {
        /// 一行表体数据行 → (命名空间, 成员格);表头 / 分隔行 / 非表行 → `None`。
        fn row_cells(line: &str) -> Option<(String, String)> {
            let line = line.trim();
            if !line.starts_with('|') {
                return None;
            }
            let cells: Vec<&str> = line.trim_matches('|').split('|').collect();
            if cells.len() != 4 {
                return None;
            }
            let ns = cells[0].trim();
            if !ns.starts_with('`') {
                return None;
            }
            Some((
                ns.trim_matches('`').to_string(),
                cells[1].trim().to_string(),
            ))
        }

        let md = generate_appendix_a_md();
        // 正向:每个登记的 stub 成员名后面**紧跟**标记。
        for meta in NAMESPACE_META {
            for stub in meta.stubs {
                let needle = format!("`{stub}` `[stub]`");
                assert!(
                    md.contains(&needle),
                    "{}: the appendix A body has no `{needle}` — the `[stub]` marker \
                     must follow the member name it annotates",
                    meta.path
                );
            }
        }
        // 反向:表里出现的**每一个** `[stub]` 记号,前面必须紧跟一个**登记为
        // stub 的成员名** —— 标记挂到别的成员身上 = 挂错成员。
        for line in md.lines() {
            let Some((ns, cell)) = row_cells(line) else {
                continue;
            };
            let toks: Vec<&str> = cell.split_whitespace().collect();
            for (i, tok) in toks.iter().enumerate() {
                if *tok != "`[stub]`" {
                    continue;
                }
                let Some(prev) = i.checked_sub(1).map(|k| toks[k]) else {
                    panic!("a `[stub]` marker with no member in front of it: {line}");
                };
                assert_ne!(prev, "`[stub]`", "two `[stub]` markers in a row: {line}");
                let member = prev.trim_matches('`');
                let registered = NAMESPACE_META
                    .iter()
                    .find(|m| m.path.trim_start_matches("wlwl:") == ns)
                    .map(|m| m.stubs.contains(&member))
                    .unwrap_or(false);
                assert!(
                    registered,
                    "`[stub]` in row `{ns}` follows `{member}`, which is not registered \
                     as a stub member of that namespace: {line}"
                );
            }
        }
    }

    // ---- R1 内核注入(M3-0)------------------------------------------------
    //
    // 「注入面不外泄」是 ADR-0021:90「混合模块以门面为对外契约层」的机械
    // 形式。断言各封一种泄漏:kernel 名字**跑进 EXPORT**(于是出现在附录 A
    // 与 IMPORT 面上)、kernel 名字**撞上全局内建**(门面里同名的 `LET` 会
    // 触发 W0030 遮蔽警告,语义上也多了一个不需要的入口)、以及共享表里躺着
    // **没人用的** kernel(死代码,且下次有人读它会以为它在生效)。
    //
    // **`kernels::KERNELS` 不是全部 kernel 的单源。** M3-0 写下这条守卫时
    // 断言的是「注入 ⊆ KERNELS」,M3-3 立刻被证伪:`std.test` 的三个 kernel
    // (`_TEST` / `_EXPECT_ERR` / `_RUN_TESTS`)住在**它自己的 R2 模块文件**
    // `test_native.rs` 里,不在 `kernels.rs`。这是对的 —— 内核代码该跟它服务
    // 的模块放在一起(规范 §0.2 说 R2 成员在 `wlwl-std`,没说在哪个文件)。
    // 于是 `KERNELS` 的真实身份是「**模块无关的共享** kernel 表」,模块
    // 还可以声明自己的。守卫因此改成要求**反方向**:KERNELS ⊆ 注入 ——
    // 那一条才是能防死代码的。

    /// `KERNELS` 里登记的共享 kernel 名字。
    fn shared_kernel_names() -> Vec<&'static str> {
        wlwl_std::kernels::KERNELS.iter().map(|(n, _)| *n).collect()
    }

    /// 全部已登记的 R2 kernel 名字(含各模块自带的)。
    fn all_kernel_names() -> Vec<&'static str> {
        wlwl_std::LANG_SOURCES
            .iter()
            .flat_map(|s| s.kernels.iter().map(|(n, _)| *n))
            .chain(shared_kernel_names())
            .collect()
    }

    #[test]
    fn r1_kernels_never_leak_to_the_export_surface() {
        for src in wlwl_std::LANG_SOURCES {
            let names = wlwl_std::lang_exports(src.source);
            let exports: std::collections::HashSet<&str> =
                names.iter().map(String::as_str).collect();
            for (name, _) in src.kernels {
                assert!(
                    name.starts_with('_'),
                    "{} injects `{name}` without the `_` prefix — it would read \
                     like a public member in the facade source",
                    src.path
                );
                assert!(
                    name.chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                    "{} injects `{name}` which is not UPPER_SNAKE",
                    src.path
                );
                assert!(
                    !exports.contains(*name),
                    "kernel `{name}` is EXPORTed by {} — it must stay private to the \
                     facade, or it lands in appendix A and on the IMPORT surface",
                    src.path
                );
                assert!(
                    !crate::registry::resolved_builtin_names().contains(name),
                    "R2 kernel `{name}` collides with a global builtin — the facade's \
                     `LET({name}, ...)` would raise W0030 (shadowing) and the kernel \
                     would shadow a language-surface name"
                );
            }
        }
    }

    /// 共享表里的每个 kernel 都得有人用,否则是死代码。
    #[test]
    fn every_shared_kernel_is_actually_injected() {
        for (name, _) in wlwl_std::kernels::KERNELS {
            let used = wlwl_std::LANG_SOURCES
                .iter()
                .any(|s| s.kernels.iter().any(|(n, _)| n == name));
            assert!(
                used,
                "`{name}` is listed in `kernels::KERNELS` but no module injects it — \
                 delete it or wire it up. (That table holds only the *shared* kernels; \
                 module-local ones live in their own R2 file.)"
            );
        }
        assert!(
            !shared_kernel_names().is_empty(),
            "`kernels::KERNELS` is empty — the shared-kernel table was emptied out"
        );
    }

    /// [D12-002] 注入表里的每个 kernel,都必须在门面的 `.wll` 源码里真的
    /// 被用到 —— 否则是死注入。
    ///
    /// 为什么需要这条:上一条 `every_shared_kernel_is_actually_injected` 只锁
    /// **一个方向**(`KERNELS` ⊆ 注入)。而 `CHANGELOG.md` v0.11.1 声称修死注入
    /// 时「并补反向守卫」, 声称的正是本条。反方向一旦没人守, 有人把死内核
    /// **同时**加回 `KERNELS` 与注入表, 全部门禁照绿 —— 这正是 v0.11.1 里
    /// `_DIAG_E0038` 死注入当初能活下来的形状。
    #[test]
    fn every_injected_kernel_is_used_in_its_module_source() {
        for src in wlwl_std::LANG_SOURCES {
            let missing =
                kernels_without_a_use_site(src.source, src.kernels.iter().map(|(n, _)| *n));
            assert!(
                missing.is_empty(),
                "{} injects {} kernel(s) that the facade source never mentions: {missing:?} — \
                 a dead injection. Remove it from the injection table, or wire it up.",
                src.path,
                missing.len()
            );
        }
    }

    /// [D12-002] 上面那条的**负向自检**:守卫写反、或者匹配逻辑退化成
    /// 「什么都找得到」时,上面会变成恒真断言 —— 这正是 D11-006 踩过的坑
    /// (当时有个 `names_match_catalog` 恒等于左右两边, 从未锁住任何东西)。
    #[test]
    fn the_dead_injection_guard_actually_rejects_one() {
        let src = "LET(x, 0), SET(x, 1)";

        // 真的没被用到 ⇒ 必须报出来。
        assert_eq!(
            kernels_without_a_use_site(src, std::iter::once("_NOT_CALLED")),
            vec!["_NOT_CALLED".to_string()],
            "a kernel with no use site must be reported as dead"
        );
        // 正向对照:出现过的名字不该被报出来(顺带证明匹配不是「一律报」)。
        assert!(
            kernels_without_a_use_site(src, std::iter::once("x")).is_empty(),
            "a name that does appear must not be reported"
        );
        // 注释与字符串字面量里的名字**不算**使用点。
        let prose = "// _KIND(x) 见文件头\nLET(s, \"_KIND is documented here\")";
        assert_eq!(
            kernels_without_a_use_site(prose, std::iter::once("_KIND")),
            vec!["_KIND".to_string()],
            "comment / string mentions must not count as use sites — collection.wll's header \
             lists every injected kernel by name, and records the *deleted* `_DIAG_E0038` in \
             prose; a naive contains() would be fooled by the very text explaining the deletion"
        );
        // 词边界:名字是别的标识符的一部分时不匹配。
        assert_eq!(
            kernels_without_a_use_site("LET(_KIND_EXTRA, 1)", std::iter::once("_KIND")),
            vec!["_KIND".to_string()],
            "`_KIND` must not be found inside `_KIND_EXTRA`"
        );
    }

    /// 把 `.wll` 源码里的**注释与字符串字面量**抹成空格,只留可执行代码。
    ///
    /// 必须抹的理由见 `the_dead_injection_guard_actually_rejects_one`:门面文件
    /// 的头注释会**逐条列出本模块注入的每个 kernel 名**,其中还记着已被删除的
    /// `_DIAG_E0038`。不抹的话,那句解释删除原因的散文本身就会成为一个
    /// 「现存的调用点」。
    fn strip_comments_and_strings(src: &str) -> String {
        let chars: Vec<char> = src.chars().collect();
        let mut out = String::with_capacity(src.len());
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                    while i < chars.len() && chars[i] != '\n' {
                        out.push(' ');
                        i += 1;
                    }
                }
                '"' => {
                    out.push(' ');
                    i += 1;
                    while i < chars.len() {
                        match chars[i] {
                            // `\"` 是字面量里的一个引号,两个字符都吞掉。
                            '\\' if i + 1 < chars.len() => {
                                out.push(' ');
                                out.push(' ');
                                i += 2;
                            }
                            '"' => {
                                out.push(' ');
                                i += 1;
                                break;
                            }
                            // 未闭合的串:退到行尾,别把后面整份文件都吃掉。
                            '\n' => {
                                out.push(' ');
                                i += 1;
                                break;
                            }
                            _ => {
                                out.push(' ');
                                i += 1;
                            }
                        }
                    }
                }
                c => {
                    out.push(c);
                    i += 1;
                }
            }
        }
        out
    }

    fn is_identifier_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// 返回「注入了但源码里没有任何使用点」的 kernel 名。
    ///
    /// 匹配的是**词边界的出现**而不是 `name(`:门面有两种合法用法,`_RANGE`
    /// 与 `_EXPECT_ERR` 都是**裸标识符改名导出**(`LET(RANGE, _RANGE);`),
    /// 要求括号会把它们误判成死注入。
    fn kernels_without_a_use_site<'a>(
        source: &str,
        kernel_names: impl Iterator<Item = &'a str>,
    ) -> Vec<String> {
        let code = strip_comments_and_strings(source);
        let mut missing = Vec::new();
        for name in kernel_names {
            let used = code.match_indices(name).any(|(i, _)| {
                let before_ok =
                    i == 0 || !is_identifier_char(code[..i].chars().next_back().unwrap());
                let after = i + name.len();
                let after_ok = after >= code.len()
                    || !is_identifier_char(code[after..].chars().next().unwrap());
                before_ok && after_ok
            });
            if !used {
                missing.push(name.to_string());
            }
        }
        missing
    }

    /// 收集器坏掉时的反向守卫:「扫不到任何 kernel 就转绿」会让上面两条
    /// 断言退化成检查零件事的空断言(它们都以 `LANG_SOURCES` 的迭代结果
    /// 或 `KERNELS` 自身为输入)。
    #[test]
    fn kernel_collection_actually_finds_something() {
        let all = all_kernel_names();
        assert!(
            all.len() >= 7,
            "found only {} kernel(s): {all:?} — the M3-0 injection path looks dead \
             and its guards would pass vacuously",
            all.len()
        );
    }

    /// [D11-019] 行扫描器与 AST 权威口径必须看到**同一组**名字。
    ///
    /// 为什么需要这条:附录 A 镜像生成器、三份契约测试的「实际」侧、
    /// 以及 kernel 不外泄守卫,全都走 `wlwl_std::lang_exports` 这一个扫描器
    /// —— 它们彼此对账时是**同源**的,所以扫描器自身的偏差会自洽地全绿。
    /// 运行期真正生效的是 AST 的 `collect_exports`,故拿它当基准。
    ///
    /// 比较集合而非序列:顺序由「成员面 == 规范表格行序」那几条锁负责,
    /// 这里只管**成员是谁**。
    #[test]
    fn the_line_scanner_agrees_with_the_ast_export_walker() {
        for src in wlwl_std::LANG_SOURCES {
            let ast = wlwl_parser::parse(src.source, "std.wll")
                .unwrap_or_else(|e| panic!("{} must parse as a module: {e}", src.path));
            let from_ast: std::collections::HashSet<String> = crate::collect_exports(&ast);
            let from_scan: std::collections::HashSet<String> =
                wlwl_std::lang_exports(src.source).into_iter().collect();
            assert_eq!(
                from_scan, from_ast,
                "{}: the line scanner and the AST walker disagree — one of them is \
                 wrong, and the appendix-A mirror follows the scanner",
                src.path
            );
            assert!(
                !from_ast.is_empty(),
                "{}: no EXPORT found at all — this guard would pass vacuously",
                src.path
            );
        }
    }
}
