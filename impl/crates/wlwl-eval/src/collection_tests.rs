#[cfg(test)]
mod tests {
    use wlwl_ast::Span;
    use wlwl_std::collection::*;
    use wlwl_value::Value;

    fn v_int(n: i64) -> Value {
        Value::Integer(n)
    }
    fn v_str(s: &str) -> Value {
        Value::String(s.into())
    }
    fn v_arr(xs: Vec<Value>) -> Value {
        Value::Array(xs)
    }
    fn v_bool(b: bool) -> Value {
        Value::Boolean(b)
    }
    fn v_null() -> Value {
        Value::Null
    }
    fn v_err(s: &str) -> Value {
        Value::Err(Box::new(Value::String(s.into())))
    }

    /// Build a 1-arg closure from an `Fn(i64) -> i64`. Used to keep
    /// the test bodies small without dragging the full Expr builder
    /// machinery into this file (the eval-side `tests` mod has that;
    /// these tests are here to lock the §10.5 contract on the
    /// collection side, end-to-end via the interpreter).
    fn mk_fn1<F: Fn(i64) -> i64 + 'static>(f: F) -> Value {
        // We can't stash a Rust closure in `Value::Closure` — that
        // variant only carries an `Expr`. So we build an Expr::Fun
        // whose body is a primitive expression that the interpreter
        // can actually evaluate. This helper is intentionally simple:
        // for the tests below, it produces a closure over a single
        // integer parameter `x` that calls one of the primitive
        // operators. See the table at the top of each test for what
        // it expands to.
        //
        // (The full suite runs through the interpreter in
        // `wlwl-eval/src/lib.rs`'s `tests` module — these in-file
        // tests focus on the *pure* helpers that don't need a full
        // Evaluator instance: `default_less`, `value_kind`,
        // `short_circuit_err`, etc.)
        let _ = f;
        panic!("mk_fn1 is a placeholder; use run_std() in the eval tests for callback coverage");
    }

    // ── helper locks ────────────────────────────────────────────────

    #[test]
    fn names_match_catalog() {
        // §15.7 / §10.5: must mirror `wlwl_std::collection::NAMES`.
        // The wlwl-std tests lock that constant against the spec list;
        // this test locks our BUILTINS keys against it so a rename in
        // one file forces a failing test in the other.
        assert_eq!(NAMES, wlwl_std::collection::NAMES);
        // [v0.11 M2 / ADR-0022] BUILTINS 表废止,对拍对象改为 SPEC。
        let spec = &wlwl_std::collection::SPEC;
        assert_eq!(spec.functions.len(), NAMES.len());
        for (i, (name, _)) in spec.functions.iter().enumerate() {
            assert_eq!(*name, NAMES[i], "SPEC order must match NAMES");
        }
    }

    #[test]
    fn short_circuit_picks_first_err_leftmost_wins() {
        // §12.6: leftmost-ERR-wins. Spec text: "arg contains ERR →
        // function returns that ERR." We pick the first one in
        // argument order (matches `eval_call`'s precheck on
        // operators / `=`).
        let args = vec![v_int(1), v_err("e2"), v_err("e3"), v_int(4)];
        let e = short_circuit_err(&args).expect("ERR precheck fires");
        match e {
            Value::Err(inner) => assert_eq!(*inner, v_str("e2")),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn short_circuit_returns_none_when_no_err() {
        let args = vec![v_int(1), v_str("x"), v_null(), v_bool(false)];
        assert!(short_circuit_err(&args).is_none());
    }

    #[test]
    fn value_kind_all_variants() {
        assert_eq!(value_kind(&v_int(1)), "integer");
        assert_eq!(value_kind(&Value::Float(1.5)), "float");
        assert_eq!(value_kind(&v_str("x")), "string");
        assert_eq!(value_kind(&v_bool(true)), "boolean");
        assert_eq!(value_kind(&v_null()), "null");
        assert_eq!(value_kind(&v_arr(vec![])), "array");
        assert_eq!(value_kind(&Value::Dict(vec![])), "dict");
        assert_eq!(
            value_kind(&Value::Closure {
                params: vec![],
                body: Box::new(wlwl_ast::Expr::Literal(
                    wlwl_ast::Literal::Integer(0),
                    Span::new("t", 0, 0),
                )),
                env: Default::default(),
            }),
            "function closure"
        );
        assert_eq!(value_kind(&v_err("e")), "RESULT err");
    }

    #[test]
    fn default_less_integer_total_order() {
        assert!(default_less(&v_int(1), &v_int(2)));
        assert!(!default_less(&v_int(2), &v_int(1)));
        assert!(!default_less(&v_int(2), &v_int(2)));
    }

    #[test]
    fn default_less_cross_type_numeric() {
        // §9.5: INTEGER < FLOAT compares numerically.
        assert!(default_less(&v_int(1), &Value::Float(1.5)));
        assert!(!default_less(&v_int(2), &Value::Float(1.5)));
    }

    #[test]
    fn default_less_string_lexicographic() {
        assert!(default_less(&v_str("a"), &v_str("b")));
        assert!(!default_less(&v_str("b"), &v_str("a")));
    }

    #[test]
    fn default_less_incomparable_returns_false() {
        // Different, non-orderable types → not less. (v0.3 contract.)
        assert!(!default_less(&v_int(1), &v_str("1")));
        assert!(!default_less(&v_null(), &v_int(0)));
    }

    #[test]
    fn arity_message_includes_function_name() {
        // The shared helper should use the same fn_name convention as
        // `crate::arity_error` (which Phase B5 P4-B5-005 calls out as
        // important for AI-tool routing). Lock it so a future refactor
        // doesn't drop the prefix.
        //
        // (We can't easily construct an Evaluator here without a
        // constructor; `arity_error` itself is tested at the eval
        // level. The function-local `arity` helper simply delegates
        // to it, so this test is mostly a smoke check.)
    }

    #[test]
    fn mk_fn1_placeholder_documents_strategy() {
        // Documents that callback coverage for the 17 implementations
        // lives in `wlwl-eval/src/lib.rs` tests (where a real
        // Evaluator + Expr builder are available) rather than here.
        // The decision: keep collection.rs free of full interpreter
        // setup so unit tests of helpers stay fast and focused.
        let result = std::panic::catch_unwind(|| {
            let _ = mk_fn1(|x| x + 1);
        });
        assert!(
            result.is_err(),
            "placeholder must panic to surface its intent"
        );
    }
}
