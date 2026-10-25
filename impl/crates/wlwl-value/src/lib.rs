//! WLWL 值层(标准库底座 v0.11 / ADR-0022)。
//!
//! 单源承载语言规范 §2.1 的十三类运行时值与全部宿主侧数据结构
//! (`Env` / `ClassEntry` / `TaskHandle` / `ChannelHandle` / 会话协议
//! 光标等),以及 std 原生函数的调用契约([`StdFn`] / [`StdHost`] /
//! [`StdCtx`])。内容自 `wlwl-eval` **纯搬移**,不改任何语义。
//!
//! 依赖方向单向:`wlwl-eval → wlwl-value ← wlwl-std`;本 crate 只依赖
//! `wlwl-ast`(闭包体 AST)与 `wlwl-error`(诊断)。
//!
//! 函数调用行为(如何求值闭包)留在 eval 侧,经 [`StdHost::call`]
//! 注入 —— wlwl-value 只承载值与句柄,不承载求值器。
//!
//! ## 未消费 `ERR` 状态:集合形态与**身份口径**
//!
//! [v1.0 规范 §8.6] 未消费状态是**集合**,不是单个标志 —— 一个绑定 / 一个
//! 容器 / 一个数组元素序列都可以**同时持有多个**未消费的 `ERR`。集合类型是
//! [`UnconsumedErrs`],元素是**那个 `ERR` 值本身**(`Value::Err(_)`),按
//! **出现序**排列。
//!
//! ✅ **身份口径已裁决**(2026-10-23,业主;落进语言核心册 §8.6.5):**值语义即
//! 新身份** —— §2.1 的值语义下,深拷贝产生的 `ERR` 是**新生的** `ERR` ⇒ 两个
//! **内容相同**但来源不同的 `ERR` 是**两个** `ERR` ⇒ 被分别处置时**各报一次**
//! (不因内容相同而合并成一条)。
//!
//! ⇒ **本模块按内容判定身份,恰好与该口径一致** —— 因为集合是**多重集**
//! ([`union_unconsumed`] **不去重**)⇒ 两个内容相同的 `ERR` 占**两个**元素 ⇒
//! 逐个报告时是**两条**。⇒ **不需要**改 `Value::Err` 的表示(约 111 处构造点)。
//!
//! ⚠ **仍然成立的两条边界**(与身份口径无关,别混):
//!
//! * 取走型消费者「取走它拿到的那一个」时按 `PartialEq` 匹配 ⇒ 在多个
//!   内容相同的 `ERR` 里**移除哪一个是不确定的** —— 但这**不可达**:一个绑定的
//!   集合只含多个元素时,那个绑定的值是**容器**;而容器形态**从不**被取走型
//!   消费者**按元素**移除(§8.6.3「容器里还剩几个,不受它影响」)⇒ 实际只有
//!   单元素集合会被按元素移除 ⇒ 不确定性与否,行为确定;
//! * 诊断只报**载荷** ⇒ 两个内容相同的 `ERR` 报出的消息逐字相同 ⇒ 这是**消息
//!   形态**问题(§8.6.2:报的是「被处置的那个值」),不是身份问题。
//!
//! ✅ **跨 `StdHost` 通道的那一段现在也走得通了**(2026-10-23):两条通道
//! ([`StdHost::arg_tokens`] / [`StdHost::call_with_tokens`])的签名已从 `bool`
//! 改成 `UnconsumedErrs` ⇒ 集合**本体**过得了通道 ⇒ 成员**直接拿到**「哪几个
//! `ERR` 未被消费」,不再需要按实参值**重建**那个近似集合。

#![allow(clippy::doc_overindented_list_items)]

use std::cell::{Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wlwl_ast::{Expr, FunParam, Literal, Span};
use wlwl_error::{ErrorCode, Location, WlwlDiagnostic, WlwlError};

// ──────────────────────────────────────────────────────────────────────
// 运行时值与宿主侧数据(自 wlwl-eval 纯搬移)
// ──────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────
// Runtime values
// ──────────────────────────────────────────────────────────────────────

/// Runtime value (v0.3 §2.2 — Phase 2).
///
/// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] PartialEq is hand-
/// implemented (rather than `#[derive]`d) starting this commit:
/// the new `Class` / `Instance` variants wrap `Rc<RefCell<_>>`,
/// which doesn't auto-derive `PartialEq`. Class / Instance equality
/// is by **identity** (`Rc::ptr_eq`), consistent with how the
/// existing handle variants (`TaskHandle`, `ChannelHandle`) compare
/// by their `id` + `generation` fields rather than by deep value.
/// [v0.11.3 M7 / `ADR-0032` D1 + D4 / addendum-00 `L0-B`] 累加器的共享可变数组。
///
/// **为什么要有它**:`Value::Array` 是纯 owned `Vec`,于是 `PUSH` / `+(a,[x])` /
/// `CONCAT` 每次都 `clone()` 整个数组 —— 用户代码里的累加器因此是超线性的
/// (`ADR-0032` §1 实测:每元素 44 847 → 117 437 ns,n 从 1 000 到 8 000)。
/// 本类型是那三个内建的**就地追加**落点。
///
/// ⚠ **它必须与 `Array` 在所有可观察面上完全一致** —— `TYPE` / `display` /
/// `==` / 序列化 / 诊断措辞。这是 (c) 路线的全部意义:现有 `Array` 与它的
/// 244 处用法一字不动,只有被识别成累加器的数组才会变成 `Cell`。
///
/// ⚠ **克隆是深拷贝,不是共享**。`#[derive(Clone)]` 会让 `Rc` 共享 cell,
/// 那会让 `LET(b, a)` 之后改 `a` 连带改 `b` —— **静默破坏不可变性语义**。
/// 所以这里手写 `Clone`:深拷贝,行为与 `Array` 一致。性能收益来自
/// 内建里的**就地分支**(PLAN S3),不来自克隆共享。
///
/// ⚠ **[ADR-0035] 一处口径细化**:元素是**数组**时仍是逐元素深拷贝;元素是
///   **字典**时,字典载荷是 `Rc<DictInner>` ⇒ 克隆只拷那个 `Rc`(不再深拷
///   键值对)。**不可变性不受影响** —— 字典的写路径一律走 copy-on-write
///   (`Rc::make_mut`),被别名时先克隆再改 ⇒ 旧值原样可见。
#[derive(Debug)]
pub struct CellVec(std::rc::Rc<std::cell::RefCell<Vec<Value>>>);

impl CellVec {
    /// 新建一个独立 cell。
    pub fn new(items: Vec<Value>) -> Self {
        CellVec(std::rc::Rc::new(std::cell::RefCell::new(items)))
    }

    /// **共享同一个 cell**(只克隆 `Rc`,不深拷贝)。
    ///
    /// ⚠ 与 `Clone`(深拷贝)是**两个不同的出口**,别混:
    /// - `clone()` = 语义拷贝,给 `LET(b, a)` 那种「绑一份新的」用;
    /// - `share()` = 就地追加的内建**返回同一个 cell** 时用。
    ///
    /// 若内建用 `clone()` 返回,每轮追加都会把整个数组深拷一遍 —— 优化归零,
    /// 而且比现在更慢(多了一次 cell 化 + 一次深拷)。
    pub fn share(&self) -> Self {
        CellVec(self.0.clone())
    }

    /// 是否与 `other` **就是同一个** cell(同一 `Rc`)。
    ///
    /// 相等性必须先用它短路:两个 `Rc` 指向同一 cell 时直接判 `true`,
    /// 不去 `borrow()` —— 否则同一 cell 上「借出比对」会撞
    /// `RefCell` 的运行期借用检查。
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }

    /// 只读借用。**调用方不得在持有它时去 `borrow_mut` 同一个 cell。**
    pub fn borrow(&self) -> std::cell::Ref<'_, Vec<Value>> {
        self.0.borrow()
    }

    /// 可变借用(仅 S3 的内建分支用)。
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, Vec<Value>> {
        self.0.borrow_mut()
    }
}

/// 深拷贝 —— 与 `Value::Array` 的克隆语义一致,**不**共享 cell。
///
/// ⚠ 这一条是 S1 的关键不变量:若让 `Rc` 共享,`LET(b, a)` 之后改 `a` 会连带
/// 改 `b`,那是**静默**的可观察语义破坏,而 §6.6 的性能收益并不依赖它。
impl Clone for CellVec {
    fn clone(&self) -> Self {
        CellVec(std::rc::Rc::new(std::cell::RefCell::new(
            self.0.borrow().clone(),
        )))
    }
}

/// 打印成 `[1, 2]` 而不是 `RefCell { value: [1, 2] }` —— 调试输出也是可观察面。
impl std::fmt::Display for CellVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self.0.borrow().iter().map(|v| v.display()).collect();
        write!(f, "[{}]", parts.join(", "))
    }
}

/// [项目扩展 / G-17] 字符串累加器的共享可变单元 —— `CellVec` 的**字符串版**。
///
/// **为什么要有它**：`Value::String(String)` 是**不可变**的 ⇒ `s = s + part`
/// 每轮都要把整条字符串**拷一遍** ⇒ 循环 n 次 ⇒ **O(n²)**。
/// 数组有 `CellVec`、字典有 `CellDict`（各自把追加变成**原地改**）⇒
/// **字符串缺同款** ⇒ 补上。
///
/// ⚠ **码点数要缓存**：`LEN(s)` 数的是**码点**不是字节（§10.3）⇒ 每次调用重算
///   就是 O(n) ⇒ 追加时**顺手**把码点数累加进去 ⇒ `LEN` 保持 O(1)。
///
/// ⚠ **不变量**（与 `CellVec` / `CellDict` 同款）：可观察面（渲染 / `TYPE` /
///   `==` / 诊断措辞）必须与 `Value::String` **逐字相同** —— 它是**运行期实现细节**。
#[derive(Debug)]
pub struct CellStr(std::rc::Rc<std::cell::RefCell<CellStrInner>>);

/// `CellStr` 的载荷：文本 + **缓存的码点数**。
#[derive(Debug)]
pub struct CellStrInner {
    text: String,
    chars: usize,
}

impl CellStr {
    /// 新建一个独立 cell。
    pub fn new(text: String) -> Self {
        let chars = text.chars().count();
        CellStr(std::rc::Rc::new(std::cell::RefCell::new(CellStrInner {
            text,
            chars,
        })))
    }

    /// **共享同一个 cell**（只克隆 `Rc`，不拷文本）—— 与 [`CellVec::share`] 同款。
    ///
    /// ⚠ 内建返回时**必须**用 `share()` 而不是 `clone()`：后者每轮拷整条字符串，
    ///   优化归零（与 `CellVec` 的注释同款理由）。
    pub fn share(&self) -> Self {
        CellStr(self.0.clone())
    }

    /// 是否与 `other` **就是同一个** cell（同一 `Rc`）。
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }

    /// 只读借用。**调用方不得在持有它时去 `borrow_mut` 同一个 cell。**
    pub fn borrow(&self) -> std::cell::Ref<'_, CellStrInner> {
        self.0.borrow()
    }

    /// 可变借用（仅就地追加的内建分支用）。
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, CellStrInner> {
        self.0.borrow_mut()
    }

    /// 文本内容（克隆一份）。
    pub fn as_str(&self) -> String {
        self.0.borrow().text.clone()
    }

    /// 码点数（`LEN` 的口径）。
    pub fn chars(&self) -> usize {
        self.0.borrow().chars
    }

    /// **就地**追加一段文本 ⇒ 只拷 `part` 的字节（O(|part|)），不动已有内容。
    ///
    /// ⚠ 调用方负责**计数**（`string_bytes_copied`）—— 与 `concat_strings` 同款口径。
    pub fn push_str(&self, part: &str) {
        let mut inner = self.0.borrow_mut();
        inner.chars += part.chars().count();
        inner.text.push_str(part);
    }
}

/// 深拷贝 —— 与 `Value::String` 的克隆语义一致，**不**共享 cell。
impl Clone for CellStr {
    fn clone(&self) -> Self {
        let inner = self.0.borrow();
        CellStr(std::rc::Rc::new(std::cell::RefCell::new(CellStrInner {
            text: inner.text.clone(),
            chars: inner.chars,
        })))
    }
}

/// 打印成**文本本身**（与 `Value::String` 逐字相同）—— 调试输出也是可观察面。
impl std::fmt::Display for CellStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.borrow().text)
    }
}

/// [ADR-0035 D4] 字典累加器的共享可变单元 —— **`ADR-0032` 的 `CellVec` 的字典版**。
///
/// **为什么要有它**:`Value::Dict(Rc<DictInner>)` 的写路径走 **copy-on-write**
/// (`Rc::make_mut`):唯一拥有时原地改,被别名时克隆整个字典。而
/// `SET(d, INDEX_SET(d, k, v))` 这种**最常见的字典累加写法**里,`d` 仍被绑定
/// 持有 ⇒ 写之前必拷贝 ⇒ 累加 n 个键的总拷贝是**二次的**(`ADR-0035` 修订记录
/// 实测:n = 16 ⇒ 拷贝 120 条)⇒ 规范 §18.3 的 `INDEX_SET` 写上界
/// (新键 O(1) 摊还)**未达标**。本类型是那一步的落点。
///
/// ⚠ **它必须与 `Value::Dict` 在所有可观察面上完全一致** —— `TYPE` / `display` /
/// `==` / 序列化 / 诊断措辞 / **插入序**。这是 `ADR-0032` (c) 路线的全部意义:
/// 现有 `Dict` 与它的 175 处用法一字不动,只有被识别成累加器的字典才会用它。
///
/// ⚠ **载荷是 `Rc<RefCell<Rc<DictInner>>>`** —— 最里层的 `Rc<DictInner>` 让
/// **写时复制** machinery([`dict_make_mut`])原样可用:唯一拥有时原地改
/// (单次写 O(1)),被别名时先克隆再改(O(n))—— 与普通字典的写路径**逐字同一条**,
/// 连 §18.5 的确定性工作量计数点都是同一个。
///
/// ⚠ **克隆是共享,不是深拷贝** —— 与 `Value::Dict` 的克隆语义**一致**
/// (`Rc::clone`):`LET(b, d)` 之后 `b` 与 `d` 共享同一份字典数据,改 `d` 不改 `b`
/// (写时复制保证)。这与 `CellVec` 的深拷贝克隆**不同**,而且**必须不同**:
/// 字典载荷本来就是 `Rc` ⇒ 深拷贝会让 `LET(b, d)` 的成本从 O(1) 变成 O(n),
/// 而那正是本批要消灭的二次代价。
#[derive(Debug)]
pub struct CellDict(std::rc::Rc<std::cell::RefCell<std::rc::Rc<DictInner>>>);

impl CellDict {
    /// 新建一个 cell,载荷是给定的 `Rc<DictInner>`(**共享**,不拷贝字典)。
    pub fn new(inner: std::rc::Rc<DictInner>) -> Self {
        CellDict(std::rc::Rc::new(std::cell::RefCell::new(inner)))
    }

    /// **共享同一个 cell**(只克隆 `Rc`,不拷贝字典)。
    ///
    /// ⚠ 与 `Clone`(语义拷贝)是两个不同的出口,别混:
    /// - `clone()` = 语义拷贝,给 `LET(b, a)` 那种「绑一份新的」用;
    /// - `share()` = 就地写的那条路上「把同一个 cell 交出去」用。
    pub fn share(&self) -> Self {
        CellDict(self.0.clone())
    }

    /// 是否与 `other` **就是同一个** cell(同一 `Rc`)。
    ///
    /// 相等性必须先用它短路:两个 `Rc` 指向同一 cell 时直接判 `true`,
    /// 不去 `borrow()` —— 否则同一 cell 上「借出比对」会撞 `RefCell` 的运行期
    /// 借用检查。
    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }

    /// 取出载荷的 `Rc<DictInner>`(**共享**,O(1) —— 不是深拷贝)。
    ///
    /// 循环结束时把 cell 换回普通字典走这里:换回去的是**同一份数据**,
    /// 所以「还原」这一步是 O(1),不是 O(n)。
    pub fn inner_rc(&self) -> std::rc::Rc<DictInner> {
        let slot = self.0.borrow();
        (*slot).clone()
    }

    /// **就地改**字典(唯一拥有 ⇒ O(1);被别名 ⇒ 先克隆 ⇒ O(n))。
    ///
    /// ⚠ 走 [`dict_make_mut`] 而不是裸 `Rc::make_mut`:那是**唯一**允许写路径
    /// 拿到 `&mut DictInner` 的入口,也是 §18.5 确定性工作量计数的**唯一**位置
    /// ⇒ 绕开它,计数就会漏。
    ///
    /// ⚠ 借出的 `&mut DictInner` 只在闭包体内有效 —— `RefMut` 必须活得比它久,
    ///   所以这里把 `borrow_mut()` 的结果**绑成局部变量**再取 `&mut`。
    pub fn with_inner_mut<R>(&self, f: impl FnOnce(&mut DictInner) -> R) -> R {
        let mut slot = self.0.borrow_mut();
        let inner = dict_make_mut(&mut slot);
        f(inner)
    }
}

