//! `wlwl:std.json` — `PARSE`, `STRINGIFY` (v0.3 §15.3 + §14.4 E0070/E0071)
//! + [v0.11.3 M7 / addendum-09 W-02] 低层词法层 `JSON_TOKENIZE` /
//!   `JSON_LOCATE` / `JSON_VALIDATE`。

use crate::compat::*;
use crate::{ModuleSpec, StdCtx, StdFn};
use wlwl_error::ErrorCode;
use wlwl_error::WlwlError;
use wlwl_value::{type_name, Outcome, StdHost, Value};

pub(crate) mod lexer;
pub(crate) mod path;
pub(crate) mod schema;

pub(super) fn std_parse_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    let s = expect_string("PARSE", &args, 0, 1)?;
    match serde_json::from_str::<StdValue>(s) {
        Ok(v) => Ok(v),
        Err(e) => Err(StdError {
            code: ErrorCode::E0070,
            message: format!("PARSE: {}", e),
        }),
    }
}

pub(super) fn std_stringify_inner(
    _ctx: &mut StdCtx,
    args: Vec<StdValue>,
) -> Result<StdValue, StdError> {
    if args.len() != 1 {
        return Err(arity_error("STRINGIFY", args.len(), 1));
    }
    // `serde_json::to_string` only fails on non-serializable types
    // (e.g. NaN, map with non-string keys). Our value type has only
    // serializable shapes, so this is a defensive E0071 — kept so the
    // spec surface is complete.
    match serde_json::to_string(&args[0]) {
        Ok(s) => Ok(StdValue::String(s)),
        Err(e) => Err(StdError {
            code: ErrorCode::E0071,
            message: format!("STRINGIFY: {}", e),
        }),
    }
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.json",
    functions: &[
        ("PARSE", std_parse as StdFn),
        ("STRINGIFY", std_stringify as StdFn),
        // [v0.11.3 M7 / addendum-09 W-02] 低层词法层。
        ("JSON_TOKENIZE", json_tokenize as StdFn),
        ("JSON_LOCATE", json_locate as StdFn),
        ("JSON_VALIDATE", json_validate as StdFn),
        // [v0.11.3 M7 / addendum-09 W-04] 路径族。
        ("JSON_GET", json_get as StdFn),
        ("JSON_SET", json_set as StdFn),
        ("JSON_DELETE", json_delete as StdFn),
        ("JSON_MERGE", json_merge as StdFn),
        ("JSON_CHUNK_ITEMS", json_chunk_items as StdFn),
        // [v0.11.3 M7 / addendum-09 W-05]
        ("JSON_CHECK", json_check as StdFn),
    ],
};

// ── [v0.11.3 M7 / addendum-09 W-02] 低层词法层 ────────────────────────
//
// ⚠ **失败口径与上面两个旧成员刻意不同**(`ADR-0033` D5):旧两成员走 `E0070` /
// `E0071` **诊断**且 §3.6 冻结不许动;新成员走规范 §11 已定的**`ERR` 值**
// `[kind, op, path, line, column, reason]`。
//
// ⚠ `path` 在本批这三个成员里**恒缺省**:`JSON_TOKENIZE` / `JSON_VALIDATE` 只做
// **语法**,不看结构(`JSON_TOKENIZE` 连结构都不看),没有路径可言。`path` 从
// `JSON_CHECK`(W-05,schema)与路径族(W-04)才开始有 —— 那时它才有意义。
// 现在塞一个恒为 `NULL` 的 `path` 只会让人以为「路径是空的」而不是「此处无路径」。

