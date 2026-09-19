//! `wlwl:std.test` — **name catalog only** for the in-process test
//! framework defined in spec v0.4 §15.9.
//!
//! ## Why a catalog, not a normal std module
//!
//! Same architecture as `wlwl-std::collection` (see that module's
//! docs and B6 `P4-B6-001` for the full rationale): the std boundary
//! (`value_to_std_value` in `wlwl-eval`) rejects `Value::Closure` /
//! `Value::NativeFn` with **E0030**, which would break:
//!
//! - `TEST(name, body)` — `body` is a user closure that must be
//!   stashed in an evaluator-local registry and later invoked by
//!   `RUN_TESTS`.
//! - `RUN_TESTS()` — drains the registry and calls each test body
//!   via `Evaluator::invoke_closure`; the resulting `ERR` (if any)
//!   is caught per-test via `TRY`.
//!
//! `ASSERT` / `ASSERT_EQ` / `ASSERT_NEQ` / `EXPECT_ERR` could in
//! principle live in `wlwl-std` (no callback), but keeping all six
//! in one place matches the collection precedent and avoids the
//! "ASSERT works, TEST doesn't" surprise that splitting would invite.
//!
//! ## The 6 names
//!
//! Per spec §15.9 table:

use crate::ModuleSpec;

/// The 6 function names exposed by `wlwl:std.test`. Kept as a
/// `const` slice so the eval side can iterate the same canonical
/// list (eval matches on `path == "wlwl:std.test"` and walks
/// `BUILTINS`; `BUILTINS` must therefore enumerate every name in
/// `NAMES` in the same order — `names_match_builtins` locks it).
pub const NAMES: &[&str] = &[
    "TEST",
    "ASSERT",
    "ASSERT_EQ",
    "ASSERT_NEQ",
    "EXPECT_ERR",
    "RUN_TESTS",
];

/// `wlwl:std.test` — name catalog. `functions` is intentionally
/// empty: see the module-level docs for the why. The eval side
/// bypasses this slice for this specific path; for all other std
/// modules the slice is the binding source of truth.
pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.test",
    functions: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_path_is_wlwl_std_test() {
        assert_eq!(SPEC.path, "wlwl:std.test");
    }

    #[test]
    fn spec_functions_is_empty() {
        // The catalog shape: `wlwl_std::resolve` returns Some(&SPEC)
        // (so IMPORT passes), but the slice is empty because the eval
        // side handles the binding itself.
        assert!(
            SPEC.functions.is_empty(),
            "test SPEC must be a name catalog (functions: &[]); \
             got {} entries",
            SPEC.functions.len()
        );
    }

    #[test]
    fn names_count_is_six() {
        // Spec §15.9 table enumerates exactly 6 rows. Lock the count
        // so the eval-side BUILTINS table cannot silently grow/shrink
        // without a failing test (and a matching history/deviations
        // note).
        assert_eq!(NAMES.len(), 6, "NAMES count drifted from §15.9 (6)");
    }

    #[test]
    fn names_are_unique() {
        let mut sorted = NAMES.to_vec();
        sorted.sort();
        let original_len = sorted.len();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            original_len,
            "duplicate name in NAMES; check §15.9 enumeration"
        );
    }

    #[test]
    fn names_match_spec_v0_4_section_15_9() {
        // The exact set from spec §15.9 (Phase B7 source of truth).
        // If you add a name here, update plan §3 / deviations.md too.
        let expected: &[&str] = &[
            "TEST",
            "ASSERT",
            "ASSERT_EQ",
            "ASSERT_NEQ",
            "EXPECT_ERR",
            "RUN_TESTS",
        ];
        assert_eq!(NAMES, expected, "NAMES drifted from spec §15.9");
    }
}
