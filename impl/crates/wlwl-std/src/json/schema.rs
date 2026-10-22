//! [v0.11.3 M7 / addendum-09 W-05] `JSON_CHECK` —— schema **闭合子集**校验。
//!
//! ## 照抄的先例:D13-009(`HTML_SANITIZE` 的 policy)
//!
//! 未知关键字的处理**不是**我按 ADR-0033 D4 转述出来的,而是读了
//! `sanitize/filter.rs::parse_policy` 的真实实现。它的形状有三个细节值得照抄:
//!
//! 1. 一个 `perr` 闭包统一加成员名前缀、**一律 `E0030`**;
//! 2. 未知键的消息**把全部合法键一并列出来**:
//!    `unknown policy key "X" — known keys: tags, attributes, url_schemes, strip_comments`
//!    —— 这比只说「未知键」有用得多,调用方不用去翻文档;
//! 3. 措辞是**小写 + 冒号**:`policy "tags" must be an ARRAY`。
//!
//! ⚠ 第 2 条**不在** ADR-0033 D4 里 —— D4 只说了「未知键 → `E0030`」。只读 ADR
//!   就会漏掉它,而它恰好是这套错误消息里最有用的一行。
//!
//! ## 🔴 D13-021 / N-013:递归必须有闸(`MAX_DEPTH` 与词法层同一口径)
//!
//! 本模块有两处**无界递归**:`parse_schema` 顺着 `properties` / `items` 往下,
//! `check` 顺着 `s.properties` / `s.items` 往下。schema 的第二个实参收的是**任意
//! wlwl 值**,而 wlwl 可以用循环**迭代**拼出 10 万层嵌套的 DICT —— 迭代构造不
//! 占调用栈,递归消费它却每层都占 ⇒ 栈溢出 ⇒ `abort`(不是诊断,是进程死)。
//! ⇒ 两处都按 `lexer::MAX_DEPTH`(128)**同一口径**加闸,**不另立数字**:
//! 另立一个数会让 `JSON_CHECK` 与 `JSON_VALIDATE` 对「同一份深层文档」的接受集
//! 分叉 —— 那正是本仓 M7 W-02 头号要防的事(`lexer.rs` 模块头那张差分表)。

use crate::json::lexer::{self, Kind, Token};
use wlwl_value::Value;

/// 嵌套深度上限。**不另立数字** —— 直接用词法层的 `lexer::MAX_DEPTH`,理由见
/// 模块头。`JSON_VALIDATE` 拒第 129 层,`JSON_CHECK` 也必须拒第 129 层。
const MAX_DEPTH: usize = lexer::MAX_DEPTH;

/// 超深时的那句话。⚠ 与 `lexer.rs` 的 `nesting deeper than {MAX_DEPTH}` 同一句式,
/// 只多一个 `schema` 前缀 —— 调用方要能一眼分出**是哪一侧**超深(schema 还是文档)。
fn too_deep() -> SchemaErr {
    format!("schema nesting deeper than {MAX_DEPTH}")
}

/// schema 支持的关键字(闭合子集)。**不在此表的键一律 `E0030`**
/// —— 半张表比没有更糟(规范 §12.1),静默忽略会让调用方以为约束生效了。
const KNOWN: [&str; 9] = [
    "type",
    "required",
    "properties",
    "items",
    "enum",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
];

const TYPES: [&str; 7] = [
    "object", "array", "string", "number", "integer", "boolean", "null",
];

/// 根路径。⚠ `JSON_CHECK` 内部**只用这一个**字面量 —— 它同时出现在
/// `check` 的初值、`position_index` 的初值与回查兜底里,三处必须一致。
/// 之前它们各自写 `String::from("$")` / `"$"`,改一处忘另两处就会让
/// 「根上的违规查不到自己的位置」而静默退回 0。
pub const ROOT_PATH: &str = "$";

/// schema 自身写错 ⇒ `E0030`(程序员的错,不是数据的错)。
pub type SchemaErr = String;

/// 数据不合 schema ⇒ `ERR` 值,带 `path` 与 `line` / `column`。
#[derive(Debug, Clone)]
pub struct Violation {
    pub path: String,
    pub offset: usize,
    pub reason: String,
}

/// 解析 schema(只做形状与关键字检查;值域在 `check` 里判)。
///
/// ⚠ 这是**不带深度的入口** —— 真正的入口在 `parse_schema_at`,它拿 `level = 1`
///   把「根就是第 1 层」这件事钉死。留着这个薄壳是因为调用方(`json.rs::json_check`)
///   不该、也不需要知道深度这件事 —— **闸是本模块自己的不变量,不是调用方的义务**。
pub fn parse_schema(schema: &Value) -> Result<Schema, SchemaErr> {
    parse_schema_at(schema, 1)
}

