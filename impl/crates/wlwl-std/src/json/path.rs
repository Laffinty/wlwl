//! [v0.11.3 M7 / addendum-09 W-04] `std.json` 的路径族。
//!
//! `JSON_GET` / `JSON_SET` / `JSON_DELETE` / `JSON_MERGE` / `JSON_CHUNK_ITEMS`。
//!
//! ## ⚠ 头号事实:路径遍历是 **O(每层键数)**
//!
//! ⚠ **[ADR-0035] 字典的表示已经变了**:`Value::Dict` 现在是
//! `Rc<DictInner>`(插入序条目 + **键 → 位置**的哈希索引)⇒ 字典**自己**的
//! 键查找是 O(1) 期望(规范 §18.3.1)。
//!
//! 但**本模块**的路径下潜仍是自己写的线性扫描(`.iter().find(...)`),
//! 没走那张索引 ⇒ 「每层扫一遍」这个成本剖面**对本模块仍然成立**。
//! 实测(W-04 只读调查,bench 档同机):
//!
//! | 键数 | 单次最坏查找 | 相对 250 键 |
//! |---|---|---|
//! |   250 |  15.9 µs | 1.0× |
//! |  1000 | 108.3 µs | 6.8× |
//! |  4000 | 276.9 µs | 17.4× |
//!
//! ⇒ **查找成本随 n 线性增长**,深路径 `["a","b","c"]` 是「每层扫一遍」。
//! (把本模块的扫描换成 `DictInner::lookup` 是一条独立的后续项 ——
//! 语义等价,但不在 ADR-0035 的 S3「机械替换」范围内。)
//!
//! ## 本模块的两条设计决定(都是被那个事实逼出来的)
//!
//! 1. **路径直接在已解析的 `Value` 树上走,绝不「重新 `STRINGIFY` 再 `PARSE`」**。
//!    后者每层要付一次完整的序列化 + 解析,常数大两三个数量级,10 KB 文档直接
//!    掉出 `ADR-0033` 引用的 `JSON_GET ≥ 100 MB/s` 目标。
//! 2. **`JSON_SET` / `JSON_DELETE` 只克隆一次**。wlwl 的 `Value` 是**深拷贝的
//!    属主树**(`Value::clone()` 是深拷贝),所以「返回新文档」这个不可变语义
//!    **必然**要付一次整份文档的拷贝 —— 这是语言的值语义,不是本批引入的。
//!    能做的是**只拷一次**,不是「不拷」。
//!
//! ⚠ **不要**把 `Value::Dict` 的**键序**改成哈希序:那是 breaking(文档序、
//!    `display()`、序列化产物全要跟着变)。`ADR-0025` Q1 第 1 条否掉的是这条路;
//!    `ADR-0035` 走的是另一条 —— **内部**加哈希索引、**外部**仍按插入序呈现
//!    (规范 §18.3.1 明写这条路合法)。

use std::rc::Rc;

use wlwl_value::Value;

/// 路径的一段:字符串 = 对象键,整数 = 数组下标(负数从尾部计数,与 `SLICE` 同口径)。
///
/// ⚠ `pub(crate)`:`json.rs` 要拿它去构造 `Vec<Seg>` 再传给本模块的函数,私有类型
///   会让父模块也用不了(报 `private type`)。
#[derive(Clone)]
pub(crate) enum Seg {
    Key(String),
    Idx(i64),
}

/// 把用户给的 `ARRAY` 路径解成 `Vec<Seg>`。
///
/// ⚠ **元素只接受 `STRING` 与 `INTEGER`**;别的(`FLOAT` / 布尔 / `NULL` / 嵌套容器)
///   一律 `E0030` —— 路径是**寻址**,不是数据,拿 `1.5` 当下标没有解释。
pub fn parse_path(path: &Value, op: &str) -> Result<Vec<Seg>, PathError> {
    let Value::Array(items) = path else {
        return Err(PathError::Shape(format!(
            "{op}: path must be an ARRAY, got {}",
            wlwl_value::type_name(path)
        )));
    };
    let mut out = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        out.push(match it {
            Value::String(s) => Seg::Key(s.clone()),
            Value::Integer(n) => Seg::Idx(*n),
            other => {
                return Err(PathError::Shape(format!(
                    "{op}: path[{i}] must be STRING or INTEGER, got {}",
                    wlwl_value::type_name(other)
                )))
            }
        });
    }
    Ok(out)
}

