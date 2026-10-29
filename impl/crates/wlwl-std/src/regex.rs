//! [v0.11.3 M6 / addendum-02 W-02 + W-03] `wlwl:std.regex` —— RE2 式线性时间正则(R2)。
//!
//! 本语言第一条模式匹配能力,而**执行时间对输入线性**。规范 §14 记着:
//! 「主流 stdlib 全员标配,而本语言零模式匹配能力」—— 功能清单里唯一的
//! 全员标配空白。
//!
//! ## 线性时间从哪来:Thompson 构造 + Pike VM
//!
//! 1. **解析**成 AST(无回溯 / 无反向引用 / 无 lookahead);
//! 2. **Thompson 构造**成 NFA 指令数组(量词靠展开成 `Split` / `Jump`,不靠循环);
//! 3. **Pike VM** 一次扫描输入,线程列表按优先级去重 ⇒ 复杂度
//!    **O(输入长度 × 指令数 × 组数)**,与模式里的量词嵌套**无关**。
//!
//! 这条是本成员存在的**唯一理由**:一个回溯引擎也能让全部常规用例全绿,而
//! `(a*)*b` 之类经典指数爆炸模式在它上面是灾难。契约里那条「病态输入 ≤
//! 50 ms」是把它钉死的断言。
//!
//! ## 六条语义(addendum-02 §3.7,已回源核对)
//!
//! 1. **左最早(leftmost-first)**,**不是**左最长。依据:`re2.h` 的
//!    `longest_match (false) search for longest match, **not first match**`,
//!    以及 Go `regexp` 的文档「leftmost-first … same semantics that Perl,
//!    Python, and other implementations use」。左最长是 RE2 的 **POSIX 模式**。
//!    ⇒ 线性时间由 NFA 模拟保证,**与选哪条语义无关**。
//! 2. **编译期失败 = 原生诊断 `E0030` 中止**(带列号);`TRY_RE` 本批不做 ——
//!    加成员是附加式、改失败形态是破坏式,先定死破坏那条。
//! 3. **空匹配**:紧贴前一个匹配的**跳过**不报;其余推进一个码点(这正是
//!    `RE_FIND_ALL("a*", "bab")` 不无限循环的保证)。⚠️ Python 的 `finditer`
//!    在这条上与 RE2/Go **不同**(它会报紧贴的空匹配),本成员跟 RE2/Go。
//! 4. `(?i)` **只折叠 ASCII**;Unicode 折叠走 `std.text`。
//!    ⚠⚠ **作用域比 RE2 / Go 窄**(`D13-063` 补记):本实现是「到**本分支**
//!    结束」,而 RE2 `doc/syntax.txt` 原文是 `(?flags)` = "set flags **within
//!    current group**"(中文版「until outer paren closes」)⇒ RE2 / Go 是
//!    「到**所在 group 的右括号**」;Go `regexp/syntax` 的 `parse.go` 同证
//!    (标志在闭合括号处才恢复,`|` **不恢复**)。`(a(?i)b|c)` 上两者**结果不同**:
//!    RE2 让 `c` 也忽略大小写,本实现不。⚠ 这条差分**此前既没在这里也没在
//!    §16.4 成文**,而 `parse_concat` 里那句写着「RE2 的口径」的代码注释
//!    **是错的** —— 本批一并改正。
//! 5. `.` 默认**不含换行**(`(?s)` 打开);`^`/`$` 默认锚**整个输入**,`(?m)`
//!    切到行锚点。⚠ **刻意不抄 RE2 的 `m` 命名**(它与 Perl 相反)。
//!    ⚠ `$` 是 **`\z` 语义、不是 `\Z`**(RE2 `doc/syntax.txt` 原文:"text end
//!    (**like `\z` not `\Z`**)")⇒ **不匹配结尾换行之前**。这与 RE2 / Go
//!    **一致**;与 Perl / Python 的 `$` 不同(后者在结尾换行**前**也成立,
//!    而 RE2 把那个形态叫 `\Z` 并标注 **NOT SUPPORTED**)。
//!    ⇒ `review-1` 把这条列为「与 RE2 的语义分歧」是**归错对象**,本批更正。
//! 6. `\p{…}` 与命名组**本批不做**(不引入 UCD 依赖,与全局 G1/G2 一致)。
//!
//! ## 不做(与线性时间互斥,不是「以后再加」)
//!
//! 回溯引用 `(?P=…)` / `\1`、lookahead `(?=…)` / `(?!…)`、lookbehind、
//! PCRE 兼容、反向正则、字素簇语义,以及**零宽断言转义** `\b` `\B` `\A` `\z`
//! `\Z`。**它们在解析期一律 `E0030`** —— 必须拒,否则线性承诺当场破。
//!
//! ⚠ **这一句曾经是空头支票**(D13-030 / N-021):`\b` `\B` `\A` `\z` `\Z`
//! 落到 `unescape_literal` 的兜底分支,被当成**普通字面量**放行
//! (`RE("\\b")` 匹配的是字符 `b`)。零宽断言在 Thompson NFA 上要**分支**
//! 而不是消费字符 —— 那会让一次抵达同时对应「锚点前」与「锚点后」两条
//! 路径,leftmost-first 的裁决键随之不再是良定义的。所以是**不能实现**,
//! 不只是「以后再加」。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.regex",
    functions: &[
        ("RE", re as StdFn),
        ("RE_TEST", re_test as StdFn),
        // [W-29 / v0.12.1 · 需求 H-5] `RE_MATCHES_FULL` = `RE_TEST` 的**别名**。
        //
        // ⚠ **只加别名,不改 `RE_TEST` 的语义**(一致性册 `:224` 禁删禁弃用):
        //   `RE_TEST` 是**整串锚定**(两端锚定、忽略 `(?m)`)⇒ 名字容易被读成
        //   「包含匹配」⇒ WAF 规则用它会**永远不命中**(静默漏防)。
        //   ⇒ `RE_MATCHES_FULL` 把「整串锚定」这件事写进名字里,**语义逐字同源**
        //   (同一个函数体,见下方 `re_matches_full`)。
        ("RE_MATCHES_FULL", re_matches_full as StdFn),
        ("RE_SEARCH", re_search as StdFn),
        ("RE_FIND_ALL", re_find_all as StdFn),
        ("RE_REPLACE", re_replace as StdFn),
        ("RE_SPLIT", re_split as StdFn),
        ("RE_GROUP_COUNT", re_group_count as StdFn),
    ],
};

// ── 字符类 ─────────────────────────────────────────────────────────────

/// 一个码点集合:排序、不重叠的闭区间。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSet {
    ranges: Vec<(u32, u32)>,
}

impl ClassSet {
    fn empty() -> Self {
        Self { ranges: Vec::new() }
    }

    fn single(c: char) -> Self {
        let v = c as u32;
        Self {
            ranges: vec![(v, v)],
        }
    }

    fn full() -> Self {
        Self {
            ranges: vec![(0, u32::MAX)],
        }
    }

    fn add(&mut self, lo: u32, hi: u32) {
        if lo <= hi {
            self.ranges.push((lo, hi));
        }
    }

    fn add_char(&mut self, c: char) {
        self.add(c as u32, c as u32);
    }

    /// ASCII 折叠:给每个 ASCII 字母补另一半。**只 ASCII** —— Unicode 折叠
    /// 要 UCD 表,那是 03 的批次。
    fn fold_ascii_case(&mut self) {
        let mut extra = Vec::new();
        for &(lo, hi) in &self.ranges {
            let top = hi.min(0x7F);
            if lo > top {
                continue;
            }
            for v in lo..=top {
                let Some(ch) = char::from_u32(v) else {
                    continue;
                };
                if ch.is_ascii_alphabetic() {
                    let other = if ch.is_ascii_uppercase() {
                        ch.to_ascii_lowercase()
                    } else {
                        ch.to_ascii_uppercase()
                    };
                    let ov = other as u32;
                    if !(lo..=hi).contains(&ov) {
                        extra.push(ov);
                    }
                }
            }
        }
        self.ranges.extend(extra.into_iter().map(|v| (v, v)));
        self.normalise();
    }

    fn normalise(&mut self) {
        if self.ranges.len() < 2 {
            return;
        }
        self.ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(self.ranges.len());
        for &(lo, hi) in &self.ranges {
            match merged.last_mut() {
                Some(last) if lo <= last.1.saturating_add(1) => last.1 = last.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        self.ranges = merged;
    }

    pub fn matches(&self, c: char) -> bool {
        let v = c as u32;
        self.ranges.iter().any(|&(lo, hi)| v >= lo && v <= hi)
    }

    /// 补集(用于 `[^…]` / `\D \W \S`)。
    fn complement(&self) -> Self {
        let mut out = Self::empty();
        let mut prev: Option<u32> = None;
        for &(lo, hi) in &self.ranges {
            let start = prev.map(|p| p.saturating_add(1)).unwrap_or(0);
            if start < lo {
                out.add(start, lo - 1);
            }
            prev = Some(hi);
        }
        if let Some(e) = prev {
            if e < u32::MAX {
                out.add(e + 1, u32::MAX);
            }
        }
        out
    }
}

/// ⚠ 零宽断言的转义形式:`\b` `\B` `\A` `\z` `\Z`。**一律拒收**(见
/// [`zero_width_rejection`])。
///
/// ⚠ 为什么 `\A` `\z` `\Z` 也在列:它们在 Perl / Python / PCRE / RE2 里都是
/// **正宗的零宽锚点**,而本成员**不实现**它们。若静默放行,`\Aabc` 会变成
/// 「匹配字面量 `Aabc`」—— 比报错更坏的是它**看起来能用**。
const ZERO_WIDTH_ESCAPES: [char; 5] = ['b', 'B', 'A', 'z', 'Z'];

/// `\` 后的字符若是零宽断言 ⇒ 给出拒收理由(否则 `None`,继续按字面量走)。
///
/// 🔴 [D13-030 / N-021] 此前这些转义**静默变成字面量**:`predefined()` 只认
/// `d w s D W S`,`unescape_literal()` 的兜底是 `other => other` ⇒
/// `RE("\\b")` 匹配的是**字符 `b`**,`RE("a\\Ac")` 匹配字面量 `"Ac"`。
///
/// ⚠ 这不是「宽容」,是**违自家规范**:本文件头第 38 行写着「零宽断言 … 在
/// 解析期一律 `E0030`」,规范 §16.3 同样列了「零宽断言」并要求 `E0030`。
/// **实现与规范冲突**,且方向是静默给错结果 ⇒ 按 §16 决策 2 拒。
///
/// ⚠ 为什么不能实现它们而要拒:零宽断言在 Thompson NFA 上要**分支**而不是
/// 消费字符 —— 那正是让优先级裁决失效的构造(一次抵达可以同时是「走到
/// `\b` 前的」与「走到 `\b` 后的」两条路径),leftmost-first 的裁决键就
/// 不再是良定义的。与 `MAX_NEST` / `MAX_PROG` 同族:宁可拒,不要静默。
fn zero_width_rejection(name: char) -> Option<String> {
    ZERO_WIDTH_ESCAPES.contains(&name).then(|| {
        format!("zero-width assertion `\\{name}` is not supported (it would break linear time)")
    })
}

fn unescape_literal(c: char) -> char {
    match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        'f' => '\u{c}',
        'v' => '\u{b}',
        'a' => '\u{7}',
        other => other,
    }
}

/// 预定义类。**全部 ASCII-only**:`\d` = `[0-9]`、`\w` = `[0-9A-Za-z_]`、
/// `\s` = `[ \t\n\r\f\v]`;大写形式取补集。
fn predefined(name: char) -> Option<ClassSet> {
    let mut s = ClassSet::empty();
    match name {
        'd' => s.add('0' as u32, '9' as u32),
        'w' => {
            s.add('0' as u32, '9' as u32);
            s.add('A' as u32, 'Z' as u32);
            s.add('a' as u32, 'z' as u32);
            s.add('_' as u32, '_' as u32);
        }
        's' => {
            for c in [' ', '\t', '\n', '\r', '\u{c}', '\u{b}'] {
                s.add_char(c);
            }
        }
        'D' | 'W' | 'S' => {
            let lower = match name {
                'D' => predefined('d')?,
                'W' => predefined('w')?,
                _ => predefined('s')?,
            };
            s = lower.complement();
        }
        _ => return None,
    }
    Some(s)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorKind {
    Start { multiline: bool },
    End { multiline: bool },
}

impl AnchorKind {
    fn holds(&self, input: &[char], at: usize) -> bool {
        match self {
            AnchorKind::Start { multiline } => {
                at == 0 || (*multiline && at <= input.len() && input.get(at - 1) == Some(&'\n'))
            }
            AnchorKind::End { multiline } => {
                at == input.len()
                    || (*multiline && at < input.len() && input.get(at) == Some(&'\n'))
            }
        }
    }
}

// ── 解析 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    ignore_case: bool,
    dot_all: bool,
    multiline: bool,
}

#[derive(Debug)]
enum Ast {
    Empty,
    Class(ClassSet),
    Anchor(AnchorKind),
    Concat(Vec<Ast>),
    Alt(Vec<Ast>),
    Repeat {
        node: Box<Ast>,
        min: u32,
        max: u32,
        greedy: bool,
    },
    Group {
        index: Option<usize>,
        node: Box<Ast>,
    },
}

/// 量词计数上限(Go 的 `maxRepeat` 同数量级):防「模式把**编译期**打爆」,
/// 与**输入长度**无关 —— 线性承诺说的是输入。
const MAX_REPEAT: u32 = 1000;
/// 编译后指令数上限(Go 的 `maxSize` 思路):宁可拒收,不要在编译期爆掉。
const MAX_PROG: usize = 64 * 1024;
/// 括号嵌套深度上限。
///
/// ⚠ [D13-024 / N-005]这道闸**此前不存在**。`MAX_REPEAT` / `MAX_PROG` 拦得住
/// 「重复太多」和「编译产物太大」,但 2 万层 `((((…))))` 只产出 2 万条指令,
/// 两道闸都过得去 —— 于是解析器的 `parse_group → parse_alt → parse_concat →
/// parse_atom → parse_group` 递归、`Compiler::compile` 的递归、以及 `Ast`
/// 析构时的递归**全部爆栈**。栈溢出是 `abort` 不是 unwind,`catch_unwind`
/// 接不住 ⇒ **纯输入打死整个解释器进程**,而不是 §16 决策 2 承诺的
/// 「语法错 → E0030 优雅中止」。
///
/// ⚠ **取值是量出来的,不是抄来的。** Go `regexp/syntax` 用 1000(`maxNestDepth`),
/// 本仓**不能照抄**:每层嵌套要占 `parse_alt`/`parse_concat`/`parse_atom`/
/// `parse_group` 四个栈帧,`cargo test` 的测试线程默认只有 **2 MiB** 栈,而
/// 门禁跑的是 **debug 构建**(帧比 release 大一个量级)。实测 1000 层直接
/// `STATUS_STACK_OVERFLOW`。取 256 后留出约 4 倍余量,且离日常模式的手写嵌套
/// 深度(实测极少超过 20)仍有数量级差距。
///
/// 同仓 `json/lexer.rs` 的 `MAX_DEPTH = 128` 是同一族闸的另一个实例 —— 那道
/// 128 是本仓既定的深度闸口径,这里同族但取值不同,因为两者的每层栈开销不同。
const MAX_NEST: usize = 256;

struct Parser {
    pat: Vec<char>,
    pos: usize,
    groups: usize,
    /// 当前括号嵌套层数(见 `MAX_NEST`)。
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.pat.get(self.pos).copied()
    }