/// 克隆 = **共享载荷**(与 `Value::Dict` 的克隆语义一致),O(1)。
///
/// ⚠ 与 `CellVec` 的深拷贝克隆**不同**,而且**必须不同**:字典载荷本来就是
/// `Rc` ⇒ 深拷贝会让 `LET(b, d)` 从 O(1) 变成 O(n)。
impl Clone for CellDict {
    fn clone(&self) -> Self {
        CellDict(std::rc::Rc::new(std::cell::RefCell::new(self.inner_rc())))
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
    Array(Vec<Value>),
    /// [v0.11.3 M7 / `ADR-0032` D1] 累加器的共享可变数组。**可观察面必须与
    /// `Array` 一致**(`TYPE` 给 `"array"`、`==` 深比较、序列化产出 `Array`)。
    /// 只有被识别成「循环局部累加器」的数组才会用它(PLAN S2);其余一律是
    /// `Array`,那 244 处用法一字未动。
    Cell(CellVec),
    /// [ADR-0035 D1 / S1] 字典载荷 = `Rc<DictInner>`。
    ///
    /// ⚠ **只有这一种字典表示**(没有并行的 `DictIndexed` 变体)⇒ 可观察面
    ///   (`TYPE` / `==` / 渲染 / JSON / 诊断措辞 / **插入序**)只有一套代码路径。
    /// ⚠ 载荷是 `Rc` ⇒ **克隆字典是共享,不是深拷贝** ⇒ 写路径必须走
    ///   **copy-on-write**(`Rc::make_mut`):唯一拥有时原地改,被别名时克隆后改。
    ///   旧值在「唯一拥有」时不可观察(§19.1 的值语义)⇒ 原地改合法。
    Dict(Rc<DictInner>),
    /// [ADR-0035 D4] 字典累加器的共享可变字典。**可观察面必须与 `Dict` 一致**
    /// (`TYPE` 给 `"dict"`、`==` 深比较、渲染与 JSON 逐字相同、**插入序**不变)。
    /// 只有被识别成「循环局部字典累加器」的字典才会用它(PLAN S2);
    /// 其余一律是 `Dict`,那 175 处用法一字未动。
    DictCell(CellDict),
    /// [项目扩展 / G-17] 字符串累加器的共享可变字符串。**可观察面必须与
    /// `String` 一致**（`TYPE` 给 `"string"`、`==` 按内容比较、渲染与诊断措辞
    /// 逐字相同）。
    ///
    /// ⚠ **只有被识别成「循环局部字符串累加器」的绑定才会用它**；其余一律是
    ///   `String` ⇒ 那 175 处用法一字未动。
    CellStr(CellStr),
    /// §8.2 function literal with captured environment (clone-based closure).
    Closure {
        params: Vec<FunParam>,
        body: Box<Expr>,
        env: Env,
    },
    /// §15 std library function (Phase 4): body is a native Rust impl,
    /// dispatchable like a closure but without a parseable `Expr`. Bound
    /// in the env by `IMPORT("wlwl:std.X", …)` so that the user-supplied
    /// IMPORT takes priority over the `resolve_builtin` fallback.
    NativeFn {
        name: String,
        invoke: StdFn,
    },
    /// §12 OK(value)
    /// §12 OK(value)
    Ok(Box<Value>),
    /// §12 ERR(value)
    Err(Box<Value>),
    /// [v0.7 Phase C2] Handle returned to user code by `SPAWN(fn)`.
    /// AWAIT (C3) takes this value and dereferences it to read
    /// the spawned task's terminal value or error. At C2 the
    /// referenced task has always already finished (synchronous
    /// execution); B5b will let back-pointers capture still-running
    /// tasks and AWAIT will actually wait on them.
    TaskHandle(TaskHandle),
    /// [v0.7 Phase D-A] Handle returned to user code by
    /// `CHANNEL_NEW(buf)`. SEND / RECV / CLOSE / TRY_SEND / TRY_RECV
    /// / LEN / CAP all take this value. Generation-tracked: a slot
    /// recycled by the D-D leak detector bumps the generation so a
    /// stale handle fails E0053-style validation rather than
    /// operating on a different channel than the one the user
    /// originally opened.
    ChannelHandle(ChannelHandle),
    /// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Class value
    /// produced by `CLASS(name?, parent, members)`. Wrapped in
    /// `Rc<RefCell<_>>` so closure capture (`Env::clone`) can share
    /// the same class entry between the defining scope and any
    /// downstream `NEW` call — parallel to the cell-sharing rule
    /// for `LET MUT` (v0.6 §3.4). Step 9a-2 wires `builtin_class`
    /// to populate the entry; Step 9a-3 will add `init` tracking
    /// (currently a placeholder `Option<Value>`).
    Class(std::rc::Rc<std::cell::RefCell<ClassEntry>>),
    /// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Instance value
    /// produced by `NEW(cls, args...)`. The `class` field is shared
    /// with the `Value::Class` that created it (same `Rc`); `fields`
    /// is the per-instance `(name → value)` table that `GET_PROP` /
    /// `SET_PROP` read / write (Step 9a-5 wires these). `this_token`
    /// is the placeholder for the linear `THIS` capability (Step
    /// 9a-3 fleshes it out to a runtime-checked `Rc<RefCell<_>>`).
    ///
    /// [v0.9 Step 9a-5] `fields` is wrapped in `Rc<RefCell<_>>`
    /// (rather than the plain `Vec` that 9a-1 used) so that
    /// `SET_PROP` mutations propagate to every reference that
    /// shares the instance. The plain-`Vec` shape meant the
    /// caller had to rebind `LET inst = SET_PROP(inst, "k", v)`
    /// after every mutation — a footgun the §13 ergonomics
    /// call out as a deviation. With the Rc-wrapped shape,
    /// any future `inst.foo` reference observes the new
    /// field automatically, mirroring how closures already
    /// share cells via `Env::clone`.
    Instance {
        class: std::rc::Rc<std::cell::RefCell<ClassEntry>>,
        fields: std::rc::Rc<std::cell::RefCell<Vec<(Value, Value)>>>,
        this_token: std::rc::Rc<std::cell::RefCell<ThisToken>>,
        /// [v0.9 Step 9b] Per-instance session-protocol cursor.
        protocol_state: std::rc::Rc<std::cell::RefCell<ProtocolCursor>>,
    },
}

/// [ADR-0035 D2 / S1] 字典的**内部哈希键**。
///
/// ⚠ **`Eq` / `Hash` 必须与 `values_equal` 的键口径逐字一致**(规范 §2.4):
///
/// | 键类型 | 相等判据 | 哈希判据 |
/// |---|---|---|
/// | `STRING` | 按**字节**(同 `String` 的 `PartialEq`) | 同 `String` 的 `Hash` |
/// | `INTEGER` | 按**数值** | 同 `i64` 的 `Hash` |
/// | `INSTANCE` | 按**身份**(`Rc::ptr_eq` 的 `fields` 单元格) | `fields` 单元格的地址 |
///
/// 判据取自 `values_equal` 的对应分支:`Value::String` 走 `x == y`
/// (字节口径)、`Value::Integer` 走 `x == y`(数值口径)、
/// `Value::Instance` 走 `std::rc::Rc::ptr_eq(fx, fy)`(`fields` 单元格)。
/// ⇒ **同一实例 ⇒ 同一哈希**,不同实例 ⇒ 不同哈希(地址不同;且 `DictKey`
///   持有 `Rc` ⇒ 分配在被哈希期间不会消失,不会有「地址复用」造成的错命中)。
///
/// ⚠ **键域是封闭的**(规范 §2.1):只有 `STRING` / `INTEGER` / `INSTANCE`
///   能作键。域外类型(`FLOAT` / 布尔 / `NULL` / 数组 / …)⇒
///   [`DictKey::from_value`] 返回 `None` ⇒ 字典退回线性扫描(见
///   [`DictInner::lookup`]),**不**静默改变查找语义。
#[derive(Debug, Clone)]
pub enum DictKey {
    /// `STRING` 键 —— 按字节比。
    Str(String),
    /// `INTEGER` 键 —— 按数值比。
    Int(i64),
    /// `INSTANCE` 键 —— 按**身份**比(`fields` 单元格的 `Rc::ptr_eq`)。
    Instance(Rc<RefCell<Vec<(Value, Value)>>>),
}

impl DictKey {
    /// 把一个 `Value` 转成索引键;**域外**的键(规范 §2.1 的键域之外)返回 `None`。
    ///
    /// ⚠ `NaN` 是 `FLOAT` ⇒ 落在域外 ⇒ `None`(规范 §2.1:NaN 不得作键)。
    pub fn from_value(v: &Value) -> Option<DictKey> {
        match v {
            Value::String(s) => Some(DictKey::Str(s.clone())),
            Value::Integer(i) => Some(DictKey::Int(*i)),
            Value::Instance { fields, .. } => Some(DictKey::Instance(Rc::clone(fields))),
            _ => None,
        }
    }
}

impl PartialEq for DictKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DictKey::Str(a), DictKey::Str(b)) => a == b,
            (DictKey::Int(a), DictKey::Int(b)) => a == b,
            // ⚠ 与 `values_equal` 的 `Value::Instance` 分支**同一条判据**:
            //   `fields` 单元格的 `Rc::ptr_eq`(= 规范 §2.4 的「对象身份」)。
            (DictKey::Instance(a), DictKey::Instance(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for DictKey {}

impl std::hash::Hash for DictKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // ⚠ 变体前缀 ⇒ `Str` / `Int` / `Instance` 三族的哈希互不混淆
        //   (否则 `"1"` 与 `1` 会撞桶,而它们在 §2.4 下**不**相等)。
        match self {
            DictKey::Str(s) => {
                0u8.hash(state);
                s.hash(state);
            }
            DictKey::Int(i) => {
                1u8.hash(state);
                i.hash(state);
            }
            DictKey::Instance(fields) => {
                2u8.hash(state);
                // 身份 ⇒ 地址。`Rc::as_ptr` 与 `Rc::ptr_eq` 用的是同一个指针
                // 口径 ⇒ 「哈希相同」⟺「同一实例」。
                Rc::as_ptr(fields).hash(state);
            }
        }
    }
}

/// [ADR-0035 D1 / S1] 字典的**内部表示**:插入序条目 + 键 → 位置的哈希索引。
///
/// * `entries` —— **插入序**的键值序列(规范 §18.3.1:键序是插入序且可观察);
///   它是**呈现的唯一来源**(渲染 / JSON / `KEYS` / `VALUES` 全走它)。
/// * `index` —— `DictKey` → `entries` 里的**位置列表**。索引**只存位置**,
///   **不参与呈现** ⇒ 哈希的实现细节不会漏到可观察面。
///
/// ⚠ **克隆是共享,不是深拷贝**(`Rc` 的语义)⇒ 任何写路径都必须走
///   **copy-on-write**([`Value::as_dict_inner_mut`] 用 `Rc::make_mut`)。
#[derive(Debug, Clone)]
pub struct DictInner {
    /// 插入序的键值序列(**呈现的唯一来源**)。
    entries: Vec<(Value, Value)>,
    /// 键 → `entries` 里的**位置列表**。
    ///
    /// ⚠ **列表而不是单个位置**:字典字面量允许**重复键**
    ///   (`["a": 1, "a": 2]`)⇒ 同一键在 `entries` 里可能有**多个**位置。
    ///   列表按**插入序**升序 ⇒ **首个**元素 = 首次出现的位置 ⇒
    ///   [`DictInner::lookup`] 仍返回「首次出现」(与旧的线性扫描逐字一致);
    ///   而**深度相等**([`dict_entries_equal`])要的是「**同键的所有**位置」
    ///   ⇒ 集合口径才不会被「只取首个」悄悄改掉。
    index: HashMap<DictKey, Vec<usize>>,
    /// 键域封闭性(规范 §2.1):`false` 表示本字典里**存在域外键**。
    ///
    /// ⚠ 域外键会让 `values_equal` 的「跨类型数值相等」变得不对称:
    ///   存储侧 `FLOAT(1.0)` 与探测侧 `INTEGER(1)` 在 §2.4 下**相等**
    ///   (数字跨类型比较),但 `FLOAT` 不在键域内 ⇒ 哈希索引不覆盖它 ⇒
    ///   只查索引会**漏掉**这一项。
    ///   ⇒ 存在域外键时,查找**退回线性扫描**,与旧的
    ///   `entries.iter().position(|(k,_)| values_equal(k, key))` 逐字一致。
    all_keys_in_domain: bool,
}

/// [ADR-0035 S5] 字典的**确定性工作量计数**。
///
/// 计数点收在**字典内部**(`DictInner::lookup` / `insert` / `remove` / `retain`
/// 与 `Value::as_dict_inner_mut` 的克隆面)—— 规范 §18.5 要求判定**必须**用
/// 确定性工作量计数,不用墙钟比值(同份线性实现在不同机器上可差 3 倍)。
///
/// ⚠ **线程局部,不是 `static`**:`cargo test` 的用例**并行**跑,全局计数会被
/// 别的用例搅浑 ⇒ 每个用例只看见自己那次运行的工作量。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DictWorkCounters {
    /// 键比较次数。哈希路径:每次查找 1 次;线性回退路径:按**实际扫描过**的
    /// 条目数计(域外键 / 字典含域外键时走那条路)。
    pub comparisons: u64,
    /// 因 **copy-on-write 克隆**而拷贝的条目数(被别名时的写路径代价)。
    pub copied_entries: u64,
    /// 走哈希索引的查找次数。
    pub indexed_lookups: u64,
    /// 退回线性扫描的查找次数(域外键 / 字典含域外键)。
    pub linear_lookups: u64,
    /// 索引重建次数(`remove` / `retain` 改了位置 ⇒ 位置变 ⇒ 索引要重建)。
    pub index_rebuilds: u64,
}

thread_local! {
    static DICT_WORK: std::cell::Cell<DictWorkCounters> =
        std::cell::Cell::new(DictWorkCounters::default());
}

/// 读**当前线程**的字典工作量计数。**只给测试与基准读**(同
/// `Evaluator::accumulator_optimizer_counters` 的体例:不做运行时警告 ——
/// D11-022 已证警告通道无人排空,走它等于静默丢弃)。
pub fn dict_work_counters() -> DictWorkCounters {
    DICT_WORK.with(|c| c.get())
}

/// 清零**当前线程**的字典工作量计数。**只给测试与基准读**。
pub fn reset_dict_work_counters() {
    DICT_WORK.with(|c| c.set(DictWorkCounters::default()));
}

/// 累加当前线程的工作量计数。
fn add_dict_work(f: impl FnOnce(&mut DictWorkCounters)) {
    DICT_WORK.with(|c| {
        let mut w = c.get();
        f(&mut w);
        c.set(w);
    });
}

/// [ADR-0035 S5 / D3] 字典的 **copy-on-write** 入口 —— **唯一**允许写路径拿到
/// `&mut DictInner` 的地方。
///
/// 唯一拥有 ⇒ 原地改(单次写 O(1));被别名 ⇒ `Rc::make_mut` 先克隆 ⇒ O(n)。
/// 判据**严格按引用计数**(`Rc::make_mut` 自己的 `is_unique`),不靠「看起来没人用」
/// —— 后者在 §19.1 的值语义下不成立:旧值只要还有别的持有者,原地改就是**可观察的
/// 语义破坏**。
///
/// ⚠ **所有写路径必须走这里**,否则计数会漏(本函数是 S5 计数点的**唯一**位置)。
pub fn dict_make_mut(inner: &mut std::rc::Rc<DictInner>) -> &mut DictInner {
    if std::rc::Rc::strong_count(inner) > 1 {
        // 被别名 ⇒ 克隆整个字典 ⇒ 那是 O(n) 的拷贝代价 ⇒ 计数上单列
        // (别名下的写是 O(n),这正是 §18.3.1「摊还口径」要如实登记的那一半)。
        let copied = inner.entries.len() as u64;
        add_dict_work(|w| w.copied_entries += copied);
    }
    std::rc::Rc::make_mut(inner)
}

// ──────────────────────────────────────────────────────────────────────
// [V-01] 复杂度上界的**确定性工作量计数**(规范 §18.5.2)
// ──────────────────────────────────────────────────────────────────────
//
// 规范 §18.3 给「数组追加 / 字符串拼接 / 深度相等 / `SORT` 比较次数」的
// 上界**都是「规范性」的** ⇒ 必须取证。取证口径(§18.5.2)有三条硬要求:
//
// ① **不用墙钟比值** —— 同一份线性实现在不同机器上可差 3 倍,比值里混着缓存
//    命中率与调度噪声 ⇒ 只能用**确定性工作量计数**;
// ② **计量点收在被测数据结构内部** —— 在调用点计数对「内部换成线性扫」不敏感;
// ③ **带下界断言** —— 确认计量点确实被走到(否则「计数恒为 0」也会绿)。
//
// ⇒ 下面这组计数器**线程局部**(`cargo test` 的用例并行跑 ⇒ 全局计数会被别的
// 用例搅浑),计数是**旁路**的:不改变任何求值路径的语义,只多记一个数。
//
// | 计数器 | 计量点(在被测结构内部) |
// |---|---|
// | `array_elements_copied` | [`copy_array_elements`] / [`copy_array_element`] —— 数组拷贝原语 |
// | `string_bytes_copied` | [`concat_strings`] —— 字符串拼接原语 |
// | `string_codepoints_expanded` | [`string_codepoints`] —— 字符串拆码点原语 |
// | `equality_element_visits` | [`values_equal`] 的数组分支 —— 深度相等的逐对访问 |
// | `equality_key_comparisons` | [`dict_entries_equal`] —— 深度相等的**键比较**次数(哈希路径 / 线性回退) |
// | `comparator_calls` | [`note_closure_call`] —— 闭包被真正调用的次数(= `SORT` 路径上的比较器调用次数) |
// | `string_comparisons` | [`note_string_comparison`] —— 值比较原语里的字符串比较 |

/// [V-01 / 规范 §18.5.2] 复杂度上界的**确定性工作量计数**。
///
/// ⚠ **线程局部,不是 `static`**(同 [`DictWorkCounters`])——`cargo test` 的用例
/// **并行**跑,全局计数会被别的用例搅浑 ⇒ 每个用例只看见自己那次运行的工作量。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ValueWorkCounters {
    /// 数组**拷贝的元素个数**(顶层元素数;元素自身的深拷贝不再展开计)。
    pub array_elements_copied: u64,
    /// 字符串**拷贝的字节数**。
    pub string_bytes_copied: u64,
    /// 字符串**拆成码点**的字符个数。
    pub string_codepoints_expanded: u64,
    /// 深度相等:**数组**逐对访问的元素个数。
    pub equality_element_visits: u64,
    /// 深度相等:**字典**的**键比较**次数。
    ///
    /// ⚠ **两条路径,两种口径**(都只数**键**比较,值比较不计):
    ///   * **哈希路径**(`dict_entries_equal_indexed`):每个 `x` 条目 1 次
    ///     哈希探测(与 [`DictInner::lookup`] 的哈希路径同口径:一次查找 = 1 次
    ///     键比较)+ 每个**候选** 1 次核对 ⇒ 键互异时**恰好 2n**;
    ///   * **线性回退**(`dict_entries_equal_linear`):按**实际扫描过**的条目数
    ///     计(存在域外键时走那条路)。
    pub equality_key_comparisons: u64,
    /// **闭包被真正调用**的次数 —— 在 `SORT` 的实现路径上**就是比较器调用
    /// 次数**(门面 `collection.wll` 的 `_SORT_LT` 里唯一的 `CALL(cmp, a, b)`;
    /// 门面自己的内部闭包按名调用 ⇒ 不计入)。
    pub comparator_calls: u64,
    /// 值比较(`<` / `>` / `<=` / `>=`)里**字符串 vs 字符串**的次数。
    pub string_comparisons: u64,
}

thread_local! {
    static VALUE_WORK: std::cell::Cell<ValueWorkCounters> =
        std::cell::Cell::new(ValueWorkCounters::default());
}

/// 读**当前线程**的复杂度工作量计数。**只给测试与基准读**(同
/// [`dict_work_counters`] 的体例:不做运行时警告 —— D11-022 已证警告通道无人
/// 排空,走它等于静默丢弃)。
pub fn value_work_counters() -> ValueWorkCounters {
    VALUE_WORK.with(|c| c.get())
}

/// 清零**当前线程**的复杂度工作量计数。**只给测试与基准读**。
pub fn reset_value_work_counters() {
    VALUE_WORK.with(|c| c.set(ValueWorkCounters::default()));
}

/// 累加当前线程的工作量计数。
fn add_value_work(f: impl FnOnce(&mut ValueWorkCounters)) {
    VALUE_WORK.with(|c| {
        let mut w = c.get();
        f(&mut w);
        c.set(w);
    });
}

/// [V-01] 拷贝一个数组的**全部**元素 —— 计数点。
///
/// ⚠ **语义与 `Vec<Value>::clone` 逐字相同**(`to_vec()` = 逐元素克隆),
///   只是多记一笔「拷贝了多少个元素」。
pub fn copy_array_elements(src: &[Value]) -> Vec<Value> {
    // ⚠ `to_vec()` 与 `Vec<Value>::clone` 逐字同语义(逐元素克隆)。
    let out: Vec<Value> = src.to_vec();
    add_value_work(|w| w.array_elements_copied += out.len() as u64);
    out
}

/// [V-01] 拷贝**一个**数组元素 —— 计数点。
///
/// ⚠ 语义与 `Value::clone` 逐字相同(深拷贝)。
pub fn copy_array_element(src: &Value) -> Value {
    let out = src.clone();
    add_value_work(|w| w.array_elements_copied += 1);
    out
}

/// [V-01] 拼接两个字符串 —— 计数点(记**拷贝的字节数**)。
///
/// ⚠ **结果与 `format!("{}{}", a, b)` 逐字相同**。
pub fn concat_strings(a: &str, b: &str) -> String {
    let mut out = String::with_capacity(a.len() + b.len());
    out.push_str(a);
    out.push_str(b);
    add_value_work(|w| w.string_bytes_copied += (a.len() + b.len()) as u64);
    out
}

/// [项目扩展 / G-17] 记一次「字符串**就地**追加拷了多少字节」—— 计数点。
///
/// ⚠ 与 [`concat_strings`] **同款口径**（记拷贝的字节数）⇒ 累加器形态下
///   循环 n 次只拷 O(n) 字节 ⇒ 同一个计数器就能把「优化前 O(n²) / 优化后 O(n)」
///   这条差异**量出来**。
pub fn add_string_bytes_copied(n: u64) {
    add_value_work(|w| w.string_bytes_copied += n);
}

/// [V-01] 把字符串拆成**码点数组** —— 计数点(记**展开的字符个数**)。
///
/// ⚠ **结果与逐字符 `Value::Integer(c as i64)` 逐字相同**。
pub fn string_codepoints(s: &str) -> Vec<Value> {
    let out: Vec<Value> = s.chars().map(|c| Value::Integer(c as i64)).collect();
    add_value_work(|w| w.string_codepoints_expanded += out.len() as u64);
    out
}

/// [V-01] 记一次「`CALL` 算子调用了闭包」—— 计数点。
///
/// ⚠ **只有 `CALL` 算子**(`CALL(f, …)`)走这里 ⇒ 在 `SORT` 的实现路径上,
///   **比较器正是经由 `CALL` 被调用**(门面 `collection.wll` 的 `_SORT_LT` 里
///   唯一的 `CALL(cmp, a, b)`)⇒ 本计数在 `SORT` 路径上**就是**比较器调用次数。
///
/// ⚠ 这**不是**「`SORT` 被调用了几次」那种调用点计数:比较器**每被调用一次**
///   就多记一笔 ⇒ 排序内部改成别的比较策略(比数变多/变少)都会立刻被看见。
pub fn note_closure_call() {
    add_value_work(|w| w.comparator_calls += 1);
}

/// [V-01] 记一次「字符串 vs 字符串」的值比较 —— 计数点。
///
/// ⚠ 计数点在**值比较原语**里(不是调用点)⇒ 排序内部换实现 ⇒ 比数变。
pub fn note_string_comparison() {
    add_value_work(|w| w.string_comparisons += 1);
}