/// 词法错误的统一形状。`path` 见上面的说明(此处不出现)。
///
/// ⚠ **`offset` 必须在载荷里**,否则这套 API 自己转不过弯:`JSON_VALIDATE` 报的是
/// `line` / `column`,而 `JSON_LOCATE` 的实参是 `offset` —— 没有 `offset` 用户就得
/// 自己把行列反算回码点下标,而本模块不提供那个反算。⇒ `line`/`column` 是给人看的,
/// `offset` 是给机器接下一棒用的,三个一起给。
fn json_err(op: &str, e: lexer::JsonError) -> Outcome {
    Outcome::normal(Value::Err(Box::new(Value::Dict(vec![
        (
            Value::String("kind".into()),
            Value::String("JsonError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (
            Value::String("offset".into()),
            Value::Integer(e.offset as i64),
        ),
        (Value::String("line".into()), Value::Integer(e.line as i64)),
        (
            Value::String("column".into()),
            Value::Integer(e.column as i64),
        ),
        (Value::String("reason".into()), Value::String(e.reason)),
    ]))))
}

fn chars_and_arg(
    host: &mut dyn StdHost,
    op: &str,
    args: Vec<Value>,
) -> Result<(Vec<char>, usize), WlwlError> {
    if args.len() != 1 {
        return Err(host.diag(
            ErrorCode::E0022,
            format!("{op}: function expects 1 argument(s), got {}", args.len()),
        ));
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        other => {
            return Err(host.diag(
                ErrorCode::E0030,
                format!("{op}: expected string, got {}", type_name(other)),
            ))
        }
    };
    Ok((s.chars().collect(), 0))
}

/// `JSON_TOKENIZE(s) -> ARRAY[DICT]`
///
/// 每个 token 是 `{kind, start, end, text}`。**纯词法、无结构**(§2.1),所以
/// `[}` 在这里合法 —— 它不是词法错误。结构由 `JSON_VALIDATE` 管。
///
/// ⚠ 偏移是**码点、0 基**(`ADR-0033` 决定):全语言唯一的字符串切分原语 `SUB` 是
/// 按码点的,字节偏移用户根本用不了。
pub fn json_tokenize(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    let (chars, _) = chars_and_arg(host, "JSON_TOKENIZE", args)?;
    match lexer::tokenize(&chars) {
        Ok(toks) => {
            let items = toks
                .iter()
                .map(|t| {
                    Value::Dict(vec![
                        (
                            Value::String("kind".into()),
                            Value::String(t.kind.as_str().into()),
                        ),
                        (
                            Value::String("start".into()),
                            Value::Integer(t.start as i64),
                        ),
                        (Value::String("end".into()), Value::Integer(t.end as i64)),
                        (
                            Value::String("text".into()),
                            Value::String(chars[t.start..t.end].iter().collect()),
                        ),
                    ])
                })
                .collect();
            Ok(Outcome::normal(Value::Array(items)))
        }
        Err(e) => Ok(json_err("JSON_TOKENIZE", e)),
    }
}

/// `JSON_LOCATE(s, offset) -> DICT` = `{line, column, token_start}`
///
/// `offset` 是**码点、0 基**;`line` / `column` 是 **1 基**。
///
/// ⚠ `offset` **不**做越界报错:越界 / 落在空白里都返回「该位置附近的」行列表,
/// 因为「报个错让你知道 offset 写错了」不如「给一个仍然有意义的附近位置」。
pub fn json_locate(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if args.len() != 2 {
        return Err(host.diag(
            ErrorCode::E0022,
            format!(
                "JSON_LOCATE: function expects 2 argument(s), got {}",
                args.len()
            ),
        ));
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        other => {
            return Err(host.diag(
                ErrorCode::E0030,
                format!("JSON_LOCATE: expected string, got {}", type_name(other)),
            ))
        }
    };
    let offset = match &args[1] {
        Value::Integer(i) if *i >= 0 => *i as usize,
        other => {
            return Err(host.diag(
                ErrorCode::E0030,
                format!(
                    "JSON_LOCATE: expected a non-negative offset, got {}",
                    type_name(other)
                ),
            ))
        }
    };
    let chars: Vec<char> = s.chars().collect();
    let (line, column) = lexer::line_col(&chars, offset.min(chars.len()));
    let token_start = lexer::token_start_at(&chars, offset.min(chars.len()))
        .map(|v| Value::Integer(v as i64))
        .unwrap_or(Value::Null);
    Ok(Outcome::normal(Value::Dict(vec![
        (Value::String("line".into()), Value::Integer(line as i64)),
        (
            Value::String("column".into()),
            Value::Integer(column as i64),
        ),
        (Value::String("token_start".into()), token_start),
    ])))
}

/// `JSON_VALIDATE(s) -> OK | ERR`
///
/// 只验语法(含结构、深度、f64 范围),**不**看 schema。schema 是 W-05 的
/// `JSON_CHECK`。
///
/// ⚠ 它与 `PARSE` 是**同一份语法的两套判定**,接受集必须逐条一致 ——
/// 见 `lexer` 模块注释里那张「serde 比 Python 更严」的三行表。
pub fn json_validate(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    let (chars, _) = chars_and_arg(host, "JSON_VALIDATE", args)?;
    match lexer::validate(&chars) {
        Ok(()) => Ok(Outcome::normal(Value::Ok(Box::new(Value::Null)))),
        Err(e) => Ok(json_err("JSON_VALIDATE", e)),
    }
}

// ── [v0.11.3 M7 / addendum-09 W-04] 路径族 ─────────────────────────────
//
// ⚠ 这五个成员与 W-02 那三个的失败口径**有两处不同**,都记在 `path.rs` 里:
//   ① 寻址冲突返 `ERR` 且**带 `path`**(W-02 那三个不带,因为它们没有路径);
//   ② `ERR` 载荷里**没有 `line` / `column`** —— 出错的是**路径**,不是文档里的
//      某个字节,填一个假的 `line: 1` 会让人以为「第一行出错了」。
// 形状错(路径不是数组 / 元素不是字符串或整数)仍是 `E0030` 诊断,与 W-02 同款。

fn json_shape_err(host: &mut dyn StdHost, msg: String) -> WlwlError {
    host.diag(ErrorCode::E0030, msg)
}

fn arg_count(host: &mut dyn StdHost, op: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{op}: function expects {want} argument(s), got {got}"),
    )
}

