//! 契约测试共用的**助手**。
//!
//! ## 一、临时目录(唯一性)
//!
//! ⚠🔴 **[D13-078] 临时目录名不能只由时钟决定。**
//!
//! 旧写法是 `format!("wlwl_xxx_{nanos}")`(`nanos` 取自 `SystemTime::now()`)。
//! 那**不是**唯一性保证,只是「大概不撞」—— 而「大概」在 CI 上是**不够**的:
//!
//! - `SystemTime::now()` 的**实际**分辨粒度远小于它的单位。macOS 的
//!   `clock_gettime(CLOCK_REALTIME)` 是 **1 微秒**(`clock_getres` 给出
//!   `tv_nsec = 1000`);Linux 是纳秒,Windows 约 100ns(实测 1000 次采样只有
//!   355 个不同的值、645 次相邻重复)。
//! - 契约测试是**并行**跑的(libtest 默认并发度 = CPU 数)⇒ **头一波**有 2~3 个
//!   线程**几乎同时**开始 ⇒ 它们的第一次取名落在**几微秒**内。
//!
//! 实测(同刻取名,按平台粒度量化,每档 300 轮):
//!
//! | 线程数 | Windows ≈100ns | **macOS 1µs** | Linux 1ns |
//! |---|---|---|---|
//! | 3(= runner 核数,头一波) | 2 轮(0.7%) | **53 轮(17.7%)** | 0 轮 |
//! | 13(= `fs_contract.rs` 的 IO 用例数) | 38 轮(12.7%) | **230 轮(76.7%)** | 43 轮 |
//!
//! ⇒ **只有 macOS 会常撞** —— 这与「CI 上 macOS 红、ubuntu/windows 绿」完全吻合。
//!
//! 撞名的后果不是「目录里多几个文件」,而是**互相 `remove_dir_all`**:一个用例
//! 起 scratch 时会把**另一个正在跑**的用例的目录整个删掉 ⇒ 那个用例下一步的
//! `COPY`/`MOVE` 找不到源文件 ⇒ 随机红(实测复刻:同名 scratch 起来后,前一个用例
//! 刚写下的 `a.bin` **消失**)。CI 上的签名正是
//! `fs_contract::copy_and_move_are_for_files` 偶发失败。
//!
//! ⇒ 名字必须**构造上唯一**,不依赖时钟分辨率:**进程 id**(跨进程)+
//! **原子计数**(同进程内并发)+ **纳秒**(跨运行残留)。
//!
//! ⚠ 同一个坑本仓库在 CLI 侧**已经踩过一次并修过** ——
//! `wlwl-cli/tests/lexer_macro_coverage.rs` 里那段注释写得很直白:
//! 「临时目录**必须按名字分开**……共用一个目录时一边刚写下的 `main.wll`
//! 会被另一边读走」。那次只修了 CLI 侧,`wlwl-eval` 侧这 22 处是**残留副本**。
//!
//! ## 二、标准库规范的「成员表」子节
//!
//! v1.0 的标准库册把成员表放进**子节**(`### 21.1 成员表` / `### 11.2 成员表`),
//! 不再平铺在章标题之下 ⇒ 抽取范围必须落在那个子节上:
//! 截到第一个 `### ` 会把成员表整个切掉(抽到 0 个成员),扫整章则会把同章的
//! 参数表 / 测试向量表一起抽进来。
//!
//! ⚠ **子节号逐章不同**(21.1 vs 11.2 vs 15.1)⇒ 按**标题文字**(`成员表`)
//! 定位,不按编号 —— 按编号定位的人改一次章就得再抄一次查找逻辑。
//!
//! ⚠ **这一段原先在 `net_contract.rs` / `encode_contract.rs` / `time_contract.rs`
//! 里各抄了一份**(`time_contract.rs` 那份还把 `### 15.1` 的**编号**写死了)。
//! 三份抄件的行为必须一致 ⇒ 收在这里一处。

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// 进程内自增的取名序号。
///
/// ⚠ **这一根「保险丝」是唯一性的真正来源**:时钟粒度再粗(1 微秒)、再多线程
/// 同时取名,同一进程内的第 N 次与第 N+1 次也**必然**不同。
static SEQ: AtomicU64 = AtomicU64::new(0);