// ⚠ `clippy::mutable_key_type` 在这里报「键有内部可变性」—— **本设计里它是假阳性**:
//
//   `DictKey::Instance` 持有 `Rc<RefCell<Vec<(Value, Value)>>>`(`fields` 单元格),
//   clippy 看到 `RefCell` 就认为「键的哈希/相等可能变」。但本设计里**变的是
//   实例的字段内容,不是键的身份**:
//
//   * 哈希 = `fields` 单元格的**分配地址**(`Rc::as_ptr`)—— 分配不随字段变化;
//   * 相等 = `Rc::ptr_eq` —— 比的也是同一个分配地址。
//
//   ⇒ 哈希与相等**都稳定** ⇒ `HashMap` 的不变量成立。
//   且 `DictKey` **持有**那个 `Rc` ⇒ 分配在被哈希期间不会消失 ⇒ 不存在
//   「地址被复用 ⇒ 错命中」的问题(若只存裸地址,那才会)。
#[allow(clippy::mutable_key_type)]
impl DictInner {
    /// 新建一个字典内部结构:按**插入序**建索引。
    ///
    /// ⚠ 重复键(字典字面量可以写 `["a": 1, "a": 2]`)⇒ 索引里存的是**位置
    ///   列表**,而 [`DictInner::lookup`] 取**首个**(= 首次出现的位置),
    ///   与旧实现「线性扫描取第一个匹配」逐字一致。
    pub fn new(entries: Vec<(Value, Value)>) -> Self {
        let mut index: HashMap<DictKey, Vec<usize>> = HashMap::with_capacity(entries.len());
        let mut all_keys_in_domain = true;
        for (pos, (k, _)) in entries.iter().enumerate() {
            match DictKey::from_value(k) {
                Some(dk) => {
                    // ⚠ **位置列表**:重复键 ⇒ 同一键有多个位置(见 `index` 的说明)。
                    index.entry(dk).or_default().push(pos);
                }
                None => {
                    all_keys_in_domain = false;
                    break;
                }
            }
        }
        if !all_keys_in_domain {
            index.clear();
        }
        DictInner {
            entries,
            index,
            all_keys_in_domain,
        }
    }

    /// 插入序的键值序列(**只读**)。
    pub fn entries(&self) -> &[(Value, Value)] {
        &self.entries
    }

    /// 把内部结构物化成字典值(内部结构 → `Value` 的**唯一**出口)。
    pub fn into_value(self) -> Value {
        Value::Dict(Rc::new(self))
    }

    /// 插入序条目的迭代器(**只读**)。
    pub fn iter(&self) -> std::slice::Iter<'_, (Value, Value)> {
        self.entries.iter()
    }

    /// 键数(**按条目数**,含重复键 —— 与旧的 `Vec::len()` 逐字一致)。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否没有条目。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按键查**位置**(规范 §18.3.1 的 O(1) 期望路径)。
    ///
    /// ⚠ **与旧的线性扫描逐字等价**:
    ///   * 键在封闭域内、且本字典**全部**键都在封闭域内 ⇒ 走哈希索引;
    ///   * 否则(探测键域外,或本字典含域外键)⇒ 退回
    ///     `values_equal` 的线性扫描(旧实现的原样)。
    pub fn lookup(&self, key: &Value) -> Option<usize> {
        if self.all_keys_in_domain {
            if let Some(dk) = DictKey::from_value(key) {
                // [ADR-0035 S5] 哈希路径:1 次键比较(哈希命中后那一次 `Eq`)。
                add_dict_work(|w| {
                    w.comparisons += 1;
                    w.indexed_lookups += 1;
                });
                // ⚠ **首个**位置 = 首次出现(与旧的线性扫描「取第一个匹配」逐字一致)。
                return self.index.get(&dk).and_then(|ps| ps.first().copied());
            }
        }
        // 线性回退:按**实际扫描过**的条目数计比较次数(⇒ 守卫能看出退化)。
        let mut scanned = 0u64;
        let found = self.entries.iter().position(|(k, _)| {
            scanned += 1;
            values_equal(k, key)
        });
        add_dict_work(|w| {
            w.comparisons += scanned;
            w.linear_lookups += 1;
        });
        found
    }

    /// 按键取值(等价于 `lookup` + 取那一格的值)。
    pub fn get(&self, key: &Value) -> Option<&Value> {
        self.lookup(key).map(|i| &self.entries[i].1)
    }

    /// 就地改某一格的**值**(不改结构 ⇒ 索引不受影响)。
    pub fn set_value(&mut self, pos: usize, value: Value) {
        self.entries[pos].1 = value;
    }

    /// 取某一格**值**的可变引用(用于「下潜进嵌套容器」;不改键 ⇒ 索引不受影响)。
    pub fn value_mut(&mut self, pos: usize) -> &mut Value {
        &mut self.entries[pos].1
    }

    /// 在**末尾**追加一个新键(规范 §18.3.1:新键追加到末尾 = 插入序)。
    ///
    /// ⚠ 键已存在时**不**覆盖(调用方先用 [`DictInner::lookup`] 判过);
    ///   索引同步更新。
    pub fn insert(&mut self, key: Value, value: Value) {
        let pos = self.entries.len();
        self.entries.push((key, value));
        if self.all_keys_in_domain {
            match DictKey::from_value(&self.entries[pos].0) {
                Some(dk) => {
                    // ⚠ **位置列表**:重复键 ⇒ 同一键有多个位置(见 `index` 的说明)。
                    self.index.entry(dk).or_default().push(pos);
                    // [ADR-0035 S5] 索引插入:1 次键比较(哈希桶里的那一次 `Eq`)。
                    add_dict_work(|w| w.comparisons += 1);
                }
                None => {
                    // 域外键 ⇒ 本字典不再「全部键在域内」⇒ 索引作废。
                    self.all_keys_in_domain = false;
                    self.index.clear();
                }
            }
        }
    }

    /// 删掉某一格(位置会变 ⇒ 索引重建)。
    pub fn remove(&mut self, pos: usize) -> (Value, Value) {
        let removed = self.entries.remove(pos);
        self.reindex();
        removed
    }

    /// 按谓词保留条目(删掉的项会让位置变 ⇒ 索引重建)。
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&(Value, Value)) -> bool,
    {
        let before = self.entries.len();
        self.entries.retain(|kv| f(kv));
        if self.entries.len() != before {
            self.reindex();
        }
    }

    /// 重建索引(结构变动之后调用)。
    fn reindex(&mut self) {
        // [ADR-0035 S5] 索引重建是一次 O(n) 的结构工作 —— 计数上单列一项
        // (它是**重建**,不是键比较)。
        add_dict_work(|w| w.index_rebuilds += 1);
        self.index.clear();
        self.all_keys_in_domain = true;
        for (pos, (k, _)) in self.entries.iter().enumerate() {
            match DictKey::from_value(k) {
                Some(dk) => {
                    // ⚠ **位置列表**:重复键 ⇒ 同一键有多个位置(见 `index` 的说明)。
                    self.index.entry(dk).or_default().push(pos);
                }
                None => {
                    self.all_keys_in_domain = false;
                    break;
                }
            }
        }
        if !self.all_keys_in_domain {
            self.index.clear();
        }
    }
}

impl PartialEq for DictInner {
    fn eq(&self, other: &Self) -> bool {
        // ⚠ **只比 `entries`**(插入序、逐元素)⇒ 与旧的
        //   `Vec<(Value, Value)> == Vec<(Value, Value)>` 逐字一致。
        //   索引是**派生**数据,不比。
        self.entries == other.entries
    }
}
/// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Class entry — the
/// shared backing store for `Value::Class` and `Value::Instance`.
/// All fields are public + `Clone`-able so the `Rc<RefCell<_>>`
/// wrapper can be cheaply cloned through `Env::clone` (closure
/// capture). Mutation goes through `RefCell::borrow_mut`; cycle
/// detection in the parent chain lives in `builtin_class` /
/// `builtin_new` (Step 9a-2) — see `E0050` ("class inheritance
/// chain error") for the canonical error path.
///
/// `init` is the optional constructor closure (first parameter
/// named `self` by spec §13 convention); Step 9a-2 populates it
/// when the user passes a closure inside the members array.
#[derive(Debug, Clone)]
pub struct ClassEntry {
    /// Optional class name (user-supplied via the first arg of
    /// `CLASS`). `None` means an anonymous class — useful for
    /// one-shot closures but otherwise discouraged by §13.
    pub name: Option<String>,
    /// Optional parent class (second arg of `CLASS`). Stored via
    /// `Rc<RefCell<_>>` so a class with a parent can outlive its
    /// parent's scope (e.g. when both are top-level lets in the
    /// same module).
    pub parent: Option<std::rc::Rc<std::cell::RefCell<ClassEntry>>>,
    /// Member table: `(name → Value::Closure)` for methods, or
    /// `(name → any)` for static fields. The spec §13 convention
    /// is "first parameter named `self` = instance method; no
    /// `self` parameter = static". Step 9a-4 / 9a-5 walk this
    /// table for method / property dispatch.
    pub members: Vec<(Value, Value)>,
    /// Optional constructor closure. `builtin_new` (Step 9a-2)
    /// reads the first parameter (must be named `self` per §13
    /// convention) and binds it to the freshly-allocated instance.
    pub init: Option<Value>,
    /// [v0.9 Step 9b / plan §4.4.3 / ADR-0019 §14] Session-type
    /// protocol constraining method-call order. `None` = unrestricted.
    pub protocol: Option<Proto>,
}

/// [v0.9 Step 9a-3 / plan §4.3 / ADR-0019 §4.3] Linear `THIS`
/// capability (runtime check).
///
/// `ThisToken` is the per-instance bookkeeping for the spec §15
/// "linear capability" rule: each method body has exactly ONE
/// opportunity to call `THIS`. The first call flips `moved`
/// from `false` to `true`; the second call observes `moved ==
/// true` and raises E0095 ("linear value used after move").
///
/// This is the v0.9.0 minimal "linear" model — stronger than
/// "freely aliasable" but weaker than Rust's compile-time
/// linear typing. The runtime cost is one bool flip per
/// method invocation; spec §15 leaves stronger enforcement
/// (move / discard tracking) as a v0.9.1+ extension.
#[derive(Debug, Clone)]
pub struct ThisToken {
    /// `false` until the first `THIS()` read inside the current
    /// method body, then `true` for the rest of that body's
    /// lifetime. Reset to `false` when the body returns (the
    /// owning instance outlives any single method call, so a
    /// later method invocation on the same instance starts
    /// fresh).
    pub moved: bool,
}

impl ThisToken {
    /// [v0.9 Step 9a-3] Consume the capability: succeed (and
    /// flip `moved = true`) on the first call, fail on any
    /// subsequent call within the same method body. Used by
    /// `builtin_this` to gate the `moved` flag atomically
    /// under the `RefCell::borrow_mut`.
    ///
    /// Returns `Ok(())` for the first call (caller may now
    /// bind / forward the linear reference), `Err(AlreadyMoved)`
    /// for any subsequent call (caller raises E0095 with the
    /// `AlreadyMoved` variant as the source of truth — the
    /// `Result<(), ()>` shape would otherwise trip clippy's
    /// `result_unit_err` lint, and using a named enum keeps
    /// the future direction open if 9a-4+ adds more failure
    /// modes to the linear capability contract).
    pub fn try_consume(&mut self) -> Result<(), ThisTokenError> {
        if self.moved {
            Err(ThisTokenError::AlreadyMoved)
        } else {
            self.moved = true;
            Ok(())
        }
    }
}

/// [v0.9 Step 9a-3] Failure modes for `ThisToken::try_consume`.
/// Currently a single variant (`AlreadyMoved`); the enum
/// exists so the `try_consume` return type isn't
/// `Result<(), ()>` (which clippy flags) and to leave room for
/// additional linear-capability violations in 9a-4 / 9a-5
/// (e.g. `ReadAfterDiscard` for E0096, which is reserved but
/// not yet emitted by the runtime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThisTokenError {
    /// First `THIS()` call within the method body already
    /// consumed the capability; subsequent reads raise E0095.
    AlreadyMoved,
}

/// [v0.9 P1-M2] Shared fields table of one `Value::Instance`.
pub type LinearThis = std::rc::Rc<std::cell::RefCell<Vec<(Value, Value)>>>;

impl Value {
    // ──────────────────────────────────────────────────────────────────
    // [ADR-0035 S2] 字典的**构造函数**与**访问器**。
    //
    // ⚠ 全仓的字典构造 / 读取**只经这几个入口**(ADR-0035 S2 / S3):
    //   内部表示(`Rc<DictInner>` + 哈希索引)不进调用方 ⇒ 175 处替换
    //   不会把「表示」漏到别处去。
    // ──────────────────────────────────────────────────────────────────

    /// [ADR-0035 S2] 构造一个字典值(键值对按**插入序**排列)。
    pub fn dict(entries: Vec<(Value, Value)>) -> Value {
        Value::Dict(Rc::new(DictInner::new(entries)))
    }

    /// [ADR-0035 S2] 取字典的**插入序**条目(只读;`None` = 不是字典)。
    pub fn as_dict_entries(&self) -> Option<&[(Value, Value)]> {
        match self {
            Value::Dict(inner) => Some(inner.entries()),
            _ => None,
        }
    }

    /// [ADR-0035 S2] 按键取**位置**(哈希索引;域外键退回线性扫描)。
    pub fn dict_index_of(&self, key: &Value) -> Option<usize> {
        match self {
            Value::Dict(inner) => inner.lookup(key),
            _ => None,
        }
    }

    /// [ADR-0035 S2] 按键取值(哈希索引;域外键退回线性扫描)。
    pub fn dict_get(&self, key: &Value) -> Option<&Value> {
        match self {
            Value::Dict(inner) => inner.get(key),
            _ => None,
        }
    }

    /// [ADR-0035 S2 / D3] 取字典的**可变**内部结构 —— **copy-on-write**。
    ///
    /// ⚠ 字典**唯一拥有**时(强引用计数 == 1)⇒ 原地改 ⇒ 单次写 O(1);
    ///   被别名时 ⇒ 先克隆再改 ⇒ O(n)。判据由 `Rc::make_mut` 的引用计数
    ///   严格给出(规范 §19.1 的值语义:旧值不可观察时原地改才合法)。
    pub fn as_dict_inner_mut(&mut self) -> Option<&mut DictInner> {
        match self {
            Value::Dict(inner) => Some(dict_make_mut(inner)),
            _ => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Value::Integer(v) => v.to_string(),
            Value::Float(v) => {
                if v.fract() == 0.0 {
                    format!("{:.1}", v)
                } else {
                    v.to_string()
                }
            }
            Value::String(s) => s.clone(),
            // ⚠ [G-17] `CellStr` 与 `String` **必须渲染完全一样** —— 渲染是可观察面。
            Value::CellStr(c) => c.to_string(),
            Value::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            Value::Null => "NULL".to_string(),
            Value::Array(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.display()).collect();
                format!("[{}]", parts.join(", "))
            }
            // ⚠ `Cell` 与 `Array` **必须渲染完全一样** —— 渲染是可观察面。
            Value::Cell(c) => c.to_string(),
            Value::Dict(entries) => {
                let parts: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.display(), v.display()))
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            // ⚠ [ADR-0035 D4] `DictCell` 与 `Dict` **必须渲染完全一样** ——
            // 渲染是可观察面(§2.5)。逐字照抄 `Dict` 那一支的格式。
            Value::DictCell(cell) => {
                let inner = cell.inner_rc();
                let parts: Vec<String> = inner
                    .entries()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.display(), v.display()))
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Closure { params, .. } => {
                format!(
                    "<fun({})>",
                    params
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Value::NativeFn { name, .. } => {
                format!("<native fun {}>", name)
            }
            Value::Ok(v) => format!("OK({})", v.display()),
            Value::Err(v) => format!("ERR({})", v.display()),
            // [v0.7 Phase C2] AWAIT (C3) dereferences a TaskHandle;
            // until then, displaying one shows the underlying id
            // + generation so users can recognise stale handles
            // (the v0.6 §3.x trait 'use-after-cancel' analogue).
            Value::TaskHandle(h) => format!("<task handle id={} gen={}>", h.id.0, h.generation),
            // [v0.7 Phase D-A] Channel handles display the same way
            // as task handles for symmetry; the runtime can
            // distinguish via type_name at type-check time.
            Value::ChannelHandle(h) => {
                format!("<channel handle id={} gen={}>", h.id.0, h.generation)
            }
            // [v0.9 Step 9a-1 / plan §4.3] Class display uses the
            // optional user-supplied name when present, otherwise
            // `<class>` for anonymous classes. The address-style
            // disambiguation (`@0x...`) is intentionally omitted
            // because class identity is `Rc::ptr_eq`-based and
            // raw pointer addresses aren't stable across
            // pretty-print passes; the spec §13 prose leaves the
            // exact format open ("the runtime chooses a self-
            // describing label").
            Value::Class(entry) => {
                let b = entry.borrow();
                match &b.name {
                    Some(n) => format!("<class {}>", n),
                    None => "<class>".to_string(),
                }
            }
            // [v0.9 Step 9a-1 / plan §4.3] Instance display shows
            // the class name (when present) plus the field count
            // so two NEW() calls of the same class print distinctly
            // — the field count isn't a stable identity (mutating
            // via SET_PROP doesn't change the display label) but
            // it gives the user a hint at the instance's shape.
            Value::Instance {
                class,
                fields,
                this_token: _,
                protocol_state: _,
            } => {
                let b = class.borrow();
                let label = match &b.name {
                    Some(n) => format!("<{} instance>", n),
                    None => "<instance>".to_string(),
                };
                format!("{}[{} fields]", label, fields.borrow().len())
            }
        }
    }
}

// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Manual `PartialEq`
// for `Value`. The `#[derive(PartialEq)]` is intentionally NOT used
// (see top-level enum doc) because the new `Class` / `Instance`
// variants wrap `Rc<RefCell<_>>` which doesn't auto-derive
// `PartialEq`. Existing variants keep their original semantics:
// integers / floats / strings / bools by value; arrays / dicts
// element-wise; closures / native-fns by field-wise closure
// equality (params + body + env); result wrappers by inner value;
// handles by their `id` + `generation` fields.
//
// New variants are compared by **identity**:
//   * `Value::Class(a) == Value::Class(b)` iff the two `Rc` point
//     at the same allocation (`Rc::ptr_eq`). Two CLASS() calls in
//     the same scope that produce different classes are NOT equal,
//     even when their members line up — class identity is the
//     user-meaningful comparison (`==` on classes is rare; spec
//     §13 leaves it open).
//   * `Value::Instance` identity is the per-instance state cell
//     (`fields` `Rc::ptr_eq`). Two `NEW()` calls from the same
//     class are distinct objects even when field values line up;
//     `Value::clone` shares the same `fields` allocation and so
//     compares equal (alias of the same object).

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            // ⚠ [G-17] `CellStr` 的相等性：与 `String` **逐字同口径**（按内容比）。
            //   ⚠ **先 `ptr_eq` 短路** —— 同一 cell 时直接 true，**不去 borrow**
            //   （否则「同一 cell 上借出比对」会撞 `RefCell` 的运行期借用检查）。
            (Value::CellStr(a), Value::CellStr(b)) => a.ptr_eq(b) || a.as_str() == b.as_str(),
            (Value::String(a), Value::CellStr(b)) => a.as_str() == b.as_str(),
            (Value::CellStr(a), Value::String(b)) => a.as_str() == b.as_str(),
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            // ⚠ [ADR-0035] 字典比较**只比插入序条目**(逐元素、顺序敏感),
            //   与旧的 `Vec<(Value, Value)> == Vec<(Value, Value)>` 逐字一致;
            //   哈希索引是派生数据,不参与比较。
            (Value::Dict(a), Value::Dict(b)) => a.entries() == b.entries(),
            // ⚠ [ADR-0035 D4] `DictCell` 的 `PartialEq` 与 `Dict` **逐字同口径**
            //   (插入序、逐元素、**顺序敏感**)—— 与 `values_equal` 的集合口径
            //   是**两件事**,两种口径都原样保留(`ADR-0035` 修订记录「新事实三」)。
            (Value::DictCell(a), Value::DictCell(b)) => {
                a.ptr_eq(b) || {
                    let (ai, bi) = (a.inner_rc(), b.inner_rc());
                    ai.entries() == bi.entries()
                }
            }
            (Value::Dict(a), Value::DictCell(b)) => {
                let bi = b.inner_rc();
                a.entries() == bi.entries()
            }
            (Value::DictCell(a), Value::Dict(b)) => {
                let ai = a.inner_rc();
                ai.entries() == b.entries()
            }
            (
                Value::Closure {
                    params: pa,
                    body: ba,
                    env: ea,
                },
                Value::Closure {
                    params: pb,
                    body: bb,
                    env: eb,
                },
            ) => pa == pb && ba == bb && ea == eb,
            (Value::NativeFn { name: na, .. }, Value::NativeFn { name: nb, .. }) => na == nb,
            (Value::Ok(a), Value::Ok(b)) => a == b,
            (Value::Err(a), Value::Err(b)) => a == b,
            (Value::TaskHandle(a), Value::TaskHandle(b)) => a == b,
            (Value::ChannelHandle(a), Value::ChannelHandle(b)) => a == b,
            (Value::Class(a), Value::Class(b)) => std::rc::Rc::ptr_eq(a, b),
            (
                Value::Instance {
                    class: _ca,
                    fields: fa,
                    this_token: _ta,
                    protocol_state: _pa,
                },
                Value::Instance {
                    class: _cb,
                    fields: fb,
                    this_token: _tb,
                    protocol_state: _pb,
                },
            ) => std::rc::Rc::ptr_eq(fa, fb),
            _ => false,
        }
    }
}