/// 形状错(路径本身写错)⇒ `E0030`;数据层错(打到一个冲突的节点上)⇒ `ERR` 值。
/// 两者的分界见 `ADR-0033` D5:调用方式错报诊断,数据不合预期返 `ERR`。
pub enum PathError {
    /// 元数 / 类型错 ⇒ `E0030` 诊断。
    Shape(String),
    /// 寻址冲突 ⇒ `ERR([kind: "JsonError", ...])`,带 `path`。
    Conflict { path: Vec<Seg>, reason: String },
}

impl PathError {
    pub fn message(&self) -> &str {
        match self {
            PathError::Shape(m) => m,
            PathError::Conflict { reason, .. } => reason,
        }
    }
}

fn resolve_idx(len: usize, i: i64) -> Option<usize> {
    let n = if i < 0 { len as i64 + i } else { i };
    if n < 0 || n >= len as i64 {
        None
    } else {
        Some(n as usize)
    }
}

/// `JSON_GET` —— 缺失返 `NULL`,**不**报错(§2.2 明写)。
pub fn get<'a>(doc: &'a Value, path: &[Seg]) -> Option<&'a Value> {
    let mut cur = doc;
    for seg in path {
        cur = match (cur, seg) {
            (Value::Dict(entries), Seg::Key(k)) => entries
                .iter()
                .find(|(ek, _)| matches!(ek, Value::String(s) if s == k))
                .map(|(_, v)| v)?,
            (Value::Array(items), Seg::Idx(i)) => items.get(resolve_idx(items.len(), *i)?)?,
            // 形状不匹配 = 没找到 ⇒ 走「缺失返 NULL」,不报错。
            _ => return None,
        };
    }
    Some(cur)
}

/// `JSON_SET` —— 返回**新**文档(不可变语义)。缺��的中间容器会**就地建出来**。
///
/// ⚠ 「缺键就建」而不是报 `ERR`:路径取值的常用形态就是 `SET(d, ["cfg","timeout"], x)`
///   在一棵还不完整的文档上逐层搭结构;为它报错等于逼调用方自己先拼骨架。
///   真正报 `ERR` 的是**类型冲突**(路径穿过一个标量或类型不符的容器)。
pub fn set(doc: &Value, path: &[Seg], new_value: &Value) -> Result<Value, PathError> {
    let mut out = doc.clone(); // 不可变语义:只克隆一次
    if path.is_empty() {
        return Ok(new_value.clone());
    }
    let mut cur = &mut out;
    for depth in 0..path.len() - 1 {
        let seg = &path[depth];
        // ⚠ 必须把**下一段**一起传下去:缺失的中间容器要按下一段的种类建
        //   (下一段是字符串键 ⇒ 建字典、是下标 ⇒ 建数组)。第一版只传了当前段,
        //   于是先塞了个 `NULL` 占位,第二轮下潜撞进「穿过标量」的冲突分支 ——
        //   `JSON_SET({"a":1}, ["b","c"], 9)` 直接返 ERR 而不是把 `b.c` 建出来,
        //   而那正是 `SET` 的常用形态。
        let next = &path[depth + 1];
        cur = descend_mut(cur, seg, next, &path[..=depth])?;
    }
    let last = &path[path.len() - 1];
    // ⚠ 类型名先取出来:下面要拿 `cur` 的**可变**借用(CoW),届时不能再借它。
    let cur_ty = wlwl_value::type_name(cur);
    // ⚠ [ADR-0035] 字典的写入走 **copy-on-write**:`as_dict_inner_mut` 内部是
    //   `Rc::make_mut` —— 唯一拥有时原地改,被别名时先克隆再改 ⇒
    //   `doc`(`out` 的来源)**不会**被就地改掉(不可变语义)。
    if let Some(inner) = cur.as_dict_inner_mut() {
        let Seg::Key(k) = last else {
            return Err(PathError::Conflict {
                path: path.to_vec(),
                reason: format!(
                    "cannot SET through a {} at path {}",
                    cur_ty,
                    render_path(&path[..path.len() - 1])
                ),
            });
        };
        match inner.lookup(&Value::String(k.clone())) {
            Some(at) => inner.set_value(at, new_value.clone()),
            None => inner.insert(Value::String(k.clone()), new_value.clone()),
        }
        return Ok(out);
    }
    match cur {
        Value::Array(items) => match last {
            Seg::Idx(i) => match resolve_idx(items.len(), *i) {
                Some(at) => {
                    items[at] = new_value.clone();
                    Ok(out)
                }
                None => {
                    // 数组下标越界 ⇒ **追加**。比报错有用的取舍:`SET` 拿不到元素下标
                    // 就没法先问「你要覆盖还是追加」,而调用方手上多半只有一个路径。
                    items.push(new_value.clone());
                    Ok(out)
                }
            },
            Seg::Key(_) => Err(PathError::Conflict {
                path: path.to_vec(),
                reason: format!(
                    "cannot SET through a {} at path {}",
                    cur_ty,
                    render_path(&path[..path.len() - 1])
                ),
            }),
        },
        _ => Err(PathError::Conflict {
            path: path.to_vec(),
            reason: format!(
                "cannot SET through a {} at path {}",
                cur_ty,
                render_path(&path[..path.len() - 1])
            ),
        }),
    }
}