    fn at(&self, n: usize) -> Option<char> {
        self.pat.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// 错误消息带**列号**(1 起),因为 `E0030` 的价值全在这条消息上。
    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!(
            "RE: {what} at column {} of the pattern",
            (self.pos + 1).min(self.pat.len().max(1))
        ))
    }

    fn parse_alt(&mut self, flags: Flags) -> Result<Ast, String> {
        let mut branches = vec![self.parse_concat(flags)?];
        while self.peek() == Some('|') {
            self.pos += 1;
            branches.push(self.parse_concat(flags)?);
        }
        Ok(if branches.len() == 1 {
            branches.pop().expect("one branch")
        } else {
            Ast::Alt(branches)
        })
    }

    fn parse_concat(&mut self, mut flags: Flags) -> Result<Ast, String> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None | Some('|') | Some(')') => break,
                // 标志组 `(?i)` / `(?s)` / `(?m)`:作用于**本分支的余下部分**。
                // 判定点必须在 `parse_atom` **之前** —— 否则 `(` 已经被
                // `parse_atom` 吃掉并进了 `parse_group`,而 group 语义是
                // 「括号内的子模式」,会把余下模式整个丢掉。
                //
                // 🔴 [D13-063 / N-025]⚠ **此前这条注释写着「RE2 的口径」——
                //   那句话是错的。**本批查过 RE2 `doc/syntax.txt` 原文:`(?flags)`
                //   = "set flags **within current group**"(中文版「set flags
                //   **until outer paren closes**」)⇒ 标志一直有效到**所在 group
                //   的右括号**。Go `regexp/syntax` 的 `parse.go` 同证:标志在
                //   **闭合括号**处才恢复,`|` **不恢复**。
                //   ⇒ 在 `(a(?i)b|c)` 上 RE2 / Go 让**两个分支都**忽略大小写,
                //   而本实现(`flags` 是 `parse_concat` 的**局部**参数、每条分支
                //   一份拷贝)只让 `b` 忽略大小写。**这是一处真实的语义差分**,
                //   此前既没写进文件头也没成文 —— 现补进 §16.4 与文件头。
                //   改成 RE2 口径要在**整个 group 层**共享 flags 而非每分支拷贝,
                //   属结构变更,已登记待独立立项,本批不动。
                Some('(')
                    if self.at(1) == Some('?') && matches!(self.at(2), Some('i' | 's' | 'm')) =>
                {
                    let f = self.at(2).expect("checked");
                    self.pos += 3; // 跳过 "(?x"
                    if self.peek() != Some(')') {
                        return self.err("a flag group must be exactly (?i), (?s) or (?m)");
                    }
                    self.pos += 1;
                    match f {
                        'i' => flags.ignore_case = true,
                        's' => flags.dot_all = true,
                        _ => flags.multiline = true,
                    }
                    continue;
                }
                _ => {}
            }
            let atom = self.parse_atom(flags)?;
            items.push(self.parse_quantifier(atom)?);
        }
        Ok(match items.len() {
            0 => Ast::Empty,
            1 => items.pop().expect("one item"),
            _ => Ast::Concat(items),
        })
    }

    fn parse_quantifier(&mut self, atom: Ast) -> Result<Ast, String> {
        let (min, max) = match self.peek() {
            Some('*') => {
                self.pos += 1;
                (0, u32::MAX)
            }
            Some('+') => {
                self.pos += 1;
                (1, u32::MAX)
            }
            Some('?') => {
                self.pos += 1;
                (0, 1)
            }
            Some('{') => match self.parse_bounds()? {
                Some(b) => b,
                None => return Ok(atom),
            },
            _ => return Ok(atom),
        };
        if matches!(atom, Ast::Anchor(_)) {
            return self.err("a quantifier cannot be applied to an anchor");
        }
        if max != u32::MAX && min > max {
            return self.err("a quantifier min is greater than its max");
        }
        let greedy = if self.peek() == Some('?') {
            self.pos += 1;
            false
        } else {
            true
        };
        Ok(Ast::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
        })
    }

    /// `{` 只有确实是 `{m}` / `{m,}` / `{m,n}` 形态时才是量词,否则当字面量
    /// (`a{b` 是合法模式)。与 Go / RE2 一致。
    fn parse_bounds(&mut self) -> Result<Option<(u32, u32)>, String> {
        let start = self.pos;
        self.pos += 1; // '{'
        let mut lo = String::new();
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            lo.push(self.bump().expect("digit"));
        }
        if lo.is_empty() {
            self.pos = start;
            return Ok(None);
        }
        let min: u32 = lo
            .parse()
            .map_err(|_| "RE: repeat count is too large".to_string())?;
        let max = if self.peek() == Some(',') {
            self.pos += 1;
            let mut hi = String::new();
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                hi.push(self.bump().expect("digit"));
            }
            if hi.is_empty() {
                u32::MAX
            } else {
                hi.parse()
                    .map_err(|_| "RE: repeat count is too large".to_string())?
            }
        } else {
            min
        };
        if self.peek() != Some('}') {
            self.pos = start;
            return Ok(None);
        }
        self.pos += 1;
        // `a{2,}` 的 max 是 u32::MAX(无上界),**不能**拿它去比上限 ——
        // 那会把合法的「2 次以上」误拒。只在有界时检查上界。
        if min > MAX_REPEAT || (max != u32::MAX && max > MAX_REPEAT) {
            return self.err(&format!("repeat count exceeds {MAX_REPEAT}"));
        }
        Ok(Some((min, max)))
    }

    fn parse_atom(&mut self, flags: Flags) -> Result<Ast, String> {
        let Some(c) = self.bump() else {
            return Ok(Ast::Empty);
        };
        match c {
            '(' => self.parse_group(flags),
            '[' => self.parse_class(flags),
            '.' => Ok(Ast::Class(if flags.dot_all {
                ClassSet::full()
            } else {
                ClassSet::full().complement_of_char('\n')
            })),
            '^' => Ok(Ast::Anchor(AnchorKind::Start {
                multiline: flags.multiline,
            })),
            '$' => Ok(Ast::Anchor(AnchorKind::End {
                multiline: flags.multiline,
            })),
            '\\' => self.parse_escape(flags),
            '*' | '+' | '?' => self.err("a quantifier has nothing to repeat"),
            other => Ok(Ast::Class(self.literal(other, flags))),
        }
    }

    fn literal(&self, c: char, flags: Flags) -> ClassSet {
        let mut s = ClassSet::single(c);
        if flags.ignore_case {
            s.fold_ascii_case();
        }
        s
    }

    /// 解析 `\` 后的字符。**反向引用 `\1`…`\9` 一律拒** —— 与线性时间互斥。
    fn parse_escape(&mut self, flags: Flags) -> Result<Ast, String> {
        let Some(c) = self.bump() else {
            return self.err("pattern ends with a backslash");
        };
        if c.is_ascii_digit() {
            return self.err("backreferences are not supported (they would break linear time)");
        }
        // ⚠ 零宽断言必须**拒**,不能落到下面的 `unescape_literal` 兜底里变字面量
        // (D13-030 / N-021)。位置在 `predefined` 之前 —— 这五个字符不在预定义
        // 集合里,顺序只影响可读性,但写在这里更醒目。
        if let Some(why) = zero_width_rejection(c) {
            return self.err(&why);
        }
        if let Some(s) = predefined(c) {
            // 预定义类**不做 ASCII 折叠**:Python 的 `re.ASCII` 下 `(?i)\w`
            // 也不折叠数字与下划线,保持一致。
            return Ok(Ast::Class(s));
        }
        Ok(Ast::Class(self.literal(unescape_literal(c), flags)))
    }

    /// 括号组。⚠ **这是解析器唯一的自递归入口**(唯一的另外一条递归路径
    /// `parse_class` 不嵌套),所以深度闸开在这里就够 —— 见 `MAX_NEST`。
    fn parse_group(&mut self, flags: Flags) -> Result<Ast, String> {
        let inner = flags;
        let index;
        if self.peek() == Some('?') {
            self.pos += 1;
            match self.bump() {
                Some(':') => index = None,
                Some('=') | Some('!') => {
                    return self.err("lookahead is not supported (it would break linear time)");
                }
                Some('<') => {
                    return self.err("lookbehind and named groups are not supported in this batch");
                }
                Some('P') => {
                    return self
                        .err("backreferences are not supported (they would break linear time)");
                }
                // `(?i)` / `(?s)` / `(?m)` 在 `parse_concat` 里处理(它们作用于本分支
                // 余下部分,不是一个独立的组)。走到这里说明形态不对。
                // ⚠ 关于那个「本分支余下部分」的口径:它**比 RE2 / Go 窄**,见
                //   `parse_concat` 里 `[D13-063 / N-025]` 那段(RE2 是
                //   「到所在 group 的右括号」)。
                Some('i' | 's' | 'm') => {
                    return self.err("a flag group must be exactly (?i), (?s) or (?m)");
                }
                _ => return self.err("unsupported group syntax"),
            }
        } else {
            if self.peek().is_none() {
                return self.err("pattern ends with an unclosed group");
            }
            index = Some(self.groups + 1);
            self.groups += 1;
        }
        self.depth += 1;
        if self.depth > MAX_NEST {
            self.depth -= 1;
            return self.err("pattern nests too deeply");
        }
        let node = self.parse_alt(inner);
        self.depth -= 1;
        let node = node?;
        if self.bump() != Some(')') {
            return self.err("missing ')'");
        }
        Ok(Ast::Group {
            index,
            node: Box::new(node),
        })
    }

    fn parse_class(&mut self, flags: Flags) -> Result<Ast, String> {
        let negated = self.peek() == Some('^');
        if negated {
            self.pos += 1;
        }
        let mut set = ClassSet::empty();
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return self.err("unterminated character class");
            };
            if c == ']' && !first {
                self.pos += 1;
                break;
            }
            first = false;
            self.pos += 1;
            let lo = if c == '\\' {
                let Some(e) = self.bump() else {
                    return self.err("pattern ends with a backslash");
                };
                if e.is_ascii_digit() {
                    return self.err("backreferences are not supported");
                }
                // ⚠ 字符类里**同样**拒,否则 `[\b]` 会变成「字符 `b` 的类」——
                // 与类外 `\b` 静默变字面量是**同一个 bug 的两个入口**
                // (D13-030 / N-021)。Go `regexp/syntax` 在类内也报
                // `invalid escape sequence \b`,口径一致。
                if let Some(why) = zero_width_rejection(e) {
                    return self.err(&why);
                }
                if let Some(sub) = predefined(e) {
                    // `[\d]` 是合法形态:直接并入。**否定情形由整类的 `^`
                    // 统一处理**(收尾处的 `set.complement()`),类内**绝不能**
                    // 先取一次补 —— 补两次等于没补。
                    //
                    // ⚠ [D13-019 / N-002]此前这里写着
                    // `let sub = if negated { sub.complement() } else { sub };`,
                    // 与紧跟的上一行注释**直接矛盾**(注释说统一处理,代码先补了)。
                    // 后果:`[^\d]` 先取补并入、再被整类取补 ⇒ 结果等于 `\d` 本身,
                    // `RE_TEST(RE("[^\\w]"), " ")` 会答 FALSE。而全仓唯一的取反
                    // 字符类用例是纯字面的 `[^a-c]` —— 契约只钉了「看起来覆盖了
                    // 取反」的形状,没钉预定义类这一支。
                    set.ranges.extend(sub.ranges);
                    // 🔴 [D13-031 / N-022]如果**下一个** token 就是一个区间连字符,
                    // 说明这个预定义类被当成了区间的**低端** —— 而预定义类不是单个
                    // 码点,`[\d-x]` 没有任何合法含义。此前是 `continue` 掉、把 `-`
                    // 留给下一轮当字面量,于是类悄悄变成了「数字 ∪ `-`..`x`」。
                    if self.peek() == Some('-') && self.at(1).is_some() && self.at(1) != Some(']') {
                        return self.err(&range_endpoint_msg(e));
                    }
                    continue;
                }
                unescape_literal(e)
            } else {
                c
            };
            if self.peek() == Some('-') && self.at(1) != Some(']') && self.at(1).is_some() {
                self.pos += 1;
                let Some(mut hi) = self.bump() else {
                    return self.err("unterminated character class");
                };
                if hi == '\\' {
                    let Some(e) = self.bump() else {
                        return self.err("pattern ends with a backslash");
                    };
                    // 🔴 [D13-031 / N-022]预定义类出现在区间**高端**。
                    // 此前这里只过 `unescape_literal`,`\d` 被取成字符 `'d'`
                    // (0x64) ⇒ `[a-\d]` 的区间 `a-d` 顺利通过校验 ⇒
                    // `RE_TEST(RE("[a-\\d]"), "c")` 答 TRUE。Go / Python 在这里
                    // 报 `invalid character class range`,本成员此前**不报**。
                    //
                    // ⚠ 零宽断言一并拒:它落到同一个兜底分支、同样会被当成码点
                    //   (`[0-\b]` ⇒ 区间 `0`..`'b'`)。与 D13-030 同源。
                    if let Some(why) = range_endpoint_rejection(e) {
                        return self.err(&why);
                    }
                    hi = unescape_literal(e);
                }
                if (hi as u32) < (lo as u32) {
                    return self.err("character class range is reversed");
                }
                set.add(lo as u32, hi as u32);
            } else {
                set.add_char(lo);
            }
        }
        set.normalise();
        if flags.ignore_case {
            set.fold_ascii_case();
        }
        if negated {
            set = set.complement();
        }
        Ok(Ast::Class(set))
    }
}

impl ClassSet {
    /// 去掉一个码点(给 `.` 用:全集减换行)。
    fn complement_of_char(&self, c: char) -> Self {
        let mut s = ClassSet::empty();
        let v = c as u32;
        if v > 0 {
            s.add(0, v - 1);
        }
        if v < u32::MAX {
            s.add(v + 1, u32::MAX);
        }
        s
    }
}

// ── Thompson 构造 ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Inst {
    Class(ClassSet),
    Split(usize, usize),
    Jump(usize),
    Save(usize),
    Assert(AnchorKind),
    Match,
}

#[derive(Debug)]
pub struct Program {
    insts: Vec<Inst>,
    groups: usize,
    /// 模式对象本身(成员需要把 `pattern` 带回给调用方看)。
    source: String,
}

impl Program {
    /// 指令数。复杂度契约要断言它**与输入长度无关**。
    pub fn len(&self) -> usize {
        self.insts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.insts.is_empty()
    }

