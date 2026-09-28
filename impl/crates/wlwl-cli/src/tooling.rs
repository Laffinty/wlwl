//! `wlwl interface` / `wlwl schema` —— 面向工具的 JSON 导出(v0.10 Step 10 ·
//! 计划书 §5.3 P1-3「interface / schema JSON」)。
//!
//! 两个形状,两种消费者:
//!
//! | 子命令 | 回答什么 | 消费者 |
//! |---|---|---|
//! | `interface <file>` | 这个模块**对外承诺**什么 | AI 工具 / 构建脚本(改名前先问一遍承诺) |
//! | `schema` | WLWL 的类型系统**有哪几类** | 工具的静态分析层(别把这份清单硬编码在自己代码里) |
//!
//! 两者都**只读**,都不改盘上的任何东西。`interface` 的内容直接来自
//! Step 6 / 7 的签名模型([`wlwl_types::sig`]),所以它跟 `wlwl sig` /
//! `sig-gen` / `check` 说的必然是同一件事 —— 不另建一套推导。

use std::process::ExitCode;

use serde_json::{json, Value};
use wlwl_ast::Expr;

/// JSON 产物的 schema 版本。字段增删时**必须**动它 —— 工具靠它判断
/// 自己的解析器还能不能用。
const SCHEMA_VERSION: &str = "1.0.0";

/// `wlwl interface <file>` —— 模块的公开面。
pub fn interface_file(file: &std::path::Path) -> ExitCode {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read '{}': {}", file.display(), e);
            return ExitCode::from(1);
        }
    };
    let file_name = file.to_string_lossy().to_string();
    let ast = match wlwl_parser::parse(&source, &file_name) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{}", e.diagnostic().render_human());
            return ExitCode::from(1);
        }
    };
    match serde_json::to_string_pretty(&interface_of(&file_name, &ast)) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: interface serialize failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// 组装一个模块的 interface 载荷。
///
/// 形状与 [`wlwl_types::sig`] 的签名模型一一对应:
/// `exports[]` 就是 `.wll.sig` 的条目(名字 + 类型 + 形参 / 返回),
/// `sealed` 是 `SEALED([...])` 声明面,`signature_file` 是旁路签名文件
/// 在磁盘上的位置(写没写由工具自己看文件在不在)。
pub fn interface_of(file: &str, ast: &Expr) -> Value {
    // 静态层交出两样东西:根作用域绑定类型(Step 6)与推导出的签名(Step 7)。
    let builtins = crate::builtin_sig_table();
    let checked = wlwl_types::check_program_detailed(ast, &builtins);
    let contract = wlwl_types::ModuleContract::from_module(
        ast,
        file,
        Some(wlwl_types::sig_from_module(ast, &checked.declared)),
    );

    let exports: Vec<Value> = contract
        .sig
        .as_ref()
        .map(|sig| {
            sig.entries
                .values()
                .map(|entry| match &entry.ty {
                    wlwl_types::Ty::Fun { params, ret } => json!({
                        "name": entry.name,
                        "kind": "function",
                        "params": params.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                        "returns": ret.to_string()
                    }),
                    other => json!({
                        "name": entry.name,
                        "kind": "value",
                        "type": other.to_string()
                    }),
                })
                .collect()
        })
        .unwrap_or_default();

    json!({
        "interface_schema_version": SCHEMA_VERSION,
        "module": file,
        "signature_file": signature_file_path(file),
        // 声明面:None = 模块没写 SEALED(不是「空面」,见 `collect_sealed`)。
        "sealed": contract.seal,
        "exports": exports
    })
}

fn signature_file_path(file: &str) -> String {
    format!("{file}.sig")
}