/// 当前的墙钟纳秒。
pub fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// 一个**构造上唯一**的名字。
///
/// ⚠ `nanos` 由调用方传入 —— 这样契约能**把时钟冻住**来验证「名字**不是**时钟的
/// 纯函数」这条不变量(见 `temp_scratch_contract.rs`)。若把时钟写死在这里,
/// 这条不变量就**没法验**了。
///
/// 名字形状:`<prefix>_<pid>_<seq>_<nanos>` —— 三个成分各管一段:
/// `pid` 管跨进程、`seq` 管同进程并发、`nanos` 管跨运行残留。
pub fn unique_name(prefix: &str, nanos: u128) -> String {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{}_{seq}_{nanos}", std::process::id())
}

/// 一个**构造上唯一**的临时目录(已创建好,可直接往里写文件)。
pub fn scratch(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(unique_name(prefix, now_nanos()));
    // 残留清理:同一 pid + 同一 seq 理论上不会重现,但 pid 会被复用 ⇒ 留着这一步
    // 才能保证「同名目录一定是空的」。
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

// ── 合并规范:「某一章」的正文切片 ─────────────────────────────────────
//
// ⚠ **这一段的行为是契约**:改了它,所有「章作用域」的契约判据抽到的内容就会变。
//
// ## 两种结束口径,**不等价,别混用**
//
// - `chapter_body(text, title)` —— 结束于**下一个 `## `**。
// - `chapter_body_until(text, title, next_title)` —— 结束于**指定的下一个章标题**。
//
// 差别在**规范新增中间章**时:
// - `chapter_body` 会**静默截断**(新章被排除,测试照绿 —— 成员表对拍看不出来);
// - `chapter_body_until` 会把新章**包进来** ⇒ 成员表对拍**失败 ⇒ 变红**
//   ⇒ 「设计变了就一起改测试」那道闸。
//
// ⇒ **凡是原来用具体章号当结束锚点的,收口后必须继续用 `chapter_body_until`**,
// 不要「统一」成通用形式 —— 那会把一道闸悄悄拆掉。
//
// ## 为什么是「找不到就 panic」
//
// 「章标题找不到」与「这一章没有成员」是两件必须分开的事:前者是**结构变了**
// (章改名 / 被删 / 册别前缀改了),后者是规范事实。安静返回空会让「结构变了」
// 变成**静默**漂移,而集合相等型断言在空集上会**假绿**(`[] == []`)。
//
// ## 为什么**不**顺手把各文件的 `spec_text()` 也换成这里的
//
// 仓级守卫 `impl/crates/wlwl-cli/tests/spec_needle_scope_guard.rs` 靠**源码形态**
// 识别「整册绑定」:它认的是「`let x = <读整本>`,且 `x` 直接当 `.contains()` 的
// 接收者」。各测试文件里**本地定义的** `spec_text()` 正是它认得的形态之一 ⇒
// 把它换成本模块的函数,守卫就**认不出**了 ⇒ 守卫静默减弱(漏报)。
// ⇒ **各文件的 `spec_path()` / `spec_text()` 保持原样**,本模块只提供切片助手。

/// 取合并规范里**某一章**的正文,结束于**下一个 `## `**。
///
/// `title` 是章标题的**前缀**(通常到章号后的空格为止),如
/// `"## 标准库规范 · 17 "`。它同时充当**定位锚点**与**报错文案**。
///
/// **章标题找不到 ⇒ panic**(带这一册实际有的章标题清单)—— 安静返回空会让
/// 「章改名 / 被删 / 册别前缀改了」变成**静默**漂移。
///
/// **下一个 `## ` 找不到 ⇒ 也 panic**:那说明这一章是全文**最后一个**二级
/// 章节 —— 而规范册的最后一节是「代理工作说明」,**不是**任何标准库章 ⇒
/// 走到这里只可能是结构变了(某章被删、或册别前缀改了)⇒ 必须响,不能安静
/// 收到末尾(那会让「取到的正文多了一截」变成静默漂移)。
pub fn chapter_body(text: &str, title: &str) -> String {
    let start = text.find(title).unwrap_or_else(|| {
        panic!(
            "{title} exists in the merged spec — {}",
            chapter_inventory(text)
        )
    });
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .unwrap_or_else(|| {
            panic!(
                "a `## ` heading follows {title} — 这一章成了全文最后一个二级章节;\
                 规范册的最后一节是「代理工作说明」,不是标准库章 ⇒ 章结构变了\
                 (被删 / 改名 / 换了册别前缀)"
            )
        });
    text[start..end].to_string()
}

/// 取合并规范里**某一章**的正文,结束于**指定的下一个章标题**。
///
/// ⚠ 与 [`chapter_body`] 的差别见模块级说明:**规范新增中间章时,本函数会把
/// 新章包进来 ⇒ 成员表对拍失败 ⇒ 变红**;而 `chapter_body` 会静默截断。
/// ⇒ 原本用具体章号当结束锚点的调用点,必须用本函数。
///
/// **`next_title` 不存在 ⇒ panic** —— 那正是「规范册的章结构变了」的信号
/// (例如被删、或换了册别前缀),必须响。
pub fn chapter_body_until(text: &str, title: &str, next_title: &str) -> String {
    let start = text.find(title).unwrap_or_else(|| {
        panic!(
            "{title} exists in the merged spec — {}",
            chapter_inventory(text)
        )
    });
    let end = text[start..]
        .find(next_title)
        .map(|i| start + i)
        .unwrap_or_else(|| {
            panic!(
                "{next_title} follows {title} — 规范册的章结构变了(被删 / 改名 / 换了册别前缀);\
                 ⚠ 收窄范围时**不要**改成「下一个 `## `」:那会静默截断,把「设计变了」这道闸拆掉"
            )
        });
    text[start..end].to_string()
}

/// 列出合并册里实际有的章标题(报错时用,让人一眼看出是「章被删」还是「前缀变了」)。
fn chapter_inventory(text: &str) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if !seen.contains(&rest) {
                seen.push(rest);
            }
        }
    }
    if seen.is_empty() {
        "(一个二级标题都没有)".to_string()
    } else {
        seen.join(" / ")
    }
}