/// 沿一段路径下潜,**必要时建出缺失的容器**(数组建数组、字符串键建字典)。
fn descend_mut<'a>(
    cur: &'a mut Value,
    seg: &Seg,
    next: &Seg,
    walked: &[Seg],
) -> Result<&'a mut Value, PathError> {
    // ⚠ 类型名先取出来:下面要拿 `cur` 的**可变**借用(CoW),届时不能再借它。
    let cur_ty = wlwl_value::type_name(cur);
    // ⚠ 先用**共享**借用判种类(立刻结束),再按种类分支 —— 否则「一条路径上
    //   持着 `cur` 的可变借用、另一条路径又要用 `cur`」会撞 E0503/E0499。
    let is_dict = matches!(&*cur, Value::Dict(_));
    if is_dict {
        // ⚠ [ADR-0035] 字典分支走 **copy-on-write**(`Rc::make_mut`):被别名时
        //   先克隆再改 ⇒ 源文档不被就地改掉(不可变语义)。
        let Value::Dict(rc) = cur else {
            unreachable!("kind checked above")
        };
        let Seg::Key(k) = seg else {
            return Err(PathError::Conflict {
                path: walked.to_vec(),
                reason: format!(
                    "cannot descend through a {} at path {}",
                    cur_ty,
                    render_path(walked)
                ),
            });
        };
        let inner = Rc::make_mut(rc);
        let at = match inner.lookup(&Value::String(k.clone())) {
            Some(at) => at,
            None => {
                // 按**下一段**的种类建容器,不是一律 `NULL` —— 见调用处的注释。
                let fresh = match next {
                    Seg::Key(_) => Value::dict(Vec::new()),
                    Seg::Idx(_) => Value::Array(Vec::new()),
                };
                inner.insert(Value::String(k.clone()), fresh);
                inner.len() - 1
            }
        };
        return Ok(inner.value_mut(at));
    }
    match cur {
        Value::Array(items) => match seg {
            Seg::Idx(i) => match resolve_idx(items.len(), *i) {
                Some(at) => Ok(&mut items[at]),
                None => {
                    let fresh = match next {
                        Seg::Key(_) => Value::dict(Vec::new()),
                        Seg::Idx(_) => Value::Array(Vec::new()),
                    };
                    items.push(fresh);
                    Ok(items.last_mut().expect("just pushed"))
                }
            },
            Seg::Key(_) => Err(PathError::Conflict {
                path: walked.to_vec(),
                reason: format!(
                    "cannot descend through a {} at path {}",
                    cur_ty,
                    render_path(walked)
                ),
            }),
        },
        _ => Err(PathError::Conflict {
            path: walked.to_vec(),
            reason: format!(
                "cannot descend through a {} at path {}",
                cur_ty,
                render_path(walked)
            ),
        }),
    }
}