/// 递归体。`level` 是**本层是第几层 schema**,根为 `1` —— 与 `lexer::validate` 里
/// 「进一个 `{` / `[` 就 `*depth += 1`、再比 `MAX_DEPTH`」的口径**逐条对齐**,
/// 这样「128 层收、第 129 层拒」在两侧是同一个数。
fn parse_schema_at(schema: &Value, level: usize) -> Result<Schema, SchemaErr> {
    // 🔴 **D13-021**:闸放在函数入口、递归**之前**。放在递归之后等于没放 ——
    //   溢出的那一刻已经晚了。
    if level > MAX_DEPTH {
        return Err(too_deep());
    }
    let mut s = Schema::default();
    let Value::Dict(entries) = schema else {
        return Err(format!(
            "schema must be a DICT, got {}",
            wlwl_value::type_name(schema)
        ));
    };
    for (k, v) in entries.iter() {
        let Value::String(key) = k else {
            return Err("schema keys must be STRING".into());
        };
        match key.as_str() {
            // ⚠ `type` **可写多个**(规范 stdlib §3.5:「可写多个(任一匹配即过)」)——
            //   单值写 `STRING`,多值写 `ARRAY[STRING]`,两种写法**完全等价**。
            //   `Schema.ty` 本就是 `Vec`(下面逐个 `push`),**数据结构一直能存多个**,
            //   缺的只是入口 —— 这条分支原来只收 `Value::String`,于是按规范书写多类型
            //   schema 的用户被硬错误拦下(`schema "type" must be a STRING`)。
            "type" => match v {
                Value::String(t) => {
                    if !TYPES.contains(&t.as_str()) {
                        return Err(format!(
                            "schema \"type\" unknown: \"{t}\" — known types: {}",
                            TYPES.join(", ")
                        ));
                    }
                    s.ty.push(t.clone());
                }
                Value::Array(items) => {
                    // ⚠ 数组里**任一**元素不合法 ⇒ **整条 schema 拒**(不留半条 schema):
                    //   半条 schema 会让调用方以为约束生效了,比直接报错更糟
                    //   (与 §12.1「半张表比没有更糟」同款理由)。
                    for it in items {
                        let Value::String(t) = it else {
                            return Err(
                                "schema \"type\" must be a STRING or an ARRAY of STRING".into()
                            );
                        };
                        if !TYPES.contains(&t.as_str()) {
                            return Err(format!(
                                "schema \"type\" unknown: \"{t}\" — known types: {}",
                                TYPES.join(", ")
                            ));
                        }
                        s.ty.push(t.clone());
                    }
                }
                // ⚠ 数字 / 布尔 / 嵌套数组等**其它形态** —— 形状错就是程序员的错
                //   ⇒ `E0030`(不是 `ERR` 值)。
                _ => return Err("schema \"type\" must be a STRING or an ARRAY of STRING".into()),
            },
            "required" => {
                let Value::Array(items) = v else {
                    return Err("schema \"required\" must be an ARRAY".into());
                };
                for it in items {
                    let Value::String(name) = it else {
                        return Err("schema \"required\" must hold STRING elements".into());
                    };
                    s.required.push(name.clone());
                }
            }
            "properties" => {
                let Value::Dict(inner) = v else {
                    return Err("schema \"properties\" must be a DICT".into());
                };
                for (pk, pv) in inner.iter() {
                    let Value::String(name) = pk else {
                        return Err("schema \"properties\" keys must be STRING".into());
                    };
                    // ⚠ `level + 1`:schema 的「一层」= 一个 DICT。`properties` 的值
                    //   与 `items` 的值都必须是 DICT(上面两行已经各自查过),
                    //   所以**只有 DICT 会继续递归**,层数与 lexer 数容器同口径。
                    s.properties
                        .push((name.clone(), parse_schema_at(pv, level + 1)?));
                }
            }
            "items" => s.items = Some(Box::new(parse_schema_at(v, level + 1)?)),
            "enum" => {
                let Value::Array(items) = v else {
                    return Err("schema \"enum\" must be an ARRAY".into());
                };
                s.enum_vals = items.clone();
                s.has_enum = true;
            }
            "minimum" => {
                s.minimum = Some(as_f64(v, "minimum")?);
                s.has_min = true;
            }
            "maximum" => {
                s.maximum = Some(as_f64(v, "maximum")?);
                s.has_max = true;
            }
            "minLength" => {
                s.min_len = Some(as_len(v, "minLength")?);
                s.has_min_len = true;
            }
            "maxLength" => {
                s.max_len = Some(as_len(v, "maxLength")?);
                s.has_max_len = true;
            }
            other => {
                return Err(format!(
                    "unknown schema key \"{other}\" — known keys: {}",
                    KNOWN.join(", ")
                ))
            }
        }
    }
    Ok(s)
}

