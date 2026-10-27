//! [W-21] wlwl 值 ↔ JSON 文本的**边界编解码**。
//!
//! ⚠ **只有数据值能过边界**：`integer` / `float` / `string` / `boolean` /
//! `null` / `array` / `dict`（以及它们的可变 cell 变体 —— 那些在**可观察面**
//! 上与普通值逐字相同）。函数、类、实例、句柄、`OK`/`ERR` **一律过不去**
//! ⇒ 报既有诊断码 `E0030`（类型错），不是新增码。
//!
//! ⚠ **字典键必须是字符串**：JSON 对象的键只能是字符串 ⇒ 非字符串键
//! （整数键 / 实例键）过不去 ⇒ `E0030`。
//!
//! 解码方向是**无损可判**的：JSON 对象 → `dict`、JSON 数组 → `array`、
//! 整数 → `integer`、其它数字 → `float`。

use serde_json::{Map, Number, Value as Json};
use wlwl_value::{type_name, DictInner, Value};

/// 值 → JSON 的失败（**只有一种**：这个值没有 JSON 对应物）。
#[derive(Debug, Clone, PartialEq)]
pub struct EncodeError {
    /// 值的类型名（`type_name` 口径，如 `"function"`）。
    pub type_name: &'static str,
    /// 失败原因（人话）。
    pub reason: String,
}

impl EncodeError {
    fn unsupported(v: &Value, reason: &str) -> EncodeError {
        EncodeError {
            type_name: type_name(v),
            reason: reason.to_string(),
        }
    }
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.reason, self.type_name)
    }
}

/// 一个值 → JSON。
pub fn value_to_json(v: &Value) -> Result<Json, EncodeError> {
    match v {
        Value::Integer(i) => Ok(Json::Number(Number::from(*i))),
        Value::Float(f) => {
            if !f.is_finite() {
                return Err(EncodeError::unsupported(
                    v,
                    "non-finite float has no JSON representation",
                ));
            }
            // ⚠ `Number::from_f64` 对 NaN/Inf 返回 `None`；上面已挡掉。
            Ok(Json::Number(
                Number::from_f64(*f).unwrap_or_else(|| Number::from(0)),
            ))
        }
        Value::String(s) => Ok(Json::String(s.clone())),
        Value::Boolean(b) => Ok(Json::Bool(*b)),
        Value::Null => Ok(Json::Null),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(value_to_json(item)?);
            }
            Ok(Json::Array(out))
        }
        // ⚠ cell 变体在可观察面上与普通数组/字典/字符串**逐字相同**
        // ⇒ 边界上不该出现第二种形态。
        Value::Cell(cell) => {
            let items = cell.borrow();
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                out.push(value_to_json(item)?);
            }
            Ok(Json::Array(out))
        }
        Value::Dict(inner) => dict_to_json(inner.entries()),
        Value::DictCell(cell) => dict_to_json(cell.inner_rc().entries()),
        Value::CellStr(cell) => Ok(Json::String(cell.as_str())),
        // ── 过不去的值 ────────────────────────────────────────────────
        Value::Ok(_) => Err(EncodeError::unsupported(
            v,
            "RESULT values cannot cross the extension boundary",
        )),
        Value::Err(_) => Err(EncodeError::unsupported(
            v,
            "RESULT values cannot cross the extension boundary",
        )),
        Value::Closure { .. } | Value::NativeFn { .. } => Err(EncodeError::unsupported(
            v,
            "function values cannot cross the extension boundary",
        )),
        Value::Class(_) | Value::Instance { .. } => Err(EncodeError::unsupported(
            v,
            "object values cannot cross the extension boundary",
        )),
        Value::TaskHandle(_) | Value::ChannelHandle(_) => Err(EncodeError::unsupported(
            v,
            "handle values cannot cross the extension boundary",
        )),
    }
}

/// 字典 → JSON 对象（键必须是字符串）。
fn dict_to_json(entries: &[(Value, Value)]) -> Result<Json, EncodeError> {
    let mut map = Map::new();
    for (k, v) in entries {
        let Value::String(key) = k else {
            return Err(EncodeError {
                type_name: type_name(k),
                reason: "dictionary keys must be strings to cross the extension boundary"
                    .to_string(),
            });
        };
        map.insert(key.clone(), value_to_json(v)?);
    }
    Ok(Json::Object(map))
}

/// JSON → wlwl 值。
pub fn json_to_value(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Boolean(*b),
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(u) = n.as_u64() {
                // u64 超过 i64::MAX ⇒ 只能是 float。
                if u <= i64::MAX as u64 {
                    Value::Integer(u as i64)
                } else {
                    Value::Float(u as f64)
                }
            } else {
                Value::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        Json::String(s) => Value::String(s.clone()),
        Json::Array(items) => Value::Array(items.iter().map(json_to_value).collect()),
        Json::Object(map) => {
            let entries: Vec<(Value, Value)> = map
                .iter()
                .map(|(k, v)| (Value::String(k.clone()), json_to_value(v)))
                .collect();
            Value::Dict(std::rc::Rc::new(DictInner::new(entries)))
        }
    }
}

/// 实参列表 → JSON 数组（`args_json` 的载荷）。
pub fn args_to_json(args: &[Value]) -> Result<Json, EncodeError> {
    let mut out = Vec::with_capacity(args.len());
    for a in args {
        out.push(value_to_json(a)?);
    }
    Ok(Json::Array(out))
}