/// `JSON_DELETE` —— 删**整段**(对象键 / 数组元素),不存在返**原文档**。
pub fn delete(doc: &Value, path: &[Seg]) -> Result<Value, PathError> {
    if path.is_empty() {
        // 删掉整份文档没有表示,只能退化成空数组 —— 与「删除键」的直觉差得远,
        // 所以按**冲突**报错,而不是静默给一个用户没要求的东西。
        return Err(PathError::Conflict {
            path: vec![],
            reason: "cannot DELETE the whole document; give a non-empty path".to_string(),
        });
    }
    let mut out = doc.clone();
    // ⚠ 第一版在这里先做了一次**不可变**的 `get_parent` 当存在性检查,再在克隆体上
    //   做可变的 `get_parent_mut` —— 那个 `parent` 绑定了却从头没用过,纯浪费一趟
    //   扫描(clippy 的 `unused variable` 报出来)。删掉它行为不变:`get_parent_mut`
    //   返回 `None` 时直接交回克隆体,就是「路径不存在 ⇒ 幂等返原文档」。
    let Some(parent) = get_parent_mut(&mut out, path) else {
        return Ok(out);
    };
    let last = &path[path.len() - 1];
    match (parent, last) {
        // ⚠ [ADR-0035] 字典分支走 **copy-on-write**(`Rc::make_mut`):被别名时
        //   先克隆再改 ⇒ 源文档不被就地改掉(不可变语义)。
        (Value::Dict(rc), Seg::Key(k)) => {
            let inner = Rc::make_mut(rc);
            inner.retain(|(ek, _)| !matches!(ek, Value::String(s) if s == k));
        }
        (Value::Array(items), Seg::Idx(i)) => {
            if let Some(at) = resolve_idx(items.len(), *i) {
                items.remove(at);
            }
        }
        _ => {}
    }
    Ok(out)
}