    pub fn group_count(&self) -> usize {
        self.groups
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

struct Compiler {
    insts: Vec<Inst>,
}

impl Compiler {
    fn emit(&mut self, i: Inst) -> Result<usize, String> {
        if self.insts.len() >= MAX_PROG {
            return Err(format!(
                "RE: compiled pattern exceeds {MAX_PROG} instructions (too many repetitions)"
            ));
        }
        self.insts.push(i);
        Ok(self.insts.len() - 1)
    }

    fn compile(&mut self, ast: &Ast) -> Result<(), String> {
        match ast {
            Ast::Empty => Ok(()),
            Ast::Class(cs) => {
                self.emit(Inst::Class(cs.clone()))?;
                Ok(())
            }
            Ast::Anchor(a) => {
                self.emit(Inst::Assert(*a))?;
                Ok(())
            }
            Ast::Concat(items) => {
                for it in items {
                    self.compile(it)?;
                }
                Ok(())
            }
            Ast::Group { index, node } => {
                if let Some(i) = *index {
                    self.emit(Inst::Save(2 * i))?;
                    self.compile(node)?;
                    self.emit(Inst::Save(2 * i + 1))?;
                } else {
                    self.compile(node)?;
                }
                Ok(())
            }
            Ast::Alt(branches) => self.compile_alt(branches),
            Ast::Repeat {
                node,
                min,
                max,
                greedy,
            } => self.compile_repeat(node, *min, *max, *greedy),
        }
    }

    fn compile_alt(&mut self, branches: &[Ast]) -> Result<(), String> {
        let mut jumps = Vec::new();
        for (n, b) in branches.iter().enumerate() {
            if n + 1 == branches.len() {
                self.compile(b)?;
                break;
            }
            let split_at = self.emit(Inst::Split(0, 0))?;
            let body_start = self.insts.len();
            self.compile(b)?;
            jumps.push(self.emit(Inst::Jump(0))?);
            let next = self.insts.len();
            self.insts[split_at] = Inst::Split(body_start, next);
        }
        let end = self.insts.len();
        for j in jumps {
            self.insts[j] = Inst::Jump(end);
        }
        Ok(())
    }

    fn compile_repeat(
        &mut self,
        node: &Ast,
        min: u32,
        max: u32,
        greedy: bool,
    ) -> Result<(), String> {
        for _ in 0..min {
            self.compile(node)?;
        }
        // **无上界量词不展开副本** —— 那会是 `u32::MAX` 次复制(编译期就爆)。
        // 它由下面的「循环 + Split」表达,副本数恒为 0。
        let optional = if max == u32::MAX { 0 } else { max - min };
        let mut splits = Vec::new();
        for _ in 0..optional {
            let split_at = self.emit(Inst::Split(0, 0))?;
            let body_start = self.insts.len();
            self.compile(node)?;
            splits.push(split_at);
            let next = self.insts.len();
            let (a, b) = if greedy {
                (body_start, next)
            } else {
                (next, body_start)
            };
            self.insts[split_at] = Inst::Split(a, b);
        }
        if max == u32::MAX {
            // 无上界:收尾一个 `Split(loop_start, exit)`,最后一次迭代回头。
            let loop_start = self.emit(Inst::Split(0, 0))?;
            let body_start = self.insts.len();
            self.compile(node)?;
            self.emit(Inst::Jump(loop_start))?;
            let exit = self.insts.len();
            let (a, b) = if greedy {
                (body_start, exit)
            } else {
                (exit, body_start)
            };
            self.insts[loop_start] = Inst::Split(a, b);
        }
        Ok(())
    }
}

/// 编译模式。**只在这里**把 AST 变成 NFA。
pub fn compile(pattern: &str) -> Result<Program, String> {
    let mut p = Parser {
        pat: pattern.chars().collect(),
        pos: 0,
        groups: 0,
        depth: 0,
    };
    let ast = p.parse_alt(Flags::default())?;
    if p.pos != p.pat.len() {
        return p.err("unexpected ')'");
    }
    let mut c = Compiler { insts: Vec::new() };
    c.emit(Inst::Save(0))?;
    c.compile(&ast)?;
    c.emit(Inst::Save(1))?;
    c.emit(Inst::Match)?;
    Ok(Program {
        insts: c.insts,
        groups: p.groups,
        source: pattern.to_string(),
    })
}

/// 编译成「整串匹配」用:在 AST 外包一层**文本锚**(忽略 `(?m)` —— fullmatch
/// 的语义是「整个输入被消费完」,而 `(?m)` 的 `$` 还会在内部换行处成立)。
/// 于是普通的一次 leftmost-first 搜索得到的就是整串匹配,不必在 VM 里
/// 另写一条「只在末尾接受」的分支。
pub fn compile_full(pattern: &str) -> Result<Program, String> {
    let inner = compile(pattern)?;
    Ok(full_from(&inner))
}

/// 由**已编译**的搜索版**派生**出整串版(不重新解析一遍)。
///
/// ⚠ 拆出来是为了让缓存**一次编译产出两种语义**(见 `compile_pair`):
///   若整串版重新解析一遍,「一个模式串编译了几次」就会变成 2 ——
///   那不是缓存没生效,是**计数口径**自己改了。
fn full_from(inner: &Program) -> Program {
    let mut insts = vec![Inst::Assert(AnchorKind::Start { multiline: false })];
    // ⚠ **两条 `Save` 都要保留** —— 剥掉它们就没有捕获可言,而捕获正是
    // `RE_TEST` / `RE_SEARCH` 报 `start` / `end` 的依据。这里只丢末尾那条
    // `Match`(末尾的 `Assert(End)` + `Match` 由本函数自己补)。
    insts.extend_from_slice(&inner.insts[1..inner.insts.len() - 1]);
    insts.push(Inst::Assert(AnchorKind::End { multiline: false }));
    insts.push(Inst::Match);
    Program {
        insts,
        groups: inner.groups,
        source: inner.source.clone(),
    }
}

// ── Pike VM ────────────────────────────────────────────────────────────

/// 一次匹配:码点区间的捕获(0 号是整体)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captures {
    pub slots: Vec<Option<usize>>,
}

impl Captures {
    pub fn start(&self) -> usize {
        self.slots.first().copied().flatten().unwrap_or(0)
    }

    pub fn end(&self) -> usize {
        self.slots.get(1).copied().flatten().unwrap_or(self.start())
    }

    pub fn group(&self, i: usize) -> Option<(usize, usize)> {
        let lo = self.slots.get(2 * i).copied().flatten()?;
        let hi = self.slots.get(2 * i + 1).copied().flatten()?;
        Some((lo, hi))
    }
}

/// `prio` 路径的**最大长度**。
///
/// ⚠ **封顶不损语义**:① 交替的选择深度是**模式里的分支数**,与输入长度无关,
///   64 绰绰有余(`a|ab`、`ab|a` 这类判别式只需 1~2 格);② 贪婪延伸靠的是
///   「同起点时**后到者**更完整」这条**抵达顺序**规则(见 `Hit::better_than` 的
///   第三键),它与路径长度无关 —— 所以路径打平之后仍由抵达顺序正确地选出
///   最长的那次匹配。
pub const PRIO_MAX: usize = 64;

/// 一条 NFA 线程。`prio` 是**分支选择路径**:每过一次 `Split`,高优先级
/// 支追加 `0`、低优先级支追加 `1`。
///
/// 为什么需要它:候选匹配必须在**跨输入位置**比较优先级,而列表下标做不到
/// —— 同一个 `Match` 指令会先后被不同路径抵达。要判「哪个候选来自模式里
/// 排在前面的那条分支」,唯一可比的量就是这条路径(字典序)。有了它,两条
/// 规矩各归其位:
///   * 同一起点 ⇒ 路径小者胜 ⇒ 交替 `a|ab` 取 `a`(分支 1 先试)、`ab|a` 取 `ab`;
///   * 贪婪延伸 = 多绕几轮循环(多几个前导 `0`)⇒ 字典序更小 ⇒ 胜,所以
///     `a+` 在 "aaa" 上取 `aaa` 而不是 `a`。
///
/// ⚠【N-026】**`prio` 是内联数组**(`[u8; PRIO_MAX]` + `prio_len`),不是
///   `Vec<u8>`:`Split` 分支要把它**克隆成两条支** —— 用 `Vec` 时那是**两次
///   堆分配**(每线程每步!),而路径长度本来就封顶在 64 ⇒ 内联之后克隆只是
///   **栈上拷 64 字节**,**零分配**。
/// ⚠【N-026】**`caps` 是 `Rc<Vec<..>>`**(写时复制):`Split` 分支要把捕获
///   槽位交给两条支 ⇒ `Vec` 克隆是**一次堆分配**;`Rc` 克隆只是**引用计数
///   +1**,真要改内容时 `Rc::make_mut` 才**复制-on-write**。
#[derive(Clone)]
struct Thread {
    pc: usize,
    caps: std::rc::Rc<Vec<Option<usize>>>,
    prio: [u8; PRIO_MAX],
    /// prio 的**有效长度**(≤ PRIO_MAX)。
    ///
    /// ⚠ 单独记一条,是因为内联数组的**全部** 64 字节都有默认值 ——
    /// 不记长度就分不清「路径走到第 3 格」与「路径走到第 3 格、后面全是 0」。
    prio_len: u8,
}

/// `mem::take` 借走 `clist` 槽位后留下的空壳(`advance` 的热路径要用)。
///
/// 为什么需要它:`advance` 过去是 `self.clist[k].clone()`,而 `Thread` 带两个
/// `Vec`(`caps` + `prio`)⇒ **每线程每步两次堆分配**,而这一步是整趟扫描里
/// 最热的一步。`mem::take` 换来的空壳用 `Vec::new()` —— **不分配**,所以
/// 「借走再还不还」是零成本的。
///
/// ⚠ `pc = usize::MAX` 是**故意的**:任何误读这个槽位都会立刻越界 panic。
/// 若按直觉填 `pc = 0`,误读会悄悄从 0 号指令跑出一条**假路径** ——
/// 「看起来能跑」的静默错误比当场崩难查得多。
impl Default for Thread {
    fn default() -> Self {
        Thread {
            pc: usize::MAX,
            caps: std::rc::Rc::new(Vec::new()),
            prio: [0u8; PRIO_MAX],
            prio_len: 0,
        }
    }
}

/// 一个抵达 `Match` 的候选。
#[derive(Clone)]
struct Hit {
    start: usize,
    end: usize,
    /// ⚠ Rc 包装:候选表里的 Hit 是**按左端去重后**留下的,每条都要把
    ///   捕获槽位带一份 ⇒ Vec 克隆是**一次堆分配**;Rc 克隆只是
    ///   **引用计数 +1**。⚠ 候选表是 O(不同起点数),不是 O(抵达次数) ——
    ///   这条 Rc 化把「每条候选一次分配」也一并消掉了。
    caps: std::rc::Rc<Vec<Option<usize>>>,
    /// ⚠ **仍是 Vec<u8>** 而不是内联数组:候选是 **O(不同起点数)** 条,
    ///   每条内联 64 字节会让候选表**变大**(实测 10 万个起点 ⇒ 多 6 MB),
    ///   而候选**不是**热路径(热路径是每线程每步的 Split 分支)。
    ///   ⇒ Thread 用内联数组(热路径),Hit 用 Vec(冷路径)。
    prio: Vec<u8>,
    /// 抵达顺序(同 `(start, prio)` 时后到者更完整)。
    arrival: usize,
}

impl Hit {
    /// 候选之间的定序:**起点优先**,同起点比**路径**,同路径比**抵达顺序**。
    ///
    /// ⚠ **`arrival` 是降序** —— 后到者更完整。抵达 `Match` 只能靠消费字符,
    /// 所以「同 `(start, prio)` 而后到」等价于「同起点、同优先级分支、
    /// 消耗了更多输入」。封顶前这条路走不到(贪含量词每绕一圈 `Split`,
    /// `prio` 就长一格,天然分出胜负);封顶后 `prio` 塌成同一向量(见
    /// `PRIO_MAX`),裁决权**全部**落到这一键。
    ///
    /// [D13-019 / N-003]此前这里是**升序**(先到者胜),而 `Hit::arrival` 字段
    /// 注释与 `PRIO_MAX` 处的注释都写着「后到者更完整」—— 注释与代码方向相反。
    ///
    /// ⚠ **症状只落在 `RE_SEARCH` / `run` 这条路径上**(负向验证实测):
    /// `RE_SEARCH(RE("a+"), REPEAT("a", 200))` 在升序下只报 `end = 65`,
    /// 而同一构建下 `RE_REPLACE` / `RE_FIND_ALL` 走 `find_all` 拿到的
    /// **是完整长度**。⇒ 修本条时**别拿 `RE_REPLACE` 当守卫** —— 那条路径
    /// 本来就没坏,用它写的契约在 bug 形态下也全绿,是条假守卫
    /// (已发生过一次:初版契约正是这么写的,变异验证当场把它拆穿)。
    fn better_than(&self, other: &Hit) -> bool {
        (self.start, &self.prio, std::cmp::Reverse(self.arrival))
            < (other.start, &other.prio, std::cmp::Reverse(other.arrival))
    }
}

/// `by_start` 里的「该左端还没有候选」标记。
const NO_SLOT: u32 = u32::MAX;

/// 测试期性能埋点(**不进 release 构建**,所以热路径不留任何运行时成本)。
///
/// 为什么需要它:性能判据不能钉绝对耗时(机器相关、CI 上必然不稳),而
/// 「有没有排序」「克隆了几次」这类性质是**整数**的 —— 可数、可复现、与
/// 机器无关。埋点分两组,缺一不可:
///
/// * **用量组**(`add_calls` / `inst_visits` / `thread_visits` /
///   `hits_recorded`):计数点在**热路径上、修复前后都在**,它们证明这条
///   路径真的被走到了 ⇒ 不会空转。
/// * **缺陷组**(`work_allocs` / `inst_clones` / `thread_clones` /
///   `hits_retained` / `order_comparisons`):在修好的构建里它们要么恒 0、
///   要么被压到 O(不同起点数)—— 那正是它们要断言的事。守卫同时断言
///   同一处的**用量组 > 0**,所以「计数器被删掉」会先被用量组拆穿。
#[cfg(test)]
#[derive(Default, Clone, Copy, Debug)]
struct Probe {
    /// `note_hit` 抵达 `Match` 的次数(**两种模式都计**,所以它证明的是
    /// 「这条路径真的抵达过候选」,而不是「`find_all` 那一支跑了」)。
    hits_recorded: usize,
    /// `find_all` 留在候选表里的条数(按左端去重**之后**)= 这一步的内存量级。
    hits_retained: usize,
    /// 定序比较次数:按左端去重时的 `prio` 比 + 选取阶段的任何排序。
    /// ⚠ **实测要点**:修复前那版 `sort_by` 在本 VM 的抵达顺序上是 **O(n) 次**
    /// 比较(左最早 ⇒ 每步活着的 `Match` 线程都是最靠左那条 ⇒ `start` 键本来
    /// 非降,自适应排序一眼认出)。所以真正消掉的是**候选表规模**(O(抵达次数)
    /// → O(不同起点数),每条还带两次堆分配),排序本身只是顺带去掉了。
    order_comparisons: usize,
    /// `advance` 分派过的指令数(证明 dispatch 埋点是活的)。
    inst_visits: usize,
    /// `advance` 每步**克隆**指令的次数(修复前 = `inst_visits`)。只对
    /// `Inst::Class` 真正分配,但计数在 dispatch 入口,与形态无关。
    inst_clones: usize,
    /// `add` 的调用次数(= 每线程每步都会来一次)。
    add_calls: usize,
    /// `add` 复用 `Vm` 常驻工作栈的次数(修复后 = `add_calls`)。
    work_reuses: usize,
    /// `add` 新建工作栈 `Vec` 的次数(修复前 = `add_calls`,修复后 = 0)。
    work_allocs: usize,
    /// `advance` 每步从 `clist` 取线程的次数。
    thread_visits: usize,
    /// `advance` 每步**克隆**线程的次数(修复前 = `thread_visits`)。
    thread_clones: usize,
    /// `add` 的 `Split` 分支被走到���次数(= 每线程每步都可能在 ε 闭包里遇到)。
    ///
    /// ⚠ **用量组**:它证明「`Split` 分支真的被走到了」—— 没有它,下面那个
    ///   「恒 0」的断言就会**空转**(没走到 ⇒ 计数点没被触发 ⇒ 假绿)。
    split_calls: usize,
    /// `Split` 分支里 `prio` 被**深拷贝**(进堆 `Vec`)的次数。
    ///
    /// ⚠ **缺陷组**:修好后 `prio` 是**内联数组** ⇒ 克隆只是栈上拷 64 字节
    ///   ⇒ 这个计数点**不存在** ⇒ 恒 0。
    prio_copies: usize,
    /// `Split` 分支里 `caps` 被**深拷贝**的次数。
    ///
    /// ⚠ **缺陷组**:修好后 `caps` 是 `Rc<Vec<..>>` ⇒ 克隆只是**引用计数 +1**
    ///   ⇒ 恒 0。
    caps_copies: usize,
    /// `Rc` 写时复制**真的发生了拷贝**的次数(`Rc::make_mut` 之前引用计数 > 1)。
    ///
    /// ⚠ **活计数器**:它是可数的 ⇒ 守卫是实的。它证明「写时复制**没有**
    ///   退化成「每步一次深拷贝」」—— 那个退化会让 `caps` 的收益整个归零。
    caps_cow_copies: usize,
}

struct Vm<'a> {
    prog: &'a Program,
    input: &'a [char],
    clist: Vec<Thread>,
    nlist: Vec<Thread>,
    /// 每个输入位置一个「代」标记:先到者胜。这既去重又实现优先级
    /// (先到 = 优先级更高,因为 ε 闭包按优先级展开)。
    seen: Vec<u32>,
    gen: u32,
    /// `add` 的 ε 闭包工作栈 —— **复用**。⚠ 过去是每次调用 `vec![start]`,
    /// 而 `add` 在「每线程每步」都会走到 ⇒ 分配次数 ∝ 线程数 × 步数。
    work: Vec<Thread>,
    /// `keep_all` 时**按起点去重**后的候选索引:`by_start[s]` = `slots` 下标,
    /// `NO_SLOT` = 该左端还没有候选。
    ///
    /// 为什么要有这张表:`find_all` 的贪心段对某个左端**只认第一个**(同左端
    /// 后来的一律 `continue`)⇒ 第二个及以后的候选**永远不参与结果**。有了
    /// 它,选取阶段按**下标升序**走就是起点升序 ⇒ **不再需要排序**。
    by_start: Vec<u32>,
    /// 去重后的候选本体。⚠ 规模是「**不同起点数**」而不是「抵达次数」——
    /// 贪含量词在**每个**位置都抵达 `Match`,而它们全在**同一个左端**
    /// (左最早)⇒ 旧实现里这一栏是 O(n),实际影响 O(n) 个左端。
    slots: Vec<Option<Hit>>,
    /// 单次搜索只要最优那一个(见 `note_hit` 的 ⚠)。
    best: Option<Hit>,
    /// ⚠⚠ **等价判据用的「每一次抵达」原始候选流**(只在 `cfg(test)` 下存在)。
    /// `note_hit` 无论去重与否都会往这里也推一份,于是测试可以把**优化前**
    /// 那套算法(`legacy_select_all`)和**优化后**(`select_all`)拿同一批候选
    /// 跑一遍比结果 —— 这是「行为逐字未变」最直接的证据,比任何一组
    /// 手挑的样例都强。
    #[cfg(test)]
    legacy_hits: Vec<Hit>,
    /// `true` = 攒全部候选(`find_all`);`false` = 只留最优(`find` / `run`)。
    keep_all: bool,
    /// 抵达计数器 —— 同一 `(start, prio)` 时区分「先到」与「后到」。
    arrivals: usize,
    #[cfg(test)]
    probe: Probe,
}