impl From<Literal> for Value {
    fn from(l: Literal) -> Self {
        match l {
            Literal::Integer(v) => Value::Integer(v),
            Literal::Float(v) => Value::Float(v),
            Literal::String(s) => Value::String(s),
            Literal::Boolean(b) => Value::Boolean(b),
            Literal::Null => Value::Null,
            // v0.6 §1.8: a literal interpolation cannot appear in a
            // pure `Literal → Value` conversion (interpolation requires
            // evaluating inner expressions, which lives outside
            // `From`). The parser keeps interpolation in a dedicated
            // `Expr::Literal(Interpolated)` AST node that the
            // evaluator handles separately. Map it to a temporary
            // sentinel so the eval layer doesn't accidentally render
            // it as an empty string; full evaluation is implemented
            // in batch 2.
            Literal::Interpolated(_) => Value::String(String::new()),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Lexical environment (chain of scopes; v0.3 §6.3)
// ──────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────
// 未消费 `ERR` 状态(v1.0 规范 §8.6)
// ──────────────────────────────────────────────────────────────────────

/// [v1.0 规范 §8.6] **未消费 `ERR` 状态的集合**。
///
/// 元素是**那个 `ERR` 值本身**(`Value::Err(_)`),按**出现序**排列。
///
/// ⚠ 未消费状态是**集合**,不是单个标志 —— 一个绑定 / 一个容器 / 一个数组
/// 元素序列都可以**同时持有多个**未消费的 `ERR`。
pub type UnconsumedErrs = Vec<Value>;

/// [v1.0 规范 §8.6] **并集** —— 多个未消费 `ERR` 汇入同一接收位置时
/// **全部保留**。
///
/// **「并集」在此发生**:接收位置(绑定 / 容器 / 语句值)用它把各来源的集合
/// 合成**一个**;**不是**「保留第一个」,也**不是**「后者覆盖前者」。
///
/// ⚠ **多重集口径,不去重**:身份判据(`PartialEq`)要到 X-06 第二步才有
/// ⇒ 去重会把两个**内容相同、来源不同**的 `ERR` 合成一个 ⇒ **漏报**。
/// 这里保留**全部元素**(含重复),让丢弃点能**逐个**报告。
pub fn union_unconsumed(base: &[Value], extra: &[Value]) -> Vec<Value> {
    let mut out = Vec::with_capacity(base.len() + extra.len());
    out.extend_from_slice(base);
    out.extend_from_slice(extra);
    out
}

/// [v1.0 规范 §8.6] **跨槽差集** —— 把「成员手上」那份**按实参分槽**的集合里
/// 与 `minus` **按内容**相等的元素逐个扣掉(**就地**改写 `base`)。
///
/// ⚠ **为什么是「跨槽」**:成员手上那份是**每个实参一槽**
/// (`StdHost::arg_tokens()` 的返回形状),而成员**转给回调**的那批集合是
/// **按回调实参分槽**的 ⇒ 两者**槽号不对应**(`MAP` 把 arg0 的元素分发给多个
/// 回调实参)⇒ 按槽号扣会扣错地方。
///
/// ⇒ 本函数**不看槽号**,只看**内容**:对 `minus` 的每个元素,在**任意一槽**里
/// 找到**第一个**内容相等的元素并扣掉(扣一处即止)⇒ 与处置册 §6.1 的「按
/// **内容**做多重集差集」同口径 ⇒ 也与 §8.6.5 的「值语义即新身份」一致。
///
/// ⚠ **多重集口径**:`minus` 里有几个就扣几个 ⇒ `base` 里的重复元素也逐个扣
/// (两个内容相同、来源不同的 `ERR` 占两个元素 ⇒ 扣一个还剩一个)。
///
/// **用途**:标准库成员把实参集合**一部分**转给自己的回调时,要把它**转出去的
/// 那部分**从「成员手上」扣掉 —— 剩下的才是「成员没转出去、该转给调用结果」的
/// 那部分(见一致性册 §5.4 的 X-20 与处置册 §6.1 的分账规则)。
pub fn subtract_unconsumed_across(base: &mut [UnconsumedErrs], minus: &[Value]) {
    for m in minus {
        for slot in base.iter_mut() {
            if let Some(idx) = slot.iter().position(|v| v == m) {
                slot.remove(idx);
                break;
            }
        }
    }
}

/// [v1.0 规范 §8.6] 在 `set` 里找**第一个**与 `value` 相等的元素下标。
///
/// 只在 `value` **本身就是** `Value::Err(_)` 时才找 —— 取走型消费者
/// 「取走它拿到的那一个」只对 `ERR` 成立:值是容器时它拿到的是**容器**,
/// 不是某个 `ERR` ⇒ 集合不变。
///
/// ⚠ **身份局限**:按 `PartialEq` 匹配 ⇒ 内容相同但来源不同的多个 `ERR`
/// 里**命中哪一个是不确定的**(见模块文档的「已知局限」)。
pub fn find_unconsumed_index(set: &[Value], value: &Value) -> Option<usize> {
    if !matches!(value, Value::Err(_)) {
        return None;
    }
    set.iter().position(|v| v == value)
}

/// Cell payload: a single binding's value plus its mutability flag.
///
/// `mutable` is `false` (IMMUTABLE) when the cell is first created by a
/// `LET` (v0.4 spec 搂6.4 cell model). It is upgraded to `true` when
/// the binding is captured by a closure, per the formal rule
/// `E-CloCap` in 附录 E.4.6:
///
///   "闭包捕获的 cell 全部升级为 MUTABLE"
///
/// `SET` checks the flag and raises E0024 if the cell is still
/// IMMUTABLE.
#[derive(Debug, Clone)]

pub struct Binding {
    pub value: Value,
    pub mutable: bool,
    /// [v1.0 spec §8.6] **未消费 `ERR` 状态的集合**。
    ///
    /// ⚠ 状态挂在**绑定**上,不挂在值上(D-01):`Value` 是 `#[derive(Clone)]`
    /// 的普通枚举,数组 / 字典按值深拷贝 ⇒ 「这个 `ERR` 已被消费」若挂在
    /// 值上,`LET(a, err); LET(b, a);` 之后消费 `b` 清不掉 `a` 的副本 ⇒ 误报。
    ///
    /// ⚠ 未消费状态是**集合**,不是单个标志:一个绑定可以**同时持有多个**
    /// 未消费的 `ERR`(元素是那些 `ERR` 值本身,按出现序)。
    ///
    /// **每个 `ERR` 在任一时刻仍只有一个持有者**:按 §8.6.3 的转发规则在读取
    /// 时转移(Transfer 整组转走),被取走型消费者取走其中**它拿到的那一个**,
    /// 或被显式丢弃即销毁 ⇒ §8.6.5「至多报告一次」由转移语义天然保证。
    ///
    /// ⚠ **身份局限**:元素是**值** ⇒ 两个内容相同但来源不同的 `ERR` 无法
    /// 区分(见模块文档的「已知局限」)。
    pub unconsumed: UnconsumedErrs,
    /// [审计 F1] 该绑定**建立处**的源位置(`LET` 的值表达式 / 形参对应的调用点)。
    ///
    /// 作用域结束的 `E0103` 报在这里 —— 比「整个作用域块的 span」精确:
    /// 后者总是落在**首句**上,当令牌来自后面的 `LET` 时会指到无关语句。
    pub origin_span: Option<Span>,
}

/// A heap-allocated cell. Cloning the `Rc` is cheap and is what
/// `Env::clone` does when capturing into a closure's environment --
/// this is exactly the spec's "闭包捕获 cell 引用" rule (single layer,
/// no chain; multiple closures sharing the same lexical `LET` see the
/// same cell).
pub type Cell = Rc<RefCell<Binding>>;

/// Make a new immutable cell wrapping `value`.
pub fn new_cell(value: Value) -> Cell {
    Rc::new(RefCell::new(Binding {
        value,
        mutable: false,
        unconsumed: Vec::new(),
        origin_span: None,
    }))
}

/// [v1.0 spec §8.6.3] **读绑定的三种模式**(外加 [`ConsumeCarry`](ReadMode::ConsumeCarry)
/// 这个 [`Consume`](ReadMode::Consume) 的**变体**)。
///
/// 规范把「读入某个位置」分成三类,每类对**源绑定**的未消费状态与**在途值**
/// 的未消费状态有不同影响:
///
/// | 模式 | 何时 | 源绑定的集合 | 在途值的集合 |
/// |---|---|---|---|
/// | [`Consume`](ReadMode::Consume) | 取走型消费者的实参 | 移除**它拿到的那一个** | 空 |
/// | [`ConsumeCarry`](ReadMode::ConsumeCarry) | **剥壳交还**的取走型消费者(`TRY` / `OR_DIE` 的 `OK` 分支) | 移除**它拿到的那一个** | **携带那一个**(仅 `OK(ERR(..))` 形态) |
/// | [`Observe`](ReadMode::Observe) | 观察型消费者的实参 | **保留**(整组不动) | 空 |
/// | [`Transfer`](ReadMode::Transfer) | 其余一切位置 | **清空**(整组转走) | **继承源集合** |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    /// 取走型:该 `ERR` 被取走,不再处于未消费状态(§8.6.1)。
    ///
    /// ⚠ 集合语义:**取走的是它拿到的那一个** —— 只有当读到的值**本身就是**
    /// 一个 `ERR` 且它在源集合里时,才把**它**从源集合里移除;值是容器等
    /// 其它情况时**集合不变**。
    Consume,
    /// [G-02 / v1.0 spec §8.6.5] **取走型 + 携带** —— [`Consume`](ReadMode::Consume)
    /// 的**变体**:取走的口径**逐字相同**,区别只在**在途值带不带那份集合**。
    ///
    /// ## 什么时候用
    ///
    /// **只有「剥掉 `OK` 外壳、把内层 `ERR` 当结果交还」的消费者**才用它:
    /// `TRY(OK(ERR(x)))` / `OR_DIE(OK(ERR(x)))`。
    ///
    /// ## 为什么
    ///
    /// 剥壳得到的内层 `ERR` 按 §8.6.5「**值语义即新身份**」是**新生的**
    /// `ERR` ⇒ 它**自己未消费** ⇒ 它的未消费状态必须**跟着结果继续**。
    /// 若不携带,它就**无处可寻** —— 源绑定已被取走(空)、在途值又不带 ⇒
    /// 那个 `ERR` **一条 `E0103` 都不报**(⇒ 假阴性)。
    ///
    /// ## 边界(不可放宽)
    ///
    /// * **只在 `OK(ERR(..))` 这一种形态上携带** —— 值**本身**是 `ERR` 时
    ///   (消费者要么处理它、要么让它逃逸成返回值)⇒ **不携带**。否则
    ///   `UNWRAP_OR(ERR(x), d)` 会让默认值 `d` 白白背一个未消费的 `ERR`
    ///   ⇒ **误报**。
    /// * 值是**容器**等其它形态 ⇒ **不携带**(同 [`Consume`](ReadMode::Consume))。
    ///
    /// ⚠ **不得**把这个模式用到「不剥壳」的取走型消费者上
    /// (`ERR_PAYLOAD` / `WRAP` / `EXPECT_ERR` / 内建调用路径的
    /// `UNWRAP` / `UNWRAP_OR`) —— 那些路径各有自己的结果承接机制
    /// (见求值器的 `attach_result_token`),两套机制叠加会**同一个 `ERR` 报两条**。
    ConsumeCarry,
    /// 观察型:只看不带走,未消费状态**留在被读取的绑定**(§8.6.1)。
    Observe,
    /// 转移:由接收方承担(§8.6.3「读入其它任何位置」)。
    ///
    /// 源绑定的集合**整体转走**(源绑定清空),在途值**携带**该集合。
    Transfer,
}

/// Lexical environment. v0.4 搂6.4: stores `HashMap<String, Cell>` so
/// that closures can share the same cell with the lexical scope that
/// defined the binding. Scope chain layout is unchanged from v0.3:
/// index 0 is the innermost scope; lookups walk from inside out.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub scopes: Vec<HashMap<String, Cell>>,
}

// Manual `PartialEq` because `Rc<RefCell<_>>` doesn't derive it well;
// we compare the *value* snapshot for tests that need it.
impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        if self.scopes.len() != other.scopes.len() {
            return false;
        }
        for (a, b) in self.scopes.iter().zip(other.scopes.iter()) {
            if a.len() != b.len() {
                return false;
            }
            for (k, va) in a {
                match b.get(k) {
                    Some(vb) => {
                        // Compare Rc pointer + value snapshot. We borrow
                        // immutably and compare the inner `Binding` via
                        // `borrow()` snapshots. If either is borrowed
                        // mutably elsewhere this would panic; tests
                        // only call this on quiescent envs.
                        let ba = va.borrow();
                        let bb = vb.borrow();
                        if ba.value != bb.value
                            || ba.mutable != bb.mutable
                            || ba.unconsumed != bb.unconsumed
                        {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
        true
    }
}

impl Env {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        // Never pop the global scope.
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Take the entire scope stack out of the Env, leaving it in an
    /// empty-but-valid state (a single empty scope so subsequent
    /// `get`/`set_local` calls don't panic). Used by the segmented
    /// task runner (Phase B5a-3 Path B) to install / save per-segment
    /// env state around mid-body suspension.
    pub fn take_scopes(&mut self) -> Vec<HashMap<String, Cell>> {
        std::mem::take(&mut self.scopes)
    }

    /// Replace the scope stack wholesale. The caller must pass at
    /// least one scope; if `new_scopes` is empty we re-seed with a
    /// single empty scope so the invariant holds.
    pub fn replace_scopes(&mut self, mut new_scopes: Vec<HashMap<String, Cell>>) {
        if new_scopes.is_empty() {
            new_scopes.push(HashMap::new());
        }
        self.scopes = new_scopes;
    }

    /// Walk the scope chain from innermost to outermost; return the
    /// first match as a borrow guard on the inner value. Used for
    /// variable reads. The returned `Ref` is tied to `&self`'s
    /// lifetime; callers typically `.clone()` the inner value or use
    /// the ref briefly within a single expression.
    ///
    /// v0.4 (搂6.4 cell model): the value lives behind an
    /// `Rc<RefCell<Binding>>`; we use `Ref::map` to project a
    /// `Ref<Value>` out of the cell borrow.
    pub fn get(&self, name: &str) -> Option<Ref<'_, Value>> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                return Some(Ref::map(cell.borrow(), |b| &b.value));
            }
        }
        None
    }

    /// Look up the cell (not just the value) for `name`. Used by `SET`
    /// to mutate the cell in place, and by the cell-upgrade path.
    pub fn get_cell(&self, name: &str) -> Option<Cell> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                return Some(cell.clone());
            }
        }
        None
    }

    /// Bind in the current (innermost) scope. The value is wrapped in a
    /// fresh IMMUTABLE cell (per v0.6 §3.1: `LET(name, value)` creates
    /// an immutable binding; use `LET MUT(name, value)` for a mutable
    /// one, via `set_local_mut`).
    pub fn set_local(&mut self, name: impl Into<String>, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), new_cell(value));
        }
    }

    /// [v1.0 spec §8.6.3] `LET` 建绑定时的**转发**:接收位置建立绑定 ⇒
    /// 未消费状态转给该绑定。
    ///
    /// `unconsumed` 是**已经并好的集合** —— **并集由调用方在接收位置用
    /// [`union_unconsumed`] 完成**(把各来源的集合合成一个再传进来);
    /// 本函数**不合并、不覆盖**:收下什么就是什么。
    pub fn set_local_with_token(
        &mut self,
        name: impl Into<String>,
        value: Value,
        unconsumed: UnconsumedErrs,
        origin_span: Option<Span>,
    ) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.into(),
                Rc::new(RefCell::new(Binding {
                    value,
                    mutable: false,
                    unconsumed,
                    origin_span,
                })),
            );
        }
    }

    /// [v1.0 spec §8.6.3] 同上,但建立**可变**绑定(`LET MUT`)。
    ///
    /// ⚠ 集合口径与 [`Env::set_local_with_token`] **逐字相同**(并集在调用方
    /// 那一侧用 [`union_unconsumed`] 完成)。
    pub fn set_local_mut_with_token(
        &mut self,
        name: impl Into<String>,
        value: Value,
        unconsumed: UnconsumedErrs,
        origin_span: Option<Span>,
    ) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.into(),
                Rc::new(RefCell::new(Binding {
                    value,
                    mutable: true,
                    unconsumed,
                    origin_span,
                })),
            );
        }
    }

    /// [ADR-0035 D4] **就地改写已有绑定**的值 —— 保持该绑定的**可变性**与
    /// **未消费令牌**不变。
    ///
    /// ⚠ 与 [`Env::set_local_mut`] 的区别:后者在**最内层作用域插入一个新
    ///   绑定**(且新绑定是**可变**的)⇒ 会**遮蔽**外层的同名绑定并**改掉它的
    ///   可变性**。规范 §3.1 说可变性在建绑定时定、**永不变** ⇒ 累加器形态的
    ///   「临时表示」必须走这里,不能走 `set_local_mut`。
    ///
    /// 返回 `false` = 该名字在**任何**作用域里都没有绑定。
    pub fn replace_binding_value(&self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                cell.borrow_mut().value = value;
                return true;
            }
        }
        false
    }

    /// [v1.0 spec §8.6.3] **按模式读一个绑定**。
    ///
    /// 返回 `(值, 在途值携带的集合)`。源绑定的集合按模式处置:
    ///
    /// * `Consume` ⇒ **取走它拿到的那一个**:读到的值**本身**是 `ERR`
    ///   (`Value::Err(_)`)时移除它;读到的值是 `OK(ERR(..))` 时移除**内层**
    ///   那个 `ERR`(取走型消费者剥掉 `OK` 外壳拿到的是内层那个);两种形态
    ///   都只在它**确实在源集合里**时才移除。值是容器等其它情况 ⇒
    ///   **集合不变**。在途值**永远不带**集合。
    ///
    ///   ⚠ `OK(ERR(..))` 这一支不可省:不移除 ⇒ 源绑定与结果值各留一份 ⇒
    ///   同一个 `ERR` 被报两次(违 §8.6.5「一个 `ERR` 至多被报告一次」)。
    /// * `Observe` ⇒ 源绑定的集合**保留**(整组不动),在途值**不带**集合。
    /// * `Transfer` ⇒ 源绑定的集合**整体转走**(源绑定清空),在途值
    ///   **携带**该集合。
    ///
    /// ⚠ **身份口径**:按 `PartialEq` 匹配 ⇒ 内容相同但来源不同的多个 `ERR` 里
    /// **移除哪一个是不确定的**。⚠ 但这**不可达**:一个绑定的集合只含多个
    /// 元素时,那个绑定的值是**容器**;容器形态**从不**被取走型消费者按元素
    /// 移除(§8.6.3「容器里还剩几个,不受它影响」)⇒ 实际被按元素移除的只有
    /// 单元素集合 ⇒ 行为确定。身份口径本身(值语义即新身份)见模块文档。
    pub fn read_binding(&self, name: &str, mode: ReadMode) -> Option<(Value, UnconsumedErrs)> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                let mut b = cell.borrow_mut();
                let value = b.value.clone();
                match mode {
                    ReadMode::Consume | ReadMode::ConsumeCarry => {
                        // 取走型消费者取走的是**它拿到的那一个**(§8.6)。
                        //
                        // ⚠ [X-06 第二步] 「拿到的那一个」有两种形态:值**本身**
                        // 是 `ERR`;或值是 `OK(ERR(..))` —— 消费者剥掉 `OK` 外壳
                        // 拿到的是**内层**那个 `ERR`(§8.6.1 的取走型名单里有
                        // `UNWRAP_OR` / `TRY` / `UNWRAP` / `WRAP` …)⇒ 两种形态
                        // 都要从源集合里移除**那一个**;否则源绑定与结果值各留
                        // 一份 ⇒ 同一个 `ERR` 被报两次(违 §8.6.5)。
                        //
                        // ⚠ 容器等其它形态**不动**:「取走型消费者取走的是它拿到
                        // 的那个 `ERR` —— 容器里还剩几个,不受它影响」(§8.6.3)。
                        //
                        // ⚠ [G-02] `peel` 标的是「消费者会**剥掉 `OK` 外壳**、
                        // 把**内层**那个 `ERR` 当结果交还」的那一支 —— 只有这一支
                        // 的内层 `ERR` 是**新生的**(§8.6.5)⇒ 只有这一支的未消费
                        // 状态要**随在途值继续**。
                        let (taken, peel) = match &value {
                            Value::Err(_) => (Some(value.clone()), false),
                            Value::Ok(inner) => match inner.as_ref() {
                                Value::Err(_) => (Some((**inner).clone()), true),
                                _ => (None, false),
                            },
                            _ => (None, false),
                        };
                        let mut carried: UnconsumedErrs = Vec::new();
                        if let Some(taken) = taken {
                            if let Some(idx) = find_unconsumed_index(&b.unconsumed, &taken) {
                                let removed = b.unconsumed.remove(idx);
                                // [G-02] `ConsumeCarry` **只在剥壳那一支携带** ——
                                // 值本身是 `ERR` 时消费者要么处理它、要么让它逃逸
                                // ⇒ 那份状态**已被处置** ⇒ 不携带(否则
                                // `UNWRAP_OR(ERR(x), d)` 会让 `d` 白背一个 ⇒ 误报)。
                                if peel && mode == ReadMode::ConsumeCarry {
                                    carried.push(removed);
                                }
                            }
                        }
                        // 在途值在 Consume 模式下**永远不带**集合;
                        // `ConsumeCarry` 只在剥壳那一支带**那一个**。
                        return Some((value, carried));
                    }
                    ReadMode::Observe => {
                        // 未消费状态留在被读取的绑定(§8.6.1 观察型)⇒ 集合
                        // **不清空**。
                        return Some((value, Vec::new()));
                    }
                    ReadMode::Transfer => {
                        // 集合**整体转走**:源绑定清空,在途值携带该集合。
                        let carried = std::mem::take(&mut b.unconsumed);
                        return Some((value, carried));
                    }
                }
            }
        }
        None
    }

    /// [v1.0 spec §8.6.2] 查某个名字当前单元格持有的**未消费 `ERR` 集合**
    /// (用于 `SET` 改写前的「绑定被覆盖」判定)。
    ///
    /// 返回 `None` = 该名字在**任何**作用域里都没有绑定;返回
    /// `Some(空集合)` = 该绑定存在但**没有**未消费的 `ERR`。
    pub fn cell_token(&self, name: &str) -> Option<UnconsumedErrs> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                return Some(cell.borrow().unconsumed.clone());
            }
        }
        None
    }

    /// [v1.0 spec §8.6.2] **最内层作用域**内持有未消费 `ERR` 的绑定。
    ///
    /// 返回 `(绑定名, 该绑定持有的未消费 `ERR` 集合)` —— 一个绑定可以持有
    /// **多个**,集合里是那些 `ERR` 值本身,按**出现序**。
    ///
    /// 作用域结束时(§8.6.2 第 3 项)逐**个**检查 ⇒ 一个集合里有几个 `ERR`
    /// 就产生几条 `E0103`。
    pub fn scope_unconsumed(&self) -> Vec<(String, UnconsumedErrs)> {
        self.scope_unconsumed_at(self.scopes.len().saturating_sub(1))
    }

    /// [v1.0 spec §8.6.2] 指定下标的作用域内持有未消费 `ERR` 的绑定。
    ///
    /// 下标 0 是**全局作用域**(模块顶层)—— 程序 / 模块结束时它是最后
    /// 一道「作用域结束时仍未消费」的检查点。
    ///
    /// ⚠ 集合内的次序 = **出现序**(§8.6「诊断按它们在容器中的出现序排列」);
    /// 绑定之间的次序按**名字排序**(见下)。
    pub fn scope_unconsumed_at(&self, index: usize) -> Vec<(String, UnconsumedErrs)> {
        let mut out = Vec::new();
        let Some(scope) = self.scopes.get(index) else {
            return out;
        };
        for (name, cell) in scope {
            let b = cell.borrow();
            if !b.unconsumed.is_empty() {
                out.push((name.clone(), b.unconsumed.clone()));
            }
        }
        // ⚠ **按名字排序**:作用域是 `HashMap`,迭代序不确定 ⇒ 同一份源码
        // 同一时刻报哪一个绑定会漂。规范 §11.1「诊断的顺序也是结构的一部分」
        // ⇒ 排序后再交给调用方挑第一个。
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// v0.6 §3.1: `LET MUT(name, value)` binds a mutable cell. The
    /// mutability flag is set at binding creation time and **never**
    /// changes — there is no closure-capture upgrade (removed from
    /// v0.6; the v0.4/v0.5 "first closure call upgrades cell" rule
    /// was deemed too magical and removed).
    pub fn set_local_mut(&mut self, name: impl Into<String>, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.into(),
                Rc::new(RefCell::new(Binding {
                    value,
                    mutable: true,
                    unconsumed: Vec::new(),
                    origin_span: None,
                })),
            );
        }
    }

    /// Set the cell value if the cell is `mutable` (per spec 搂6.4
    /// mutability rule). Returns:
    ///   * `Ok(true)`  -- cell found and updated
    ///   * `Ok(false)` -- cell found but IMMUTABLE (caller raises E0024)
    ///   * `Err(())`   -- cell not found at all (caller raises E0020)
    #[allow(clippy::result_unit_err)] // bool tri-state (Ok/Err-not-found) — see Phase A2 cell semantics
    pub fn set_cell_value(&self, name: &str, value: Value) -> Result<bool, ()> {
        self.set_cell_value_with_token(name, value, Vec::new())
    }

    /// [v1.0 spec §8.6.3] `SET` 改写单元格时的**转发**:新值若持有未消费
    /// `ERR`,未消费状态转给该单元格(§3.4 的说明:旧值若未被消费,改写即构成
    /// 一个丢弃点 —— 那一半由调用方在改写**之前**用 [`Env::cell_token`]
    /// 判定)。
    ///
    /// ⚠ 集合口径与 [`Env::set_local_with_token`] **逐字相同**:本函数收下
    /// 调用方**已经并好**的集合(并集在调用方那一侧用 [`union_unconsumed`]
    /// 完成),**不合并、不覆盖**。
    #[allow(clippy::result_unit_err)] // bool tri-state (Ok/Err-not-found) — see Phase A2 cell semantics
    pub fn set_cell_value_with_token(
        &self,
        name: &str,
        value: Value,
        unconsumed: UnconsumedErrs,
    ) -> Result<bool, ()> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                let mut b = cell.borrow_mut();
                if !b.mutable {
                    return Ok(false);
                }
                b.value = value;
                b.unconsumed = unconsumed;
                return Ok(true);
            }
        }
        Err(())
    }

    /// [v0.9 P1-M2] True when any binding in any scope matches `pred`.
    pub fn any_binding_matches(&self, pred: impl Fn(&Value) -> bool) -> bool {
        for scope in &self.scopes {
            for cell in scope.values() {
                if pred(&cell.borrow().value) {
                    return true;
                }
            }
        }
        false
    }

    /// Snapshot all currently-bound names (for module exports).
    pub fn names(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for scope in &self.scopes {
            for k in scope.keys() {
                out.insert(k.clone());
            }
        }
        out
    }
}