fn as_f64(v: &Value, key: &str) -> Result<f64, SchemaErr> {
    match v {
        Value::Integer(i) => Ok(*i as f64),
        Value::Float(f) if f.is_finite() => Ok(*f),
        other => Err(format!(
            "schema \"{key}\" must be a finite number, got {}",
            wlwl_value::type_name(other)
        )),
    }
}

fn as_len(v: &Value, key: &str) -> Result<usize, SchemaErr> {
    match v {
        Value::Integer(i) if *i >= 0 => Ok(*i as usize),
        other => Err(format!(
            "schema \"{key}\" must be a non-negative INTEGER, got {}",
            wlwl_value::type_name(other)
        )),
    }
}

#[derive(Default)]
pub struct Schema {
    /// 允许的 `type`;空 = 不约束。
    ty: Vec<String>,
    required: Vec<String>,
    properties: Vec<(String, Schema)>,
    items: Option<Box<Schema>>,
    enum_vals: Vec<Value>,
    has_enum: bool,
    minimum: Option<f64>,
    maximum: Option<f64>,
    min_len: Option<usize>,
    max_len: Option<usize>,
    has_min: bool,
    has_max: bool,
    has_min_len: bool,
    has_max_len: bool,
}

/// `kind_of` —— wlwl 的两个数值种类在 schema 里按 JSON Schema 的两支区分。
fn matches_type(v: &Value, t: &str) -> bool {
    match t {
        "object" => matches!(v, Value::Dict(_)),
        "array" => matches!(v, Value::Array(_) | Value::Cell(_)),
        "string" => matches!(v, Value::String(_)),
        "number" => matches!(v, Value::Integer(_) | Value::Float(_)),
        // ⚠ `integer`:wlwl 的 `FLOAT` 若是 `2.0` 也算整数吗?—— **不算**。
        //   `PARSE("2.0")` 给 FLOAT(实测,冻结在 `number_kind_mapping_is_frozen`),
        //   而 JSON Schema 的 `integer` 指「没有小数部分的值」。两条路选一条并写明:
        //   这里取**严**的一支 —— 只有 `INTEGER` 算 `integer`。
        //   理由:wlwl 的类型系统**记得**原来是不是整数(`INTEGER` / `FLOAT` 是两个
        //   种类),若在这里把它们抹平,`JSON_CHECK` 就丢掉了本仓比 Python 多出来的
        //   那份信息,且用户无法从 `TYPE()` 的结果预判校验结果。
        //   ⚠ 这是一条**有意分歧**,与 `STDDEV_S([7])` 那条同款 —— 契约里显式钉住。
        "integer" => matches!(v, Value::Integer(_)),
        "boolean" => matches!(v, Value::Boolean(_)),
        "null" => matches!(v, Value::Null),
        _ => false,
    }
}

/// 逐条校验;返回**第一条**违规(规范只要「带位置的一条」,不要一堆)。
///
/// ⚠ `offset` **不在这里产出**:`check` 只知道值与 schema,不知道它在文本里的位置。
///   位置由 `json_check` 按违规的 `path` 去 `position_index` 里回查 —— 单一出处,
///   免得「递归里顺手传一个 offset」传错层级。
///
/// ⚠ 这是**不带深度的入口**,理由同 `parse_schema`:闸是本模块自己的不变量。
pub fn check(v: &Value, s: &Schema, prefix: &str) -> Option<Violation> {
    check_at(v, s, prefix, 1)
}