impl<'a> Vm<'a> {
    fn new(prog: &'a Program, input: &'a [char]) -> Self {
        Self {
            prog,
            input,
            clist: Vec::new(),
            nlist: Vec::new(),
            seen: vec![0; prog.insts.len()],
            gen: 0,
            work: Vec::new(),
            by_start: Vec::new(),
            slots: Vec::new(),
            best: None,
            keep_all: false,
            arrivals: 0,
            #[cfg(test)]
            probe: Probe::default(),
            #[cfg(test)]
            legacy_hits: Vec::new(),
        }
    }

    /// 切到 `find_all` 模式(攒全部候选)。
    ///
    /// ⚠ `by_start` 是**唯一一处随输入长度线性增长的额外缓冲**(每起点 4 字节
    /// ≈ 输入自身 `Vec<char>` 的一半)。这笔买卖划算:换掉的是「候选表随
    /// **抵达次数**增长(每条带两次堆分配)+ 一次全量排序」,而输出在空匹配
    /// 密集的输入上本来就是 O(n) ⇒ 这块内存**不改变**渐近阶,只改变常数。
    /// ⚠ 也就是说:**别把它当成「find_all 现在是 O(1) 内存」** —— 它不是。
    fn keep_all(&mut self) {
        self.keep_all = true;
        self.by_start = vec![NO_SLOT; self.input.len() + 1];
    }

    /// 把 `t.pc` 的 ε 闭包按优先级压进 `list`。
    ///
    /// `Split(a, b)` **先展开 a 再展开 b** ⇒ a 的线程落在 list 更靠前的位置。
    /// 配合 `seen` 的先到者胜,「分支顺序」就变成了「匹配选择的优先级」。
    fn add(&mut self, list: &mut Vec<Thread>, start: Thread, gen: u32, pos: usize) {
        #[cfg(test)]
        {
            self.probe.add_calls += 1;
            self.probe.work_reuses += 1;
        }
        // ⚠ **工作栈复用**。过去这里是 `vec![start]` —— 每次 `add` 调用一次堆
        // 分配,而 `add` 在「每线程每步」都会走到 ⇒ 分配次数 ∝ 线程数 × 步数。
        // 换成 `Vm` 上的常驻缓冲后,整趟扫描只分配**一次**。
        //
        // ⚠ 直接借 `&mut self.work`(而不是先 `mem::take`):`list` 是调用方的
        // 可变借用,而三个调用点传进来的都**是从 `self` 上取走的局部 `Vec`**
        // (`std::mem::take`)⇒ 它与 `self.work` 不可能别名。
        // ⚠ 若哪天有人改成直接传 `&mut self.nlist`,这里会**当场编译不过**
        // —— 那正是想要的:别名会让 ε 闭包写坏 nlist。
        let work = &mut self.work;
        work.clear();
        work.push(start);
        while let Some(mut t) = work.pop() {
            if self.seen[t.pc] == gen {
                continue;
            }
            self.seen[t.pc] = gen;
            // ⚠ **借用指令,不要克隆它**。`Inst` 不是 `Copy`(它带 `ClassSet`),
            // 过去这里每展开一个 ε 节点克隆一次 ⇒ 每扫到一个 `Class` 指令一次
            // 堆分配,而 `Class` 正是**消费字符**的那条指令 = 每输入位置一次。
            // 这里任何一个分支都不需要**拥有**指令本身(只读字段 / 读 `pc`),
            // 所以借用即可,克隆纯属白付一次分配。
            match &self.prog.insts[t.pc] {
                Inst::Jump(target) => {
                    t.pc = *target;
                    work.push(t);
                }
                Inst::Split(a, b) => {
                    // 低优先级先入栈 ⇒ 高优先级先弹出;`prio` 追加 0 / 1。
                    //
                    // ⚠ **路径长度必须封顶。** 贪婪量词每绕一圈循环就经过
                    // 一次 `Split` ⇒ 路径每字符增长一格,而 `add` 要克隆它
                    // ⇒ `a*` 在第 n 个字符处路径长 n,整趟扫描退回 **O(n²)**。
                    // 这不是理论担忧:W-04 的基准实测 100 000 个 a 要 1.4 s
                    // (每字节成本随规模上涨 6 倍)。
                    //
                    // **封顶不损语义**:① 交替的选择深度是**模式里的分支数**,
                    // 与输入长度无关,64 绰绰有余(`a|ab`、`ab|a` 这类判别式
                    // 只需 1~2 格);② 贪婪延伸靠的是「同起点时**后到者**更
                    // 完整」这条**抵达顺序**规则(见 `Hit::better_than` 的
                    // 第三键),它与路径长度无关 —— 所以路径打平之后仍由抵达
                    // 顺序正确地选出最长的那次匹配。
                    // ⚠【N-026】这一段原先**每次分叉都要 3 次堆分配**
                    //   (2 × `prio` 的 `Vec` 克隆 + 1 × `caps` 的 `Vec` 克隆)。
                    //   而 `Split` 在「每线程每步」都会走到 ⇒ 分配次数 ∝ 线程数 × 步数。
                    //   ⇒ 现在:① `prio` 是**内联数组** ⇒ 克隆它只是**栈上拷
                    //   64 字节**(零分配);② `caps` 是 `Rc` ⇒ 克隆它只是
                    //   **引用计数 +1**(零分配)。
                    #[cfg(test)]
                    {
                        self.probe.split_calls += 1;
                    }
                    let mut hi_prio = t.prio;
                    let mut hi_len = t.prio_len;
                    if (hi_len as usize) < PRIO_MAX {
                        hi_prio[hi_len as usize] = 0;
                        hi_len += 1;
                    }
                    let mut lo_prio = t.prio;
                    let mut lo_len = t.prio_len;
                    if (lo_len as usize) < PRIO_MAX {
                        lo_prio[lo_len as usize] = 1;
                        lo_len += 1;
                    }
                    work.push(Thread {
                        pc: *b,
                        caps: t.caps.clone(),
                        prio: lo_prio,
                        prio_len: lo_len,
                    });
                    work.push(Thread {
                        pc: *a,
                        caps: t.caps,
                        prio: hi_prio,
                        prio_len: hi_len,
                    });
                }
                Inst::Save(slot) => {
                    if *slot < t.caps.len() {
                        // ⚠【N-026】`Rc` 的**写时复制**:只有当这份共享真的被
                        //   **多个线程**持有时才会真拷一份;否则**就地改**(零分配)。
                        //   ⚠ 为什么不能直接 `t.caps[*slot] = ...`:`Rc` 不可变
                        //   引用不能可变地索引 ⇒ 必须走 `make_mut`。
                        #[cfg(test)]
                        if std::rc::Rc::strong_count(&t.caps) > 1 {
                            // ⚠ 引用计数 > 1 ⇒ `make_mut` 会**真的拷一份**。
                            self.probe.caps_cow_copies += 1;
                        }
                        std::rc::Rc::make_mut(&mut t.caps)[*slot] = Some(pos);
                    }
                    t.pc += 1;
                    work.push(t);
                }
                Inst::Assert(a) => {
                    if a.holds(self.input, pos) {
                        t.pc += 1;
                        work.push(t);
                    }
                    // 否则线程在此消亡。
                }
                Inst::Class(_) | Inst::Match => list.push(t),
            }
        }
    }

    fn start_thread(&self, at: usize) -> Thread {
        let mut caps = vec![None; 2 * (self.prog.groups + 1)];
        caps[0] = Some(at);
        Thread {
            pc: 0,
            caps: std::rc::Rc::new(caps),
            prio: [0u8; PRIO_MAX],
            prio_len: 0,
        }
    }

    /// 扫到第一个 `Match`(最高优先级)时的捕获;扫完仍无则 `None`。
    ///
    /// `Match` 线程**不吃字符、也不消亡**,它一路留在列表里 —— 于是每一步
    /// 扫到的「列表里第一个 `Match`」就是当前最高优先级的候选,覆盖上一步的
    /// 记录。leftmost 由「新注入的起始线程排在列表尾部」保证,priority 由
    /// 「列表顺序 = ε 展开顺序」保证。
    fn run(&mut self, anchored: bool) -> Option<Captures> {
        self.scan(anchored);
        self.best_hit().map(|h| Captures {
            slots: (*h.caps).clone(),
        })
    }

    /// 一次完整扫描。`anchored` 为真时不注入后续起始线程(整串匹配用)。
    fn scan(&mut self, anchored: bool) {
        self.start();
        for i in 0..self.input.len() {
            self.advance(self.input, i, !anchored);
        }
        self.finish(self.input);
    }

    /// 记下一次 `Match` 抵达。**取舍方式按模式分**:`find_all` 攒下全部
    /// 候选(它要的是非重叠的一串),`find` / `run` 只留**最优那一个**。
    ///
    /// ⚠ **两者必须分开** —— W-04 的基准抓到的就是这个:早期实现无条件把
    /// 每次抵达都攒进 `hits`,而 `a*` / `.*` 这类**在每个位置都命中**的模式
    /// 于是攒下 O(n) 份拷贝 ⇒ 单次 `RE_SEARCH` 退化成 **O(n²)**
    /// (实测 10 000 字符 20.6 ms → 100 000 字符 **1.267 s**,每字节成本从
    /// 2.06 µs 涨到 12.67 µs)。单次搜索只需要最优解,边扫边替换即可。
    ///
    /// 两种模式都按 `(start, prio, arrival)` 定序 —— 取舍是「跨步骤比较
    /// 优先级」,而 clist 下标只反映**当前这一步**的顺序(跨步不可比)。
    /// 把它当「clist 里第一个 Match」用会让 `a|ab` 取成 `ab`、`a+` 取成 `a`。
    fn note_hit(&mut self, t: &Thread) {
        let (Some(lo), Some(hi)) = (
            t.caps.first().copied().flatten(),
            t.caps.get(1).copied().flatten(),
        ) else {
            return;
        };
        let arrival = self.arrivals;
        self.arrivals += 1;
        #[cfg(test)]
        {
            self.probe.hits_recorded += 1;
        }

        if self.keep_all {
            #[cfg(test)]
            {
                self.legacy_hits.push(Hit {
                    start: lo,
                    end: hi,
                    caps: t.caps.clone(),
                    prio: t.prio[..t.prio_len as usize].to_vec(),
                    arrival,
                });
            }
            // ⚠ **同一起点只需要最优的那一个**(`find_all` 的贪心段对某个左端
            // 只认定序后的**第一个**,同左端后来的全部 `continue`;被第一个
            // 挡下之后 `out.last()` 已经是同左端的那一个,后续候选一律撞上
            // `prev.start() == h.start` 分支 ⇒ **一个都进不了结果**)。
            // 旧实现却把它们全攒下来再排序,于是 `a*` 在 n 个 a 上攒了 n+1 份
            // 拷贝(全部同一个左端)⇒ O(n) 内存 + 一次全量排序。
            //
            // ⚠ **裁决口径不许为了省这次比较而分叉**:`arrival` 单调递增 ⇒
            // 第三键 `Reverse(arrival)` 恒让「后到者」更小 ⇒
            // 「新候选更优」精确等价于 `t.prio <= cur.prio`,与
            // `Hit::better_than` 的前两键**逐字同源**(D13-019 / N-003)。
            // 一旦这里换成别的裁决,单次搜索与 `find_all` 会在封顶区给出
            // **两个不同的答案**。
            // ⚠ 负向验证实测:换成升序(先到者胜)时 `find_all` 这条路径
            // **仍是完整长度** —— 也就是说这一处保持降序是为了**两个路径同
            // 口径**,它本身不被任何契约钉住。别为了快把它改掉。
            let slot = self.by_start[lo];
            let wins = match self.slots.get(slot as usize) {
                Some(Some(cur)) => {
                    #[cfg(test)]
                    {
                        self.probe.order_comparisons += 1;
                    }
                    t.prio[..t.prio_len as usize] <= cur.prio[..]
                }
                _ => true,
            };
            if wins {
                let hit = Hit {
                    start: lo,
                    end: hi,
                    caps: t.caps.clone(),
                    prio: t.prio[..t.prio_len as usize].to_vec(),
                    arrival,
                };
                if slot == NO_SLOT {
                    #[cfg(test)]
                    {
                        self.probe.hits_retained += 1;
                    }
                    self.by_start[lo] = self.slots.len() as u32;
                    self.slots.push(Some(hit));
                } else {
                    self.slots[slot as usize] = Some(hit);
                }
            }
            return;
        }

        let hit = Hit {
            start: lo,
            end: hi,
            caps: t.caps.clone(),
            prio: t.prio[..t.prio_len as usize].to_vec(),
            arrival,
        };
        let better = match &self.best {
            None => true,
            Some(cur) => hit.better_than(cur),
        };
        if better {
            self.best = Some(hit);
        }
    }

    /// 全部候选里的最优者(两种模式都适用:一个只有 `best`,一个只有 `slots`)。
    fn best_hit(&self) -> Option<&Hit> {
        let from_slots = self
            .slots
            .iter()
            .flatten()
            .reduce(|a, b| if b.better_than(a) { b } else { a });
        match (&self.best, from_slots) {
            (None, None) => None,
            (Some(b), None) => Some(b),
            (None, Some(h)) => Some(h),
            (Some(b), Some(h)) => Some(if h.better_than(b) { h } else { b }),
        }
    }