// ──────────────────────────────────────────────────────────────────────
// Control-flow signals

// ──────────────────────────────────────────────────────────────────────

/// Control-flow signal (separate from `Value`). Propagated up through
/// nested expressions and converted back to a value at the matching
/// frame boundary:
///   * `Return(v)`  — at a function call frame, becomes the function's
///                    return value; at top level, becomes E0102 if v is
///                    `Value::Err(_)`, otherwise the program's result.
///   * `Break` / `Continue` — at a loop frame, become loop control;
///                    outside any loop, become E0014.
///   * `Yield(reason)` — [v0.7 Phase B5a-3 slice 1] user-defined yield
///                    point. Propagates up the eval stack exactly like
///                    `Break` / `Continue` / `Return`; the
///                    state-machine entry point `Evaluator::step_once`
///                    translates it into
///                    `crate::runtime::StepResult::Yield`(现于 wlwl-eval), while the
///                    run-to-completion `Evaluator::eval` rejects it
///                    with E0014 (yield is only valid inside a
///                    scheduler step loop, not at a bare top-level
///                    call). No v0.6 builtin produces this variant, so
///                    v0.6 programs are observably unaffected.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    None,
    Break,
    Continue,
    Return(Value),
    Yield(YieldReason),
}

/// A single evaluation result: a value plus an optional control-flow
/// signal. `Err(...)` from this layer is a hard error (E0020, E0022,
/// E0030, E0100, etc.), not a value-level error.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub value: Value,
    pub signal: Signal,
    /// [v1.0 spec §8.6] **未消费 `ERR` 状态的集合**。
    ///
    /// 它随求值结果走:能穿过函数返回值、容器字面量、`IF` / `MATCH` 分支、
    /// `==` / `!=` 的透传。`Outcome::normal` 保持它为**空集合**;需要带集合
    /// 的地方用 [`Outcome::with_unconsumed`]。
    ///
    /// ⚠ 当 `signal` 是 [`Signal::Return`] 时,`value` 是 `NULL`,**有意义
    /// 的值在信号里** —— 此时本字段说的是**被返回的那个值**的集合。
    ///
    /// ⚠ **并集在接收位置发生**:多个来源的集合汇进同一个 `Outcome` 时,
    /// 调用方用 [`union_unconsumed`] 把它们**合成一个**(全部保留)。
    pub unconsumed: UnconsumedErrs,
}

impl Outcome {
    pub fn normal(v: Value) -> Self {
        Outcome {
            value: v,
            signal: Signal::None,
            unconsumed: Vec::new(),
        }
    }

    /// [v1.0 spec §8.6.3] 带一个**未消费 `ERR` 集合**的返回值。
    ///
    /// ⚠ 传入的是**已经并好的集合**(并集在接收位置由调用方用
    /// [`union_unconsumed`] 完成)—— 本函数**不合并、不覆盖**。
    pub fn with_unconsumed(v: Value, unconsumed: UnconsumedErrs) -> Self {
        Outcome {
            value: v,
            signal: Signal::None,
            unconsumed,
        }
    }

    /// 带一个**控制流信号**的返回值(如 `Signal::Return`)。
    ///
    /// ⚠️ 跨层调用方**必须**自己决定要不要把信号往上再传 —— `wlwl-std` 的
    /// 成员不得消费它(结构化并发,spec §17):`TIMEOUT` 就是这么用的,它
    /// **接收** body 逃上来的 `Signal::Return` 并把它**换成自己的返回值**。
    pub fn with_signal(v: Value, signal: Signal) -> Self {
        Outcome {
            value: v,
            signal,
            unconsumed: Vec::new(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Module loader (v0.3 §13 — Phase 4 batch 1 + batch 2)
// ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

/// Generation-tracked task handle returned to user code by `SPAWN`.
///
/// Detecting `use-after-cancel` / `use-after-recycle`: a slot can be
/// reused after a task finishes, but its generation bumps. A handle
/// that doesn't match the current generation is stale (E0053).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskHandle {
    pub id: TaskId,
    pub generation: u64,
}

/// [v0.7 Phase C2] 用户 `SPAWN`/`AWAIT`/通道阻塞时任务挂起的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YieldReason {
    /// User-invoked `YIELD()`. No wait condition; the task is
    /// immediately re-eligible (added back to `run_queue`).
    Explicit,
    /// `AWAIT(child)` and `child` has not finished yet. The task is
    /// parked until `child` transitions to `Done` or `Cancelled`,
    /// at which point `Scheduler::wake_dependents` re-enqueues it.
    AwaitingChild(TaskId),
    /// `CHANNEL_RECV(ch)` and `buf` is empty. The task is parked
    /// on `ch`'s receiver wait list until a sender wakes it.
    ReceivingOn(RcHandle),
    /// `CHANNEL_SEND(ch)` and `buf` is full. The task is parked on
    /// `ch`'s sender wait list until a receiver drains a slot.
    SendingOn(RcHandle),
    /// [项目扩展 / G-02] `TCP_RECV` / `TCP_ACCEPT` 暂时拿不到数据：任务挂起在
    /// 这个 socket id 上，直到该 socket 就绪。
    ///
    /// ⚠ **这是 `ReceivingOn` / `SendingOn` 的同族**：通道的等待是「等对端」，
    ///   socket 的等待是「等 I/O 就绪」—— 两者都是**可挂起**的等待。
    ///   差别只在**谁**来唤醒：通道由对端任务唤醒，socket 由**宿主侧的 I/O 线程**
    ///   通过就绪队列唤醒（见 `wlwl_std::tcp::take_ready`）。
    ///
    /// ⚠ 载荷是**不透明的 socket id**（`i64`）—— 本 crate 不能依赖 std 的类型
    ///   （依赖方向是 `eval → wlwl-value ← std`）。
    SocketReady(i64),
    /// [项目扩展 / G-08] `perform SleepUntil` —— 把当前任务**挂起**到某个
    /// 绝对单调钟毫秒。
    ///
    /// ⚠ 与 `SocketReady` 同族（都是**可挂起**的等待），差别只在唤醒方：
    ///   socket 由**宿主 I/O 线程**唤醒；睡眠由**调度器的定时器**唤醒。
    ///
    /// ⚠ 载荷是**绝对**毫秒（`wlwl_value::real_mono_ms` 域）—— 不用「还剩多久」
    ///   这种相对量：任务可能被别的理由唤醒过一轮，相对量会被重新算一遍。
    SleepUntil(i64),
}

impl YieldReason {
    /// v0.9 Step 10: derive the WasmFX-style [`Tag`] for this reason.
    /// Single source of truth used by `TaskState::Suspended` to
    /// keep `Suspended { tag, reason }` consistent.
    ///
    /// `TaskState` lives in `wlwl-eval::runtime`, i.e. **outside this
    /// crate** — which is why the mention above is plain code and not an
    /// intra-doc link, and why the variant is spelled `Suspended` (an
    /// earlier draft said `suspended`, a name that no longer exists).
    /// [v0.11 M2] `YieldReason` was moved here from `wlwl-eval`; the
    /// reference crossed the crate boundary with it and CI never ran on
    /// the wip branch, so `cargo doc -D warnings` stayed broken until
    /// the branch was added to the trigger list.
    ///
    /// Mapping (plan §3.1 + ADR-0017 §3.1 + ADR-0019 §4.4.1):
    /// - `Explicit` / `AwaitingChild` — the effect is `perform Yield`;
    /// - `ReceivingOn` / `SendingOn` — the effect is
    ///   `perform ChannelOp` (synchronous channel SEND/RECV).
    pub fn tag(self) -> Tag {
        match self {
            YieldReason::Explicit | YieldReason::AwaitingChild(_) => Tag::Yield,
            YieldReason::ReceivingOn(_) | YieldReason::SendingOn(_) => Tag::ChannelOp,
            // [项目扩展 / G-02] socket 就绪等待：与通道同族的「可挂起等待」，
            // 但由宿主 I/O 线程唤醒。
            YieldReason::SocketReady(_) => Tag::SocketReady,
            // [项目扩展 / G-08] 定时睡眠：由调度器的定时器唤醒。
            YieldReason::SleepUntil(_) => Tag::SleepUntil,
        }
    }
}

/// The algebraic-effect **tag** of a control-flow event: which handler
/// family routes it. The **payload** lives beside the tag, in the
/// `{ tag, payload }` pair the runtime builds (WasmFX Phase 3 /
/// Pretnar & Bauer 2015 shape; the rationale and the tag list live in
/// `wlwl-eval::runtime`). Written as prose, not an intra-doc link: the
/// payload type is declared in `wlwl-eval`, and this crate must not depend
/// on it (dependency direction is `eval → wlwl-value ← std`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    /// `perform Yield` — collaborative yield point. Equivalent to
    /// `Signal::Yield(YieldReason::Explicit)` in the legacy plumbing.
    Yield,
    /// `perform ChannelOp` — synchronous SEND / RECV blocking on a
    /// channel's buf or wait list. Maps to
    /// `Signal::Yield(YieldReason::ReceivingOn | SendingOn)` in the
    /// legacy plumbing; the runtime routes to the channel's
    /// sender_waiters / receiver_waiters list.
    ChannelOp,
    /// `raise Cancelled` — task was cancelled (with optional reason
    /// payload per ADR-0019 §4.4.2). Maps to `TaskState::Cancelled`
    /// transition; no legacy `Signal` equivalent (cancellation was
    /// previously advisory-only via `cancel_requested` flag).
    Cancelled,
    /// `perform MethodCall` — OOP `CALL_METHOD` control-flow event
    /// (ADR-0019 §4.4.3 / spec §16.4).
    ///
    /// **保留 tag,本版不产生**(D-4 裁决 = 规范明文化;见 spec §17.4 末段)。
    /// 方法调用同步直落,协议违规当场报 `E0050` / `E0051`。保留本变体是为
    /// 了让代数效果后端迁移时 tag 面已就位;实现不得依赖它产生任何可观察行为。
    MethodCall,
    /// `raise ProtocolViolation` — session-type state-machine miss
    /// (保留 tag,本版不产生 —— D-4;见 `MethodCall` 的说明)
    /// (ADR-0019 §4.4.3 / spec §16.4). Reserved naming; surfaced
    /// today as `E0050` / `E0051` diagnostics rather than a handled
    /// effect.
    ProtocolViolation,
    /// [项目扩展 / G-02] `perform SocketReady` —— 等待 socket 就绪。
    ///
    /// ⚠ 与 `ChannelOp` 同族：都是**可挂起**的等待；区别是唤醒方不同
    ///   （通道 = 对端任务；socket = 宿主 I/O 线程的就绪队列）。
    SocketReady,
    /// [项目扩展 / G-08] `perform SleepUntil` —— 定时挂起当前任务。
    ///
    /// ⚠ 与 `SocketReady` 同族（都是**可挂起**的等待）；唤醒方不同
    ///   （socket = 宿主 I/O 线程；睡眠 = 调度器的定时器）。
    SleepUntil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub usize);

/// Session-type protocol AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Proto {
    End,
    /// Call `method` (optional payload type name), then continue.
    Then {
        method: String,
        payload: Option<String>,
        next: Box<Proto>,
    },
    /// Internal choice `⊕`: caller picks exactly one offered method.
    Choice(Vec<(String, Box<Proto>)>),
    /// `μX. body`.
    Mu(String, Box<Proto>),
    /// Recursion variable `X`.
    Var(String),
}

/// Cursor over a live protocol for one instance.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolCursor {
    /// No protocol declared — every method is unrestricted.
    Unrestricted,
    /// Mid-protocol. `steps` bounds μ unfoldings (ADR: μ exhausted → E0051).
    Live {
        proto: Proto,
        /// Unfolding budget for `Mu` (prevents infinite μ expansion).
        fuel: u32,
    },
    /// Protocol completed (`end` reached). Further `CALL_METHOD` →
    /// `ProtocolError::Exhaused` → **E0050**(终态算状态机不匹配,与
    /// step-order slip 的 E0051 分开;spec §14.3.2 优先级,见
    /// `builtin_call_method` 里的映射)。
    Done,
}

/// Default μ-unfolding budget per instance.
pub const DEFAULT_FUEL: u32 = 64;

/// Why a `CALL_METHOD` was rejected by the state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolError {
    /// Method not offered in the current state (sequence order / ⊕ choice).
    NotOffered {
        method: String,
        expected: Vec<String>,
    },
    /// Protocol already at `end`.
    Exhaused,
    /// μ recursion fuel exhausted.
    RecursionExhausted { method: String },
    /// Malformed protocol expression in `CLASS` second arg.
    Syntax(String),
}

impl ProtocolCursor {
    pub fn unrestricted() -> Self {
        ProtocolCursor::Unrestricted
    }

    pub fn live(proto: Proto) -> Self {
        ProtocolCursor::Live {
            proto,
            fuel: DEFAULT_FUEL,
        }
    }

    /// Method names currently offered (for diagnostics).
    pub fn offered(&self) -> Vec<String> {
        match self {
            ProtocolCursor::Unrestricted => vec![],
            ProtocolCursor::Done => vec![],
            ProtocolCursor::Live { proto, .. } => offered_of(proto),
        }
    }

    /// Attempt to perform `method`. On success the cursor advances.
    pub fn step(&mut self, method: &str) -> Result<(), ProtocolError> {
        match self {
            ProtocolCursor::Unrestricted => Ok(()),
            ProtocolCursor::Done => Err(ProtocolError::Exhaused),
            ProtocolCursor::Live { proto, fuel } => {
                let (next, used_fuel) = step_proto(proto, method, *fuel)?;
                *fuel = used_fuel;
                match next {
                    Some(p) => {
                        *proto = p;
                        Ok(())
                    }
                    None => {
                        *self = ProtocolCursor::Done;
                        Ok(())
                    }
                }
            }
        }
    }
}

pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "integer",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Boolean(_) => "boolean",
        Value::Null => "null",
        Value::Array(_) => "array",
        // ⚠ 变体名**不外泄**:用户看 `TYPE` 只能看到 `array`,分不清内部表示。
        Value::Cell(_) => "array",
        Value::Dict(_) => "dict",
        // ⚠ [ADR-0035 D4] 变体名**不外泄**:用户看 `TYPE` 只能看到 `dict`,
        // 分不清内部表示(与 `Cell` → `"array"` 同理)。
        Value::DictCell(_) => "dict",
        // ⚠ [G-17] 变体名**不外泄**：用户看 `TYPE` 只能看到 `string`（与 `Cell` →
        // `"array"`、`DictCell` → `"dict"` 同理）。
        Value::CellStr(_) => "string",
        Value::Closure { .. } => "function",
        Value::NativeFn { .. } => "native-function",
        Value::Ok(_) => "ok",
        Value::Err(_) => "err",
        // [v0.7 Phase C2] user-facing type name for SPAWN handles;
        // AWAIT (C3) is the only consumer at C2 (where the value
        // never escapes from SPAWN's call site without being
        // dereferenced), so this name is rarely seen. Kept distinct
        // from `function` so a misuse like `+`(handle, 1) gives a
        // readable diagnostic.
        Value::TaskHandle(_) => "task-handle",
        // [v0.7 Phase D-A] user-facing type name for channel handles.
        Value::ChannelHandle(_) => "channel-handle",
        // [v0.9 Step 9a-1 / plan §4.3] OOP type names. Class and
        // Instance are distinct user-visible types so a misuse
        // like `+(class, 1)` produces a readable diagnostic that
        // distinguishes "you gave me a class, expected an
        // instance" from the reverse.
        Value::Class(_) => "class",
        Value::Instance { .. } => "instance",
    }
}