/// 共用助手自身的**自检** —— 把「找不到就报错」「两种结束口径不等价」这两条
/// 行为钉住。
///
/// ⚠ 意义在于**防止将来被改成安静返回空**:那种改法不会让任何一张契约表立刻
/// 变红(集合相等型断言在空集上会**假绿**),而是把「章结构变了」变成**静默**
/// 漂移 ⇒ 守卫必须在**今天**就钉住「它会响」。
#[cfg(test)]
mod chapter_body_self_check {
    use super::{chapter_body, chapter_body_until};
    use std::any::Any;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// 一段**假的**合并册片段:两章 + 一段散文。
    const FAKE: &str = "\
## 标准库规范 · 16 `std.regex` —— 线性时间正则

### 16.1 成员表

| 签名 | 语义 |
|---|---|
| `RE(x)` | 编译 |

## 标准库规范 · 17 `std.rand` —— 显式播种的随机数

### 17.1 成员表

| 签名 | 语义 |
|---|---|
| `SEED(x)` | 播种 |
";

    fn panic_message(err: &Box<dyn Any + Send>) -> String {
        if let Some(s) = err.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = err.downcast_ref::<&str>() {
            (*s).to_string()
        } else {
            "(panic 载荷不是字符串)".to_string()
        }
    }

    #[test]
    fn chapter_body_stops_at_the_next_h2() {
        let b = chapter_body(FAKE, "## 标准库规范 · 16 ");
        assert!(b.contains("`RE(x)`"));
        assert!(
            !b.contains("`SEED(x)`"),
            "chapter_body 必须停在下一个 `## ` 之前,不能把下一章包进来"
        );
    }

