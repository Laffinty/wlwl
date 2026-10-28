//! 诊断的**打印侧** —— 一个错误可能携带**多条**诊断。
//!
//! [v1.0 规范 §8.6]「丢弃时逐个报告」:一个丢弃点上丢弃的集合里有几个
//! `ERR`,就产生**几条** `E0103`;诊断按它们在容器中的**出现序**排列
//! (顺序是规范的一部分,§11.1)。
//!
//! ⇒ 打印侧**必须逐条打印**。只取 `WlwlError::diagnostic()`(第一条)
//! 会让同一个丢弃点上的第 2..N 条诊断**静默消失** —— 用户看到的是
//! 「只丢了一个」,而实际上丢了一堆。
//!
//! 这个模块是 CLI 里**唯一**渲染诊断文本、**唯一**定退出码的地方:
//! 别的模块只负责**收集**诊断(`Vec<WlwlDiagnostic>`)或把错误**交**过来
//! (`&WlwlError`)。
//!
//! ⚠ 为什么单独成文件:「N 条诊断 ⇒ N 段输出」这条规则必须**可测**。
//! `wlwl-cli` 是纯二进制 crate(没有 lib target)⇒ `tests/` 下的集成测试
//! 只能驱动**进程**,摸不到内部函数 ⇒ 规则与它的单元测试住在同一个
//! 模块里(本文件的 `mod tests`),才能被直接盯住。
//!
//! ── [F-4 / W-33] 人形态的两处**增量** ────────────────────────────────
//!
//! 诊断要能被**人**读懂,光有「哪一行、哪一列」不够 —— 还要看得见**那一段
//! 源码**与**该怎么改**。本模块在既有形状(`wlwl_error::render_human`)
//! **之上**加两样东西,**只在 `human` 形态里加**:
//!
//! 1. **列号高亮**:源码行下面补一行 `^` 指针,指到出错的**列**。
//! 2. **提示语**:把 `suggestion_code` 里的 `Note` 提成 `= hint:` 行 ——
//!    实现侧早就给每个码写好了修法,但它们此前**只在 JSON 形态里可见**。
//!
//! ⚠ `json` / `jsonl` 是**机器读**路径 ⇒ 字段与结构**一个字节都没动**。

use std::process::ExitCode;

use wlwl_error::{Suggestion, WlwlDiagnostic, WlwlError};

use crate::OutputFormat;

/// 一个错误渲染成的**输出文本**(每条诊断自带行尾换行)。
///
/// **逐条**:错误里有几条诊断,文本里就有几段 —— 顺序 = 出现序(§11.1)。
///
/// 每段后面各补一个 `\n`,因此调用方 `eprint!` 出来的字节流与「逐条
/// `eprintln!`」**完全一致** ⇒ 单条诊断的输出**一个字节都没变**。
pub fn render_error_text(err: &WlwlError, format: OutputFormat) -> String {
    let mut out = String::new();
    for d in err.diagnostics() {
        out.push_str(&render_diag_text(d, format));
        out.push('\n');
    }
    out
}

/// 一条诊断的文本。
///
/// ⚠ 三种形态的**行尾**不一致,这是既有形状,这里**原样保留**:
/// `human` 自带行尾换行(`render_human` 的既有形状),`json` / `jsonl`
/// 不带。调用方一律**再补一个** `\n` ⇒ 出来的字节流与旧的逐条
/// `eprintln!` **完全一致**。
///
/// ⚠ [F-4 / W-33] `human` 形态在**既有形状之上**加两样东西:
/// **列号高亮**(源码行下面的一行 `^`)与**提示语**
/// (`suggestion_code` 里的 `Note` —— 此前它们**只在 JSON 形态里可见**)。
/// 两样都**只加在 `human`** ⇒ `json` / `jsonl` 的字节流**一个字节都没变**。
pub fn render_diag_text(d: &WlwlDiagnostic, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => render_human(d),
        OutputFormat::Json => d.render_json(),
        OutputFormat::Jsonl => d.render_jsonl(),
    }
}