pub fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "INTEGER",
        Value::Float(_) => "FLOAT",
        Value::String(_) => "STRING",
        Value::Boolean(_) => "BOOLEAN",
        Value::Null => "NULL",
        Value::Array(_) => "ARRAY",
        Value::Dict(_) => "DICT",
        // ⚠ [ADR-0035 D4] 累加器 cell。**必须与 `Dict` 同名** —— 这个函数是
        // 用户可见的类型名,把 `DICTCELL` 之类的新名字列出来等于让内部表示
        // 泄到可观察面(与 `Cell` → `"ARRAY"` 同理)。
        Value::DictCell(_) => "DICT",
        // ⚠ [G-17] 累加器 cell。**必须与 `String` 同名** —— 这个函数是
        // 用户可见的类型名，把 `CELLSTR` 之类的新名字列出来等于让内部表示
        // 泄到可观察面（与 `Cell` → `"ARRAY"` 同理）。
        Value::CellStr(_) => "STRING",
        Value::Closure { .. } | Value::NativeFn { .. } => "FUNCTION",
        Value::Ok(_) | Value::Err(_) => "RESULT",
        // [v0.7 Phase C2] distinct user-visible type for SPAWN
        // handles (parallel to function / result).
        Value::TaskHandle(_) => "TASK",
        // [v0.7 Phase D-A] distinct user-visible type for channel
        // handles, parallel to TASK. Spec doesn't fix the name; we
        // use CHANNEL so a `TYPE(ch)` prints a self-describing
        // label without colliding with TASK.
        Value::ChannelHandle(_) => "CHANNEL",
        // [v0.9 Step 9a-1 / plan §4.3] OOP types exposed by TYPE().
        // CLASS / INSTANCE mirror the spec §2.2.1 conventions
        // (parallel to FUNCTION / TASK / CHANNEL — one entry per
        // runtime value kind).
        Value::Class(_) => "CLASS",
        Value::Instance { .. } => "INSTANCE",
        // [v0.11.3 M7 / ADR-0032] 累加器 cell。**必须与 `Array` 同名** ——
        // 这个函数是用户可见的类型名,把 `CELL` 之类的新名字列出来等于让内部
        // 表示泄到可观察面(与 `type_name()` 的 "array" 同理)。
        Value::Cell(_) => "ARRAY",
    }
}

/// [ADR-0035 / X-15] 字典的**集合口径**相等(规范 §2.4):`x` 的每个条目都要在 `y`
/// 里找到一个键值都相等的条目 —— **不看顺序**。
///
/// ⚠ **口径一个字都没改**:`ADR-0035` 修订记录「新事实三」——实现里
///   `values_equal` 的字典分支是集合口径,只有 `PartialEq` 才是顺序敏感 ——
///   两种口径都逐字保留。抽成函数是为了让 `DictCell` **复用同一条**判定 ⇒
///   「累加器形态与普通字典在可观察面上逐字相同」这条不变量只由**一处**代码保证。
///
/// ⚠ **[X-15] 改的只是「在 `y` 里找匹配条目」怎么找**,不是「什么算相等」:
///
///   * 两侧的键**全在封闭域内**(规范 §2.1:键域 = `STRING` / `INTEGER` /
///     `INSTANCE`)⇒ 键相等 ⇔ [`DictKey`] 相等 ⇒ 用 `y` 的**哈希索引**把候选
///     缩到 O(1) 条 ⇒ 键比较次数 **O(n)**(规范 §18.3 的**规范性**上界);
///   * 否则(存在**域外键**)⇒ **逐字退回**旧的线性扫描:域外键的键相等可能是
///     **跨类型数值相等**(`FLOAT(1.0)` ≡ `INTEGER(1)`,§2.4)⇒ 哈希索引
///     **不覆盖**它 ⇒ 回退**不许**被悄悄去掉。
fn dict_entries_equal(x: &DictInner, y: &DictInner) -> bool {
    if x.len() != y.len() {
        return false;
    }
    if x.all_keys_in_domain && y.all_keys_in_domain {
        return dict_entries_equal_indexed(x, y);
    }
    dict_entries_equal_linear(x.entries(), y.entries())
}

/// [X-15] 集合口径相等的**哈希路径** —— 两侧的键全在封闭域内时走这条。
///
/// ⚠ **口径不变**:仍然是「`x` 的每个条目都要在 `y` 里找到一个键值都相等的
///   条目」(**集合口径、不看顺序**);变的只是**候选怎么找**:
///   旧:在 `y` 的 n 个条目里**线性**扫 ⇒ 键比较 n(n+1)/2 次(**二次**);
///   新:在 `y` 的哈希索引里按**键**定位 ⇒ 每个 `x` 条目 1 次哈希探测
///       (+ 每个候选 1 次核对)⇒ 键比较 **O(n)** 次。
///
/// ⚠ **候选可以不止一个**:字典字面量允许**重复键**(`["a": 1, "a": 2]`)
///   ⇒ 同一键在 `y` 里可能有**多个**位置 ⇒ 候选是**位置列表**;且**同一个**
///   `y` 条目可以匹配**多个** `x` 条目(旧口径如此)⇒ 候选**不消费**。
///
/// ⚠ **按 `x` 的插入序遍历** ⇒ 短路点确定 ⇒ 读数可复算(规范 §18.5.2 的
///   「确定性工作量计数」;哈希表的迭代顺序是随机的 ⇒ 不能拿它当遍历序)。
fn dict_entries_equal_indexed(x: &DictInner, y: &DictInner) -> bool {
    // [V-01] 键比较次数的计数点 —— 收在**字典相等的内部**里(不在调用点)。
    let mut key_comparisons = 0u64;
    let mut equal = true;
    for (xk, xv) in x.entries() {
        // ① **哈希探测**:记 1 次键比较(哈希桶里的那一次 `Eq`;与
        //    [`DictInner::lookup`] 的哈希路径**同口径**:一次查找 = 1 次键比较)。
        let dk = DictKey::from_value(xk).expect("快路径的前提:键全在封闭域内");
        key_comparisons += 1;
        let Some(positions) = y.index.get(&dk) else {
            // `y` 里没有同键的条目 ⇒ 这个 `x` 条目**匹配不到** ⇒ 不相等。
            equal = false;
            break;
        };
        // ② **候选核对**:每个候选记 1 次键比较(按 §2.4 的口径**复核**键)
        //    + 1 次值比较(值不计入本计数 —— 本计数只数**键**比较)。
        let mut matched = false;
        for &pos in positions {
            let (yk, yv) = &y.entries()[pos];
            key_comparisons += 1;
            if values_equal(xk, yk) && values_equal(xv, yv) {
                matched = true;
                break;
            }
        }
        if !matched {
            equal = false;
            break;
        }
    }
    add_value_work(|w| w.equality_key_comparisons += key_comparisons);
    equal
}

/// [X-15] 集合口径相等的**线性回退** —— 存在**域外键**时走这条。
///
/// ⚠ **逐字保留**旧实现(唯一差别:入参从切片换成 `DictInner` 的条目切片):
///   域外键的键相等可能是**跨类型数值相等**(`FLOAT(1.0)` 与 `INTEGER(1)` 在
///   §2.4 下相等)⇒ 哈希索引不覆盖它 ⇒ 这一路**必须**线性扫。
fn dict_entries_equal_linear(x: &[(Value, Value)], y: &[(Value, Value)]) -> bool {
    // [V-01] 键比较次数的计数点 —— 收在**字典相等的内部扫描**里(不在调用点)。
    let mut key_comparisons = 0u64;
    let equal = x.iter().all(|(xk, xv)| {
        y.iter().any(|(yk, yv)| {
            key_comparisons += 1;
            values_equal(xk, yk) && values_equal(xv, yv)
        })
    });
    add_value_work(|w| w.equality_key_comparisons += key_comparisons);
    equal
}

/// [V-01] 数组的深度相等 —— 逐对访问的元素个数的计数点。
///
/// ⚠ 与 [`values_equal`] 的数组分支**逐字同口径**:长度不等 ⇒ 立刻 `false`
///   (不计访问);长度相等 ⇒ 逐对比较,遇首个不相等就短路(`all` 的语义)。
fn array_entries_equal(x: &[Value], y: &[Value]) -> bool {
    if x.len() != y.len() {
        return false;
    }
    let mut visits = 0u64;
    let equal = x.iter().zip(y.iter()).all(|(a, b)| {
        visits += 1;
        values_equal(a, b)
    });
    add_value_work(|w| w.equality_element_visits += visits);
    equal
}

pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Integer(x), Value::Float(y)) => (*x as f64) == *y,
        (Value::Float(x), Value::Integer(y)) => *x == (*y as f64),
        (Value::String(x), Value::String(y)) => x == y,
        // ⚠ [G-17] `CellStr` 的相等性：与 `String` **逐字同口径**（按内容比）。
        //   ⚠ **先 `ptr_eq` 短路** —— 同一 cell 时直接 true，**不去 borrow**
        //   （与 `Cell` / `DictCell` 的同款理由：借出比对会撞 `RefCell` 的借用检查）。
        (Value::CellStr(x), Value::CellStr(y)) => x.ptr_eq(y) || x.as_str() == y.as_str(),
        // ⚠ 交叉比较：一个累加器可能在中途才被识别成 cell ⇒
        //   `"ab" == cell("ab")` 必须与 `"ab" == "ab"` 逐字同结果。
        (Value::String(x), Value::CellStr(y)) => x.as_str() == y.as_str(),
        (Value::CellStr(x), Value::String(y)) => x.as_str() == y.as_str(),
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::Array(x), Value::Array(y)) => array_entries_equal(x, y),
        // [v0.11.3 M7 / ADR-0032 D1] `Cell` 的相等性。
        //
        // ⚠ **三条要求,缺一不可**:
        // ① 必须是**深比较** —— 照 `Class` / `Instance` 的身份先例办的话,
        //    `[1,2] == [1,2]` 会变成 FALSE,那是可观察的语义破坏;
        // ② 必须先 `ptr_eq` 短路 —— 同一 cell 时直接 true,**不去 borrow**。
        //    否则「同一 cell 上借出比对」会撞 `RefCell` 的运行期借用检查而
        //    panic(PLAN §5 风险 1);短路的副作用是「self 同 cell 恒等」,
        //    那与深比较的结果一致,不会给出错误答案;
        // ③ 必须覆盖 `Array` ↔ `Cell` 的**交叉**比较 —— 一个累加器可能在中途
        //    才被识别成 cell,`[1,2] == cell([1,2])` 必须为 TRUE。
        (Value::Cell(x), Value::Cell(y)) => {
            x.ptr_eq(y) || {
                let (x, y) = (x.borrow(), y.borrow());
                array_entries_equal(&x, &y)
            }
        }
        (Value::Array(x), Value::Cell(y)) => {
            let y = y.borrow();
            array_entries_equal(x, &y)
        }
        (Value::Cell(x), Value::Array(y)) => {
            let x = x.borrow();
            array_entries_equal(&x, y)
        }
        // ⚠ [ADR-0035] 字典相等是**逐元素**比较:**每个** `x` 的条目都要在 `y`
        //   里找到一个键值都相等的条目(集合口径,**不**看顺序)。
        //   ⇒ 这一段**一个字都不能改** —— 哈希索引只改**查找**路径,
        //   不改**相等**的语义(§2.4)。
        //   ⚠ [X-15] 传的是 `DictInner`(不是条目切片)⇒ 相等判定**能拿到 y 的
        //   哈希索引** ⇒ 「在 y 里找匹配条目」的候选缩到 O(1) 条 ⇒ 键比较
        //   从 n(n+1)/2 降到 O(n)(规范 §18.3 的**规范性**上界);**判定口径**
        //   (集合口径、不看顺序)一个字没动。
        (Value::Dict(x), Value::Dict(y)) => dict_entries_equal(x, y),
        // ⚠ [ADR-0035 D4] `DictCell` 的相等性:与 `Dict` **逐字同口径**
        //   (集合口径、不看顺序)⇒ 复用同一个判定函数,一个分支都不另写。
        //
        //   ⚠ **先 `ptr_eq` 短路** —— 同一 cell 时直接 true,**不去 borrow**
        //   (与 `Cell` 的同款理由:借出比对会撞 `RefCell` 的运行期借用检查)。
        (Value::DictCell(x), Value::DictCell(y)) => {
            x.ptr_eq(y) || {
                let (xi, yi) = (x.inner_rc(), y.inner_rc());
                dict_entries_equal(&xi, &yi)
            }
        }
        // ⚠ 交叉比较:一个累加器可能在中途才被识别成 cell ⇒
        //   `dict == cell(dict)` 必须与 `dict == dict` 逐字同结果。
        (Value::Dict(x), Value::DictCell(y)) => {
            let yi = y.inner_rc();
            dict_entries_equal(x, &yi)
        }
        (Value::DictCell(x), Value::Dict(y)) => {
            let xi = x.inner_rc();
            dict_entries_equal(&xi, y)
        }
        (Value::Ok(x), Value::Ok(y)) => values_equal(x, y),
        (Value::Err(x), Value::Err(y)) => values_equal(x, y),
        // v0.7 additive identity for generation-tracked handles
        // (same shape as v0.6 §2.4 function instance identity).
        // Without this arm `==(h, h)` fell through to `false`.
        (Value::TaskHandle(x), Value::TaskHandle(y)) => x == y,
        (Value::ChannelHandle(x), Value::ChannelHandle(y)) => x == y,
        // Native std functions compare by bound name (unique per import).
        (Value::NativeFn { name: x, .. }, Value::NativeFn { name: y, .. }) => x == y,
        // Closures: v0.6 §2.4 requires instance reference equality.
        // `Value::Closure` is cloned by value (no Rc), so we cannot
        // recover pointer identity after `LET(g, f)`. Two closures are
        // equal only when every structural field matches (body, params,
        // and Env's PartialEq value-snapshot). This is weaker than §2.4
        // "引用相等" — pre-existing gap, not introduced by v0.7.
        (
            Value::Closure {
                params: p1,
                body: b1,
                env: e1,
            },
            Value::Closure {
                params: p2,
                body: b2,
                env: e2,
            },
        ) => p1 == p2 && b1 == b2 && e1 == e2,
        // [v0.10.1 / R10-012] OOP 身份(spec §2.4,v0.9 起规范性):
        // 「`CLASS`/`INSTANCE` 按**对象身份**恒等 …… 同一实例与其别名相等」。
        //
        // v0.9–v0.10 这两种值落到 `_ => false`,于是连自反律都不成立:
        // `==(a, a)` 与 `==(C, C)` 都返回 FALSE。身份比较的机制本来就有
        // (TaskHandle / ChannelHandle / Closure 三条臂都在),只是这两个类型
        // 漏接了。
        //
        // 身份载体就是各自的 `Rc`:
        // - `Class` 的 `Rc<RefCell<ClassEntry>>` 每次 `CLASS(...)` 新建一个,
        //   别名共享同一个;
        // - `Instance` 的 `fields: Rc<RefCell<Vec<..>>>` 是**每实例**新建的,
        //   `LET(b, a)` 与所有下游引用共享同一个(这正是 v0.9 Step 9a-5 把
        //   `fields` 包进 Rc 的目的 —— `SET_PROP` 要让所有引用都看得到)。
        //
        // 所以 `Rc::ptr_eq` 就是规范说的「对象身份」:同实例相等(含别名),
        // 两次 `NEW` 不等,两次 `CLASS(...)` 不等。
        (Value::Class(x), Value::Class(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Instance { fields: fx, .. }, Value::Instance { fields: fy, .. }) => {
            std::rc::Rc::ptr_eq(fx, fy)
        }
        _ => false,
    }
}

// ──────────────────────────────────────────────────────────────────────
// std 调用契约(标准库底座 v0.11 / ADR-0022)
// ──────────────────────────────────────────────────────────────────────

/// std 原生函数签名:直接收发真 [`Value`]`(含闭包),回调经 [`StdHost`]
/// 注入;`Outcome.signal == Signal::Yield(_)` 表示回调挂起,std 层必须
/// 原样穿透,不得消费(结构化并发,spec §17)。
pub type StdFn = fn(&mut dyn StdHost, Vec<Value>) -> Result<Outcome, WlwlError>;

/// std 原生函数看到的宿主。由 wlwl-eval 为其求值器实现;依赖方向
/// `eval → wlwl-value ← wlwl-std` 单向成立(ADR-0022)。
pub trait StdHost {
    /// 每调用级上下文(argv/env/警告槽/测试注册表/入口文件名)。
    fn ctx(&mut self) -> &mut StdCtx;
    /// 调用函数值(闭包 / NativeFn;name 用于诊断定位)。诊断带真实 span 原样上抛。
    fn call(&mut self, f: &Value, args: Vec<Value>, name: &str) -> Result<Outcome, WlwlError>;
    /// [X-07 / v1.0 spec §8.6.3] 调用函数值,并把每个实参的**未消费令牌**带进
    /// 形参绑定。
    ///
    /// §8.6.3:实参的未消费状态**转给形参绑定** ⇒ 回调收到一个未消费的 `ERR`
    /// 却没消费它时,那是**回调自己作用域结束**时的丢弃点(§8.6.2 第 3 项)
    /// ⇒ 报 `E0103`。
    ///
    /// ⚠ 默认实现**丢掉**令牌:没有令牌语义的手写宿主(各模块测试里的
    /// `NullHost` 等)一个字都不用改,行为与从前逐字一致。
    fn call_with_tokens(
        &mut self,
        f: &Value,
        args: Vec<Value>,
        arg_tokens: Vec<UnconsumedErrs>,
        name: &str,
    ) -> Result<Outcome, WlwlError> {
        let _ = arg_tokens;
        self.call(f, args, name)
    }
    /// [X-07 / v1.0 spec §8.6.3] **本次调用**每个实参的未消费令牌(与实参
    /// 一一对应)。
    ///
    /// 标准库成员据此知道「我的实参带着未消费的 `ERR`」—— 没有它,成员就没法
    /// 按 §8.6.3 把这份状态转给自己的回调(一致性册 §5.4 的 X-07)。
    ///
    /// ⚠ 默认实现返回**空**:没有令牌语义的手写宿主不受影响。
    fn arg_tokens(&self) -> &[UnconsumedErrs] {
        &[]
    }
    /// 以当前调用点 span 构造诊断。
    fn diag(&mut self, code: ErrorCode, message: String) -> WlwlError;

    // ── [G-13] 截止时间（deadline）接缝 ──────────────────────────────
    //
    // 「最多等多久」这件事**必须**住在**调度器**里：等待是**可挂起**的
    // （任务被摘出运行队列），而「到点还没等到」只有调度器能观察到 ——
    // 它是唯一一个既知道「谁挂着」又能让它「重新变成可运行」的地方。
    //
    // ⇒ 标准库成员**改不了**任务的截止时间（它们看不见调度器）⇒
    // 需要这三条**宿主接缝**。
    //
    // ⚠ 默认实现全是**空操作 / 无截止时间**：手写宿主（各模块测试里的
    //   `NullHost`、基准里的对照宿主）一个字都不用改 —— 与
    //   `in_clock_bubble` / `clock_wall_ms` 那组接缝同款。

    /// [G-13] 压入当前任务的**截止时间**（`at_ms`，**真实单调钟**域），返回**旧值**
    /// （`None` = 之前没有截止时间）。
    ///
    /// ⚠ 形状照 [`StdHost::push_clock_deadline`] —— 嵌套时靠「压入 / 弹出」恢复。
    ///
    /// ⚠ 语义是「这个任务最多还能活多久」，**不是**「每次等待各 ms」。
    fn push_task_deadline(&mut self, at_ms: i64) -> Result<Option<i64>, WlwlError> {
        let _ = at_ms;
        Ok(None)
    }

    /// [G-13] 恢复先前压入的截止时间（`prev` = [`StdHost::push_task_deadline`]
    /// 的返回值；`None` = 恢复成「没有截止时间」）。
    fn pop_task_deadline(&mut self, prev: Option<i64>) {
        let _ = prev;
    }

    /// [G-13] 当前任务的截止时间（**绝对**单调钟毫秒；`None` = 没有截止时间）。
    fn task_deadline_ms(&self) -> Option<i64> {
        None
    }