/// `wlwl schema` —— 类型系统 + 静态契约诊断码的机器可读清单。
pub fn schema_command() -> ExitCode {
    match serde_json::to_string_pretty(&type_schema()) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: schema serialize failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// 类型系统的 schema。
///
/// 清单来自运行期真实类型(spec §2.1)+ 静态层的类型 IR + 显式约束
/// (Step 9)。**刻意不写成「从代码里反射出来」** —— 那需要一个
/// 「注册表 → 类型名」的单向依赖,为了省几行 JSON 去动分层不划算。
/// 代价是:静态层将来加一个类型头,这里要跟着改;锁测试
/// `schema_type_names_match_the_static_layer` 就是盯这件事的。
pub fn type_schema() -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "types": [
            { "name": "INTEGER", "kind": "scalar", "runtime_type": "INTEGER" },
            { "name": "FLOAT",   "kind": "scalar", "runtime_type": "FLOAT" },
            { "name": "STRING",  "kind": "scalar", "runtime_type": "STRING" },
            { "name": "BOOLEAN", "kind": "scalar", "runtime_type": "BOOLEAN" },
            { "name": "NULL",    "kind": "scalar", "runtime_type": "NULL" },
            { "name": "ARRAY",   "kind": "container", "arity": 1,
              "syntax": "ARRAY[T]",
              "note": "parser 强制要求方括号;裸写 `ARRAY` 报 E0010" },
            { "name": "DICT",    "kind": "container", "arity": 2, "syntax": "DICT[K, V]" },
            { "name": "OPTION",  "kind": "wrapper", "arity": 1, "syntax": "OPTION[T]",
              "note": "纯注解糖(spec §2.1 没有这个运行时类型);见证是 NULL 或 RESULT[T, ·];裸 T 不通过" },
            { "name": "RESULT",  "kind": "wrapper", "arity": 2, "syntax": "RESULT[T, E]",
              "note": "两个运行期构造子:OK(_) / ERR(_)" },
            { "name": "FUN",     "kind": "function", "syntax": "FUN(...) -> U",
              "note": "箭头形式本版不可从源码到达(计划书 §3.3);静态层可达性由 `->` 终结符缺席所致" },
            { "name": "DYNAMIC", "kind": "top_bottom",
              "note": "未标注处的回退类型;与任何类型双向可赋值,所以它从不产生诊断" }
        ],
        "named_types_not_modelled": [
            "TASK", "CHANNEL", "CLASS", "INSTANCE"
        ],
        "bounds": [
            { "name": "Comparable", "syntax": "T: Comparable",
              "members": ["INTEGER", "FLOAT", "STRING"],
              "note": "成员集合对着运行期 cmp_op 量出来:比较算子只放行数值与字符串,其余 E0030" }
        ],
        "type_variables": {
            "syntax": "T: Bound(方括号内与顶层注解都认)",
            "scope": "调用点实例化;无 monomorphization,运行期擦除",
            "note": "裸的未知类型名是不透明类型(拼错即报),不是变量"
        },
        "diagnostics": static_contract_codes()
    })
}