    /// 消费 `input[i]`,把 `clist` 推进成新的 `clist`,并在**末尾**注入
    /// 「起点为 `i + 1`」的新起始线程。
    ///
    /// ⚠ **注入的时机就是 `at` 的取值**:nlist 里的线程会在**下一步**消费
    /// `input[i+1]`,所以起点为 `i+1` 的线程必须在第 `i` 步末尾注入。
    /// 早前在第 `i` 步注入 `start_thread(i)`,会让 caps[0] = i 的线程从
    /// `input[i+1]` 开始消费 —— 症状是匹配整体右移一位、报出的 `matched`
    /// 前面多一个字符(`[a-c]+` 在 "xabcz" 上报 "xabc" 而非 "abc")。
    /// `start()` 单独负责 `start_thread(0)`,循环里**不再**重复注入 0。
    fn advance(&mut self, input: &[char], i: usize, inject_next: bool) {
        let c = input[i];
        self.gen += 1;
        let gen = self.gen;
        // ⚠ 把 `prog` **复制**成局部。`self.prog` 是 `&'a Program`(`Copy`),
        // 于是 `&prog.insts[..]` 的借用挂在 `'a` 上、**不与 `&mut self` 冲突**。
        // 直接写 `&self.prog.insts[..]` 会让循环体里的 `self.note_hit(&t)` /
        // `self.add(..)` 借不过(同一个 `self`)。
        let prog = self.prog;
        let mut nlist = std::mem::take(&mut self.nlist);
        nlist.clear(); // `take` 拿到的是**上一轮的 clist**,不清就会残留
                       // ⚠ **顺序即优先级**:既有线程的后继先入,新注入的起始线程**最后**入。
                       // 反过来写会让「最年轻的路径」压过「更前进的线程」(Go 的 `machine.step`
                       // 也是这个顺序),症状是贪婪量词只吃到一半、且匹配起点会莫名右移。
        for k in 0..self.clist.len() {
            #[cfg(test)]
            {
                self.probe.thread_visits += 1;
                self.probe.inst_visits += 1;
            }
            // ⚠ **借走,不要克隆**。`clone()` 每线程每步两次堆分配(`caps` +
            // `prio`),而这是扫描里最热的一步。槽位借走后本轮不会再被读
            // (每个 `k` 只访问一次)⇒ 留下一个**不分配**的空壳。
            // ⚠ `Thread::default` 的 `pc = usize::MAX` 是故意的:误读当场炸。
            let t = std::mem::take(&mut self.clist[k]);
            // ⚠ 同上:`Inst` 不 `Copy`,克隆它等于每输入位置白付一次分配。
            match &prog.insts[t.pc] {
                Inst::Class(cs) => {
                    if cs.matches(c) {
                        let mut n = t;
                        n.pc += 1;
                        self.add(&mut nlist, n, gen, i + 1);
                    }
                }
                Inst::Assert(a) => {
                    if a.holds(input, i) {
                        // 断言**零宽**:过了它位置不变,所以传 `i` 而不是 `i + 1`。
                        let mut n = t;
                        n.pc += 1;
                        self.add(&mut nlist, n, gen, i);
                    }
                }
                // `Match` 不吃字符:**记下候选,不再往后带**。「已匹配」这件事
                // 由 `best` 跨步持有,不需要一个常驻线程 —— 而常驻线程会在
                // 同一个 `Match` 指令上与新候选抢位,把 `find_all` 的后续
                // 候选整轮挡掉。
                Inst::Match => self.note_hit(&t),
                _ => {}
            }
        }
        if inject_next {
            let t = self.start_thread(i + 1);
            self.add(&mut nlist, t, gen, i + 1);
        }
        self.nlist = nlist;
        std::mem::swap(&mut self.clist, &mut self.nlist);
    }

    /// 输入末尾的收尾:让 `$` 与整体 `Match` 有机会成立。
    fn finish(&mut self, input: &[char]) {
        self.gen += 1;
        let gen = self.gen;
        let pos = input.len();
        // 与 `advance` 同理:复制出来才能在循环体里 `&mut self`。
        let prog = self.prog;
        let mut final_list = std::mem::take(&mut self.clist);
        let mut out = Vec::new();
        for t in final_list.drain(..) {
            // ⚠ 借用指令:`finish` 也要跟着去掉 `Inst` 克隆(同上)。
            match &prog.insts[t.pc] {
                Inst::Assert(a) => {
                    if a.holds(input, pos) {
                        let mut n = t;
                        n.pc += 1;
                        self.add(&mut out, n, gen, pos);
                    }
                }
                Inst::Match => self.note_hit(&t),
                _ => {}
            }
        }
        self.clist = out;
    }

    /// 从**按左端去重后**的候选里挑出非重叠的一串(`find_all` 的选取阶段)。
    ///
    /// ⚠ **遍历顺序就是起点升序**,因为 `by_start` 的下标即起点 ⇒ 这里
    /// **不需要排序**。它与旧代码「全量 `sort_by` + 同左端只留第一个」看到的
    /// 候选序列**逐字相同**(同左端的第二个及以后的候选在旧代码里也全部撞上
    /// `prev.start() == h.start` 而 `continue`,见 `note_hit` 的 ⚠)。
    ///
    /// ⚠ 定序键本身没被动:仍然是「起点 → 分支路径 → 抵达顺序**降序**」,
    /// 只是第二 / 第三键的比较挪到了**抵达时**(`note_hit` 里那一次 `prio` 比),
    /// 第一键退化成了下标顺序。代价从 O(n log n) 次定序比较降到 O(1) 次。
    fn select_all(&mut self) -> Vec<Captures> {
        let mut out: Vec<Captures> = Vec::new();
        for start in 0..self.by_start.len() {
            let slot = self.by_start[start];
            if slot == NO_SLOT {
                continue;
            }
            let Some(h) = self.slots[slot as usize].take() else {
                continue;
            };
            let empty = h.start == h.end;
            match out.last() {
                None => {}
                Some(prev) => {
                    let pe = prev.end();
                    if empty && h.start <= pe {
                        continue; // 紧贴前一个匹配的空匹配:跳过(RE2/Go 规则)
                    }
                    if prev.start() == h.start {
                        // ⚠ 有了按左端去重,这个分支**正常走不到**了 —— 同左端
                        // 只剩一个候选。留着它是防御:万一有人改回去,这里仍是
                        // 最后一道「左最早」的闸。
                        //
                        // [D13-019 / N-004]此前这里是
                        // `if h.end <= prev.end() { continue } out.pop()`,
                        // 后果是 `RE_FIND_ALL(RE("a|ab"), "ab")` 答 `["ab"]`,
                        // 而 Python / Go 答 `["a"]`。契约里**照不到**:§3.7
                        // 登记的六个 leftmost-first 判别式全部走 `RE_SEARCH`
                        // (走 `best_hit`,不经这条分支),`RE_REPLACE` /
                        // `RE_SPLIT` 建在 `find_all` 上,连带受影响。
                        continue;
                    } else if h.start < pe {
                        continue; // 与上一个匹配重叠
                    }
                }
            }
            out.push(Captures {
                slots: (*h.caps).clone(),
            });
        }
        out
    }

    /// 从起点 0 起跑一次扫描。
    fn start(&mut self) {
        self.gen += 1;
        let gen = self.gen;
        let t = self.start_thread(0);
        let mut clist = std::mem::take(&mut self.clist);
        self.add(&mut clist, t, gen, 0);
        self.clist = clist;
    }
}

/// leftmost-first 搜索的**第一个**匹配(整串匹配用 `anchored = true`)。
pub fn find(prog: &Program, text: &str, anchored: bool) -> Option<Captures> {
    let input: Vec<char> = text.chars().collect();
    let mut vm = Vm::new(prog, &input);
    vm.run(anchored)
}

/// 全部非重叠匹配。**单遍扫描**(不是「每个起点重跑一遍」—— 那会是
/// O(n²),与线性承诺直接冲突)。
///
/// 空匹配的邻接规则(§3.7.2-2)落在一个数上:记下 `next_allowed_start`,
/// 非空匹配取 `end`、空匹配取 `end + 1` ⇒ **紧贴前一个匹配的空匹配
/// 自然被排除**(它的 `start` 落在门槛之前),而其余空匹配照常上报。
/// 「推进一个码点」由 VM 继续扫输入天然完成。
pub fn find_all(prog: &Program, text: &str) -> Vec<Captures> {
    let input: Vec<char> = text.chars().collect();
    let mut vm = Vm::new(prog, &input);
    vm.keep_all(); // 这一条要非重叠的一串 ⇒ 必须攒候选(按左端去重之后)
    vm.scan(false);
    vm.select_all()
}

// ── 编译缓存(W-23 · D-1)──────────────────────────────────────────────
//
// ## 为什么需要它
//
// `RE_*` 此前**每次调用都重新编译**模式(规范 §16.5 的实现注如实记着这一点):
// 解析 → Thompson 构造 → NFA 指令数组。WAF 这类用法是「一个规则集 × 每个请求」
// ⇒ 每个请求都把全部规则重编一遍,而「省掉重编译」的收益每次都拿得到。
//
// ## 为什么**可以**做(规范侧零改动)
//
// 规范 §16.5 的**实现注(非规范)**明写:「模式对象已携带组数,实现**可以**缓存
// 编译结果并以 O(1) 回答」⇒ 缓存是**规范允许**的实现自由度 ⇒ 本项**零规范
// 改动、零新成员**(D-1 的非目标)。
//
// ## 缓存里放什么、不放什么
//
// ⚠ **只放编译产物**(`Program`:指令数组 + 组数 + 模式源串)—— 它只含
//   `Vec` / `usize` / `String` ⇒ `Send + Sync` ⇒ 可以放进进程级表。
// ⚠ **绝不放 `Value`** —— `Value` 是 `Rc` 语义、**不是 `Send`**(见 `wlwl-value`),
//   放一个进去整张表就变成 `!Send`,而 `static` 要求 `Sync`。
//
// ## 为什么是进程级、为什么要锁
//
// 同一进程里可以跑**两个求值器**(测试套件、嵌入 wlwl 的宿主程序)⇒ 缓存是
// **共享**的 ⇒ 必须加锁。样板照 `std.net` 的 TCP 族(`tcp.rs` 的 `READY`):
// `LazyLock<Mutex<…>>`。
//
// ## 句柄在哪
//
// ⚠ 句柄**就是模式串本身**(= 缓存的键),而 `RE()` 的返回值**一字不变**
//   (仍是 `[pattern, groups]` 两键 —— 规范 §16.5 明写「含 pattern 与 groups
//   两键」)。**不要**往那个字典里加第三个「句柄」键:那会改
//   `LEN(RE(...))` / `KEYS(RE(...))` 这类**可观察面**(撞一致性册 `:223`),
//   而规范侧对缓存的要求只是「**可以**缓存」—— 句柄不需要进语言面。

/// 缓存里最多留多少个**不同**的模式串。
///
/// ⚠ **上界是硬要求**(D-1 的验收条目之一):WAF 这类长跑进程会把规则集里
/// 的模式**全部**编一遍;不设上界,缓存就随规则数无上限增长。
/// 取 256:一条规则集通常是几十到几百条模式,256 足够把「一个规则集」装下;
/// 再多就走 LRU 淘汰(最久没用的先走)。
const MAX_CACHE_ENTRIES: usize = 256;

/// 编译计数表最多留多少个模式串的计数。
///
/// ⚠ 它是**读数**用的表(不是缓存)⇒ 上界与缓存**分开**算:淘汰的是**最老的**
///   那条(先进先出)⇒ 被淘汰模式的计数读作 0(**下界**,不是精确值)。
///   ⇒ 断言用的模式要**当场连打**:1000 次连打同一个模式,计数不会被淘汰。
const MAX_TRACKED_PATTERNS: usize = 1024;

/// 缓存里所有**模式串**的总字节上限。
///
/// ⚠ 条数上界(`MAX_CACHE_ENTRIES`)管的是「条目数」,**管不住「单条模式串有
///   多长」** —— 一个 1 MB 的模式串 × 256 条 = 256 MB ⇒ 对长跑进程而言那
///   仍然是「无上限增长」⇒ 另加一条**字节上界**:超了就继续淘汰(仍是 LRU);
///   单条就超上界的模式**不进缓存**(照旧编译,只是不驻留)⇒
///   「总量上限」两条(条数 / 字节)都封死。
const MAX_CACHE_BYTES: usize = 4 * 1024 * 1024;

/// 一个模式串的编译产物(两种匹配语义各一份)。
#[derive(Debug)]
struct CacheEntry {
    /// 搜索语义:`RE_SEARCH` / `RE_FIND_ALL` / `RE_REPLACE` / `RE_SPLIT` /
    /// `RE_GROUP_COUNT`。
    search: Arc<Program>,
    /// 整串语义:`RE_TEST`(两端锚定、忽略 `(?m)`;见 `compile_full`)。
    full: Arc<Program>,
    /// 最近一次使用的序号(LRU 淘汰用)。
    stamp: u64,
}

/// 缓存 + 计数的状态(进程级唯一,见 [`CACHE`]）。
#[derive(Debug, Default)]
struct CacheState {
    /// 模式串 ⇒ 编译产物。**上界** [`MAX_CACHE_ENTRIES`]。
    entries: HashMap<String, CacheEntry>,
    /// 模式串 ⇒ **至今被尝试编译过几次**。
    ///
    /// ⚠ 它记的是**实际发生的编译**(成功与失败都算),不是「缓存里有没有
    ///   这一条」—— 「把命中缓存的短路去掉」那个变异会让同一个模式被编译
    ///   N 次,而这条计数正是那个变异的**见证**(验收断言读的就是它)。
    compiles: HashMap<String, u64>,
    /// `compiles` 的插入顺序(先进先出 ⇒ 给计数表自己一个上界)。
    compile_order: VecDeque<String>,
    /// LRU 的单调时钟。
    clock: u64,
    /// 缓存里所有**模式串**的总字节数(上界 [`MAX_CACHE_BYTES`]）。
    bytes: usize,
    /// 累计命中次数(命中 ⇒ 没有重新编译)。
    hits: u64,
    /// 累计**真正编译**的次数。
    compiles_total: u64,
    /// 累计淘汰次数(超过上界时被淘汰的模式数)。
    evictions: u64,
    /// 累计**拒收**次数(单条模式串就超字节上界 ⇒ 不进缓存)。
    rejected: u64,
}

/// **进程级**的正则编译缓存。
///
/// ⚠ 锁中毒时**照常继续**(`unwrap_or_else(|e| e.into_inner())`)—— 同 `tcp.rs`:
///   缓存是**加速器**,不是正确性的前提;一把中毒的锁不该让整个解释器
///   进程从此不可用。
static CACHE: LazyLock<Mutex<CacheState>> = LazyLock::new(|| Mutex::new(CacheState::default()));

/// 缓存的对外读数。**只给测试 / 基准读** —— 同 `wlwl_value::dict_work_counters`
/// 的体例:不进诊断通道、**不新增语言面成员**(D-1 的非目标:零新成员)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RegexCacheStats {
    /// 当前缓存里的模式数(≤ `capacity`)。
    pub entries: usize,
    /// 缓存上界。
    pub capacity: usize,
    /// 累计命中次数(命中 ⇒ 没有重新编译)。
    pub hits: u64,
    /// 累计**真正编译**的次数。
    pub compiles: u64,
    /// 累计淘汰次数(超过上界时被淘汰的模式数)。
    pub evictions: u64,
    /// 累计**拒收**次数(单条模式串就超字节上界 ⇒ 编译了但不进缓存)。
    pub rejected: u64,
    /// 缓存里所有**模式串**的总字节数(上界见 `capacity_bytes`)。
    pub bytes: usize,
    /// 缓存的**字节**上界。
    pub capacity_bytes: usize,
}