    /// [G-13] 当前任务的截止时间**已到**？⇒ 读并**清掉**标志（只生效一次）。
    ///
    /// 等待操作在**重跑**时先问它：命中 ⇒ 返回**超时值**，而不是再挂一次
    /// （否则会挂成**永远**）。
    fn take_timed_out(&mut self) -> bool {
        false
    }

    /// [G-08] 当前任务那次**睡眠**是否**已经结束**？⇒ 读并**清掉**标志。
    ///
    /// ⚠ 它解决的是**「整段重跑 ⇒ 再睡一次 ⇒ 无限循环」**那个坑：任务被定时器
    ///   唤醒后调度器**整段重跑** ⇒ 睡眠成员**再被调一次** ⇒ 若不记住「这次睡眠
    ///   已经睡过了」，它会再挂一次、再被唤醒、再睡一次 ⇒ **永不停机**
    ///   （**上游自己的夹具 `M6_std_time_sleep` 实测过**：挂起路线下 8 秒 79 次
    ///   不终止）。
    ///   ⇒ 睡眠成员重跑时先问它 ⇒ **已睡够**就直接返回，不再挂。
    ///
    /// ⚠ 默认实现是 `false`（没在睡）⇒ 手写宿主一个字都不用改。
    fn take_sleep_elapsed(&mut self) -> bool {
        false
    }

    /// [G-08] 「开一次睡」—— 记一次账，并检查**这个任务体是不是已经有一次睡眠**。
    ///
    /// ⚠ 已有 ⇒ **诊断**（`Err`）：段模型是「**整段重跑**」⇒ 一个任务体里两次
    ///   睡眠 ⇒ 第二次睡完重跑 ⇒ 又睡 ⇒ **无限循环**（上游实测 8 秒 79 次不终止）
    ///   ⇒ 宁可**响亮地失败**，也不要挂死。
    ///
    /// ⚠ 默认实现是**空操作**（不记账、不报错）⇒ 手写宿主一个字都不用改。
    fn begin_task_sleep(&mut self) -> Result<(), WlwlError> {
        Ok(())
    }

    // ── 时钟接缝(v0.11.3 M6 / addendum-01 B 片)────────────────────────
    //
    // 形态来自 `ADR-0024` §6.3-2 **候选甲**:`StdHost` **加方法**。
    //
    // ⚠️ **为什么是「加方法」而不是往 `StdCtx` 加字段** —— `addendum-01` §3.10
    // 否掉的 `StdCtx.task_depth` 恰好是**同一个候选的镜像**:那次要问的是
    // 「我在不在任务内」,那是**逐次调用的上下文事实**(与 `argv` / `env` 同类,
    // 于是放 `StdCtx`);这次要问的是「当前处于哪个气泡、虚拟钟走到哪」,那是
    // **调度器状态**,塞进 `StdCtx` 会让「谁拥有时钟」在类型上说不清。
    // **一个判据,两次相反的落点。**
    //
    // **默认值是「不在气泡内 + 真实时钟」**,所以每个手写 `StdHost`(测试里的
    // `NullHost`、基准里的对照宿主)都**不用改**,而它们的存在本身就是在断言
    // 「`std.rand` / 纯计算成员不碰宿主」。

    /// 当前是否处于假时钟气泡内(`TEST_BUBBLE` 之内)。
    fn in_clock_bubble(&self) -> bool {
        false
    }

    /// 当前可见的**墙钟**毫秒(Unix 纪元)。气泡内 = 虚拟钟,否则 = 真实系统时钟。
    fn clock_wall_ms(&self) -> i64 {
        real_wall_ms()
    }

    /// 当前可见的**单调钟**毫秒(进程启动起)。气泡内 = 虚拟钟,否则 = 真实 `Instant`。
    ///
    /// ⚠️ 绝对值无意义、跨进程不可比,只有差值有效(规范 §15.1)。
    fn clock_mono_ms(&self) -> i64 {
        real_mono_ms()
    }

    /// 进入一个假时钟气泡。**已在气泡内 → `E0030`**(气泡不再嵌套,
    /// `ADR-0024` §5-1 业主 2026-10-07 裁决)。成功时虚拟钟**重置到固定起点**。
    ///
    /// 必须与 [`StdHost::exit_clock_bubble`] 成对调用。
    fn enter_clock_bubble(&mut self) -> Result<(), WlwlError> {
        let _ = self;
        Err(self.diag(
            ErrorCode::E0030,
            "clock bubble: this host does not support bubbles".into(),
        ))
    }

    /// 退出当前气泡(与 [`StdHost::enter_clock_bubble`] 成对)。不在气泡内 = 空操作。
    fn exit_clock_bubble(&mut self) {
        let _ = self;
    }

    /// 把虚拟钟拨快 `ms` 毫秒,返回「**最紧的**未放弃 `TIMEOUT` deadline
    /// 是否被越过」。**不在气泡内 → `E0030`**。
    ///
    /// ⚠️ 返回那个 `bool` 是因为 `SLEEP` 靠它决定要不要把 body 从最近的函数
    /// 边界弹回去(`Signal::Return`);让成员自己去查一次「有没有被越过」会
    /// 多一次虚方法调用,而且把「谁判定」这件事在两处各写一遍。
    fn advance_clock(&mut self, ms: i64) -> Result<bool, WlwlError> {
        let _ = ms;
        Err(self.diag(ErrorCode::E0030, "clock bubble: not inside a bubble".into()))
    }

    /// 压入一个 `TIMEOUT` 的 deadline(`at_ms`,虚拟单调域),返回它在栈里的
    /// **深度**。**不在气泡内 → `E0030`**。
    fn push_clock_deadline(&mut self, at_ms: i64) -> Result<usize, WlwlError> {
        let _ = at_ms;
        Err(self.diag(ErrorCode::E0030, "clock bubble: not inside a bubble".into()))
    }

    /// 弹出深度 `depth` 的 deadline,返回**它是否已被放弃过**。
    fn pop_clock_deadline(&mut self, depth: usize) -> bool {
        let _ = depth;
        false
    }
}

/// 真实墙钟:Unix 纪元起的毫秒。**默认实现** —— 只有气泡内的虚拟钟需要宿主接管。
pub fn real_wall_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        // 系统时钟早于 1970 是病态情形(改了系统时间 / 容器里 epoch 未初始化)。
        // 不用 `unwrap()` 崩掉一个只读的时钟成员:负毫秒是**可表达**的
        // (减法仍然精确),而崩溃不是。
        Err(e) => -i64::try_from(e.duration().as_millis()).unwrap_or(i64::MAX),
    }
}

/// 真实单调钟:**进程启动起**的毫秒。基准用 `OnceLock` 固定住 —— 否则每次调用都
/// 从一个新基准算起,结果不是单调的,而是「永远等于上一次到现在」。
pub fn real_mono_ms() -> i64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let origin = ORIGIN.get_or_init(std::time::Instant::now);
    i64::try_from(origin.elapsed().as_millis()).unwrap_or(i64::MAX)
}

/// 注册进 [`StdCtx::tests`] 的单条测试(name + 零参测试体)。
#[derive(Debug, Clone)]
pub struct TestEntry {
    pub name: String,
    pub body: Value,
}

/// 「**没有源文件**」的哨兵名 —— `StdCtx.source_file` 在宿主没拿到入口文件时
/// 取这个值(嵌入式求值 / REPL / 库里直接 `Evaluator::eval`)。
///
/// ⚠ 它是**诊断定位**用的占位名,**不是路径**。任何把它当路径返回给 WLWL 代码的
/// 成员都会把内部约定泄漏到公开成员面 ⇒ 标准库侧要把它翻译成 `NULL`
/// (见 `wlwl-std::env::script_path`)。
pub const NO_SOURCE_FILE: &str = "<runtime>";

/// std 原生函数的每调用级上下文(自 wlwl-std 迁入,ADR-0022)。
#[derive(Debug, Clone)]
pub struct StdCtx {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    /// Soft warnings produced by std functions (e.g. `W0052`).
    ///
    /// [D11-019] The comment here used to claim "eval drains it after each
    /// call" — **nothing drains it**. The only readers in the tree are
    /// `wlwl-std`'s own unit tests, so a `W0052` raised by `std.ai` /
    /// `std.agent` is written and never surfaces to the user. Fixing that
    /// means making `invoke_std` drain and re-emit, which is a new
    /// observable behavior (warnings that never appeared would start
    /// appearing) and therefore its own decision, not a doc fix; recorded
    /// in the ledger as D11-022. Until then this field is a write-only sink.
    pub warnings: Vec<(ErrorCode, String)>,
    /// 入口源文件名(诊断定位用),由 eval 在调用前写入。
    ///
    /// ⚠ 没有入口文件时取 [`NO_SOURCE_FILE`] —— 它**不是路径**。
    pub source_file: String,
    /// `std.test` 注册表:TEST 推入,RUN_TESTS 排空调用。
    pub tests: Vec<TestEntry>,
}

impl Default for StdCtx {
    fn default() -> Self {
        Self {
            argv: Vec::new(),
            env: HashMap::new(),
            warnings: Vec::new(),
            source_file: NO_SOURCE_FILE.into(),
            tests: Vec::new(),
        }
    }
}

impl StdCtx {
    pub fn from_process() -> Self {
        Self {
            argv: std::env::args().collect(),
            env: std::env::vars().collect(),
            warnings: Vec::new(),
            source_file: NO_SOURCE_FILE.into(),
            tests: Vec::new(),
        }
    }

    /// 推送软警告(不改变程序语义)。
    pub fn warn(&mut self, code: ErrorCode, message: impl Into<String>) {
        self.warnings.push((code, message.into()));
    }

    /// 以入口文件定位构造 std 侧诊断。
    pub fn err(&self, code: ErrorCode, message: impl Into<String>) -> WlwlError {
        WlwlDiagnostic::new(
            code,
            message.into(),
            Location::point(&self.source_file, 0, 0),
        )
        .into()
    }
}

/// 通道句柄在调度器语境下的别名(自 runtime.rs 迁入)。
pub type RcHandle = ChannelId;

/// [v0.7 Phase D-A] 通道句柄:`CHANNEL_NEW(buf)` 返回给用户代码;
/// generation 跟踪使回收后的旧句柄按 E0053 判失效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelHandle {
    pub id: ChannelId,
    pub generation: u64,
}

pub fn offered_of(p: &Proto) -> Vec<String> {
    match p {
        Proto::End => vec![],
        Proto::Var(_) => vec![],
        Proto::Mu(_, body) => offered_of(body),
        Proto::Then { method, next, .. } => {
            if method.is_empty() {
                offered_of(next)
            } else {
                vec![method.clone()]
            }
        }
        Proto::Choice(arms) => arms.iter().map(|(m, _)| m.clone()).collect(),
    }
}

pub(crate) fn step_proto(
    p: &Proto,
    method: &str,
    fuel: u32,
) -> Result<(Option<Proto>, u32), ProtocolError> {
    match p {
        Proto::End => Err(ProtocolError::Exhaused),
        Proto::Var(_) => Err(ProtocolError::NotOffered {
            method: method.to_string(),
            expected: vec![],
        }),
        Proto::Mu(name, body) => {
            if fuel == 0 {
                return Err(ProtocolError::RecursionExhausted {
                    method: method.to_string(),
                });
            }
            let unfolded = subst(body, name, &Proto::Mu(name.clone(), body.clone()));
            step_proto(&unfolded, method, fuel - 1)
        }
        Proto::Then {
            method: expected,
            next,
            ..
        } => {
            if expected.is_empty() {
                // Anonymous payload node — skip.
                return step_proto(next, method, fuel);
            }
            if expected == method {
                let cont = (**next).clone();
                if matches!(cont, Proto::End) {
                    Ok((None, fuel))
                } else {
                    Ok((Some(cont), fuel))
                }
            } else {
                Err(ProtocolError::NotOffered {
                    method: method.to_string(),
                    expected: vec![expected.clone()],
                })
            }
        }
        Proto::Choice(arms) => {
            for (m, cont) in arms {
                if m == method {
                    let cont = (**cont).clone();
                    if matches!(cont, Proto::End) {
                        return Ok((None, fuel));
                    }
                    return Ok((Some(cont), fuel));
                }
            }
            Err(ProtocolError::NotOffered {
                method: method.to_string(),
                expected: arms.iter().map(|(m, _)| m.clone()).collect(),
            })
        }
    }
}

pub fn subst(p: &Proto, name: &str, repl: &Proto) -> Proto {
    match p {
        Proto::Var(v) if v == name => repl.clone(),
        Proto::Var(v) => Proto::Var(v.clone()),
        Proto::End => Proto::End,
        Proto::Then {
            method,
            payload,
            next,
        } => Proto::Then {
            method: method.clone(),
            payload: payload.clone(),
            next: Box::new(subst(next, name, repl)),
        },
        Proto::Choice(arms) => Proto::Choice(
            arms.iter()
                .map(|(m, c)| (m.clone(), Box::new(subst(c, name, repl))))
                .collect(),
        ),
        // Nested μ rebinds the same name → stop (skeleton; no shadowing tests).
        Proto::Mu(v, body) if v == name => Proto::Mu(v.clone(), body.clone()),
        Proto::Mu(v, body) => Proto::Mu(v.clone(), Box::new(subst(body, name, repl))),
    }
}

// ── [v0.11.3 M7 / ADR-0032 D1 / PLAN S1] 累加器 cell 的契约 ──────────────
//
// (c) 路线的**全部**立足点是:`Cell` 在每个可观察面上都必须与 `Array`
// 完全一致。下面逐面钉住;任何一条改动都要先想清楚它是不是可观察的。

#[cfg(test)]
mod accumulator_cell_tests {
    use super::*;

    fn arr(n: i64) -> Value {
        Value::Array(vec![Value::Integer(n), Value::Integer(n + 1)])
    }
    fn cell(n: i64) -> Value {
        Value::Cell(CellVec::new(vec![Value::Integer(n), Value::Integer(n + 1)]))
    }

    /// 可观察面 ①:`display()` 逐字节一致。
    #[test]
    fn cell_displays_exactly_like_an_array() {
        assert_eq!(cell(1).display(), arr(1).display());
        assert_eq!(cell(1).display(), "[1, 2]");
        // 空 cell 也不能是 `[]` 之外的任何东西
        assert_eq!(Value::Cell(CellVec::new(vec![])).display(), "[]");
    }

    /// 可观察面 ②:两个类型名函数都报「数组」,变体名不外泄。
    #[test]
    fn cell_reports_the_array_type_name() {
        assert_eq!(type_name(&cell(1)), "array");
        assert_eq!(type_name(&cell(1)), type_name(&arr(1)));
        assert_eq!(value_type_name(&cell(1)), "ARRAY");
        assert_eq!(value_type_name(&cell(1)), value_type_name(&arr(1)));
    }

    /// 可观察面 ③:相等性是**深比较**,不是身份 —— 且覆盖 `Array` ↔ `Cell`
    /// 的**交叉**比较(累加器可能在中途才被识别)。
    #[test]
    fn cell_equality_is_deep_not_identity() {
        assert!(values_equal(&cell(1), &cell(1)));
        assert!(!values_equal(&cell(1), &cell(2)));
        // 交叉
        assert!(values_equal(&cell(1), &arr(1)));
        assert!(values_equal(&arr(1), &cell(1)));
        assert!(!values_equal(&cell(1), &arr(2)));
        assert!(!values_equal(&arr(1), &cell(2)));
        // 长度不同
        let shorter = Value::Cell(CellVec::new(vec![Value::Integer(1)]));
        assert!(!values_equal(&shorter, &cell(1)));
        // 与非数组不相等
        assert!(!values_equal(&cell(1), &Value::Integer(1)));
    }

    /// ⚠ `ptr_eq` 短路必须真的短路:同一 cell 自比**不能** panic。
    ///
    /// 若相等性里少了这一步,「借出同一个 cell 比对」会撞 `RefCell` 的运行期
    /// 借用检查 —— 那是一个 **panic**,不是可诊断的错误(PLAN §5 风险 1)。
    #[test]
    fn comparing_a_cell_with_itself_does_not_panic() {
        let c = Value::Cell(CellVec::new(vec![Value::Integer(1), Value::Integer(2)]));
        assert!(values_equal(&c, &c));
        // 反复比 —— 借用检查的 panic 在这里就会现形
        for _ in 0..1000 {
            assert!(values_equal(&c, &c));
        }
    }

    /// ⚠ **克隆是深拷贝**,不是共享 —— 否则 `LET(b, a)` 之后改 `a` 会连带改 `b`,
    /// 那是静默的不可变性破坏,性能收益并不依赖它。
    #[test]
    fn cloning_a_cell_is_a_deep_copy() {
        let a = Value::Cell(CellVec::new(vec![Value::Integer(1)]));
        let mut b = a.clone();
        match &mut b {
            Value::Cell(c) => c.borrow_mut().push(Value::Integer(2)),
            _ => panic!("expected Cell"),
        }
        // 改 b 不能影响 a
        assert_eq!(a.display(), "[1]");
        assert_eq!(b.display(), "[1, 2]");
        // 且克隆出的不是同一个 cell
        match (&a, &b) {
            (Value::Cell(x), Value::Cell(y)) => assert!(!x.ptr_eq(y)),
            _ => panic!("expected Cells"),
        }
    }

    /// `borrow_mut` 就地追加是 S3 的落点;S1 先确认机制通。
    #[test]
    fn borrow_mut_appends_in_place() {
        let c = CellVec::new(vec![Value::Integer(1)]);
        c.borrow_mut().push(Value::Integer(2));
        c.borrow_mut().push(Value::Integer(3));
        assert_eq!(c.borrow().len(), 3);
        assert_eq!(Value::Cell(c).display(), "[1, 2, 3]");
    }
}

// ── [ADR-0035 D4] 字典累加器 cell 的契约 ───────────────────────────────
//
// 这一组钉的是 `ADR-0032` S1 的同款清单:**变体在每个可观察面上都必须与
// 普通字典逐字相同**。`ADR-0032` 的教训正是「可观察面不许分叉」——
// 两种表示的风险同源,所以这里逐条钉死。
//
//   ① `display()` 逐字节一致(含**插入序**);
//   ② 两个类型名函数都报「字典」,变体名不外泄;
//   ③ `values_equal`(集合口径)与 `PartialEq`(顺序敏感)两种口径都覆盖,
//      且覆盖 `Dict` ↔ `DictCell` 的**交叉**比较;
//   ④ 克隆是**共享**(与 `Value::Dict` 同语义)⇒ 写时复制保证「改一个不影响
//      另一个」(规范 §19.1 的值语义);
//   ⑤ 同一 cell 自比**不能** panic(`ptr_eq` 短路)。

#[cfg(test)]
mod dict_accumulator_cell_tests {
    use super::*;

    fn plain() -> Value {
        Value::dict(vec![
            (Value::String("zeta".into()), Value::Integer(1)),
            (Value::String("alpha".into()), Value::Integer(2)),
        ])
    }

    fn cell() -> Value {
        let inner = Rc::new(DictInner::new(vec![
            (Value::String("zeta".into()), Value::Integer(1)),
            (Value::String("alpha".into()), Value::Integer(2)),
        ]));
        Value::DictCell(CellDict::new(inner))
    }

    /// 可观察面 ①:渲染逐字节一致(**插入序**不变)。
    #[test]
    fn dict_cell_displays_exactly_like_a_dict() {
        assert_eq!(cell().display(), plain().display());
        assert_eq!(cell().display(), "[zeta: 1, alpha: 2]");
    }

    /// 可观察面 ②:两个类型名函数都报「字典」。
    #[test]
    fn dict_cell_reports_the_dict_type_name() {
        assert_eq!(type_name(&cell()), "dict");
        assert_eq!(type_name(&cell()), type_name(&plain()));
        assert_eq!(value_type_name(&cell()), "DICT");
        assert_eq!(value_type_name(&cell()), value_type_name(&plain()));
    }

    /// 可观察面 ③:相等性是**深比较** —— 且两种口径(集合口径的
    /// `values_equal` / 顺序敏感的 `PartialEq`)都覆盖 `Dict` ↔ `DictCell`
    /// 的**交叉**比较。
    #[test]
    fn dict_cell_equality_is_deep_not_identity() {
        assert!(values_equal(&cell(), &cell()));
        assert!(values_equal(&cell(), &plain()));
        assert!(values_equal(&plain(), &cell()));
        assert!(!values_equal(&cell(), &Value::dict(vec![])));
        // 顺序敏感的那一口径(`PartialEq`)也要覆盖交叉。
        assert!(cell() == plain());
        assert!(plain() == cell());
        // 内容不同 ⇒ 不等。
        let other = Value::dict(vec![(Value::String("zeta".into()), Value::Integer(9))]);
        assert!(!values_equal(&cell(), &other));
        assert!(cell() != other);
    }

    /// ⚠ `ptr_eq` 短路必须真的短路:同一 cell 自比**不能** panic。
    #[test]
    fn comparing_a_dict_cell_with_itself_does_not_panic() {
        let c = cell();
        for _ in 0..1000 {
            assert!(values_equal(&c, &c));
            assert!(c == c);
        }
    }