fn get_parent_mut<'a>(doc: &'a mut Value, path: &[Seg]) -> Option<&'a mut Value> {
    let mut cur = doc;
    for seg in &path[..path.len() - 1] {
        cur = match (cur, seg) {
            // ⚠ [ADR-0035] 字典分支:CoW 后按下标取**值**的可变引用。
            //   ⚠ 这里必须**解构**出 `rc`(`&mut Rc<DictInner>`)而不是走
            //   `Value` 上的访问器:解构会**移动** `&mut Value`,于是内部的借用
            //   能活到与原 `&mut Value` 同样的寿命 ⇒ 这里才敢返回指向**嵌套值**
            //   的 `&mut`(下面 `cur = …` 的再借用链要求这一点)。
            (Value::Dict(rc), Seg::Key(k)) => {
                let inner = Rc::make_mut(rc);
                let at = inner.lookup(&Value::String(k.clone()))?;
                inner.value_mut(at)
            }
            // ⚠ 长度必须**先**取出来再 `get_mut`:写成 `items.get_mut(resolve_idx(items.len(), ..)?)`
            //   会在同一个表达式里同时持有可变与不可变借用(E0502)。
            (Value::Array(items), Seg::Idx(i)) => {
                let len = items.len();
                items.get_mut(resolve_idx(len, *i)?)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

/// `JSON_MERGE(a, b)` —— **对象递归深合并;数组整体替换**(`ADR-0033` D3)。
///
/// ⚠ 逐字写死的两条,含糊的 merge 是配置事故的常见源:
///   * 对象深合并 ⇒ `{"a":{"x":1}}` 合 `{"a":{"y":2}}` 得 `{"a":{"x":1,"y":2}}`,
///     **不是**顶层替换(那样会把 `x` 悄悄丢掉)。
///   * 数组**整体替换** ⇒ `{"a":[1]}` 合 `{"a":[2,3]}` 得 `{"a":[2,3]}`,
///     **不拼接**(拼接不可逆,且与「merge 是覆盖语义」的直觉相反)。
///   * `b` 侧是标量而 `a` 侧是容器时,**`b` 赢** —— 覆盖语义的一致延伸。
pub fn merge(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Dict(_), Value::Dict(_)) => {
            // ⚠ [ADR-0035] 合并结果必须是**新**字典 ⇒ `a.clone()` 只是**共享**
            //   了 `Rc`,真正动手前先 CoW(`as_dict_inner_mut` = `Rc::make_mut`)
            //   ⇒ `a` 本身不被就地改掉(不可变语义)。
            let mut out_value = a.clone();
            let out = out_value.as_dict_inner_mut().expect("matched a DICT above");
            for (bk, bv) in b.as_dict_entries().unwrap_or(&[]) {
                // ⚠ 键的比较口径**逐字保留**原实现:`==`(§2.4 的 `PartialEq`),
                //   不是 `values_equal`(后者多一条「整数 ↔ 浮点跨类型相等」)。
                match out.entries().iter().position(|(ak, _)| ak == bk) {
                    Some(at) => {
                        let merged = merge(&out.entries()[at].1, bv);
                        out.set_value(at, merged);
                    }
                    None => out.insert(bk.clone(), bv.clone()),
                }
            }
            out_value
        }
        _ => b.clone(),
    }
}

/// `JSON_CHUNK_ITEMS(doc, size?)` —— 把顶层数组**切块**,每块一个数组。
///
/// ⚠ 这是**消费层的惰性替代**,不是流式解析(`ADR-0033` D1:本批仍不做流式解析)。
///   `Value::Array` 是属主 `Vec`,所以「惰性」在本语言里**做不到真惰性** ——
///   每块都是一次切片拷贝,总量仍是 O(n)。能保证的是**不重新解析**:
///   块直接从已解析的数组里切,不经过 `STRINGIFY` / `PARSE`。
pub fn chunk_items(doc: &Value, size: usize, op: &str) -> Result<Value, PathError> {
    let Value::Array(items) = doc else {
        return Err(PathError::Shape(format!(
            "{op}: expected a top-level ARRAY, got {}",
            wlwl_value::type_name(doc)
        )));
    };
    if size == 0 {
        return Err(PathError::Shape(format!("{op}: size must be >= 1")));
    }
    Ok(Value::Array(
        items
            .chunks(size)
            .map(|c| Value::Array(c.to_vec()))
            .collect(),
    ))
}

/// 把 `Vec<Seg>` 渲染成 `a[0].b` 形态,给 `ERR` 载荷的 `path` 用。
pub fn render_path(path: &[Seg]) -> String {
    let mut s = String::new();
    for seg in path {
        match seg {
            Seg::Key(k) => {
                if s.is_empty() {
                    s.push_str(k);
                } else {
                    s.push('.');
                    s.push_str(k);
                }
            }
            Seg::Idx(i) => {
                s.push('[');
                s.push_str(&i.to_string());
                s.push(']');
            }
        }
    }
    if s.is_empty() {
        "$".to_string()
    } else {
        s
    }
}

/// `ERR` 载荷。路径族的 `path` **有值**(与 W-02 那三个成员相反,见 D7.3)。
///
/// ⚠ `line` / `column` **恒缺省**:W-02 的词法层才有行列,而这里的错误是**寻址**
///   冲突 —— 出错的是**路径**,不是文档里的某个字节。填一个假的 `line: 1` 会让人
///   以为「第一行出错了」,而那不是位置信息,是**假信息**。
///
/// ⚠ 这里直接构造 `wlwl_value::Value`,**不**走 `crate::compat::StdValue` ——
///   后者是 serde_json 那一侧的旧边界类型,拿它拼出来的 `Err`/`Dict` 根本不是
///   wlwl 的值。第一版就是这么写的,编译报「`serde_json::Value` 没有 `Err` 变体」。
pub fn path_err(op: &str, e: &PathError) -> Value {
    let path = match e {
        PathError::Conflict { path, .. } => render_path(path),
        PathError::Shape(_) => "$".to_string(),
    };
    Value::Err(Box::new(Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("JsonError".into()),
        ),
        (Value::String("op".into()), Value::String(op.into())),
        (Value::String("path".into()), Value::String(path)),
        (
            Value::String("reason".into()),
            Value::String(e.message().into()),
        ),
    ])))
}