/// `JSON_GET(doc, path) -> v | NULL` —— 路径用 `ARRAY` 表达,缺失返 `NULL`。
pub fn json_get(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if args.len() != 2 {
        return Err(arg_count(host, "JSON_GET", args.len(), 2));
    }
    let segs = match path::parse_path(&args[1], "JSON_GET") {
        Ok(v) => v,
        Err(path::PathError::Shape(m)) => return Err(json_shape_err(host, m)),
        Err(e) => return Ok(json_err_from_path("JSON_GET", &e)),
    };
    Ok(match path::get(&args[0], &segs) {
        Some(v) => Outcome::normal(v.clone()),
        None => Outcome::normal(Value::Null),
    })
}

/// `JSON_SET(doc, path, value) -> doc'` —— 返回**新**文档,原文档不动。
pub fn json_set(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if args.len() != 3 {
        return Err(arg_count(host, "JSON_SET", args.len(), 3));
    }
    let segs = match path::parse_path(&args[1], "JSON_SET") {
        Ok(v) => v,
        Err(path::PathError::Shape(m)) => return Err(json_shape_err(host, m)),
        Err(e) => return Ok(json_err_from_path("JSON_SET", &e)),
    };
    match path::set(&args[0], &segs, &args[2]) {
        Ok(v) => Ok(Outcome::normal(v)),
        Err(path::PathError::Shape(m)) => Err(json_shape_err(host, m)),
        Err(e) => Ok(json_err_from_path("JSON_SET", &e)),
    }
}

/// `JSON_DELETE(doc, path) -> doc'` —— 路径不存在时**幂等**返原文档。
pub fn json_delete(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if args.len() != 2 {
        return Err(arg_count(host, "JSON_DELETE", args.len(), 2));
    }
    let segs = match path::parse_path(&args[1], "JSON_DELETE") {
        Ok(v) => v,
        Err(path::PathError::Shape(m)) => return Err(json_shape_err(host, m)),
        Err(e) => return Ok(json_err_from_path("JSON_DELETE", &e)),
    };
    match path::delete(&args[0], &segs) {
        Ok(v) => Ok(Outcome::normal(v)),
        Err(path::PathError::Shape(m)) => Err(json_shape_err(host, m)),
        Err(e) => Ok(json_err_from_path("JSON_DELETE", &e)),
    }
}

/// `JSON_MERGE(a, b) -> doc` —— **对象递归深合并;数组整体替换**(`ADR-0033` D3)。
pub fn json_merge(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if args.len() != 2 {
        return Err(arg_count(host, "JSON_MERGE", args.len(), 2));
    }
    Ok(Outcome::normal(path::merge(&args[0], &args[1])))
}

