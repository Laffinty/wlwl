//! [v0.9 Step 9b / plan §4.4.3 / ADR-0019 §14] Session-typed method
//! protocols (顺序 + ⊕ 选择 + μ 递归骨架).
//!
//! Value encoding (parser already accepts these as ordinary expressions;
//! no new surface syntax in v0.9.0):
//!
//! | Form | Meaning |
//! |------|---------|
//! | `"end"` | terminal (`Proto::End`) |
//! | `["var", "X"]` | μ recursion variable |
//! | `["mu", "X", body]` | recursion binder `μX. body` |
//! | `["choice", {m1: c1, m2: c2}]` / `["choice", [[m1, c1], …]]` | internal choice `⊕` |
//! | `["then", "m", next]` | call `m`, then `next` |
//! | `["recv", "int", next]` | call with `?int` payload type, then `next` |
//! | `{m1: v1, m2: v2, …}` | **sequence** `m1 → m2 → …` (plan `{a: T1, b: T2, c: end}`) |
//!
//! Dict values in a sequence are payload-type names (`"int"`, `"end"`, …)
//! or nested protocol forms used as the continuation after that method.
//!
//! v0.9.0 scope: 顺序 + ⊕ + μ. External selection `&`, named protocols,
//! and `par` are deferred (plan §4.4.3).

use crate::Value;

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
    /// Protocol completed (`end` reached). Further `CALL_METHOD` → E0051.
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

/// Parse a protocol `Value` into a `Proto`.
///
/// Returns `Err(ProtocolError::Syntax)` on malformed input (caller maps
/// to E0050 per ADR-0019).
pub fn parse_proto(v: &Value) -> Result<Proto, ProtocolError> {
    parse(v, &mut Vec::new())
}

fn parse(v: &Value, binders: &mut Vec<String>) -> Result<Proto, ProtocolError> {
    match v {
        Value::String(s) if s == "end" => Ok(Proto::End),
        Value::String(s) if binders.iter().any(|b| b == s) => Ok(Proto::Var(s.clone())),
        Value::String(s) => Err(ProtocolError::Syntax(format!(
            "unknown protocol atom '{}' (expected 'end', a μ variable, or a compound form)",
            s
        ))),
        Value::Array(items) if !items.is_empty() => {
            let tag = match &items[0] {
                Value::String(t) => t.as_str(),
                other => {
                    return Err(ProtocolError::Syntax(format!(
                        "protocol array tag must be STRING, got {}",
                        crate::type_name(other)
                    )))
                }
            };
            match tag {
                "end" if items.len() == 1 => Ok(Proto::End),
                "var" if items.len() == 2 => match &items[1] {
                    Value::String(s) if binders.iter().any(|b| b == s) => Ok(Proto::Var(s.clone())),
                    Value::String(s) => Err(ProtocolError::Syntax(format!(
                        "unbound protocol variable '{}'",
                        s
                    ))),
                    other => Err(ProtocolError::Syntax(format!(
                        "protocol variable name must be STRING, got {}",
                        crate::type_name(other)
                    ))),
                },
                "mu" if items.len() == 3 => match &items[1] {
                    Value::String(name) => {
                        binders.push(name.clone());
                        let body = parse(&items[2], binders)?;
                        binders.pop();
                        Ok(Proto::Mu(name.clone(), Box::new(body)))
                    }
                    other => Err(ProtocolError::Syntax(format!(
                        "μ binder must be STRING, got {}",
                        crate::type_name(other)
                    ))),
                },
                "choice" if items.len() == 2 => parse_choice(&items[1], binders),
                "then" | "seq" if items.len() == 3 => {
                    // ["then", "m", next] or ["then", ["recv", "int", …] style]
                    let method = match &items[1] {
                        Value::String(s) => s.clone(),
                        other => {
                            return Err(ProtocolError::Syntax(format!(
                                "then/seq method must be STRING, got {}",
                                crate::type_name(other)
                            )))
                        }
                    };
                    let next = parse(&items[2], binders)?;
                    Ok(Proto::Then {
                        method,
                        payload: None,
                        next: Box::new(next),
                    })
                }
                "recv" if items.len() == 3 => {
                    // ["recv", "int", next] ≡ Then { payload: Some("int"), next }
                    // is only meaningful as a continuation after a method;
                    // as a standalone form we treat the method as "" (anon).
                    let payload = match &items[1] {
                        Value::String(s) => s.clone(),
                        other => {
                            return Err(ProtocolError::Syntax(format!(
                                "recv payload type must be STRING, got {}",
                                crate::type_name(other)
                            )))
                        }
                    };
                    let next = parse(&items[2], binders)?;
                    Ok(Proto::Then {
                        method: String::new(),
                        payload: Some(payload),
                        next: Box::new(next),
                    })
                }
                "seq" if items.len() >= 2 => {
                    // ["seq", "a", "b", "c"] — flat sequence of method names.
                    let mut acc = Proto::End;
                    for item in items[1..].iter().rev() {
                        let method = match item {
                            Value::String(s) if s != "end" => s.clone(),
                            Value::String(_) => continue,
                            other => {
                                return Err(ProtocolError::Syntax(format!(
                                    "seq method must be STRING, got {}",
                                    crate::type_name(other)
                                )))
                            }
                        };
                        acc = Proto::Then {
                            method,
                            payload: None,
                            next: Box::new(acc),
                        };
                    }
                    Ok(acc)
                }
                other => Err(ProtocolError::Syntax(format!(
                    "unknown protocol form '{}'",
                    other
                ))),
            }
        }
        Value::Dict(entries) => parse_dict_seq(entries, binders),
        other => Err(ProtocolError::Syntax(format!(
            "protocol must be STRING / ARRAY / DICT, got {}",
            crate::type_name(other)
        ))),
    }
}