/// **人形态**:既有 `render_human()` 的逐字形状 **+ 列号高亮 + 提示语**。
///
/// ⚠ 既有形状**一个字节都没动** —— 本函数只做两件事:
/// ① 在**源码行**下面插一行 `^` 指针(没有源码行 ⇒ 不插);
/// ② 把 `suggestion_code` 里的 `Note` 提成 `= hint:` 行
/// (与 `hint` 字段**文字相同**的那条跳过 ⇒ 不重复渲染)。
///
/// ⚠ **只在 `human` 里做**:机器读的两条路(`json` / `jsonl`)直接调
/// `render_json` / `render_jsonl`,字段与结构**一个都没动**。
fn render_human(d: &WlwlDiagnostic) -> String {
    let base = d.render_human();
    let caret = caret_row(d);
    let hints = suggestion_rows(d);
    if caret.is_none() && hints.is_empty() {
        return base;
    }
    // `render_human` 的既有形状:源码行 = `{:>3} | <源码>` 那一行。
    let gutter = format!("{:>3} | ", d.location.line);
    let mut rows: Vec<String> = base.split_inclusive('\n').map(str::to_string).collect();
    // ① 源码行在哪一行(没有 `source_line` ⇒ 没有这一行 ⇒ 什么也不插)。
    let src_row = rows.iter().position(|r| r.starts_with(&gutter));
    // ② 列号高亮:紧跟在源码行下面。
    if let (Some(c), Some(i)) = (&caret, src_row) {
        rows.insert(i + 1, format!("{c}\n"));
    }
    // ③ 提示语:紧跟在既有 `= hint:` 之后;没有 `hint` 就跟在源码块之后;
    //    两者都没有就追加在末尾。
    if !hints.is_empty() {
        let at = rows
            .iter()
            .rposition(|r| r.starts_with("  = hint: "))
            .map(|i| i + 1)
            .or_else(|| src_row.map(|i| i + 1 + usize::from(caret.is_some())))
            .unwrap_or(rows.len());
        for (k, h) in hints.into_iter().enumerate() {
            rows.insert(at + k, h);
        }
    }
    rows.concat()
}

/// **列号高亮**那一行 —— 源码行下面的 `^` 指针。
///
/// ⚠ **列号是字节列,不是字符列**:词法器每吃一个**字节**就 `col += 1`
/// (见 `wlwl-lexer` 的 `bump()`)⇒ 一行里若有中文,字节列与字符列**不等**
/// (实测:`PRINT("中文", UNDEF_X);` 的 `UNDEF_X` 报在**第 17 字节**,
/// 而它在字符列上是第 13 列)⇒ 直接拿 `col` 当空格数,指针会指到**后面**去。
/// 这里先把字节偏移折成**字符偏移**再排指针。
///
/// ⚠ `col_end` 是**开区间**的右端(词法器的 token span 形如
/// `(line, col, line, col_after_token)`)⇒ 高亮宽度 = `col_end - col` 个字符。
/// 零宽(诊断点不在任何 token 上,例如「这里缺一个 `;`」)⇒ 至少一个 `^`。
fn caret_row(d: &WlwlDiagnostic) -> Option<String> {
    let src = d.source_line.as_deref()?;
    if d.location.line == 0 || d.location.col == 0 {
        return None;
    }
    let start = (d.location.col as usize).saturating_sub(1);
    let end = if d.location.col_end > d.location.col {
        (d.location.col_end as usize).saturating_sub(1)
    } else {
        start
    };
    let start_char = char_offset(src, start);
    let end_char = char_offset(src, end);
    let width = end_char.saturating_sub(start_char).max(1);
    Some(format!(
        "{:>3} | {}{}",
        d.location.line,
        " ".repeat(start_char),
        "^".repeat(width)
    ))
}

/// 字节偏移 → **字符**偏移(向下取整到字符边界)。
///
/// ⚠ 字节偏移落在多字节字符**中间**时(token 起点必在字符边界上,但 `col`
/// 也可能来自别处的构造)⇒ 取**该字符之前**的字符数,绝不让切片 panic。
fn char_offset(src: &str, byte: usize) -> usize {
    let mut n = 0usize;
    for (i, _) in src.char_indices() {
        if i >= byte {
            break;
        }
        n += 1;
    }
    n
}