/// `JSON_CHUNK_ITEMS(doc, size?) -> ARRAY[ARRAY]` —— 顶层数组切块。
///
/// ⚠ **不是流式解析**(`ADR-0033` D1):`Value::Array` 是属主 `Vec`,真惰性在本语言里
///   做不到,每块都是一次切片拷贝。能保证的是**不重新解析** —— 块直接从已解析的
///   数组里切,不经过 `STRINGIFY` / `PARSE`。
pub fn json_chunk_items(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if args.is_empty() || args.len() > 2 {
        return Err(arg_count(host, "JSON_CHUNK_ITEMS", args.len(), 2));
    }
    let size = match args.get(1) {
        None => 1usize,
        Some(Value::Integer(n)) if *n >= 1 => *n as usize,
        Some(other) => {
            return Err(json_shape_err(
                host,
                format!(
                    "JSON_CHUNK_ITEMS: expected a positive INTEGER size, got {}",
                    type_name(other)
                ),
            ))
        }
    };
    match path::chunk_items(&args[0], size, "JSON_CHUNK_ITEMS") {
        Ok(v) => Ok(Outcome::normal(v)),
        Err(path::PathError::Shape(m)) => Err(json_shape_err(host, m)),
        Err(e) => Ok(json_err_from_path("JSON_CHUNK_ITEMS", &e)),
    }
}

/// 路径冲突的 `ERR` 载荷(带 `path`,**不带** `line`/`column` —— 见 `path.rs`)。
fn json_err_from_path(op: &str, e: &path::PathError) -> Outcome {
    Outcome::normal(path::path_err(op, e))
}

/// `JSON_CHECK(doc, schema) -> OK | ERR` —— schema **闭合子集**校验。
///
/// ⚠ **`doc` 收 STRING,不是已解析的值** —— 这是对 `addendum-09` §2.3 签名的一处
///   **有意偏离**,理由写在这里:
///
///   §2.3 的签名是 `JSON_CHECK(doc, schema)`(读起来 `doc` 是值),却把
///   `line` / `column` 列为「本份最有价值的部分:现在报错不带位置」。**这两条不能
///   同时成立** —— 同一个值可以由任何一段文本产生,值里**没有位置可指**。
///
///   ⇒ 收文本。解析走 W-02 的词法层(位置现成),schema 校验走 `schema.rs`。
///   调用方手上通常就是刚读进来的一整段响应,多一次 `PARSE` 是它本来就要付的。
///
/// ⚠ **未知 schema 关键字 → `E0030`**,不是静默忽略(`ADR-0033` D4 + 规范 §12.1
///   「半张表比没有更糟」)。消息的**形状照抄 D13-009 的 `parse_policy` 真实实现**:
///   把全部合法键一并列出来 —— 那条不在 ADR 里,是读代码才发现的。
pub fn json_check(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    if args.len() != 2 {
        return Err(arg_count(host, "JSON_CHECK", args.len(), 2));
    }
    let Value::String(text) = &args[0] else {
        return Err(json_shape_err(
            host,
            format!(
                "JSON_CHECK: expected a JSON STRING as the first argument, got {} \
                 (a parsed value has no position to report — see the member doc)",
                type_name(&args[0])
            ),
        ));
    };
    let sch = match schema::parse_schema(&args[1]) {
        Ok(s) => s,
        // schema 自身写错 ⇒ `E0030`(程序员的错,不是数据的错)。
        Err(msg) => return Err(host.diag(ErrorCode::E0030, format!("JSON_CHECK: {msg}"))),
    };
    let chars: Vec<char> = text.chars().collect();
    let toks = match lexer::tokenize(&chars) {
        Ok(t) => t,
        Err(e) => return Ok(json_err("JSON_CHECK", e)),
    };
    if let Err(e) = lexer::validate(&chars) {
        // 语法错先报语法错 —— 拿一份坏文本去校验 schema 没有意义,
        // 而且那时报出来的 schema 违规位置会是**假的**。
        return Ok(json_err("JSON_CHECK", e));
    }
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        // 走不到:`validate` 已经保证语法合法,而 serde 是本仓唯一的 JSON 实现。
        return Err(host.diag(
            ErrorCode::E0030,
            "JSON_CHECK: internal: lexer accepted what serde_json rejected".to_string(),
        ));
    };
    let doc = json_to_value(doc);
    // 位置表:把 token 流与值树**同步配对**,产出 `path -> 起始码点偏移`。
    //
    // ⚠ 第一版这里取的是 `value_offsets(..)[0]`,而那个函数先压内层再压容器,
    //   于是 `{"a":1}` 的「根」被报成 5(指向 `1`)。位置是给用户看的,指错地方
    //   比没有位置更糟。现在按文档序产出,根必然第一个。
    let index = schema::position_index(&doc, &toks);
    match schema::check(&doc, &sch, schema::ROOT_PATH) {
        Some(mut v) => {
            // 用违规自己的 `path` 回查它在文本里的位置。
            //
            // ⚠ **查不到就退回父级/文档开头**,不猜:一个 `required`
            //   缺失的违规在文本里**没有对应的值节点**(那个值本来就不存在),
            //   此时指向**u5bf9象自身**才是对的。
            let parent = match v.path.find('.') {
                Some(i) => format!("${}", &v.path[..i]),
                None => schema::ROOT_PATH.to_string(),
            };
            v.offset = index
                .iter()
                .find(|(p, _)| *p == v.path)
                .or_else(|| index.iter().find(|(p, _)| *p == parent))
                .map(|(_, o)| *o)
                .unwrap_or(0);
            Ok(Outcome::normal(schema::violation_err(
                "JSON_CHECK",
                &chars,
                &v,
            )))
        }
        None => Ok(Outcome::normal(Value::Ok(Box::new(Value::Null)))),
    }
}

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_parse(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, std_parse_inner, args)
}