fn parse_choice(v: &Value, binders: &mut Vec<String>) -> Result<Proto, ProtocolError> {
    let pairs: Vec<(String, &Value)> = match v {
        Value::Dict(entries) => entries
            .iter()
            .map(|(k, val)| match k {
                Value::String(s) => Ok((s.clone(), val)),
                other => Err(ProtocolError::Syntax(format!(
                    "choice label must be STRING, got {}",
                    crate::type_name(other)
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?,
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Array(pair) if pair.len() == 2 => match &pair[0] {
                    Value::String(s) => Ok((s.clone(), &pair[1])),
                    other => Err(ProtocolError::Syntax(format!(
                        "choice label must be STRING, got {}",
                        crate::type_name(other)
                    ))),
                },
                other => Err(ProtocolError::Syntax(format!(
                    "choice entry must be [STRING, proto], got {}",
                    crate::type_name(other)
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?,
        other => {
            return Err(ProtocolError::Syntax(format!(
                "choice body must be DICT or ARRAY, got {}",
                crate::type_name(other)
            )))
        }
    };
    if pairs.is_empty() {
        return Err(ProtocolError::Syntax("choice must offer ≥ 1 method".into()));
    }
    let mut arms = Vec::with_capacity(pairs.len());
    for (m, cont) in pairs {
        arms.push((m, Box::new(parse(cont, binders)?)));
    }
    Ok(Proto::Choice(arms))
}

/// Plan §4.4.3 `{a: T1, b: T2, c: end}` — ordered sequence of methods.
///
/// Each key is a method that becomes available after the previous one
/// completes. Values are payload-type names or nested continuations.
fn parse_dict_seq(
    entries: &[(Value, Value)],
    binders: &mut Vec<String>,
) -> Result<Proto, ProtocolError> {
    // Nested continuation on a value replaces "rest of dict".
    // Build from the tail so `Then` nests correctly.
    let mut named: Vec<(String, &Value)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        match k {
            Value::String(s) => named.push((s.clone(), v)),
            other => {
                return Err(ProtocolError::Syntax(format!(
                    "sequence method name must be STRING, got {}",
                    crate::type_name(other)
                )))
            }
        }
    }
    if named.is_empty() {
        return Ok(Proto::End);
    }
    // If any value is a compound protocol form, it is the continuation
    // *after* that method (remaining keys are ignored — use `then` for
    // explicit chaining). Otherwise values are payload types and keys
    // form a flat sequence.
    let has_compound = named.iter().any(|(_, v)| !matches!(v, Value::String(_)));
    if has_compound {
        // Chain: method_i → (value_i as continuation if compound,
        // else rest of dict).
        let mut acc = Proto::End;
        for (m, v) in named.iter().rev() {
            let next = if matches!(v, Value::String(_)) {
                acc
            } else {
                parse(v, binders)?
            };
            let payload = match v {
                Value::String(s) if s != "end" => Some(s.clone()),
                _ => None,
            };
            acc = Proto::Then {
                method: m.clone(),
                payload,
                next: Box::new(next),
            };
        }
        Ok(acc)
    } else {
        // Flat sequence with optional payload-type annotations.
        let mut acc = Proto::End;
        for (m, v) in named.iter().rev() {
            let payload = match v {
                Value::String(s) if s == "end" => None,
                Value::String(s) => Some(s.clone()),
                _ => None,
            };
            acc = Proto::Then {
                method: m.clone(),
                payload,
                next: Box::new(acc),
            };
        }
        Ok(acc)
    }
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

fn offered_of(p: &Proto) -> Vec<String> {
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

/// Step the protocol. Returns `(next_proto_or_None_if_done, remaining_fuel)`.
fn step_proto(p: &Proto, method: &str, fuel: u32) -> Result<(Option<Proto>, u32), ProtocolError> {
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

/// Capture-avoiding-enough substitution: replace `Var(name)` with `repl`.
fn subst(p: &Proto, name: &str, repl: &Proto) -> Proto {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Value {
        Value::String(x.into())
    }

    fn arr(items: Vec<Value>) -> Value {
        Value::Array(items)
    }

    fn dict(entries: Vec<(&str, Value)>) -> Value {
        Value::Dict(
            entries
                .into_iter()
                .map(|(k, v)| (Value::String(k.into()), v))
                .collect(),
        )
    }

    #[test]
    fn parse_end() {
        assert_eq!(parse_proto(&s("end")).unwrap(), Proto::End);
    }

    #[test]
    fn parse_dict_sequence() {
        let v = dict(vec![("a", s("int")), ("b", s("int")), ("c", s("end"))]);
        let p = parse_proto(&v).unwrap();
        let mut cur = ProtocolCursor::live(p);
        cur.step("a").unwrap();
        cur.step("b").unwrap();
        cur.step("c").unwrap();
        assert_eq!(cur, ProtocolCursor::Done);
    }

    #[test]
    fn sequence_rejects_out_of_order() {
        let v = dict(vec![("a", s("end")), ("b", s("end"))]);
        let p = parse_proto(&v).unwrap();
        let mut cur = ProtocolCursor::live(p);
        let err = cur.step("b").unwrap_err();
        assert!(matches!(err, ProtocolError::NotOffered { .. }));
    }

    #[test]
    fn choice_selects_one_arm() {
        let v = arr(vec![
            s("choice"),
            dict(vec![("open", s("end")), ("close", s("end"))]),
        ]);
        let p = parse_proto(&v).unwrap();
        let mut cur = ProtocolCursor::live(p.clone());
        cur.step("open").unwrap();
        assert_eq!(cur, ProtocolCursor::Done);

        let mut cur = ProtocolCursor::live(p);
        cur.step("close").unwrap();
        assert_eq!(cur, ProtocolCursor::Done);
    }

    #[test]
    fn choice_rejects_unoffered() {
        let v = arr(vec![
            s("choice"),
            dict(vec![("open", s("end")), ("close", s("end"))]),
        ]);
        let p = parse_proto(&v).unwrap();
        let mut cur = ProtocolCursor::live(p);
        let err = cur.step("other").unwrap_err();
        match err {
            ProtocolError::NotOffered { expected, .. } => {
                assert_eq!(expected, vec!["open".to_string(), "close".to_string()]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn mu_loops_via_var() {
        // μX. { get: ?int.X, close: end }  ≈
        // ["mu","X",["choice",[["get",["recv","int",["var","X"]]],["close","end"]]]]
        let v = arr(vec![
            s("mu"),
            s("X"),
            arr(vec![
                s("choice"),
                dict(vec![
                    (
                        "get",
                        arr(vec![s("recv"), s("int"), arr(vec![s("var"), s("X")])]),
                    ),
                    ("close", s("end")),
                ]),
            ]),
        ]);
        let p = parse_proto(&v).unwrap();
        let mut cur = ProtocolCursor::live(p);
        for _ in 0..5 {
            cur.step("get").unwrap();
        }
        cur.step("close").unwrap();
        assert_eq!(cur, ProtocolCursor::Done);
    }

    #[test]
    fn mu_fuel_exhaustion() {
        // Infinite get-loop with no close: fuel must trip.
        let v = arr(vec![
            s("mu"),
            s("X"),
            arr(vec![s("then"), s("get"), arr(vec![s("var"), s("X")])]),
        ]);
        let p = parse_proto(&v).unwrap();
        let mut cur = ProtocolCursor::live(p);
        let mut last = Ok(());
        for _ in 0..(DEFAULT_FUEL + 5) {
            last = cur.step("get");
            if last.is_err() {
                break;
            }
        }
        assert!(matches!(
            last,
            Err(ProtocolError::RecursionExhausted { .. })
        ));
    }

    #[test]
    fn then_get_before_next_inc() {
        // inc → get → X   (plan Counter skeleton)
        // μX. { inc: then get → X, close: end }
        let v = arr(vec![
            s("mu"),
            s("X"),
            arr(vec![
                s("choice"),
                dict(vec![
                    (
                        "inc",
                        arr(vec![s("then"), s("get"), arr(vec![s("var"), s("X")])]),
                    ),
                    ("close", s("end")),
                ]),
            ]),
        ]);
        let p = parse_proto(&v).unwrap();
        let mut cur = ProtocolCursor::live(p);
        cur.step("inc").unwrap();
        // After inc, only get is offered — not another inc.
        let err = cur.step("inc").unwrap_err();
        assert!(matches!(err, ProtocolError::NotOffered { .. }));
        cur.step("get").unwrap();
        cur.step("close").unwrap();
    }

    #[test]
    fn syntax_error_on_bad_tag() {
        let v = arr(vec![s("nope"), s("x")]);
        assert!(matches!(parse_proto(&v), Err(ProtocolError::Syntax(_))));
    }
}