/// `suggestion_code` 里的 `Note` → `= hint:` 行。
///
/// ⚠ **为什么此前终端里看不到**:实现侧早就给每个码写好了「该怎么办」的
/// `Note`(解析器的 `E0010` / `E0011` / `E0012` / `E0013` / `E0043`,
/// 求值器的 `E0014` / `E0020` / `E0022` / `E0023` / `E0030` / `E0040` /
/// `E0041` / `E0043` / `E0061` / `E0070` / `E0102` / ……)—— 但
/// `render_human` 只渲染 `hint` 与 `related`,**`suggestion_code` 只出现在
/// JSON 形态** ⇒ 终端里的用户**看不到**自己代码的修法。这里把它们提到
/// 人形态里。
///
/// ⚠ **去重**:`E0020` 的「did you mean one of: …?」**同时**写进 `hint`
/// 与 `suggestion_code`(两条路都得带候选名)⇒ 与 `hint` **文字相同**的那条
/// **跳过**,不重复渲染。
fn suggestion_rows(d: &WlwlDiagnostic) -> Vec<String> {
    let mut out = Vec::new();
    for s in &d.suggestion_code {
        let Suggestion::Note { description } = s else {
            continue;
        };
        if d.hint.as_deref() == Some(description.as_str()) {
            continue;
        }
        out.push(format!("  = hint: {}\n", description));
    }
    out
}

/// 退出码契约(见 spec §11.5)。
///
/// - `0` 成功
/// - `1` 程序不对:运行期诊断,或静态层拦下的类型 / 契约诊断
/// - `2` 命令行用法错误(clap 自己定的,未改)
/// - `3` **源文件没能解析出来**
///
/// [v0.10.1] 改之前只有 `0` / `1` / `2`,而 `1` 同时表示「语法错」「类型错」
/// 「未定义名」「除零」…… 脚本拿到 `1` 无法判断该回去改代码还是改逻辑。
/// 只加**一个**码,不加一串:能解析出来但不对的,都还是「程序不对」,归 `1`。
///
/// 取**数值**形态(`u8`)而不是 `ExitCode`:`ExitCode` 没有 `PartialEq`,
/// 规则本身就没法被单元测试盯住。
pub fn exit_code_value(d: &WlwlDiagnostic) -> u8 {
    if d.code.is_parse_failure() {
        3
    } else {
        1
    }
}

/// 多条诊断时的退出码 —— **按第一条**的类别定。
///
/// 规范 §11.4 的退出码表是按**故障类别**给的,而一个错误里的多条诊断通常
/// **同类**(同一个丢弃点上的 N 条 `E0103`)。取第一条 ⇒ 与「只有一条诊断」
/// 时的行为**完全一致** ⇒ 同一个故障不会因为多报了几条而拿到不同的退出码。
///
/// ⚠ **不改退出码表本身**:这里只回答「多条时按哪一条定」。
///
/// ⚠ 空诊断集:不可能来自 `WlwlError`(空集在构造侧就 panic 了),但这里
/// 仍然给 `1` 而不是 panic —— 退出码函数不该成为崩溃源。
pub fn exit_code_value_for_diags(diags: &[WlwlDiagnostic]) -> u8 {
    match diags.first() {
        Some(d) => exit_code_value(d),
        None => 1,
    }
}

