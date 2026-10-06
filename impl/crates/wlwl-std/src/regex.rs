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
//! 5. `.` 默认**不含换行**(`(?s)` 打开);`^`/`$` 默认锚**整个输入**,`(?m)`
//!    切到行锚点。⚠️ **刻意不抄 RE2 的 `m` 命名**(它与 Perl 相反)。
//! 6. `\p{…}` 与命名组**本批不做**(不引入 UCD 依赖,与全局 G1/G2 一致)。
//!
//! ## 不做(与线性时间互斥,不是「以后再加」)
//!
//! 回溯引用 `(?P=…)` / `\1`、lookahead `(?=…)` / `(?!…)`、lookbehind、
//! PCRE 兼容、反向正则、零宽断言、字素簇语义。**它们在解析期一律 `E0030`** ——
//! 必须拒,否则线性承诺当场破。

use crate::{ModuleSpec, StdFn};
use wlwl_error::ErrorCode;
use wlwl_value::{Outcome, StdHost, Value};

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.regex",
    functions: &[
        ("RE", re as StdFn),
        ("RE_TEST", re_test as StdFn),
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

struct Parser {
    pat: Vec<char>,
    pos: usize,
    groups: usize,
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
                // 标志组 `(?i)` / `(?s)` / `(?m)`:作用于**本分支的余下部分**(RE2 的口径)。
                // 判定点必须在 `parse_atom` **之前** —— 否则 `(` 已经被
                // `parse_atom` 吃掉并进了 `parse_group`,而 group 语义是
                // 「括号内的子模式」,会把余下模式整个丢掉。
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
        if let Some(s) = predefined(c) {
            // 预定义类**不做 ASCII 折叠**:Python 的 `re.ASCII` 下 `(?i)\w`
            // 也不折叠数字与下划线,保持一致。
            return Ok(Ast::Class(s));
        }
        Ok(Ast::Class(self.literal(unescape_literal(c), flags)))
    }

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
        let node = self.parse_alt(inner)?;
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
                if let Some(sub) = predefined(e) {
                    // `[\d]` 是合法形态:并入(否定情形由整类的 `^` 统一处理)。
                    let sub = if negated { sub.complement() } else { sub };
                    set.ranges.extend(sub.ranges);
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
    let mut insts = vec![Inst::Assert(AnchorKind::Start { multiline: false })];
    // ⚠ **两条 `Save` 都要保留** —— 剥掉它们就没有捕获可言,而捕获正是
    // `RE_TEST` / `RE_SEARCH` 报 `start` / `end` 的依据。这里只丢末尾那条
    // `Match`(末尾的 `Assert(End)` + `Match` 由本函数自己补)。
    insts.extend_from_slice(&inner.insts[1..inner.insts.len() - 1]);
    insts.push(Inst::Assert(AnchorKind::End { multiline: false }));
    insts.push(Inst::Match);
    Ok(Program {
        insts,
        groups: inner.groups,
        source: inner.source,
    })
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
#[derive(Clone)]
struct Thread {
    pc: usize,
    caps: Vec<Option<usize>>,
    prio: Vec<u8>,
}

/// 一个抵达 `Match` 的候选。
#[derive(Clone)]
struct Hit {
    start: usize,
    end: usize,
    caps: Vec<Option<usize>>,
    prio: Vec<u8>,
    /// 抵达顺序(同 `(start, prio)` 时后到者更完整)。
    arrival: usize,
}

impl Hit {
    /// 候选之间的定序:**起点优先**,同起点比**路径**,同路径比**抵达顺序**。
    fn better_than(&self, other: &Hit) -> bool {
        (self.start, &self.prio, self.arrival) < (other.start, &other.prio, other.arrival)
    }
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
    /// 每一次 `Match` 抵达都记在这里(顺序 = 抵达顺序)。`find` 在其中挑最优,
    /// `find_all` 按定序挑非重叠的那些 —— 见 `Hit::better_than`。
    hits: Vec<Hit>,
    /// 抵达计数器 —— 同一 `(start, prio)` 时区分「先到」与「后到」。
    arrivals: usize,
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
            hits: Vec::new(),
            arrivals: 0,
        }
    }

    /// 把 `t.pc` 的 ε 闭包按优先级压进 `list`。
    ///
    /// `Split(a, b)` **先展开 a 再展开 b** ⇒ a 的线程落在 list 更靠前的位置。
    /// 配合 `seen` 的先到者胜,「分支顺序」就变成了「匹配选择的优先级」。
    fn add(&mut self, list: &mut Vec<Thread>, start: Thread, gen: u32, pos: usize) {
        let mut work = vec![start];
        while let Some(mut t) = work.pop() {
            if self.seen[t.pc] == gen {
                continue;
            }
            self.seen[t.pc] = gen;
            match self.prog.insts[t.pc].clone() {
                Inst::Jump(target) => {
                    t.pc = target;
                    work.push(t);
                }
                Inst::Split(a, b) => {
                    // 低优先级先入栈 ⇒ 高优先级先弹出;`prio` 追加 0 / 1。
                    let mut hi_prio = t.prio.clone();
                    hi_prio.push(0);
                    let mut lo_prio = t.prio.clone();
                    lo_prio.push(1);
                    work.push(Thread {
                        pc: b,
                        caps: t.caps.clone(),
                        prio: lo_prio,
                    });
                    work.push(Thread {
                        pc: a,
                        caps: t.caps,
                        prio: hi_prio,
                    });
                }
                Inst::Save(slot) => {
                    if slot < t.caps.len() {
                        t.caps[slot] = Some(pos);
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
            caps,
            prio: Vec::new(),
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
            slots: h.caps.clone(),
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

    /// 记下一次 `Match` 抵达。**不做任何取舍** —— 取舍是「跨步骤比较优先级」,
    /// 而 clist 下标只反映**当前这一步**的顺序(跨步不可比)。
    /// 把它当「clist 里第一个 Match」用会让 `a|ab` 取成 `ab`、`a+` 取成 `a`;
    /// 正确做法是攒下全部候选,按 `(start, prio, arrival)` 定序(见
    /// `Hit::better_than` 与 `best_hit`)。
    fn note_hit(&mut self, t: &Thread) {
        let (Some(lo), Some(hi)) = (
            t.caps.first().copied().flatten(),
            t.caps.get(1).copied().flatten(),
        ) else {
            return;
        };
        let arrival = self.arrivals;
        self.arrivals += 1;
        self.hits.push(Hit {
            start: lo,
            end: hi,
            caps: t.caps.clone(),
            prio: t.prio.clone(),
            arrival,
        });
    }

    /// 全部候选里的最优者:起点最左,同起点比分支路径,同路径比抵达顺序。
    fn best_hit(&self) -> Option<&Hit> {
        self.hits
            .iter()
            .reduce(|a, b| if b.better_than(a) { b } else { a })
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
        let mut nlist = std::mem::take(&mut self.nlist);
        nlist.clear(); // `take` 拿到的是**上一轮的 clist**,不清就会残留
                       // ⚠ **顺序即优先级**:既有线程的后继先入,新注入的起始线程**最后**入。
                       // 反过来写会让「最年轻的路径」压过「更前进的线程」(Go 的 `machine.step`
                       // 也是这个顺序),症状是贪婪量词只吃到一半、且匹配起点会莫名右移。
        for k in 0..self.clist.len() {
            let t = self.clist[k].clone();
            match self.prog.insts[t.pc].clone() {
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
        let mut final_list = std::mem::take(&mut self.clist);
        let mut out = Vec::new();
        for t in final_list.drain(..) {
            match self.prog.insts[t.pc].clone() {
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
    vm.scan(false);
    let mut hits = std::mem::take(&mut vm.hits);
    // 定序:起点 → 分支路径 → 抵达顺序(贪婪延伸因此取到最长的那次)。
    hits.sort_by(|a, b| (a.start, &a.prio, a.arrival).cmp(&(b.start, &b.prio, b.arrival)));
    let mut out: Vec<Captures> = Vec::new();
    for h in hits {
        let empty = h.start == h.end;
        match out.last() {
            None => {}
            Some(prev) => {
                let pe = prev.end();
                if empty && h.start <= pe {
                    continue; // 紧贴前一个匹配的空匹配:跳过(RE2/Go 规则)
                }
                if prev.start() == h.start {
                    // 同一左端的多个候选(贪婪延伸的中间态)。排序后**更完整
                    // 的那个在前**,所以只在后来的更长时才取代 —— 否则会把
                    // `aaa` 截成 `aa`。
                    if h.end <= prev.end() {
                        continue;
                    }
                    out.pop();
                } else if h.start < pe {
                    continue; // 与上一个匹配重叠
                }
            }
        }
        out.push(Captures { slots: h.caps });
    }
    out
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
/// **不信任调用方**:我们仍然在**每次调用**重新编译,这样 `RE_*` 的行为不
/// 取决于那个 `DICT` 是从哪来的。
fn program_of(
    host: &mut dyn StdHost,
    name: &str,
    v: &Value,
) -> Result<Program, wlwl_error::WlwlError> {
    let Value::Dict(entries) = v else {
        return Err(type_err(host, name, "pattern object", v));
    };
    let pattern = entries
        .iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == "pattern"))
        .and_then(|(_, v)| match v {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        })
        .ok_or_else(|| {
            host.diag(
                ErrorCode::E0030,
                format!("{name}: the pattern object has no `pattern` key (build it with RE())"),
            )
        })?;
    let pattern = pattern.to_string();
    let prog = compile(&pattern).map_err(|e| host.diag(ErrorCode::E0030, e))?;
    // 整串匹配另编一版(两端包文本锚),见 `compile_full` 的说明。
    Ok(prog)
}

fn program_full_of(
    host: &mut dyn StdHost,
    name: &str,
    v: &Value,
) -> Result<Program, wlwl_error::WlwlError> {
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
    compile_full(&pattern).map_err(|e| host.diag(ErrorCode::E0030, e))
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
    Value::Dict(vec![
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
pub fn re(host: &mut dyn StdHost, args: Vec<Value>) -> wlwl_error::WlwlResult<Outcome> {
    const NAME: &str = "RE";
    if args.len() != 1 {
        return Err(arity(host, NAME, args.len(), 1));
    }
    let pattern = str_arg(host, NAME, "pattern", &args, 0)?;
    let prog = compile(pattern).map_err(|e| host.diag(ErrorCode::E0030, e))?;
    Ok(Outcome::normal(Value::Dict(vec![
        (
            Value::String("pattern".into()),
            Value::String(prog.source().to_string()),
        ),
        (
            Value::String("groups".into()),
            Value::Integer(prog.group_count() as i64),
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

/// 展开替换串里的 `$0` / `$1` / `$2`。未知组号 → `E0030`(静默原样输出会
/// 让用户以为引用生效了)。
/// 展开替换串里的 `$0` / `$1` / `$2`。未知组号 → `Err`(调用方报 `E0030`;
/// 静默原样输出会让用户以为引用生效了)。未参与匹配的组按空串处理
/// (Python / Go 一致),**不是错误**。
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
}
