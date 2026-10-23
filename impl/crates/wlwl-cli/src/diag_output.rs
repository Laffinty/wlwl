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

use std::process::ExitCode;

use wlwl_error::{WlwlDiagnostic, WlwlError};

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
pub fn render_diag_text(d: &WlwlDiagnostic, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => d.render_human(),
        OutputFormat::Json => d.render_json(),
        OutputFormat::Jsonl => d.render_jsonl(),
    }
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