/// 递归体,`level` 语义同 `parse_schema_at`(根为 `1`)。
///
/// ⚠ **D13-021:这一道闸与 `parse_schema_at` 那道不是重复,是两道不同的边界。**
///   `parse_schema_at` 守的是**schema 侧**(第二个实参,wlwl 程序可迭代构造);
///   这里守的是**值与 schema 联合下潜的那条链** —— 两者深度只要有一侧失控,
///   递归就会溢。
///   ⚠ 今天从 `json_check` 走进来时,值侧已经被 `lexer::validate` 的同一口径
///   `MAX_DEPTH` 限住了(schema 侧又被 `parse_schema_at` 限住),所以这道闸**当前
///   不可达**。留着它是**为了让「不溢出」成为本模块自己的不变量**:这道函数是
///   `pub` 的,下一个调用方(内置 schema、从别处反序列化出来的 schema)完全可能
///   不经过 `parse_schema`。
///   ⚠ 超深在**这里**只能以 `Violation` 出现(本函数返回 `Option`,升不了诊断),
///   与 `parse_schema_at` 那条 `E0030` 不同形 —— 但**正常的调用路径**上报出去的是
///   `parse_schema_at` 那条:它先跑,先拒。
fn check_at(v: &Value, s: &Schema, prefix: &str, level: usize) -> Option<Violation> {
    let bad = |reason: String| {
        Some(Violation {
            path: prefix.to_string(),
            offset: 0, // 由调用方按 `path` 回查(见 `json_check`)
            reason,
        })
    };
    if level > MAX_DEPTH {
        return bad(too_deep());
    }
    // `type` 写在前面:类型都不对时,报别的约束只会让人更困惑。
    if !s.ty.is_empty() && !s.ty.iter().any(|t| matches_type(v, t)) {
        return bad(format!(
            "expected type {} but got {}",
            s.ty.iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(" or "),
            wlwl_value::type_name(v)
        ));
    }
    if s.has_enum && !s.enum_vals.iter().any(|e| e == v) {
        return bad(format!("value not in enum ({} allowed)", s.enum_vals.len()));
    }
    if let (true, Some(min)) = (s.has_min, s.minimum) {
        if let Some(x) = numeric(v) {
            if x < min {
                return bad(format!("{x} < minimum {min}"));
            }
        }
    }
    if let (true, Some(max)) = (s.has_max, s.maximum) {
        if let Some(x) = numeric(v) {
            if x > max {
                return bad(format!("{x} > maximum {max}"));
            }
        }
    }
    if let (true, Some(n)) = (s.has_min_len, s.min_len) {
        if let Value::String(x) = v {
            let len = x.chars().count();
            if len < n {
                return bad(format!("length {len} < minLength {n}"));
            }
        }
    }
    if let (true, Some(n)) = (s.has_max_len, s.max_len) {
        if let Value::String(x) = v {
            let len = x.chars().count();
            if len > n {
                return bad(format!("length {len} > maxLength {n}"));
            }
        }
    }
    match v {
        Value::Dict(entries) => {
            for name in &s.required {
                if !entries
                    .iter()
                    .any(|(k, _)| matches!(k, Value::String(x) if x == name))
                {
                    return Some(Violation {
                        path: join(prefix, name),
                        offset: 0,
                        reason: format!("required property \"{name}\" is missing"),
                    });
                }
            }
            for (k, sub) in &s.properties {
                if let Some((_, val)) = entries
                    .iter()
                    .find(|(ek, _)| matches!(ek, Value::String(x) if x == k))
                {
                    // ⚠ **前缀传下去**,不是让子节点回溯改父路径。
                    //   第一版让每层返回后由父层改写 `vi.path`,而数组那层是
                    //   `format!("[{i}]{}", ...)` —— **覆盖**了整条路径,于是
                    //   嵌套违规的 `path` 变成 `a.b.[0]`(多一个点、少了前缀语义)。
                    //   这种 bug 只有逐条看输出才发现:断言全绿、路径是错的。
                    if let Some(mut vi) = check_at(val, sub, &join(prefix, k), level + 1) {
                        vi.offset = 0;
                        return Some(vi);
                    }
                }
            }
            None
        }
        Value::Array(_) | Value::Cell(_) => {
            let items: Vec<Value> = match v {
                Value::Array(a) => a.clone(),
                // `Cell` 是运行期临时表示,`JSON_CHECK` 收刚解析的文本,不可能出现。
                Value::Cell(c) => c.borrow().clone(),
                _ => unreachable!(),
            };
            if let Some(sub) = &s.items {
                for (i, it) in items.iter().enumerate() {
                    if let Some(mut vi) = check_at(it, sub, &format!("{prefix}[{i}]"), level + 1) {
                        vi.offset = 0;
                        return Some(vi);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// `minimum` / `maximum` 比对时把值取成 `f64`。
///
/// ⚠ 非有限值返 `None` 而不是 `Some(inf)` —— 否则边界比较会拿 `inf` 去做,
///   得到一个「通过了」或「没通过」但语义含混的结论。`NaN` / `±inf` 在 JSON 里
///   本就不该出现(`JSON_VALIDATE` 已拒),这里只是不给它们一条偷跑的旁路。
fn numeric(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) if f.is_finite() => Some(*f),
        _ => None,
    }
}

/// 走**值树与 token 流**同步配对,产出 `path -> 起始码点偏移`。
///
/// ⚠ 为什么需要它:`addendum-09` §2.3 把 `line` / `column` 列为 `JSON_CHECK` 的
///   主要产出,但那要求输入是**文本**;若 `JSON_CHECK` 收的是已解析的**值**,
///   位置就**无从计算** —— 同一个值可以由任何一段文本产生。⇒ `JSON_CHECK` 收
///   **STRING**(见 `json.rs`),这张表负责把文本位置接回每个值节点。
///
/// ⚠ **第一版写错了,而且错得看不出**:它先把**内层**节点压进表、最后才压容器,
///   于是 `offs[0]` 是**内层第一个值**而不是根 —— `{"a":1}` 的根偏移被报成 5
///   (指向 `1`)。位置是**给用户看的**,报错的偏移会让人指着一个无关的地方,
///   比没有位置更糟。本版按**文档序**产出,根必然第一个,嵌套节点紧随其父之后。
///
/// JSON 文法无歧义 ⇒ 值树与 token 流可以逐节点对齐。对不齐就返回已收集的部分
/// (调用方据此退化成「位置指向文档开头」),**不猜**。
///
/// ⚠ **这里没有深度闸,与上面两个不是同一回事**:`doc` 一定是 `lexer::validate`
///   放行过的**文本**解析出来的,而那个闸已经把嵌套限在 `MAX_DEPTH` 以内 ⇒ 这条
///   递归**天然有界**。schema 那边不是:它的输入是 wlwl 程序可以迭代拼出来的值。
pub fn position_index(doc: &Value, toks: &[Token]) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut cur = 0usize;
    walk(doc, toks, &mut cur, ROOT_PATH, &mut out);
    out
}

fn walk(v: &Value, toks: &[Token], cur: &mut usize, path: &str, out: &mut Vec<(String, usize)>) {
    let start = toks.get(*cur).map(|t| t.start).unwrap_or(0);
    out.push((path.to_string(), start));
    match v {
        Value::Dict(entries) => {
            *cur += 1; // `{`
            for (k, val) in entries.iter() {
                *cur += 1; // 键的 string token
                *cur += 1; // `:`
                let child = match k {
                    Value::String(name) => join(path, name),
                    _ => path.to_string(),
                };
                walk(val, toks, cur, &child, out);
                // 下一个 token 应当是 `,` 或 `}`;跳过它,让下一轮对齐。
                if matches!(toks.get(*cur).map(|t| t.kind), Some(Kind::Comma)) {
                    *cur += 1;
                }
            }
            if matches!(toks.get(*cur).map(|t| t.kind), Some(Kind::RBrace)) {
                *cur += 1;
            }
        }
        Value::Array(items) => {
            *cur += 1; // `[`
            for (i, it) in items.iter().enumerate() {
                let child = format!("{path}[{i}]");
                walk(it, toks, cur, &child, out);
                if matches!(toks.get(*cur).map(|t| t.kind), Some(Kind::Comma)) {
                    *cur += 1;
                }
            }
            if matches!(toks.get(*cur).map(|t| t.kind), Some(Kind::RBracket)) {
                *cur += 1;
            }
        }
        Value::Cell(c) => {
            // `Cell` 是运行期临时表示,`JSON_CHECK` 收的是刚解析的文本,不可能出现。
            // 真出现了就按数组走一遍,不为它编造位置。
            let items = c.borrow();
            *cur += 1;
            for (i, it) in items.iter().enumerate() {
                let child = format!("{path}[{i}]");
                walk(it, toks, cur, &child, out);
                if matches!(toks.get(*cur).map(|t| t.kind), Some(Kind::Comma)) {
                    *cur += 1;
                }
            }
        }
        _ => {
            // 标量恰好一个 token。字符串的 `text` 已被 lexer 验证过。
            *cur += 1;
        }
    }
}

fn join(path: &str, key: &str) -> String {
    if path == ROOT_PATH {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// `JSON_CHECK` 用的 `ERR` 载荷。⚠ 这里**有** `line` / `column` —— 与 W-02 的
/// 词法错误、W-04 的寻址冲突都不同,三者的差别记在 `ADR-0033`。
pub fn violation_err(op: &str, text: &[char], v: &Violation) -> Value {
    let (line, column) = lexer::line_col(text, v.offset);
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("JsonError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("path".into()), Value::String(v.path.clone())),
        (
            Value::String("offset".into()),
            Value::Integer(v.offset as i64),
        ),
        (Value::String("line".into()), Value::Integer(line as i64)),
        (
            Value::String("column".into()),
            Value::Integer(column as i64),
        ),
        (
            Value::String("reason".into()),
            Value::String(v.reason.clone()),
        ),
    ])))
}