    #[test]
    fn chapter_body_until_stops_at_the_named_chapter() {
        let b = chapter_body_until(FAKE, "## 标准库规范 · 16 ", "## 标准库规范 · 17 ");
        assert!(b.contains("`RE(x)`"));
        assert!(!b.contains("`SEED(x)`"));
    }

    /// ⚠ **两种口径的差别就是这道闸**:若 §16 与 §17 之间**插入**一章,
    /// `chapter_body` 会**静默截断**(新章被排除),`chapter_body_until`
    /// 会把它**包进来** ⇒ 调用方的成员表对拍会失败 ⇒ 变红。
    #[test]
    fn the_two_end_modes_differ_when_a_chapter_is_inserted() {
        const WITH_MIDDLE: &str = "\
## 标准库规范 · 16 `std.regex` —— 线性时间正则

### 16.1 成员表

| 签名 | 语义 |
|---|---|
| `RE(x)` | 编译 |

## 标准库规范 · 16.5 `std.newcomer` —— 插在中间的新章

| 签名 | 语义 |
|---|---|
| `NEWCOMER(x)` | 新成员 |

## 标准库规范 · 17 `std.rand` —— 显式播种的随机数
";
        let generic = chapter_body(WITH_MIDDLE, "## 标准库规范 · 16 ");
        assert!(
            !generic.contains("`NEWCOMER(x)`"),
            "前提:chapter_body 会在下一个 `## ` 处截断"
        );
        let specific =
            chapter_body_until(WITH_MIDDLE, "## 标准库规范 · 16 ", "## 标准库规范 · 17 ");
        assert!(
            specific.contains("`NEWCOMER(x)`"),
            "chapter_body_until 必须把插入的新章**包进来** —— \
             调用方的成员表对拍会因此失败 ⇒ 变红 ⇒ 「设计变了就一起改测试」那道闸"
        );
    }

    #[test]
    fn chapter_body_panics_when_the_chapter_is_missing() {
        let err = catch_unwind(AssertUnwindSafe(|| {
            chapter_body(FAKE, "## 标准库规范 · 99 ")
        }))
        .expect_err("章标题找不到必须 panic —— 不许安静返回空");
        let msg = panic_message(&err);
        assert!(
            msg.contains("99") && msg.contains("标准库规范 · 16"),
            "panic 消息必须说清缺哪一章、实际有哪些:{msg}"
        );
    }

    /// ⚠ 结束锚点也是**严格**的:这一章成了全文最后一个二级章节 ⇒ panic。
    ///
    /// 安静收到末尾会让「取到的正文多了一截」变成**静默**漂移 —— 而成员表
    /// 对拍会因此抽进别的章的成员却仍然绿。
    #[test]
    fn chapter_body_panics_when_there_is_no_following_h2() {
        const LAST: &str = "\
## 标准库规范 · 98 `std.example` —— 全文最后一章

| 签名 | 语义 |
|---|---|
| `ONLY(x)` | 唯一一个成员 |
";
        let err = catch_unwind(AssertUnwindSafe(|| {
            chapter_body(LAST, "## 标准库规范 · 98 ")
        }))
        .expect_err("没有下一个 `## ` 时必须 panic —— 不许安静收到末尾");
        let msg = panic_message(&err);
        assert!(
            msg.contains("98"),
            "panic 消息必须说清是哪一章成了最后一章:{msg}"
        );
    }

    #[test]
    fn chapter_body_until_panics_when_the_named_next_chapter_is_missing() {
        let err = catch_unwind(AssertUnwindSafe(|| {
            chapter_body_until(FAKE, "## 标准库规范 · 16 ", "## 标准库规范 · 99 ")
        }))
        .expect_err("指定的下一个章标题找不到必须 panic —— 那正是「章结构变了」的信号");
        let msg = panic_message(&err);
        assert!(msg.contains("99"), "panic 消息必须说清缺哪个章:{msg}");
    }
}

// ── 标准库规范:「成员表」子节的定位与成员名抽取 ──────────────────────
//
// ⚠ **这一段的行为是契约**:改了它,`net_contract.rs` / `encode_contract.rs` /
// `time_contract.rs` 抽到的成员集合就会变 ⇒ 那三张表的断言会红。
// 「找不到就返回空」的写法尤其危险:它让「表结构变了」变成**静默**漂移,
// 而集合相等型断言在空集上会**假绿**(`[] == []`)。