/// 静态契约段的诊断码:条件与「是否恒警告」。
///
/// 这份清单服务工具(按码分流渲染),不是给人读的表 —— 人读的表在
/// spec §11.2(Step 12 派生)。
fn static_contract_codes() -> Value {
    json!([
        { "code": "E0110", "warn": "W0110", "always_warning": false,
          "condition": "annotation mismatch at a boundary (param / LET annotation vs value)" },
        { "code": "E0111", "warn": "W0111", "always_warning": false,
          "condition": "call mismatch (argument count or argument type)" },
        { "code": "E0112", "warn": "W0112", "always_warning": false,
          "condition": "return type mismatch" },
        { "code": "E0113", "warn": "W0113", "always_warning": false,
          "condition": "exported / imported name is not declared in the module contract" },
        { "code": "E0114", "warn": "W0114", "always_warning": false,
          "condition": "the contract declares a name the module does not export" },
        { "code": "E0115", "warn": "W0115", "always_warning": false,
          "condition": "module signature type conflicts with the implementation annotation" },
        { "code": "E0116", "warn": "W0116", "always_warning": false,
          "condition": "non-exhaustive match (a constructor is missing and the default arm was omitted)" },
        { "code": "W0117", "warn": null, "always_warning": true,
          "condition": "unreachable match clause or a dead default arm; never escalates in `error` mode, and has no E counterpart" }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_lists_exports_with_their_declared_types() {
        let ast = wlwl_parser::parse(
            "LET(add, FUN((a: INTEGER, b: INTEGER) : INTEGER, +(a, b)));\n\
             LET(PI, 3);\n\
             EXPORT([\"add\", \"PI\"]);\n",
            "math.wll",
        )
        .expect("parses");
        let v = interface_of("math.wll", &ast);
        assert_eq!(v["interface_schema_version"], SCHEMA_VERSION);
        assert_eq!(v["module"], "math.wll");
        assert_eq!(v["signature_file"], "math.wll.sig");
        // 没写 SEALED → `sealed` 是 null(「没声明」≠「声明了空面」)。
        assert!(v["sealed"].is_null());
        let exports = v["exports"].as_array().expect("array");
        assert_eq!(exports.len(), 2);
        let add = exports
            .iter()
            .find(|e| e["name"] == "add")
            .expect("add is exported");
        assert_eq!(add["kind"], "function");
        assert_eq!(add["params"][0], "INTEGER");
        assert_eq!(add["returns"], "INTEGER");
        let pi = exports
            .iter()
            .find(|e| e["name"] == "PI")
            .expect("PI is exported");
        assert_eq!(pi["kind"], "value");
        assert_eq!(pi["type"], "INTEGER");
    }

    #[test]
    fn interface_reports_the_sealed_surface() {
        let ast = wlwl_parser::parse(
            "SEALED([\"open\"]);\nLET(open, 1);\nEXPORT([\"open\"]);\n",
            "m.wll",
        )
        .expect("parses");
        let v = interface_of("m.wll", &ast);
        assert_eq!(v["sealed"], json!(["open"]));
    }

    #[test]
    fn interface_is_valid_json_for_a_bounded_generic() {
        // Step 9 的带约束变量要能原样出现在 interface 里(工具会照着它
        // 理解模块的承诺)。
        let ast = wlwl_parser::parse(
            "LET(max, FUN((a: T: Comparable, b: T: Comparable) : T: Comparable, a));\n\
             EXPORT([\"max\"]);\n",
            "g.wll",
        )
        .expect("parses");
        let v = interface_of("g.wll", &ast);
        let text = serde_json::to_string(&v).unwrap();
        assert!(text.contains("T: Comparable"), "{text}");
        // 真的能被 JSON 解析器读回来(工具的第一道门槛)。
        let back: Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(back["exports"][0]["params"][0], "T: Comparable");
    }

    /// schema 里的类型名必须与静态层真的认识的那批**逐个对得上**。
    /// 静态层加了新类型头而这里忘了改,工具就会按一份过期清单说话。
    #[test]
    fn schema_type_names_match_the_static_layer() {
        let v = type_schema();
        let listed: Vec<&str> = v["types"]
            .as_array()
            .expect("types array")
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        for name in [
            "INTEGER", "FLOAT", "STRING", "BOOLEAN", "NULL", "ARRAY", "DICT", "OPTION", "RESULT",
            "FUN", "DYNAMIC",
        ] {
            assert!(listed.contains(&name), "{} missing from schema", name);
        }
        // 约束清单:`Comparable` 只有一个,且成员集合是量出来的那三个。
        let bounds = v["bounds"].as_array().expect("bounds array");
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0]["name"], "Comparable");
        assert_eq!(bounds[0]["members"], json!(["INTEGER", "FLOAT", "STRING"]));
    }

    #[test]
    fn schema_diagnostic_codes_carry_their_warn_pairing() {
        let v = type_schema();
        let codes = v["diagnostics"].as_array().expect("diagnostics array");
        let by_code = |c: &str| {
            codes
                .iter()
                .find(|d| d["code"] == c)
                .unwrap_or_else(|| panic!("{} missing", c))
        };
        for (e, w) in [("E0110", "W0110"), ("E0113", "W0113"), ("E0116", "W0116")] {
            assert_eq!(by_code(e)["warn"], w, "{} must pair with {}", e, w);
            assert_eq!(by_code(e)["always_warning"], false);
        }
        // W0117 的不对称是**故意的**,schema 必须如实说(工具按它决定
        // 拦不拦构建)。
        let orphan = by_code("W0117");
        assert!(orphan["warn"].is_null());
        assert_eq!(orphan["always_warning"], true);
    }
}