/// [v0.11 M2 / ADR-0022] 直通边界包装:Value→内部表示→Value。
pub fn std_stringify(host: &mut dyn StdHost, args: Vec<Value>) -> Result<Outcome, WlwlError> {
    crate::wrap(host, std_stringify_inner, args)
}

// ── Value ↔ serde_json(仅作内部表示转换;ADR-0022 §4)──

pub(crate) fn values_to_json(
    args: &[Value],
) -> Result<Vec<serde_json::Value>, (wlwl_error::ErrorCode, String)> {
    let mut out = Vec::with_capacity(args.len());
    for a in args {
        out.push(value_to_json(a)?);
    }
    Ok(out)
}

pub(crate) fn value_to_json(
    v: &Value,
) -> Result<serde_json::Value, (wlwl_error::ErrorCode, String)> {
    let bad = |expected: &str, got: &str| {
        (
            wlwl_error::ErrorCode::E0030,
            format!("std argument: expected {expected}, got {got}"),
        )
    };
    Ok(match v {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .ok_or_else(|| bad("finite number", "NaN/Inf float"))?,
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(value_to_json(item)?);
            }
            serde_json::Value::Array(out)
        }
        Value::Dict(entries) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in entries {
                // [D11-019] 键只接受 `STRING`。整数键是被 M2 的边界转换
                // 顺手放开的(M2 的目标是「搬移」,不是「放宽」),会让
                // `FORMAT("{0}", ["a":1]` 之后 INDEX_SET 出来的整数键字典
                // 悄悄变成字符串键 `{"2": …}`;旧边界一律 `E0030`。
                let key = match k {
                    Value::String(s) => s.clone(),
                    other => return Err(bad("string dict key", type_name(other))),
                };
                obj.insert(key, value_to_json(val)?);
            }
            serde_json::Value::Object(obj)
        }
        // [D11-019] `OK(v)` 传**内层**,不是 `{"Ok": v}`。§12 的 `OK` 只是
        // 值的一层包装,跨界时拆掉(旧边界 `value_to_std_value` 就是这么写的,
        // M2 的「纯搬移」纪律要求逐字保持)。包成对象会让
        // `FORMAT("{0}", OK(1))` 从 `1` 变成 `Ok: 1`。
        Value::Ok(inner) => value_to_json(inner)?,
        // [D11-019] `ERR` 仍然拒绝(旧边界同)。语言层面它**到不了这里**:
        // §12.6 的 ERR 透明传播在调用边界就短路了(实测 `FORMAT("{0}",
        // ERR("e"))` 直接返回那个 ERR,成员体根本不执行),所以这一臂是
        // 给「本函数被直接调用」兜底的 —— 留着是为了不让它变成一个
        // 永远走不到、却把 ERR 变成 `{"Err": …}` 的地雷。
        Value::Err(_) => {
            return Err(bad("OK/primitives at std boundary", "ERR(...)"));
        }
        other => return Err(bad("json-representable value", type_name(other))),
    })
}