/// 取一章正文里「成员表」子节的正文。
///
/// **定位口径**:找**第一个**标题行**以 `### ` 开头**且**标题文字以「成员表」
/// **结尾**的子节,取该子节到下一个 `### ` 或 `## ` 为止(标题行本身**不含**
/// 在返回的正文里)。
///
/// - ⚠ **按标题文字定位,不按子节号** —— 子节号逐章不同(`21.1` / `11.2` / `15.1`),
///   按编号定位的人每改一章就得再抄一次查找逻辑。
/// - ⚠ **围栏代码块(```)里的 `#` 行是注释,不是标题** —— 跳过,否则围栏里的
///   `# 注释` 会被当成标题,把子节的边界掐错。
///
/// **找不到 ⇒ panic(不返回空)**:「找不到成员表」与「这个命名空间没有成员」是
/// 两件必须分开的事 —— 前者是**表结构变了**(章改版 / 子节改名 / 子节被删),
/// 后者是规范事实。安静返回空 ⇒ 抽取器抽到 0 个 ⇒ 集合相等型断言可能**假绿**。
/// panic 消息里带上这一章实际有的标题,好让人一眼看出「子节改名了」还是
/// 「这一章根本没有这个子节」。
///
/// `chapter` 只用于报错文案(如 `"§21"`),不参与定位。
pub fn member_table_section(chapter: &str, chapter_body: &str) -> String {
    let mut fenced = false;
    let mut in_table = false;
    let mut found: Option<String> = None;
    let mut headings: Vec<String> = Vec::new();
    let mut out = String::new();

    for line in chapter_body.lines() {
        // 围栏代码块里的 `#` 行不是标题,别让它把 in_table 掐掉。
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if line.starts_with("### ") || line.starts_with("## ") {
            if in_table {
                // 子节到下一个 `### ` / `## ` 为止。
                break;
            }
            let title = line.trim_start_matches('#').trim().to_string();
            headings.push(title.clone());
            if title.ends_with("成员表") {
                in_table = true;
                found = Some(title);
            }
            continue;
        }
        if in_table {
            out.push_str(line);
            out.push('\n');
        }
    }

    match found {
        Some(_) => out,
        None => {
            let seen = if headings.is_empty() {
                "(一个标题都没有)".to_string()
            } else {
                headings.join(" / ")
            };
            panic!(
                "{chapter}: 这一章里找不到「成员表」子节 —— 期望一个形如 `### 21.1 成员表` \
                 的标题行(子节标题文字以「成员表」结尾;子节号逐章不同,所以按文字定位、\
                 不按编号)。这一章实际有的标题:{seen}"
            );
        }
    }
}

/// 从「成员表」子节的正文里抽成员名。
///
/// **抽取口径**(v1.0 的行首形状 `` | `NAME(sig) -> T` | ``):取**反引号或左括号
/// 前**的内容;只收**全大写标识符**(成员表里若混进非成员行 —— 表头、散文里的
/// 标识符 —— 不许它们进成员集合)。
///
/// ⚠ 名字必须在**反引号与左括号中取较早的那个** —— 只看到反引号会把
/// `BASE64_ENCODE(s, url_safe?)` 整个当成名字,然后被「全大写」这一关过滤掉,
/// **静默抽成 0 个**。
///
/// **抽到 0 个 ⇒ panic(不返回空)**:同样是「表结构变了」(列数变了 / 签名写法变了),
/// 不是「这个命名空间没有成员」。
///
/// `chapter` 只用于报错文案(如 `"§21"`),不参与抽取。
pub fn member_names(chapter: &str, table: &str) -> Vec<String> {
    let mut spec: Vec<String> = Vec::new();
    for line in table.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| *c != '`' && *c != '(')
            .collect();
        if name.is_empty() {
            continue;
        }
        // 只收全大写标识符 —— 成员表里若混进非成员行(表头 / 散文里的标识符),
        // 不许它们进成员集合。
        if name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            spec.push(name);
        }
    }
    if spec.is_empty() {
        panic!(
            "{chapter}: 「成员表」子节里一个成员都没抽出来 —— 表结构变了(行首不再是 \
             `| `NAME(sig)` |` 这个形状),不是「这个命名空间没有成员」。"
        );
    }
    spec
}