/// 读缓存的计数。**只给测试 / 基准读**。
pub fn cache_stats() -> RegexCacheStats {
    let st = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    RegexCacheStats {
        entries: st.entries.len(),
        capacity: MAX_CACHE_ENTRIES,
        hits: st.hits,
        compiles: st.compiles_total,
        evictions: st.evictions,
        rejected: st.rejected,
        bytes: st.bytes,
        capacity_bytes: MAX_CACHE_BYTES,
    }
}

/// 某个模式串**至今被尝试编译过几次**。
///
/// ⚠ **成功的与失败的都算** —— 它量的是**工作量**(D-1 的验收判据是「确定性
///   工作量计数」,不是墙钟)。失败**不进缓存** ⇒ 一个坏模式每被用一次就
///   重编一次 ⇒ 计数逐次增长(那正是「失败不驻留」的可见形状)。
///
/// ⚠ 计数表**有上界**(`MAX_TRACKED_PATTERNS`),满了之后淘汰**最老的**那条
///   ⇒ 被淘汰模式的计数读作 **0**(**下界**,不是精确值)。
///   ⇒ 断言用的模式要**当场连打**:1000 次连打同一个模式,计数不会被淘汰。
pub fn pattern_compiles(pattern: &str) -> u64 {
    let st = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    st.compiles.get(pattern).copied().unwrap_or(0)
}

/// 一个模式串的两种语义产物。
#[derive(Debug)]
struct CachedPrograms {
    search: Arc<Program>,
    full: Arc<Program>,
}

/// 取一个模式串的编译产物:命中缓存就返回,没命中就**编译一次**并登记。
///
/// ⚠ **编译在锁内做** —— 这是「同一模式串在同一进程内**至多编译一次**」的
///   保证:两个线程同时要同一个模式时,第二个拿到的是第一个刚编好的那一份,
///   不会各编一遍(计数因此恒为 1,而不是「并发下可能 2」)。
///   代价:编译在进程内**串行**。编译是微秒级的(模式只有几十条指令),
///   而规则集在启动时编一次 ⇒ 这点串行代价可以接受。
///
/// ⚠ **失败不进缓存**:模式语法错每次都重新编译、每次都报**同一条**诊断
///   (带列号)⇒ 诊断文本逐字不变。(把失败也存下来就得额外保证「重放的错误
///   文本与当初一字不差」—— 不值得:失败路径本就不热。)
fn cached_programs(pattern: &str) -> Result<CachedPrograms, String> {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let st = &mut *guard;
    st.clock += 1;
    let clock = st.clock;
    let hit = st.entries.get_mut(pattern).map(|entry| {
        entry.stamp = clock;
        (entry.search.clone(), entry.full.clone())
    });
    if let Some((search, full)) = hit {
        st.hits += 1;
        return Ok(CachedPrograms { search, full });
    }
    // miss ⇒ 真编译(计数就在这一行下面加)
    let (search, full) = match compile_pair(pattern) {
        Ok(pair) => pair,
        Err(e) => {
            // ⚠ **失败也是一次编译尝试** ⇒ 也计数:这个计数器量的是**工作量**,
            //   不是「成功了几次」。(失败不进缓存 ⇒ 每次调用都重编 ⇒ 计数
            //   逐次增长 —— 那正是「失败不驻留」的可见形状。)
            record_compile(st, pattern);
            return Err(e);
        }
    };
    record_compile(st, pattern);
    insert_entry(st, pattern, search.clone(), full.clone());
    Ok(CachedPrograms { search, full })
}

/// 编译一个模式串 ⇒ 两种匹配语义各一份。
///
/// ⚠ **一次编译产出两份**:`RE_TEST` 用的整串版是搜索版的**派生**(两端包文本锚,
///   见 `full_from`),不是重新解析一遍 ⇒ 「一个模式串编译了几次」的答案是 **1**,
///   不是 2(否则「编译次数 = 1」这条验收会被自己拆穿)。
fn compile_pair(pattern: &str) -> Result<(Arc<Program>, Arc<Program>), String> {
    let search = compile(pattern)?;
    let full = full_from(&search);
    Ok((Arc::new(search), Arc::new(full)))
}

/// 记一次**真正的编译**(全局计数 + 每个模式串的计数)。
fn record_compile(st: &mut CacheState, pattern: &str) {
    st.compiles_total += 1;
    match st.compiles.get_mut(pattern) {
        Some(n) => *n += 1,
        None => {
            st.compiles.insert(pattern.to_string(), 1);
            st.compile_order.push_back(pattern.to_string());
            while st.compile_order.len() > MAX_TRACKED_PATTERNS {
                if let Some(old) = st.compile_order.pop_front() {
                    st.compiles.remove(&old);
                }
            }
        }
    }
}

/// 登记一个模式串的编译产物;缓存满了就**淘汰最久没用的那一条**(LRU)。
///
/// ⚠ **两条上界都要守**:条数(`MAX_CACHE_ENTRIES`)与字节(`MAX_CACHE_BYTES`)。
///   单条模式串就超字节上界的 ⇒ **不进缓存**(`rejected`):编译照旧(语义不变),
///   只是不驻留 ⇒ 缓存的字节数永远不超过上界。
fn insert_entry(st: &mut CacheState, pattern: &str, search: Arc<Program>, full: Arc<Program>) {
    let stamp = st.clock;
    let need = pattern.len();
    if need > MAX_CACHE_BYTES {
        st.rejected += 1;
        return;
    }
    while !st.entries.is_empty()
        && (st.entries.len() >= MAX_CACHE_ENTRIES || st.bytes + need > MAX_CACHE_BYTES)
    {
        let Some(victim) = st
            .entries
            .iter()
            .min_by_key(|(_, e)| e.stamp)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        st.bytes -= victim.len();
        st.entries.remove(&victim);
        st.evictions += 1;
    }
    st.bytes += need;
    st.entries.insert(
        pattern.to_string(),
        CacheEntry {
            search,
            full,
            stamp,
        },
    );
}

// ── 成员层 ─────────────────────────────────────────────────────────────

fn arity(host: &mut dyn StdHost, name: &str, got: usize, want: usize) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{name}: function expects {want} argument(s), got {got}"),
    )
}

fn type_err(host: &mut dyn StdHost, name: &str, label: &str, got: &Value) -> wlwl_error::WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!(
            "{name}: expected {} for {label}, got {}",
            expected_kind(label),
            crate::value_kind(got)
        ),
    )
}

fn expected_kind(label: &str) -> &'static str {
    if label == "pattern object" {
        "the dictionary returned by RE()"
    } else if label == "n" {
        "an integer"
    } else {
        "a string"
    }
}

/// `pattern object` 实参:必须带 `pattern` 键(RE() 的返回值)。
///
/// ⚠ **仍然不信任调用方**:我们只取那个 `pattern` 键的**字符串**;编译产物是
///   按**模式串**在进程级缓存里查的 ⇒ `RE_*` 的行为**不取决于**那个 `DICT`
///   是从哪来的(自己 `RE()` 出来的,还是手工拼的同形字典)。
///   ⚠ 缓存**没有**放松这个判断:手工拼的 `{"pattern": "a"}` 与 `RE("a")`
///   命中的是**同一份**编译产物(键就是模式串)⇒ 行为逐字相同。
fn pattern_of(
    host: &mut dyn StdHost,
    name: &str,
    v: &Value,
) -> Result<String, wlwl_error::WlwlError> {
    let Value::Dict(entries) = v else {
        return Err(type_err(host, name, "pattern object", v));
    };
    let pattern = entries
        .iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == "pattern"))
        .and_then(|(_, v)| match v {
            Value::String(s) => Some(s.to_string()),
            _ => None,
        })
        .ok_or_else(|| {
            host.diag(
                ErrorCode::E0030,
                format!("{name}: the pattern object has no `pattern` key (build it with RE())"),
            )
        })?;
    Ok(pattern)
}

/// 搜索语义的编译产物(命中缓存 ⇒ 不重新编译)。
fn program_of(
    host: &mut dyn StdHost,
    name: &str,
    v: &Value,
) -> Result<Arc<Program>, wlwl_error::WlwlError> {
    let pattern = pattern_of(host, name, v)?;
    cached_programs(&pattern)
        .map(|c| c.search)
        .map_err(|e| host.diag(ErrorCode::E0030, e))
}

/// 整串语义的编译产物(`RE_TEST` 用;两端包文本锚,见 `compile_full`)。
fn program_full_of(
    host: &mut dyn StdHost,
    name: &str,
    v: &Value,
) -> Result<Arc<Program>, wlwl_error::WlwlError> {
    let pattern = pattern_of(host, name, v)?;
    cached_programs(&pattern)
        .map(|c| c.full)
        .map_err(|e| host.diag(ErrorCode::E0030, e))
}

fn str_arg<'a>(
    host: &mut dyn StdHost,
    name: &str,
    label: &str,
    args: &'a [Value],
    idx: usize,
) -> Result<&'a str, wlwl_error::WlwlError> {
    match &args[idx] {
        Value::String(s) => Ok(s.as_str()),
        other => Err(type_err(host, name, label, other)),
    }
}

fn slice(input: &[char], lo: usize, hi: usize) -> String {
    input[lo.min(input.len())..hi.min(input.len())]
        .iter()
        .collect()
}

/// 捕获组数组(元素是匹配文本,未参与匹配的位置给 `NULL`)。
fn groups_value(input: &[char], caps: &Captures, groups: usize) -> Value {
    let mut items = Vec::new();
    for i in 0..=groups {
        items.push(match caps.group(i) {
            Some((lo, hi)) => Value::String(slice(input, lo, hi)),
            None => Value::Null,
        });
    }
    Value::Array(items)
}

fn match_dict(input: &[char], caps: &Captures, groups: usize) -> Value {
    Value::dict(vec![
        (
            Value::String("matched".into()),
            Value::String(slice(input, caps.start(), caps.end())),
        ),
        (
            Value::String("start".into()),
            Value::Integer(caps.start() as i64),
        ),
        (
            Value::String("end".into()),
            Value::Integer(caps.end() as i64),
        ),
        (
            Value::String("groups".into()),
            groups_value(input, caps, groups),
        ),
    ])
}

/// `RE(pattern) -> DICT`
///
/// 返回**模式对象** `[pattern, groups]`。为什么不是字符串:wlwl 没有对象类型,
/// 而「编译一次、查多次组数」是计划里 `RE_GROUP_COUNT` 存在的理由 —— 组数
/// 放进返回的 `DICT`,调用方就不必为拿组数再编一遍。
///
/// **语法错 → 原生诊断 `E0030` 中止**(带列号)。不匹配**不是**错误:那是正常的
/// 业务分支,所以 `RE_*` 返回成功值或 `NULL`,唯一的诊断是元数 / 类型 / 编译错
/// —— 与 §11.1「程序员错误走原生诊断、数据违例才走 ERR 值」同一条线。
///
/// ⚠ [W-23 · D-1] 这里把编译产物**登记进进程级缓存** ⇒ 后续 `RE_SEARCH` /
///   `RE_TEST` / … 对同一模式串**不再重新编译**。
///   ⚠ 返回值**一字不变**:仍是 `[pattern, groups]` **两键**(规范 §16.5:
///   「`RE` 的返回值是 `DICT`,含 `pattern` 与 `groups` 两键」)——
///   缓存的句柄**不进语言面**(加第三个键会改 `LEN(RE(...))` / `KEYS(RE(...))`
///   这类可观察面,撞一致性册 `:223`)。
pub fn re(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let pattern = str_arg(host, NAME, "pattern", &args, 0)?;
    let progs = cached_programs(pattern).map_err(|e| host.diag(ErrorCode::E0030, e))?;
    Ok(Outcome::normal(Value::dict(vec![
        (
            Value::String("pattern".into()),
            Value::String(progs.search.source().to_string()),
        ),
        (
            Value::String("groups".into()),
            Value::Integer(progs.search.group_count() as i64),
        ),
    ])))
}

/// `RE_TEST(re, s) -> BOOLEAN`
///
/// **整串**是否匹配(两端锚定,且忽略 `(?m)` —— fullmatch 的语义是「整个输入
/// 被消费完」,`(?m)` 的 `$` 还会在内部换行处成立)。
pub fn re_test(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE_TEST";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let prog = program_full_of(host, NAME, &args[0])?;
    let text = str_arg(host, NAME, "subject", &args, 1)?;
    let hit = find(&prog, text, true).is_some();
    Ok(Outcome::normal(Value::Boolean(hit)))
}

/// `RE_MATCHES_FULL(re, s) -> BOOLEAN` —— **整串锚定**(`RE_TEST` 的别名)
///
/// **整串锚定**:两端锚定、忽略 `(?m)` —— 与 `RE_TEST` **逐字同语义**
/// (整串的语义是「整个输入被消费完」,`(?m)` 的 `$` 还会在内部换行处成立)。
///
/// [W-29 / v0.12.1 · 需求 H-5] 这是**别名**,**不是**新语义:
/// - `RE_TEST` 一字未改(不改名、不弃用、不改语义 —— 一致性册 `:224` 禁删禁弃用);
/// - 本函数**直接委托** `re_test` ⇒ 匹配语义**逐字同源**,不存在「两套实现
///   慢慢漂移」的可能;
/// - 诊断消息沿用**规范名** `RE_TEST`(与 `POP`→`AT_K` / `DEL`→`REMOVE_KEY`
///   同款:别名不另起一套消息)⇒ 两个名字的**可观察行为逐字相同**。
pub fn re_matches_full(
    host: &mut dyn StdHost,
    args: Vec<Value>,
) -> wlwl_error::WlwlResult<Outcome> {
    re_test(host, args)
}

/// `RE_SEARCH(re, s) -> DICT | NULL`
///
/// 第一个匹配:`[matched, start, end, groups]`。`start` / `end` 是**码点**
/// 下标(与 `CHAR_AT` / `INDEX_OF` 同一坐标系),不是字节偏移。
pub fn re_search(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE_SEARCH";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let prog = program_of(host, NAME, &args[0])?;
    let text = str_arg(host, NAME, "subject", &args, 1)?;
    let input: Vec<char> = text.chars().collect();
    let out = match find(&prog, text, false) {
        Some(caps) => match_dict(&input, &caps, prog.group_count()),
        None => Value::Null,
    };
    Ok(Outcome::normal(out))
}

/// `RE_FIND_ALL(re, s) -> ARRAY of DICT`
///
/// 全部**非重叠**匹配,形状与 `RE_SEARCH` 逐项相同。
/// 空匹配的邻接规则见 `find_all` 的文档(紧贴前一个匹配者跳过)。
pub fn re_find_all(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE_FIND_ALL";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let prog = program_of(host, NAME, &args[0])?;
    let text = str_arg(host, NAME, "subject", &args, 1)?;
    let input: Vec<char> = text.chars().collect();
    let items = find_all(&prog, text)
        .into_iter()
        .map(|caps| match_dict(&input, &caps, prog.group_count()))
        .collect();
    Ok(Outcome::normal(Value::Array(items)))
}

/// `RE_REPLACE(re, s, repl) -> STRING`
///
/// 替换全部非重叠匹配。`repl` 里 `$0` 是整体,`$1`… 是数字捕获组,
/// `${name}` 形式**不支持**(命名组本批不做,§3.7.2-6)—— 见到就 `E0030`,
/// 不静默原样输出。
pub fn re_replace(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE_REPLACE";
    if args.len() != 3 {
        return Err(arity(host, NAME, args.len(), 3));
    }
    let prog = program_of(host, NAME, &args[0])?;
    let text = str_arg(host, NAME, "subject", &args, 1)?;
    let repl = str_arg(host, NAME, "replacement", &args, 2)?;
    if repl.contains("${") {
        return Err(host.diag(
            ErrorCode::E0030,
            format!(
                "{NAME}: the replacement refers to a named group with ${{...}}, but named \
                      groups are not supported in this batch"
            ),
        ));
    }
    let input: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut cursor = 0usize;
    for caps in find_all(&prog, text) {
        let (lo, hi) = (caps.start(), caps.end());
        out.push_str(&slice(&input, cursor, lo));
        out.push_str(
            &expand(repl, &input, &caps)
                .map_err(|why| host.diag(ErrorCode::E0030, format!("{NAME}: {why}")))?,
        );
        cursor = hi;
    }
    out.push_str(&slice(&input, cursor, input.len()));
    Ok(Outcome::normal(Value::String(out)))
}