/// infallible:json 只可能来自 Value 的 round-trip。
pub(crate) fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(items) => {
            Value::Array(items.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => Value::Dict(
            obj.into_iter()
                .map(|(k, v)| (Value::String(k), json_to_value(v)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_object() {
        let mut ctx = StdCtx::default();
        let v = std_parse_inner(
            &mut ctx,
            vec![StdValue::String(r#"{"a": 1, "b": [2, 3]}"#.into())],
        )
        .unwrap();
        let expected: StdValue = serde_json::from_str(r#"{"a": 1, "b": [2, 3]}"#).unwrap();
        assert_eq!(v, expected);
    }

    #[test]
    fn parse_array() {
        let mut ctx = StdCtx::default();
        let v = std_parse_inner(&mut ctx, vec![StdValue::String("[1, 2, 3]".into())]).unwrap();
        assert_eq!(v, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn parse_invalid_is_e0070() {
        let mut ctx = StdCtx::default();
        let err =
            std_parse_inner(&mut ctx, vec![StdValue::String("{not json}".into())]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0070);
    }

    #[test]
    fn stringify_object() {
        let mut ctx = StdCtx::default();
        let v = std_stringify_inner(&mut ctx, vec![serde_json::json!({"x": 1, "y": "z"})]).unwrap();
        // serde_json::to_string produces compact form (no spaces).
        assert_eq!(v, StdValue::String(r#"{"x":1,"y":"z"}"#.into()));
    }

    #[test]
    fn stringify_array() {
        let mut ctx = StdCtx::default();
        let v = std_stringify_inner(&mut ctx, vec![serde_json::json!([1, 2, 3])]).unwrap();
        assert_eq!(v, StdValue::String("[1,2,3]".into()));
    }

    #[test]
    fn arity_mismatch_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_parse_inner(&mut ctx, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn spec_lists_all_eleven_members() {
        assert_eq!(SPEC.path, "wlwl:std.json");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        // ⚠ [v0.11.3 M7 / addendum-09] **2 → 5 → 10 → 11**。加成员时这条会红 ——
        //   那正是它的用途:名册是契约面,悄悄多一个/少一个成员必须有人喊。
        //   附录 A 镜像由 `gen-appendix-a` 同步,那边另有一道闸。
        //   (它已经红了三次:W-02 的 2→5、W-04 的 5→10、W-05 的 10→11 —— 三次都是
        //   **正当**变更,那正是它存在的理由。)
        assert_eq!(
            names,
            vec![
                "PARSE",
                "STRINGIFY",
                // W-02:低层词法层
                "JSON_TOKENIZE",
                "JSON_LOCATE",
                "JSON_VALIDATE",
                // W-04:路径族
                "JSON_GET",
                "JSON_SET",
                "JSON_DELETE",
                "JSON_MERGE",
                "JSON_CHUNK_ITEMS",
                // W-05:schema 闭合子集
                "JSON_CHECK",
            ]
        );
    }

    // ── [v0.11.3 M7 / addendum-09 W-02] 词法层与 serde_json 的接受集对齐 ──
    //
    // 这几条与 `tests/json_contract.rs` 的 76 条对账同族,但留在**内核层**是为了
    // 让它们不依赖解释器 —— 词法器是纯函数,单测就能断。契约那 76 条走公开面,
    // 两者互补:内核层证明词法器本身对,契约层证明它与 `PARSE` 对得上。

    #[test]
    fn lexer_matches_serde_on_the_three_stricter_points() {
        let accepts = |s: &str| lexer::validate(&s.chars().collect::<Vec<_>>()).is_ok();
        // 孤立代理 / 换序代理对:serde 拒(Python 收)⇒ 词法层也必须拒
        assert!(!accepts(r#""\uD800""#));
        assert!(!accepts(r#""\uDC00""#));
        assert!(!accepts(r#""\uDD1E\uD834""#));
        // 合法代理对必须收(这条曾经因为一个 `i += 4` / `i += 2` 的偏移写错而误判)
        assert!(accepts(r#""\uD834\uDD1E""#));
        // f64 溢出:serde 拒(Python 收 inf)⇒ 词法层也必须拒
        assert!(!accepts("1e400"));
        assert!(accepts("1e-400")); // 下溢到 0.0 是**合法**的
                                    // 深度上限 128
        assert!(accepts(&format!("{}{}", "[".repeat(30), "]".repeat(30))));
        assert!(!accepts(&format!("{}{}", "[".repeat(200), "]".repeat(200))));
    }

    /// `ADR-0033` D3 的两条 merge 语义,在**内核层**断一遍(不经解释器)。
    /// 契约那组另有一份走公开面的;两层互补:这里证明内核本身对,那里证明公开面
    /// 与内核一致。
    #[test]
    fn merge_is_recursive_on_objects_and_replaces_arrays() {
        use crate::json::path;
        // ⚠ 夹具走 `json_to_value`(serde 表示 → wlwl 值),**不能**直接
        //   `serde_json::from_str::<wlwl_value::Value>` —— 后者没有
        //   `Deserialize` 实现(编译报 `T: Deserialize` 未满足)。
        let v = |s: &str| {
            crate::json::json_to_value(serde_json::from_str::<serde_json::Value>(s).expect("json"))
        };
        // 对象深合并:`x` 不丢
        assert_eq!(
            path::merge(&v(r#"{"a":{"x":1}}"#), &v(r#"{"a":{"y":2}}"#)).display(),
            "[a: [x: 1, y: 2]]"
        );
        // 数组整体替换,不拼接
        assert_eq!(
            path::merge(&v(r#"{"a":[1,2]}"#), &v(r#"{"a":[9]}"#)).display(),
            "[a: [9]]"
        );
        // b 侧是标量 ⇒ b 赢
        assert_eq!(
            path::merge(&v(r#"{"a":{"x":1}}"#), &v(r#"{"a":5}"#)).display(),
            "[a: 5]"
        );
    }

    #[test]
    fn tokenize_is_purely_lexical_so_bracket_brace_pair_is_not_an_error() {
        // §2.1「纯词法,无结构」:`[}` 的括号与花括号都是合法 token ⇒ 词法层不报错。
        // 结构由 `validate` 管。这两条同时断,免得有人把两层的职责混掉。
        let chars: Vec<char> = "[}".chars().collect();
        assert_eq!(lexer::tokenize(&chars).unwrap().len(), 2);
        assert!(lexer::validate(&chars).is_err());
    }

    // ── [D11-019] 边界转换的语义锁 ──
    //
    // M2 把边界从 eval 侧的 `value_to_std_value` 搬进本文件时,三处
    // 可观察行为被顺手改了(报「纯搬移」,实际是搬移 + 放宽)。这三条
    // 逐条钉回 M2 之前的口径;`wrap` 由 io / fs / json / format / ai /
    // agent 六个模块共用,所以这里是唯一的收口点。

    #[test]
    fn boundary_unwraps_ok_to_its_inner_value() {
        let got = values_to_json(&[Value::Ok(Box::new(Value::Integer(1)))]).unwrap();
        assert_eq!(got, vec![serde_json::json!(1)]);
        // 嵌套也要拆(旧边界递归下去)。
        let nested = values_to_json(&[Value::Ok(Box::new(Value::Ok(Box::new(Value::String(
            "x".into(),
        )))))])
        .unwrap();
        assert_eq!(nested, vec![serde_json::json!("x")]);
    }

    #[test]
    fn boundary_rejects_err() {
        let err = values_to_json(&[Value::Err(Box::new(Value::String("e".into())))]).unwrap_err();
        assert_eq!(err.0, wlwl_error::ErrorCode::E0030);
    }

    #[test]
    fn boundary_rejects_integer_dict_keys() {
        let d = Value::Dict(vec![(Value::Integer(2), Value::String("v".into()))]);
        let err = values_to_json(&[d]).unwrap_err();
        assert_eq!(err.0, wlwl_error::ErrorCode::E0030);
        assert!(
            err.1.contains("string dict key"),
            "message should name the old expectation, got: {}",
            err.1
        );
    }

    #[test]
    fn boundary_accepts_string_dict_keys() {
        let d = Value::Dict(vec![(Value::String("a".into()), Value::Integer(1))]);
        assert_eq!(
            values_to_json(&[d]).unwrap(),
            vec![serde_json::json!({"a": 1})]
        );
    }

    /// ADR-0022 §4 明确保留的唯一一条跨模块契约:闭包跨界一律 `E0030`。
    /// 它与上面三条同属「边界不许悄悄放宽」这一族,故放在一起钉。
    #[test]
    fn boundary_still_rejects_closures() {
        let f = Value::Closure {
            params: vec![],
            body: Box::new(wlwl_ast::Expr::Literal(
                wlwl_ast::Literal::Null,
                wlwl_ast::Span::new("t.wll", 0, 0),
            )),
            env: wlwl_value::Env::new(),
        };
        let err = values_to_json(&[f]).unwrap_err();
        assert_eq!(err.0, wlwl_error::ErrorCode::E0030);
    }
}