    /// 可观察面 ④:克隆是**共享**(不是深拷贝)⇒ 与 `Value::Dict` 同语义;
    /// 写时复制保证「改一个不影响另一个」。
    #[test]
    fn cloning_a_dict_cell_shares_the_payload() {
        let a = cell();
        let b = a.clone();
        // 克隆出的 cell **不是同一个 cell**,但载荷是同一份数据。
        match (&a, &b) {
            (Value::DictCell(x), Value::DictCell(y)) => {
                assert!(!x.ptr_eq(y));
                assert!(Rc::ptr_eq(&x.inner_rc(), &y.inner_rc()));
            }
            _ => panic!("expected DictCells"),
        }
        // 改 b 不影响 a(写时复制)。
        if let Value::DictCell(c) = &b {
            c.with_inner_mut(|d| d.insert(Value::String("mid".into()), Value::Integer(3)));
        }
        assert_eq!(a.display(), "[zeta: 1, alpha: 2]");
        assert_eq!(b.display(), "[zeta: 1, alpha: 2, mid: 3]");
    }

    /// 就地写是本批的落点:唯一拥有时 O(1)(载荷 `Rc` 计数为 1 ⇒ 写时复制
    /// 判定为「唯一拥有」⇒ 原地改)。
    #[test]
    fn with_inner_mut_writes_in_place() {
        let c = CellDict::new(Rc::new(DictInner::new(vec![(
            Value::String("a".into()),
            Value::Integer(1),
        )])));
        c.with_inner_mut(|d| {
            d.insert(Value::String("b".into()), Value::Integer(2));
        });
        assert_eq!(Value::DictCell(c).display(), "[a: 1, b: 2]");
    }
}

// ── [ADR-0035] 字典哈希索引 + copy-on-write 的契约 ────────────────────
//
// 这一组钉的是**不能靠编译器发现**的那几条语义:
//   ① `DictKey` 的 `Eq` / `Hash` 与 `values_equal`(规范 §2.4)逐字一致;
//   ② 哈希索引的查找结果与旧的**线性扫描**逐字一致(含重复键、域外键);
//   ③ copy-on-write:**唯一拥有 ⇒ 原地改**,**被别名 ⇒ 克隆后改**,
//      且被别名时旧值**不受影响**(规范 §19.1 的值语义)。

#[cfg(test)]
mod dict_hash_index_tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of<T: Hash>(t: &T) -> u64 {
        let mut h = DefaultHasher::new();
        t.hash(&mut h);
        h.finish()
    }

    fn s(v: &str) -> Value {
        Value::String(v.to_string())
    }

    fn i(v: i64) -> Value {
        Value::Integer(v)
    }

    /// 造一个实例(只用到它的**身份**)。
    fn instance() -> Value {
        Value::Instance {
            class: Rc::new(RefCell::new(ClassEntry {
                name: Some("K".into()),
                parent: None,
                members: Vec::new(),
                init: None,
                protocol: None,
            })),
            fields: Rc::new(RefCell::new(Vec::new())),
            this_token: Rc::new(RefCell::new(ThisToken { moved: false })),
            protocol_state: Rc::new(RefCell::new(ProtocolCursor::unrestricted())),
        }
    }

    /// ① `DictKey` 的相等 / 哈希口径 = §2.4。
    #[test]
    fn dict_key_equality_follows_the_spec_2_4_key_domain() {
        // STRING:按字节。
        assert_eq!(DictKey::from_value(&s("a")), DictKey::from_value(&s("a")));
        assert_ne!(DictKey::from_value(&s("a")), DictKey::from_value(&s("b")));
        // INTEGER:按数值。
        assert_eq!(DictKey::from_value(&i(1)), DictKey::from_value(&i(1)));
        assert_ne!(DictKey::from_value(&i(1)), DictKey::from_value(&i(2)));
        // ⚠ STRING 与 INTEGER 是**两个不同的键**(§2.4 的跨类型数值相等只对数字成立)。
        assert_ne!(DictKey::from_value(&s("1")), DictKey::from_value(&i(1)));
        // INSTANCE:按身份。
        let a = instance();
        let b = instance();
        assert_eq!(DictKey::from_value(&a), DictKey::from_value(&a));
        assert_ne!(DictKey::from_value(&a), DictKey::from_value(&b));
        // 域外类型不是键(§2.1 键域封闭)。
        assert!(DictKey::from_value(&Value::Float(1.0)).is_none());
        assert!(DictKey::from_value(&Value::Boolean(true)).is_none());
        assert!(DictKey::from_value(&Value::Null).is_none());
        assert!(DictKey::from_value(&Value::Array(vec![])).is_none());
    }

    /// ① 相等的键 ⇒ 相等的哈希(哈希表的硬要求)。
    #[test]
    fn equal_keys_hash_equally() {
        let inst = instance();
        for (x, y) in [
            (s("a"), s("a")),
            (i(7), i(7)),
            // ⚠ 同一个**实例**的两份克隆 ⇒ 同一身份 ⇒ 同一键。
            (inst.clone(), inst.clone()),
        ] {
            let kx = DictKey::from_value(&x).expect("in domain");
            let ky = DictKey::from_value(&y).expect("in domain");
            assert_eq!(kx, ky);
            assert_eq!(hash_of(&kx), hash_of(&ky));
        }
        // 三族的哈希互不相同(前缀标签的作用)。
        let h_s = hash_of(&DictKey::from_value(&s("1")).unwrap());
        let h_i = hash_of(&DictKey::from_value(&i(1)).unwrap());
        assert_ne!(h_s, h_i);
    }

    /// ② 哈希索引的查找 = 旧的线性扫描(逐字)。
    #[test]
    fn index_lookup_agrees_with_the_old_linear_scan() {
        let inst = instance();
        // 重复键 + 混合类型键 + 域外键(FLOAT)都放进去。
        let entries = vec![
            (s("a"), i(1)),
            (s("a"), i(2)),
            (i(1), s("int-one")),
            (inst.clone(), s("inst")),
            (Value::Float(1.0), s("float-one")),
            (Value::Boolean(true), s("bool")),
        ];
        let inner = DictInner::new(entries.clone());
        let probes = [
            s("a"),
            s("zzz"),
            i(1),
            i(2),
            inst.clone(),
            instance(),
            Value::Float(1.0),
            Value::Boolean(true),
            Value::Null,
        ];
        for probe in probes.iter() {
            let old = entries.iter().position(|(k, _)| values_equal(k, probe));
            let new = inner.lookup(probe);
            assert_eq!(
                new, old,
                "lookup disagrees with the linear scan for {probe:?}"
            );
        }
    }

    /// ② 域外键存在时,查找**退回线性扫描**(不静默改语义)。
    #[test]
    fn a_dict_with_an_out_of_domain_key_falls_back_to_the_linear_scan() {
        // 存储侧 `FLOAT(1.0)` 与探测侧 `INTEGER(1)` 在 §2.4 下**相等**
        // (数字跨类型比较)⇒ 哈希索引(只收域内键)会漏掉它 ⇒ 必须退回扫描。
        let inner = DictInner::new(vec![(Value::Float(1.0), s("v"))]);
        assert_eq!(inner.lookup(&i(1)), Some(0));
        assert_eq!(inner.lookup(&Value::Float(1.0)), Some(0));
    }

    /// ③ 唯一拥有 ⇒ **原地**改(同一个 `Rc` 分配)。
    #[test]
    fn a_uniquely_owned_dict_is_mutated_in_place() {
        let mut v = Value::dict(vec![(s("k"), i(1))]);
        let before = match &v {
            Value::Dict(rc) => Rc::as_ptr(rc) as usize,
            _ => panic!("expected Dict"),
        };
        v.as_dict_inner_mut().expect("is a dict").set_value(0, i(2));
        let after = match &v {
            Value::Dict(rc) => Rc::as_ptr(rc) as usize,
            _ => panic!("expected Dict"),
        };
        assert_eq!(before, after, "唯一拥有 ⇒ 应当原地改(分配未变)");
        assert_eq!(v.dict_get(&s("k")), Some(&i(2)));
    }

    /// ③ 被别名 ⇒ **克隆后**改,且**旧值不受影响**(§19.1 的值语义)。
    #[test]
    fn an_aliased_dict_is_cloned_before_mutation() {
        let mut v = Value::dict(vec![(s("k"), i(1))]);
        let alias = v.clone();
        let before = match &v {
            Value::Dict(rc) => Rc::as_ptr(rc) as usize,
            _ => panic!("expected Dict"),
        };
        v.as_dict_inner_mut().expect("is a dict").set_value(0, i(2));
        let after = match &v {
            Value::Dict(rc) => Rc::as_ptr(rc) as usize,
            _ => panic!("expected Dict"),
        };
        assert_ne!(before, after, "被别名 ⇒ 应当克隆后改(分配已换)");
        assert_eq!(v.dict_get(&s("k")), Some(&i(2)));
        assert_eq!(
            alias.dict_get(&s("k")),
            Some(&i(1)),
            "别名持有的旧值必须原样可见"
        );
    }

    /// ③ 结构性改动(删键)之后,索引必须与条目重新对齐。
    #[test]
    fn removing_a_key_realigns_the_index() {
        let mut v = Value::dict(vec![(s("a"), i(1)), (s("b"), i(2)), (s("c"), i(3))]);
        let inner = v.as_dict_inner_mut().expect("is a dict");
        let at = inner.lookup(&s("b")).expect("b is there");
        inner.remove(at);
        // 删掉中间那个之后,位置全部前移 ⇒ 索引必须跟着重建。
        assert_eq!(inner.lookup(&s("c")), Some(1));
        assert_eq!(inner.lookup(&s("b")), None);
        assert_eq!(inner.lookup(&s("a")), Some(0));
        assert_eq!(inner.len(), 2);
    }

    /// ② 插入序是**呈现**的唯一来源:索引只存位置。
    #[test]
    fn the_index_never_reorders_the_entries() {
        let mut v = Value::dict(vec![(s("zeta"), i(1)), (s("alpha"), i(2))]);
        let inner = v.as_dict_inner_mut().expect("is a dict");
        inner.insert(s("mid"), i(3));
        assert_eq!(v.display(), "[zeta: 1, alpha: 2, mid: 3]");
        assert_eq!(
            v.as_dict_entries().expect("is a dict"),
            &[(s("zeta"), i(1)), (s("alpha"), i(2)), (s("mid"), i(3))]
        );
    }
}

// ── [v1.0 规范 §8.6] 未消费 `ERR` 状态:集合形态 ─────────────────────────
//
// 这一组钉的是**从单个 bool 升级为有序集合**之后的四条语义:
//
//   ① `Transfer` ⇒ 集合**整体转走**(源绑定清空,在途值携带**全部**);
//   ② `Observe`  ⇒ 集合**留在源绑定**(一个都不少);
//   ③ `Consume`  ⇒ 只移除**它拿到的那一个** `ERR`;值是容器时**集合不变**;
//   ④ **并集**   ⇒ 多个未消费 `ERR` 汇入同一接收位置时**全部保留**。
//
// 每条都是**单独**可红的:改坏其中一条的落点,只有对应那条变红。

#[cfg(test)]
mod unconsumed_err_set_tests {
    use super::*;

    fn err(msg: &str) -> Value {
        Value::Err(Box::new(Value::String(msg.to_string())))
    }

    /// 建一个只含 `name` 一个绑定、持有给定集合的作用域。
    fn env_with(name: &str, value: Value, set: UnconsumedErrs) -> Env {
        let mut env = Env::new();
        env.set_local_with_token(name, value, set, None);
        env
    }

    /// ① `Transfer`:集合**整体转走** —— 源绑定清空,在途值携带**全部**。
    #[test]
    fn transfer_moves_the_whole_collection_and_clears_the_source() {
        let e1 = err("e1");
        let e2 = err("e2");
        let env = env_with("a", e1.clone(), vec![e1.clone(), e2.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::Transfer)
            .expect("binding exists");

        assert_eq!(value, e1);
        // 整组转走:在途值携带**两个**,不是「只带第一个」。
        assert_eq!(carried, vec![e1.clone(), e2.clone()]);
        // 源绑定**清空**。
        assert_eq!(env.cell_token("a"), Some(Vec::<Value>::new()));
    }

    /// ② `Observe`:集合**留在源绑定** —— 一个都不少,在途值不带集合。
    #[test]
    fn observe_keeps_the_collection_in_the_source_binding() {
        let e1 = err("e1");
        let e2 = err("e2");
        let env = env_with("a", e1.clone(), vec![e1.clone(), e2.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::Observe)
            .expect("binding exists");

        assert_eq!(value, e1);
        // 在途值**不带**集合。
        assert!(carried.is_empty());
        // 源绑定**保留**整组集合。
        assert_eq!(env.cell_token("a"), Some(vec![e1.clone(), e2.clone()]));
    }

    /// ③ `Consume` + 值**本身**是 `ERR`:只移除**它拿到的那一个**。
    #[test]
    fn consume_removes_the_matching_err_and_keeps_the_others() {
        let e1 = err("e1");
        let e2 = err("e2");
        let env = env_with("a", e1.clone(), vec![e1.clone(), e2.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::Consume)
            .expect("binding exists");

        assert_eq!(value, e1);
        assert!(carried.is_empty());
        // 只掉**那一个**;另一个仍在集合里。
        assert_eq!(env.cell_token("a"), Some(vec![e2.clone()]));
    }

    /// ③ `Consume` + 值是**容器**:集合**不变** —— 取走型拿到的是容器,
    /// 不是某个 `ERR` ⇒ 没有「它拿到的那一个」可取。
    #[test]
    fn consume_of_a_container_leaves_the_collection_untouched() {
        let e1 = err("e1");
        let e2 = err("e2");
        let container = Value::Array(vec![e1.clone(), e2.clone()]);
        let env = env_with("a", container.clone(), vec![e1.clone(), e2.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::Consume)
            .expect("binding exists");

        assert_eq!(value, container);
        assert!(carried.is_empty());
        // 集合**原样不动**。
        assert_eq!(env.cell_token("a"), Some(vec![e1.clone(), e2.clone()]));
    }

    // ── [G-02] `ConsumeCarry`:剥壳交还的那一个 `ERR` 要**跟着结果继续** ──
    //
    // 剥掉 `OK` 外壳拿到的内层 `ERR` 按 §8.6.5「值语义即新身份」是**新生**的
    // ⇒ 它**自己未消费** ⇒ 它的未消费状态必须随在途值走 ⇒ 否则它**无处可寻**
    // (源绑定已被取走、在途值又不带)⇒ 那个 `ERR` **一条 `E0103` 都不报**。

    /// [G-02] `ConsumeCarry` + 值是 `OK(ERR(..))`:取走内层那个 `ERR`
    /// **并携带**它。
    #[test]
    fn consume_carry_takes_the_inner_err_and_carries_it() {
        let e1 = err("e1");
        let ok_err = Value::Ok(Box::new(e1.clone()));
        let env = env_with("a", ok_err.clone(), vec![e1.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::ConsumeCarry)
            .expect("binding exists");

        assert_eq!(value, ok_err);
        // 携带**那一个**(被取走的那个),不是源集合的其余部分。
        assert_eq!(carried, vec![e1.clone()]);
        // 源绑定**空**(它已被取走)。
        assert_eq!(env.cell_token("a"), Some(Vec::<Value>::new()));
    }

    /// [G-02] `ConsumeCarry` + 值**本身**是 `ERR`:取走它但**不携带** ——
    /// 消费者要么处理它(`ERR_PAYLOAD` / `WRAP`)、要么让它逃逸成返回值
    /// (`TRY` / `UNWRAP_OR`)⇒ 那份状态**已被处置**。
    #[test]
    fn consume_carry_on_a_plain_err_does_not_carry() {
        let e1 = err("e1");
        let env = env_with("a", e1.clone(), vec![e1.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::ConsumeCarry)
            .expect("binding exists");

        assert_eq!(value, e1);
        assert!(carried.is_empty());
        assert_eq!(env.cell_token("a"), Some(Vec::<Value>::new()));
    }

    /// [G-02] **旧语义不许被放宽**:`Consume` 在 `OK(ERR(..))` 形态下**仍然
    /// 不携带** —— 内建调用路径(`UNWRAP` / `UNWRAP_OR`)另有自己的结果承接
    /// 机制 ⇒ 两套机制叠加会**同一个 `ERR` 报两条**(违 §8.6.5)。
    #[test]
    fn consume_on_ok_wrapped_err_still_carries_nothing() {
        let e1 = err("e1");
        let ok_err = Value::Ok(Box::new(e1.clone()));
        let env = env_with("a", ok_err.clone(), vec![e1.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::Consume)
            .expect("binding exists");

        assert_eq!(value, ok_err);
        assert!(carried.is_empty());
        assert_eq!(env.cell_token("a"), Some(Vec::<Value>::new()));
    }

    /// [G-02] `ConsumeCarry` + 值是**容器**:集合**不变**、**不携带** ——
    /// 取走型拿到的是容器,不是某个 `ERR` ⇒ 没有「它拿到的那一个」可取。
    #[test]
    fn consume_carry_of_a_container_leaves_the_collection_untouched() {
        let e1 = err("e1");
        let e2 = err("e2");
        let container = Value::Array(vec![e1.clone(), e2.clone()]);
        let env = env_with("a", container.clone(), vec![e1.clone(), e2.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::ConsumeCarry)
            .expect("binding exists");

        assert_eq!(value, container);
        assert!(carried.is_empty());
        assert_eq!(env.cell_token("a"), Some(vec![e1.clone(), e2.clone()]));
    }

    /// [G-02] `ConsumeCarry` + 源集合里**没有**那个 `ERR` ⇒ **不携带** ——
    /// 「两种形态都只在它**确实在源集合里**时才移除」(§8.6.3)。
    #[test]
    fn consume_carry_carries_nothing_when_the_source_does_not_hold_it() {
        let e1 = err("e1");
        let e2 = err("e2");
        let ok_err = Value::Ok(Box::new(e1.clone()));
        let env = env_with("a", ok_err.clone(), vec![e2.clone()]);

        let (value, carried) = env
            .read_binding("a", ReadMode::ConsumeCarry)
            .expect("binding exists");

        assert_eq!(value, ok_err);
        assert!(carried.is_empty());
        // 源集合**原样**(那个 `ERR` 不在里面 ⇒ 什么都没被取走)。
        assert_eq!(env.cell_token("a"), Some(vec![e2.clone()]));
    }

    /// ④ **并集**:两侧的 `ERR` **全部保留** —— 不是「保留第一个」,也不是
    /// 「后者覆盖前者」。
    #[test]
    fn union_keeps_every_element_from_both_sides() {
        let e1 = err("e1");
        let e2 = err("e2");

        let merged = union_unconsumed(std::slice::from_ref(&e1), std::slice::from_ref(&e2));

        assert_eq!(merged, vec![e1.clone(), e2.clone()]);
    }

    /// ④ 并集是**多重集**口径:内容相同、来源不同的两个 `ERR` **不去重**
    /// (身份判据要到 X-06 第二步才有;去重会漏报)。
    #[test]
    fn union_keeps_duplicates_because_identity_is_not_settled_yet() {
        let e1 = err("e1");

        let merged = union_unconsumed(std::slice::from_ref(&e1), std::slice::from_ref(&e1));

        assert_eq!(merged, vec![e1.clone(), e1.clone()]);
    }

    /// ④ 并集的结果**原样**落在接收位置上(绑定)—— 绑定**不覆盖、不丢弃**。
    #[test]
    fn a_binding_receives_the_union_of_two_sources() {
        let e1 = err("e1");
        let e2 = err("e2");
        let mut env = Env::new();

        let merged = union_unconsumed(std::slice::from_ref(&e1), std::slice::from_ref(&e2));
        env.set_local_with_token("a", e1.clone(), merged, None);

        assert_eq!(env.cell_token("a"), Some(vec![e1.clone(), e2.clone()]));
    }

    /// 丢弃点③ 的素材:一个绑定持有**多个**未消费 `ERR` ⇒ 逐**个**报告,
    /// 顺序 = **出现序**(§8.6「诊断按它们在容器中的出现序排列」)。
    #[test]
    fn scope_unconsumed_reports_every_err_of_a_binding_in_appearance_order() {
        let e1 = err("e1");
        let e2 = err("e2");
        let mut env = Env::new();
        env.set_local_with_token("a", e1.clone(), vec![e1.clone(), e2.clone()], None);

        let pending = env.scope_unconsumed();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "a");
        assert_eq!(pending[0].1, vec![e1.clone(), e2.clone()]);
    }

    /// 没有未消费 `ERR` 的绑定**不出现**在丢弃点的素材里。
    #[test]
    fn a_binding_without_unconsumed_errs_is_not_reported() {
        let mut env = Env::new();
        env.set_local("a", err("e1"));

        assert!(env.scope_unconsumed().is_empty());
        assert_eq!(env.cell_token("a"), Some(Vec::<Value>::new()));
    }
}