/// 多条诊断时的退出码(`ExitCode` 形态)。
pub fn exit_code_for_diags(diags: &[WlwlDiagnostic]) -> ExitCode {
    ExitCode::from(exit_code_value_for_diags(diags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_error::{ErrorCode, Location};

    fn diag(code: ErrorCode, message: &str, line: u32) -> WlwlDiagnostic {
        WlwlDiagnostic::new(code, message, Location::point("t.wll", line, 1))
    }

    /// 一个丢弃点丢了 3 个 `ERR` ⇒ 3 条 `E0103`(§8.6)。
    fn three_discarded() -> WlwlError {
        WlwlError::from(vec![
            diag(ErrorCode::E0103, "unhandled ERR was discarded: boom", 1),
            diag(ErrorCode::E0103, "unhandled ERR was discarded: bang", 2),
            diag(ErrorCode::E0103, "unhandled ERR was discarded: zap", 3),
        ])
    }

    /// [v1.0 §8.6] 一个错误带 3 条诊断 ⇒ 输出里出现 **3** 条。
    ///
    /// 这条测试是「只打第一条」那个缺陷的**守门人**:把渲染改成只取
    /// `diagnostics()[..1]` ⇒ 这里立刻红。
    #[test]
    fn one_error_with_three_diagnostics_renders_three() {
        let text = render_error_text(&three_discarded(), OutputFormat::Human);
        assert_eq!(text.matches("E0103").count(), 3, "{text}");
        assert_eq!(text.matches("-->").count(), 3, "{text}");
    }

    /// 顺序 = 出现序(§11.1):不排序、不去重、不倒过来。
    #[test]
    fn diagnostics_are_rendered_in_occurrence_order() {
        let text = render_error_text(&three_discarded(), OutputFormat::Human);
        let boom = text.find("boom").expect("boom");
        let bang = text.find("bang").expect("bang");
        let zap = text.find("zap").expect("zap");
        assert!(boom < bang && bang < zap, "{text}");
    }

    // ── [F-4 / W-33] 人形态:列号高亮 + 提示语 ─────────────────────────

    /// **人形态的完整形状**(逐字):既有形状 **+ 高亮行 + 提示行**。
    ///
    /// ⚠ 这条测试是「只加高亮、不动别的」的**反向守卫**:既有形状的每一行
    /// 都**逐字**在这条期望里 ⇒ 改坏了形状(多一行、少一行、顺序变了)
    /// 这里立刻红。
    #[test]
    fn the_human_shape_is_exactly_the_old_one_plus_highlight_and_hint() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            "undefined name `UNDEF_X`",
            Location::range("t.wll", 1, 7, 1, 14),
        )
        .with_source_line("PRINT(UNDEF_X);")
        .with_hint("did you mean one of: INDEX?")
        .with_suggestion(Suggestion::Note {
            description: "no binding for this name".into(),
        });
        let expected = concat!(
            "error[E0020]: undefined name `UNDEF_X`\n",
            "  --> t.wll:1:7\n",
            "  |\n",
            "  1 | PRINT(UNDEF_X);\n",
            // 高亮行:源码行下面,指针落在**第 7 列**(= `UNDEF_X` 的起点),
            // 宽度 = `col_end - col` = 7 个字符。
            "  1 | ",
            "      ",
            "^^^^^^^\n",
            "  = hint: did you mean one of: INDEX?\n",
            "  = hint: no binding for this name\n",
        );
        assert_eq!(render_diag_text(&d, OutputFormat::Human), expected);
    }

    /// ⚠ **列号是字节列**:源码行里有中文时,指针必须落在**字符列**上。
    ///
    /// 实测口径(2026-10-28):词法器每吃一个**字节** `col += 1`
    /// (`wlwl-lexer` 的 `bump()`)⇒ `PRINT("中文", UNDEF_X);` 里的
    /// `UNDEF_X` 起于**第 17 字节**,而在**字符列**上是第 13 列。
    /// ⇒ 拿字节列当空格数,指针会指到**后面**去。
    #[test]
    fn the_caret_uses_character_columns_not_byte_columns() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            "undefined name `UNDEF_X`",
            Location::range("t.wll", 1, 17, 1, 24),
        )
        .with_source_line("PRINT(\"中文\", UNDEF_X);");
        let text = render_diag_text(&d, OutputFormat::Human);
        let row = text.lines().nth(4).expect("caret row");
        // 行首 6 个字符(`  1 | `)+ 12 个字符的前缀 ⇒ 指针在第 18 个字符位。
        assert_eq!(row.find('^'), Some(18), "{row}");
        // `UNDEF_X` = 7 个字符 ⇒ 7 个 `^`。
        assert_eq!(row.matches('^').count(), 7, "{row}");
    }

    /// `suggestion_code` 里的 `Note` 此前**只在 JSON 里可见** ⇒ 终端里
    /// 看不到「该怎么改」。现在提到人形态。
    #[test]
    fn human_output_surfaces_the_suggestion_notes() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0013,
            "expected ';' after expression",
            Location::point("t.wll", 1, 18),
        )
        .with_suggestion(Suggestion::Note {
            description: "add `;` to terminate the preceding statement".into(),
        });
        let text = render_diag_text(&d, OutputFormat::Human);
        assert!(
            text.contains("= hint: add `;` to terminate the preceding statement"),
            "{text}"
        );
    }

    /// `hint` 与 `suggestion_code` **文字相同**的那条**只渲染一次**。
    ///
    /// (`E0020` 的「did you mean one of: …?」两条路都带 ⇒ 不去重就重复了。)
    #[test]
    fn a_note_equal_to_the_hint_is_not_rendered_twice() {
        let note = "did you mean one of: INDEX?";
        let d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            "undefined name `UNDEF_X`",
            Location::point("t.wll", 1, 7),
        )
        .with_hint(note)
        .with_suggestion(Suggestion::Note {
            description: note.into(),
        });
        let text = render_diag_text(&d, OutputFormat::Human);
        assert_eq!(
            text.matches("did you mean one of: INDEX?").count(),
            1,
            "{text}"
        );
    }

    /// ⚠ 机器读的两条路**一个字节都没变**。
    #[test]
    fn json_and_jsonl_are_byte_identical_to_the_error_crate_rendering() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0013,
            "expected ';' after expression",
            Location::range("t.wll", 1, 18, 1, 19),
        )
        .with_source_line("LET(a, (PRINT(1) PRINT(2)));")
        .with_suggestion(Suggestion::Note {
            description: "add `;`".into(),
        });
        assert_eq!(render_diag_text(&d, OutputFormat::Json), d.render_json());
        assert_eq!(render_diag_text(&d, OutputFormat::Jsonl), d.render_jsonl());
    }

    /// 没有源码行 ⇒ **不插**高亮行(既有形状原样)。
    #[test]
    fn a_diagnostic_without_a_source_line_gets_no_highlight() {
        let d = WlwlDiagnostic::new(
            ErrorCode::E0041,
            "circular IMPORT detected: a -> b -> a",
            Location::point("<module>", 0, 0),
        );
        assert_eq!(render_diag_text(&d, OutputFormat::Human), d.render_human());
    }

    /// JSONL 形态:一条诊断一行 ⇒ N 行。
    #[test]
    fn jsonl_renders_one_record_per_diagnostic() {
        let text = render_error_text(&three_discarded(), OutputFormat::Jsonl);
        assert_eq!(text.lines().count(), 3, "{text}");
        assert_eq!(text.matches("\"E0103\"").count(), 3, "{text}");
    }

    /// JSON 形态:一条诊断一个 JSON 对象(与既有逐条打印的形状一致)。
    #[test]
    fn json_renders_one_object_per_diagnostic() {
        let text = render_error_text(&three_discarded(), OutputFormat::Json);
        assert_eq!(text.matches("\"E0103\"").count(), 3, "{text}");
    }

    /// 单条诊断的输出**一个字节都没变**(与旧的逐条 `eprintln!` 等价)。
    #[test]
    fn single_diagnostic_output_is_byte_identical_to_the_old_path() {
        let err = WlwlError::from(diag(ErrorCode::E0103, "boom", 1));
        for format in [OutputFormat::Human, OutputFormat::Json, OutputFormat::Jsonl] {
            let text = render_error_text(&err, format);
            let one = format!("{}\n", render_diag_text(err.diagnostic(), format));
            assert_eq!(text, one, "{format:?}");
        }
    }

    /// 退出码:多条时**按第一条**的类别定(§11.4)。
    #[test]
    fn exit_code_follows_the_first_diagnostic() {
        // 第一条是解析失败 ⇒ 3
        let parse_first = WlwlError::from(vec![
            diag(ErrorCode::E0010, "expected expression", 1),
            diag(ErrorCode::E0103, "boom", 2),
        ]);
        assert_eq!(exit_code_value_for_diags(parse_first.diagnostics()), 3);

        // 第一条不是解析失败 ⇒ 1(哪怕后面跟着一条解析失败)
        let parse_second = WlwlError::from(vec![
            diag(ErrorCode::E0103, "boom", 1),
            diag(ErrorCode::E0010, "expected expression", 2),
        ]);
        assert_eq!(exit_code_value_for_diags(parse_second.diagnostics()), 1);
    }

    /// 单条诊断时退出码与旧口径**逐字一致**(回归护栏)。
    #[test]
    fn single_diagnostic_exit_code_is_unchanged() {
        let parse = WlwlError::from(diag(ErrorCode::E0010, "expected expression", 1));
        assert_eq!(exit_code_value_for_diags(parse.diagnostics()), 3);
        let other = WlwlError::from(diag(ErrorCode::E0103, "boom", 1));
        assert_eq!(exit_code_value_for_diags(other.diagnostics()), 1);
    }
}