/// 🔴 [D13-031 / N-022]字符类的**区间端点**位置上不能出现的转义。
///
/// 预定义类不是一个码点 —— `[a-\d]` 里的 `\d` 若被取成字符 `'d'`(0x64),
/// 区间 `a-d` 会**顺利通过**校验,得到一个与作者意图毫无关系的类。
/// Go `regexp/syntax` 与 Python 都在这里报 `invalid character class range`;
/// 本成员此前**不报**,而契约里也没有一条 `[<预定义类>]` 用例(全仓唯一的
/// 取反字符类用例是纯字面的 `[^a-c]`)⇒ **守卫的扫描范围小于被承诺的行为**。
///
/// 零宽断言一并纳入:它落到同一个 `unescape_literal` 兜底分支,同样会被当成
/// 码点(`[0-\b]` ⇒ 区间 `0`..`'b'`),与 D13-030 同源。
fn range_endpoint_rejection(escape: char) -> Option<String> {
    if let Some(why) = zero_width_rejection(escape) {
        return Some(why);
    }
    predefined(escape).map(|_| range_endpoint_msg(escape))
}

/// 区间端点用了预定义类时的那句话。
fn range_endpoint_msg(escape: char) -> String {
    format!("`\\{escape}` is a predefined class and cannot be a character range endpoint")
}

/// 展开替换串里的 `$0` / `$1` / `$2`。
///
/// ⚠ [D13-032 / N-023]这段 doc 此前是**两段重复**且互相矛盾的拷贝:第一份写
/// 「未知组号 → `E0030`」就断在半句,第二份写「未知组号 → `Err`(调用方报
/// `E0030`)」。**两句话都不准确** —— 实际是**三分**:
///
/// | 情形 | 本成员 | Python | Go |
/// |---|---|---|---|
/// | `${name}` | `E0030`(在 [`re_replace`] 里判,不在本函数) | `error: unknown group name` | 原样输出 |
/// | `$N` 的 `N` **超出 `usize`**(如 `$99999999999999999999`) | `Err` ⇒ 调用方报 `E0030` | 同 Python | 原样输出 |
/// | `$N` 的 `N` 是合法数字但**没有这个组** | **静默展开成空串** | `error: invalid group reference` | 原样输出 |
/// | 组存在但**没参与这次匹配** | 空串 | 空串 | 空串 |
///
/// ⚠ **第三行那格是本成员的已知语义差分**,不是笔误:规范 §16 没有要求
/// `RE_REPLACE` 对越界组号报错,改它属**可观察行为变更**(今天展开成空串的
/// 程序会开始报错),按 §14.1 体例须单独立项。差分已在 §16.5 登记。
/// 下面那句「没参与匹配的组按空串处理(Python / Go 一致)」是**对的**,保留。
fn expand(repl: &str, input: &[char], caps: &Captures) -> Result<String, String> {
    let chars: Vec<char> = repl.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let Some(first) = chars.get(i + 1) else {
            out.push('$');
            i += 1;
            continue;
        };
        if !first.is_ascii_digit() {
            out.push('$');
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let mut num = String::new();
        while let Some(d) = chars.get(j) {
            if d.is_ascii_digit() {
                num.push(*d);
                j += 1;
            } else {
                break;
            }
        }
        let idx: usize = num
            .parse()
            .map_err(|_| "RE: bad group number".to_string())?;
        // 没参与匹配的组按空串处理(Python / Go 一致),**不是错误**。
        if let Some((lo, hi)) = caps.group(idx) {
            out.push_str(&slice(input, lo, hi));
        }
        i = j;
    }
    Ok(out)
}

/// `RE_SPLIT(re, s) -> ARRAY of STRING`
///
/// 按匹配切分(匹配本身不保留)。相邻空匹配不产生空段 —— 与 `find_all` 的
/// 邻接规则同源。
pub fn re_split(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE_SPLIT";
    if args.len() != 2 {
        return Err(arity(host, NAME, args.len(), 2));
    }
    let prog = program_of(host, NAME, &args[0])?;
    let text = str_arg(host, NAME, "subject", &args, 1)?;
    let input: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    for caps in find_all(&prog, text) {
        let (lo, hi) = (caps.start(), caps.end());
        if hi == lo {
            continue; // 空匹配不做切分点
        }
        if lo < cursor {
            continue; // 已被前面的匹配覆盖
        }
        out.push(Value::String(slice(&input, cursor, lo)));
        cursor = hi;
    }
    out.push(Value::String(slice(&input, cursor, input.len())));
    Ok(Outcome::normal(Value::Array(out)))
}