/// 共用助手自身的**自检** —— 把「找不到就报错」这条行为钉住。
///
/// ⚠ 这条守卫的意义在于**防止将来被改成安静返回空**:那种改法不会让任何一张
/// 契约表立刻变红(集合相等型断言在空集上会**假绿**),而是把「表结构变了」
/// 变成**静默**漂移 ⇒ 守卫必须在**今天**就钉住「它会响」。
#[cfg(test)]
mod self_check {
    use super::{member_names, member_table_section};
    use std::any::Any;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// 一段**故意没有「成员表」子节**的假章节正文。
    ///
    /// ⚠ 它刻意**有**别的 `### ` 子节、也有 `` | ` `` 开头的表格行 ——
    /// 「本章有子节」与「本章有成员表子节」是两件事,守卫要守的是后者。
    const NO_MEMBER_TABLE: &str = "\
## 标准库规范 · 99 `std.example` —— 假章

### 99.1 失败口径

| 签名 | 语义 |
|---|---|
| `NOT_A_MEMBER_TABLE(x)` | 长得像成员行,但所在的子节不叫「成员表」 |

### 99.2 设计依据

散文。
";

    /// 一个**有**「成员表」子节、但表里**没有成员行**的假章节正文。
    const EMPTY_MEMBER_TABLE: &str = "\
## 标准库规范 · 98 `std.example` —— 假章

### 98.1 成员表

| 签名 | 语义 |
|---|---|
| 说明 | 这一行不是成员行 |
";

    /// 取 panic 消息。
    fn panic_message(err: &Box<dyn Any + Send>) -> String {
        if let Some(s) = err.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = err.downcast_ref::<&str>() {
            (*s).to_string()
        } else {
            "(panic 载荷不是字符串)".to_string()
        }
    }

    #[test]
    fn member_table_section_panics_when_the_subsection_is_missing() {
        let err = catch_unwind(AssertUnwindSafe(|| {
            member_table_section("§99", NO_MEMBER_TABLE)
        }))
        .expect_err("找不到「成员表」子节必须 panic —— 不许安静返回空");
        let msg = panic_message(&err);
        assert!(
            msg.contains("§99") && msg.contains("成员表"),
            "panic 消息必须说清是哪一章、期望的子节标题形态:{msg}"
        );
    }

    #[test]
    fn member_names_panics_when_the_table_has_no_member_row() {
        // 这一章**有**「成员表」子节,但子节里一行成员都没有 ⇒ 这是
        // 「表结构变了」,与上一条「找不到子节」是**两件**事 ⇒ 两条都要响。
        let table = member_table_section("§98", EMPTY_MEMBER_TABLE);
        let err = catch_unwind(AssertUnwindSafe(|| member_names("§98", &table)))
            .expect_err("抽到 0 个成员必须 panic —— 不许安静返回空");
        let msg = panic_message(&err);
        assert!(msg.contains("§98"), "panic 消息必须说清是哪一章:{msg}");
    }

    #[test]
    fn member_table_section_finds_the_subsection_by_its_title_not_by_its_number() {
        // 子节号故意写成 `99.7` ⇒ 按编号定位会失手,按标题文字定位不会。
        let body = "\
## 标准库规范 · 97 `std.example` —— 假章

### 97.1 失败口径

| 签名 | 语义 |
|---|---|
| `NOPE(x)` | 不是成员 |

### 97.7 成员表

| 签名 | 语义 |
|---|---|
| `ALPHA(x) -> INTEGER` | 一个成员 |
";
        let table = member_table_section("§97", body);
        let names = member_names("§97", &table);
        assert_eq!(names, vec!["ALPHA".to_string()]);
    }
}