/// `RE_GROUP_COUNT(re) -> INTEGER`
///
/// 捕获组个数(不含整体)。供调用方在解码侧索引,免得每个成员各返回一种形状。
///
/// ⚠ [W-23 · D-1] **复杂度没有被缓存升格**:查缓存要先**哈希整个模式串**
///   ⇒ O(模式串长)⇒ 仍是 **O(p)**(p = 编译后指令数)。计划 §4 W-23 的非目标
///   明写「**不**把 `RE_GROUP_COUNT` 的复杂度从 O(p) 升格为 O(1」(那是规范性
///   改动 ⇒ 撞一致性册 `:223`)—— 这条口径靠「键就是模式串」天然成立。
pub fn re_group_count(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE_GROUP_COUNT";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let prog = program_of(host, NAME, &args[0])?;
    Ok(Outcome::normal(Value::Integer(prog.group_count() as i64)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 期望值**不是本实现的输出** —— 全部取自 Python `re`(leftmost-first,
    /// 与 Go/RE2 同族)或 RE2/Go 的文档规则。这是 D13-001 纪律在正则上的形状:
    /// 第二个实现交叉核对,不是自己抄自己。
    fn search(pat: &str, s: &str) -> Option<(String, usize, usize)> {
        let p = compile(pat).expect("compiles");
        let caps = find(&p, s, false)?;
        let input: Vec<char> = s.chars().collect();
        Some((
            slice(&input, caps.start(), caps.end()),
            caps.start(),
            caps.end(),
        ))
    }

    fn full(pat: &str, s: &str) -> bool {
        let p = compile_full(pat).expect("compiles");
        find(&p, s, true).is_some()
    }

    #[test]
    fn leftmost_first_alternation() {
        // 这四条是 leftmost-first 的判别式。Python `re` 逐条核对过:
        //   a|ab / "ab" -> "a"     ab|a / "ab" -> "ab"
        //   ab|b / "ab" -> "ab"    b|ab / "ab" -> "ab"
        assert_eq!(search("a|ab", "ab"), Some(("a".into(), 0, 1)));
        assert_eq!(search("ab|a", "ab"), Some(("ab".into(), 0, 2)));
        assert_eq!(search("ab|b", "ab"), Some(("ab".into(), 0, 2)));
        assert_eq!(search("b|ab", "ab"), Some(("ab".into(), 0, 2)));
        // 再加两条更刁的(Python 同样核对过):
        assert_eq!(search("a|ab|abc", "abc"), Some(("a".into(), 0, 1)));
        assert_eq!(search("abc|ab|a", "abc"), Some(("abc".into(), 0, 3)));
    }

    #[test]
    fn greedy_and_lazy_quantifiers() {
        assert_eq!(search("a+", "aaa"), Some(("aaa".into(), 0, 3)));
        assert_eq!(search("a+?", "aaa"), Some(("a".into(), 0, 1)));
        assert_eq!(search("(a+)(a*)", "aaa"), Some(("aaa".into(), 0, 3)));
        assert_eq!(search("a{2,3}", "aaaa"), Some(("aaa".into(), 0, 3)));
        assert_eq!(search("a{2,}", "aaaa"), Some(("aaaa".into(), 0, 4)));
    }

    #[test]
    fn classes_are_ascii_only() {
        assert_eq!(search("[a-c]+", "xabcz"), Some(("abc".into(), 1, 4)));
        assert_eq!(search("[^a-c]+", "xabcz"), Some(("x".into(), 0, 1)));
        assert_eq!(search(r"\d+", "ab123"), Some(("123".into(), 2, 5)));
        assert_eq!(search(r"\w+", "a_1-"), Some(("a_1".into(), 0, 3)));
        assert_eq!(search(r"\s+", "x \ty"), Some((" \t".into(), 1, 3)));
        // `\d` 不吃非 ASCII 数字 —— 这是 §3.7.2-5 钉的 ASCII 口径。
        assert_eq!(search(r"\d", "９"), None);
    }

    #[test]
    fn dot_and_anchors() {
        // `.` 默认不含换行(re2.h 的 dot_nl (false) 为权威)。
        assert_eq!(search("a.b", "a\nb"), None);
        assert_eq!(search("(?s)a.b", "a\nb"), Some(("a\nb".into(), 0, 3)));
        // `^`/`$` 默认锚**整个输入**;`(?m)` 才切行锚点。
        assert_eq!(search("^b$", "a\nb"), None);
        assert_eq!(search("(?m)^b$", "a\nb"), Some(("b".into(), 2, 3)));
        // ⚠ 与 RE2 的 `(?m)` 命名**故意相反** —— 见文件头 §5。
        assert!(full("^abc$", "abc"));
        assert!(!full("^b$", "a\nb"));
    }

    #[test]
    fn case_insensitive_is_ascii_only() {
        assert_eq!(search("(?i)abc", "ABC"), Some(("ABC".into(), 0, 3)));
        assert_eq!(search("abc", "ABC"), None);
        // 只 ASCII:`(?i)` 不折叠重音字母(Python 的 re.ASCII 同款)。
        assert_eq!(search("(?i)abc", "ÁBC"), None);
    }

    #[test]
    fn captures() {
        let p = compile("(a)(b)?").expect("compiles");
        let input: Vec<char> = "ac".chars().collect();
        let caps = find(&p, "ac", false).expect("matches");
        assert_eq!((caps.start(), caps.end()), (0, 1));
        assert_eq!(caps.group(1), Some((0, 1)));
        assert_eq!(caps.group(2), None, "an unmatched group is None, not empty");
        assert_eq!(slice(&input, 0, 1), "a");

        let p = compile("((a)(b))").expect("compiles");
        let caps = find(&p, "ab", false).expect("matches");
        assert_eq!(caps.group(1), Some((0, 2)));
        assert_eq!(caps.group(2), Some((0, 1)));
        assert_eq!(caps.group(3), Some((1, 2)));
        assert_eq!(p.group_count(), 3);
    }

    #[test]
    fn find_all_and_the_empty_match_rule() {
        let p = compile("a*").expect("compiles");
        let all = find_all(&p, "bab");
        let spans: Vec<(usize, usize)> = all.iter().map(|c| (c.start(), c.end())).collect();
        // RE2/Go 规则:紧贴前一个匹配的空匹配**跳过**(§3.7.2-2)。
        // Python 的 finditer 在这里**不同**(它会报 (2,2)),所以这条不是抄 Python。
        assert_eq!(spans, vec![(0, 0), (1, 2), (3, 3)]);

        let p = compile("[0-9]+").expect("compiles");
        let all = find_all(&p, "ab12cd345");
        let texts: Vec<String> = all
            .iter()
            .map(|c| slice(&c_text("ab12cd345"), c.start(), c.end()))
            .collect();
        assert_eq!(texts, vec!["12", "345"]);
    }

    fn c_text(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    // ── 性能守卫(埋点在 `Vm` 上,见 `Probe`)────────────────────────────
    //
    // ⚠ **判据是「可数出的代理指标」,不是绝对耗时。** 耗时在 CI 的机器上
    // 必然不稳;而「排序比较了几次」「候选表里留了几条」「克隆了几次」都是
    // 整数 —— 可复现、与机器无关、可做负向验证。
    //
    // ⚠ 埋点是 `#[cfg(test)]`,所以这些守卫只能落在**本文件**里:`wlwl-eval`
    // 的契约是另一个 crate,拿不到 `cfg(test)` 项。**对外行为的**守卫在那边。

    /// 跑一次 `find_all`(与 `find_all` 走同一条 `scan` + `select_all`),
    /// 把性能埋点带回来。
    fn probe_find_all(pat: &str, text: &str) -> (Vec<Captures>, Probe) {
        let p = compile(pat).expect("compiles");
        let input: Vec<char> = text.chars().collect();
        let mut vm = Vm::new(&p, &input);
        vm.keep_all();
        vm.scan(false);
        let caps = vm.select_all();
        (caps, vm.probe)
    }

    /// 同上,但走**单次搜索**那条路(`RE_SEARCH` / `RE_TEST`)。
    fn probe_find(pat: &str, text: &str) -> (Option<Captures>, Probe) {
        let p = compile(pat).expect("compiles");
        let input: Vec<char> = text.chars().collect();
        let mut vm = Vm::new(&p, &input);
        let caps = vm.run(false);
        (caps, vm.probe)
    }

    /// ⚠ 先证明埋点是**活的**。没有这一步,下面那些 `== 0` 就是空转 ——
    /// 「计数器没被接线」和「缺陷真的没了」在断言里长得一模一样。
    fn assert_probe_is_live(p: &Probe) {
        assert!(
            p.hits_recorded > 0 && p.add_calls > 0 && p.inst_visits > 0 && p.thread_visits > 0,
            "性能埋点是**死的**:hits_recorded={} add_calls={} inst_visits={} thread_visits={} \
             —— 下面那些断言会全部空转",
            p.hits_recorded,
            p.add_calls,
            p.inst_visits,
            p.thread_visits
        );
    }

    /// ⚠⚠ **等价判据用的旧算法** —— 逐字照抄自 [N-024] 那次性能改动**之前**
    /// 的 `find_all`:全量 `sort_by`(起点 → 分支路径 → 抵达顺序降序)+ 贪心
    /// 挑非重叠。它现在只活在测试里,当 `select_all` 的**等价判据**。
    ///
    /// ⚠ 为什么值得留着它:本成员对外承诺「leftmost-first + RE2/Go 空匹配
    /// 邻接」,而 [N-024] 那笔优化把「同左端只认第一个」从「排序之后」挪到了
    /// 「抵达之时」。这个等价**没有**任何一条手挑的样例能证明到「逐字」这个
    /// 程度 —— 有了它,改一边不动另一边就会当场红。
    fn legacy_select_all(mut hits: Vec<Hit>) -> Vec<Captures> {
        hits.sort_by(|a, b| {
            (a.start, &a.prio, std::cmp::Reverse(a.arrival)).cmp(&(
                b.start,
                &b.prio,
                std::cmp::Reverse(b.arrival),
            ))
        });
        let mut out: Vec<Captures> = Vec::new();
        for h in hits {
            let empty = h.start == h.end;
            match out.last() {
                None => {}
                Some(prev) => {
                    let pe = prev.end();
                    if empty && h.start <= pe {
                        continue;
                    }
                    if prev.start() == h.start {
                        // 同一左端只留排序后的第一个(优化前由 sort_by 保证)。
                        continue;
                    } else if h.start < pe {
                        // 与上一个匹配重叠。
                        continue;
                    }
                }
            }
            out.push(Captures {
                slots: (*h.caps).clone(),
            });
        }
        out
    }

    /// [N-024] 「优化前 = 优化后」的**逐字等价**判据。
    ///
    /// ⚠ 走**同一批抵达候选**的两条路:优化后的 `select_all`(按左端去重 +
    /// 按下标升序遍历)vs 优化前的 `legacy_select_all`(全量 `sort_by` + 贪心)。
    /// 手挑样例只能证明「我挑的那些还对」;这条证明的是**整批候选逐字同解**。
    ///
    /// ⚠ 语料含**左最长/leftmost-first 判别式**(`a|ab`、`ab*`、`(a|)`)、
    /// **空匹配密集**(`a*`、`\w*`)、**多左端**、**嵌套可选组**
    /// (`(a?)(b?)`)—— 也就是去重最可能出错的那几类形状。伪随机语料用
    /// 确定性 LCG(不引依赖),失败时能原样复现。
    #[test]
    fn find_all_is_literally_equal_to_the_pre_optimization_algorithm() {
        let pats = [
            "a*", "a+", "a|ab", "(a)(b)?", "(?:ab)+", "[a-c]*", "a*?", "[0-9]+", "x?", "a{0,3}",
            "(a+)(b*)", "\\w*", "b*|a*", "(a|ab)*c", "a+b*", ".?", "(a|)", "(a?)(b?)", "[ab]*c",
            "^a*$", "a.c",
        ];
        let texts = [
            "",
            "a",
            "b",
            "ab",
            "bab",
            "abab",
            "aa",
            "aaabbb",
            "abcabc",
            "aabaa",
            "xyz",
            "aaa",
            "abababab",
            "9a1b22c333",
            "aaXaa",
            "ababab",
            "cab",
            "a\nb",
            "aaaaabaaaaa",
        ];
        let mut n = 0usize;
        for pat in pats {
            let prog = compile(pat).unwrap_or_else(|e| panic!("{pat}: {e}"));
            for text in texts {
                let input: Vec<char> = text.chars().collect();
                let mut vm = Vm::new(&prog, &input);
                vm.keep_all();
                vm.scan(false);
                let optimized = vm.select_all();
                let legacy = legacy_select_all(std::mem::take(&mut vm.legacy_hits));
                assert_eq!(
                    optimized, legacy,
                    "pattern {pat:?} on {text:?}: optimized != legacy"
                );
                n += 1;
            }
        }
        // 伪随机加料:确定性 LCG(不引依赖),长度 0..14 的 {a,b,c} 串。
        let mut seed: u64 = 0x2545F4914F6CDD1D;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let alphabet = ['a', 'b', 'c'];
        for pat in pats {
            let prog = compile(pat).unwrap_or_else(|e| panic!("{pat}: {e}"));
            for _ in 0..300 {
                let len = (next() % 15) as usize;
                let text: String = (0..len).map(|_| alphabet[(next() % 3) as usize]).collect();
                let input: Vec<char> = text.chars().collect();
                let mut vm = Vm::new(&prog, &input);
                vm.keep_all();
                vm.scan(false);
                let optimized = vm.select_all();
                let legacy = legacy_select_all(std::mem::take(&mut vm.legacy_hits));
                assert_eq!(
                    optimized, legacy,
                    "pattern {pat:?} on {text:?}: optimized != legacy"
                );
                n += 1;
            }
        }
        assert!(
            n >= 6_000,
            "语料被削到 {n} 对了 —— 等价判据的覆盖面塌了,不是它变绿了"
        );
    }

    /// [N-024] `find_all` 的候选表必须**按左端去重**。
    ///
    /// 规模取 `a*` × n 个 a:贪含量词在**每个**位置都抵达 `Match`,而「左最早」
    /// 让这些候选**共用同一个左端 0** ⇒ 抵达次数 O(n) 而「不同起点数」= 1。
    /// 这正是旧实现的病态形态:攒 n+1 份拷贝(每份带 `caps` + `prio` 两次堆
    /// 分配)再 `sort_by` 一遍。
    #[test]
    fn find_all_dedups_by_left_end_instead_of_sorting() {
        const N: usize = 200;
        let (caps, p) = probe_find_all("a*", &"a".repeat(N));
        // 语义没变:整串一整个匹配(贪心的定义 = 尽可能长)。
        assert_eq!(caps.len(), 1, "a* on {N} a's is one match");
        assert_eq!((caps[0].start(), caps[0].end()), (0, N));
        assert_probe_is_live(&p);
        assert!(
            p.hits_recorded >= N,
            "每一步都该有一次 Match 抵达(实测 {})",
            p.hits_recorded
        );

        // ⚠ **这条才是抓住 N-024 的那条断言**(负向验证实测:缺陷形态下
        // 201 条,修好后 1 条)。它量的是**候选表规模** = 这一步的内存量级。
        assert_eq!(
            p.hits_retained, 1,
            "同一左端只该留最优的一个,实测留了 {} 条",
            p.hits_retained
        );

        // ⚠⚠ **下面这条不是抓住 N-024 的那条** —— 别把它当成守卫的证据。
        // 它是一条**上界**:没有比较排序 ⇒ 每条候选至多一次 `prio` 比。
        // 负向验证实测:缺陷形态(`sort_by` 那一版)在这个输入上是 **200 次**,
        // 修好后也是 **200 次** —— 因为抵达顺序里 `start` 本来就非降
        // (左最早 ⇒ 每一步留在列表里的 `Match` 线程都是当前最靠左的那条),
        // Rust 的自适应排序一眼就认出「已排好」而只花了 O(n) 次比较。
        // ⇒ 报告里「O(n log n) 排序」那句在**最坏情况**上成立,实测不是
        // 主要开销;**主要开销是那 201 份候选拷贝**,也就是上面那条断言。
        // 留这条断言是为了挡住「哪天换了个抵达顺序,排序真的跑成 n log n」。
        assert!(
            p.order_comparisons <= p.hits_recorded,
            "定序比较({})不该超过抵达次数({})—— 排序回来了",
            p.order_comparisons,
            p.hits_recorded
        );

        // 两档规模:候选表条数不得跟着**输入长度**长。
        let (caps4, p4) = probe_find_all("a*", &"a".repeat(4 * N));
        assert_eq!((caps4[0].start(), caps4[0].end()), (0, 4 * N));
        assert_eq!(
            p4.hits_retained, 1,
            "规模 ×4 时候选表仍是 1 条(实测 {})",
            p4.hits_retained
        );
        assert!(p4.order_comparisons <= p4.hits_recorded);
    }

    /// [N-026] `Split` 分支**不许**再深拷贝 `prio` / `caps`。
    ///
    /// ⚠ **判据是可数出的代理指标,不是绝对耗时** —— 耗时在 CI 的机器上必然不稳。
    ///   缺陷形态:每次 `Split` **3 次堆分配**(2 × `prio` 的 `Vec` 克隆 +
    ///   1 × `caps` 的 `Vec` 克隆),而 `Split` 在「每线程每步」都会走到 ⇒
    ///   分配次数 ∝ 线程数 × 步数。
    ///   修好后:① `prio` 是**内联数组** ⇒ 克隆只是栈上拷 64 字节;
    ///   ② `caps` 是 `Rc<Vec<..>>` ⇒ 克隆只是**引用计数 +1**。
    ///
    /// ⚠ **写时复制不许退化成「每步一次深拷贝」** —— 那个退化会让 `caps` 的
    ///   收益整个归零(引用计数白加了)。⇒ `caps_cow_copies` 不得超过
    ///   `split_calls`:每次 `Split` 至多制造**一份**共享,至多被拷贝一次。
    #[test]
    fn n026_the_split_branch_no_longer_deep_copies_prio_or_caps() {
        let (caps, p) = probe_find_all("(a+)(b*)", &"aabbb aa".repeat(64));
        assert!(!caps.is_empty());
        // ⚠ 用量组:先证明 `Split` 分支真的被走到了 —— 否则下面两条
        //   「== 0」会**空转**(没走到 ⇒ 计数点没触发 ⇒ 假绿)。
        assert!(
            p.split_calls > 0,
            "`Split` 分支一次都没走到 —— 下面的断言会全部空转"
        );
        assert_eq!(
            p.prio_copies, 0,
            "`Split` 分支深拷贝了 `prio` {} 次 —— 内联数组被改回去了",
            p.prio_copies
        );
        assert_eq!(
            p.caps_copies, 0,
            "`Split` 分支深拷贝了 `caps` {} 次 —— `Rc` 被改回去了",
            p.caps_copies
        );
        assert!(
            p.caps_cow_copies <= p.split_calls,
            "写时复制拷贝了 {} 次,超过了 `Split` 次数 {} ⇒ 它退化成了「每步一次拷贝」",
            p.caps_cow_copies,
            p.split_calls
        );
    }

    /// [N-024] 同一左端留下的是**最优**那一个 —— 不是先到的那一个。
    ///
    /// ⚠ 这条是**语义**守卫而非性能守卫:去重做错成「先到者胜」时结果会从
    /// `end = n` 掉成 `end = 0`(空匹配),所以它能直接拆穿方向搞反的优化。
    /// `note_hit` 的注记记着:那一处**曾经**是升序,实测那条路径仍是完整长度
    /// —— 也就是说「方向搞反」在别的用例上不一定露馅,必须单独钉住。
    #[test]
    fn find_all_keeps_the_best_candidate_not_the_first_one() {
        for n in [1usize, 2, 64, 65, 200] {
            let (caps, _) = probe_find_all("a*", &"a".repeat(n));
            assert_eq!(
                (caps[0].start(), caps[0].end()),
                (0, n),
                "greedy a* on {n} a's must keep the longest reach"
            );
        }
    }

    /// [N-026] 每线程每步的分配热点:`Inst` 克隆 / `vec![start]` 工作栈 /
    /// 线程克隆。修好前这三处分别是「每输入位置」「每次 `add` 调用」
    /// 「每线程每步」的一次堆分配。
    #[test]
    fn the_scan_clones_nothing_per_thread_per_step() {
        // ⚠ 末尾那个 `c` 不是凑数:输入**必须真的匹配得上**,否则 `note_hit`
        // 一次都不会被调,`hits_recorded` 恒 0,埋点的活性断言会先红。
        // 单次搜索那条路(`RE_SEARCH` / `RE_TEST`)才是日常热点:`find_all`
        // 之外它才是默认路径,两条路的 `add` / `advance` 是同一份代码。
        let text = format!("{}c", "ab".repeat(100));
        for (label, p) in [
            ("find", probe_find("[ab]+c", &text).1),
            ("find_all", probe_find_all("[ab]+c", &text).1),
        ] {
            assert_probe_is_live(&p);
            assert_eq!(
                p.inst_clones, 0,
                "[{label}] Inst 是每线程每步克隆的 ⇒ 每扫过一个 Class 就分配一次"
            );
            assert_eq!(
                p.thread_clones, 0,
                "[{label}] Thread 带 caps+prio 两个 Vec ⇒ 每线程每步两次分配"
            );
            assert_eq!(
                p.work_allocs, 0,
                "[{label}] ε 闭包的工作栈必须是 Vm 上的常驻缓冲,不是每次调用新建"
            );
            assert_eq!(
                p.work_reuses, p.add_calls,
                "[{label}] 每次 add 都得复用同一个工作栈"
            );
        }
    }

    #[test]
    fn pathological_pattern_stays_linear() {
        // `(a*)*b` 是回溯引擎的经典指数爆炸模式。本成员没有回溯 ⇒ 它在
        // 10 000 个 a 上**不匹配且立刻返回**。
        let p = compile("(a*)*b").expect("compiles");
        let text = "a".repeat(10_000);
        let caps = find(&p, &text, false);
        assert!(caps.is_none(), "a*)*b cannot match a run of a's");
        // 指令数与输入长度**无关** —— 这是「线性」的可核对形状。
        assert!(
            p.len() < 100,
            "program grew with the input: {} insts",
            p.len()
        );
    }

    #[test]
    fn unsupported_constructs_are_rejected_at_compile_time() {
        // 这三条是本成员存在的理由:必须在**编译期**拒,否则线性承诺当场破。
        for pat in [r"(?=a)", r"(?!a)", r"(a)\1", r"(?<=a)b"] {
            let err = compile(pat).expect_err(&format!("{pat} must be rejected"));
            assert!(
                err.contains("not supported"),
                "{pat}: unexpected error {err:?}"
            );
        }
        // 真的拒了,而不是「能编译但慢」—— 用一个真的指数爆炸模式对比。
        let text = "a".repeat(30);
        let p = compile("(a*)*b").expect("compiles");
        assert!(find(&p, &text, false).is_none());
    }

    #[test]
    fn syntax_errors_carry_a_column() {
        let err = compile("a(b").expect_err("unclosed group must fail");
        assert!(err.contains("column"), "{err:?}");
        let err = compile("[a-").expect_err("unterminated class must fail");
        assert!(err.contains("column"), "{err:?}");
        assert!(
            compile("a{b").is_ok(),
            "a{{ that is not a quantifier is a literal"
        );
        assert!(compile("a\\{2}").is_ok(), "an escaped brace is a literal");
    }

    // ── 编译缓存(W-23 · D-1)──────────────────────────────────────────
    //
    // ⚠ 这几条只钉**缓存自身**的不变量(「同一模式串只编一次」「命中缓存拿到
    //   的是**同一份**产物」)。**对外行为**的守卫在 `wlwl-eval` 那边
    //   (`tests/regex_cache_contract.rs`)—— 跨 crate 读不到本文件的
    //   `#[cfg(test)]` 项,反过来那边也读不到这里的私有计数。
    //
    // ⚠ 模式串**全文件唯一**:缓存是**进程级**的,同一个模式串若被两个用例
    //   同时编译,「只编一次」那条会被对方先编掉。

    #[test]
    fn the_cache_compiles_a_pattern_once_and_hands_out_the_same_artifact() {
        const PAT: &str = "^unit-cache-probe[0-9]+q$";
        assert_eq!(
            pattern_compiles(PAT),
            0,
            "a pattern nobody compiled yet must start at 0 compiles"
        );
        let first = cached_programs(PAT).expect("compiles");
        let second = cached_programs(PAT).expect("compiles");
        assert_eq!(
            pattern_compiles(PAT),
            1,
            "the pattern must have been compiled exactly once"
        );
        // ⚠ **同一份产物**(不是「又编了一遍但结果一样」):指针相等才是
        //   「命中缓存」的证据 —— 只断言结果相同的话,「每次重编」也全绿。
        assert!(
            Arc::ptr_eq(&first.search, &second.search),
            "the second lookup must hand out the very same compiled program"
        );
        assert!(
            Arc::ptr_eq(&first.full, &second.full),
            "the full-match variant must be cached too"
        );
        // 两种语义都还得能用(缓存不是「只缓存了其中一半」)。
        assert!(find(&first.search, "unit-cache-probe7q", false).is_some());
        assert!(find(&first.full, "unit-cache-probe7q", true).is_some());
        assert_eq!(first.search.group_count(), 0);
    }

    #[test]
    fn a_broken_pattern_is_not_cached_and_reports_the_same_diagnostic() {
        // ⚠ 失败**不进缓存** ⇒ 每次都重新编译、每次都报**同一条**诊断
        //   (带列号)⇒ 诊断文本逐字不变。
        const BAD: &str = "unit-cache-broken(";
        let e1 = cached_programs(BAD).expect_err("must fail");
        let e2 = cached_programs(BAD).expect_err("must fail");
        assert_eq!(e1, e2, "the diagnostic must be byte-identical");
        assert!(e1.contains("column"), "{e1:?}");
        // ⚠ 失败**也计入工作量** ⇒ 两次尝试 = 2 次编译(而不是「失败不计数」
        //   那样读出 0 —— 那会把「真的编了两遍」说成「一遍都没编」)。
        assert_eq!(
            pattern_compiles(BAD),
            2,
            "each attempt on a broken pattern must be counted as one compilation"
        );
    }
}
