//! `wlwl:std.collection` — **name catalog only** for the 17 higher-order
//! collection functions defined in spec v0.4 §15.7 / §10.5.
//!
//! ## Why a catalog, not a normal std module
//!
//! Every other std module in this crate (`std.io`, `std.fs`, `std.format`,
//! …) follows the same shape: a `pub static SPEC: ModuleSpec` whose
//! `functions` slice points at `StdFn` shims that operate on
//! `serde_json::Value` at the std boundary. The reason std modules use
//! `serde_json::Value` is to keep `wlwl-std` free of the `wlwl-eval`
//! dependency (it would be a cycle — `wlwl-eval` imports this crate for
//! `IMPORT("wlwl:std.X", …)`).
//!
//! The catch: `value_to_std_value` in `wlwl-eval` rejects `Value::Closure`
//! and `Value::NativeFn` with **E0030** before the std shim ever runs
//! (this is the contract that locked B5 P4-B5-006 in place:
//! "`FORMAT("{0}", FUN((x), x))` through `wlwl:std.format` is E0030").
//!
//! Higher-order collection functions like `MAP` / `FILTER` / `REDUCE` /
//! `SORT` / `SORT_BY` / `ANY` / `ALL` / `FIND` / `GROUP_BY` take **the
//! user's callback closure** as a mandatory positional argument, so the
//! existing std boundary would reject them at the conversion step —
//! breaking the spec's own worked example in §15.7:
//!
//! ```wlwl
//! IMPORT("wlwl:std.collection", ["MAP", "FILTER", "REDUCE", "SORT"]);
//! LET(squared, MAP([1, 2, 3], FUN((x), *(x, x))));   // would E0030
//! ```
//!
//! (The 8 functions that don't take a callback — `RANGE` / `ZIP` /
//! `ENUMERATE` / `TAKE` / `DROP` / `FLAT` / `UNIQ` / `JOIN` — could
//! theoretically fit the std path, but the spec explicitly forbids
//! splitting a single module's behavior across two value worlds; a
//! "MAP works, SORT doesn't" surprise would be worse than either
//! failure mode alone.)
//!
//! ## How the catalog works
//!
//! This module exposes the 17 names through a `ModuleSpec` with an empty
//! `functions` slice. `wlwl_std::resolve("wlwl:std.collection")` still
//! returns `Some(&SPEC)`, so the IMPORT path check passes and the std
//! module is "found". The actual binding happens in
//! `Evaluator::load_std` (see `wlwl-eval`): when the resolved path is
//! `wlwl:std.collection`, the eval side ignores `spec.functions` and
//! instead walks `wlwl_eval::collection::BUILTINS` (eval-internal
//! `BuiltinFn` shims that operate on the rich `Value` type and can
//! invoke user closures via `Evaluator::invoke_closure`).
//!
//! ## The 17 names
//!
//! Per spec §10.5 the 16-row table expands to 17 names when `TAKE` and
//! `DROP` are counted separately (the plan v0.2 §3 row "std.collection
//! 16 个高阶函数" treats them as a single row).

use crate::ModuleSpec;

/// The 17 function names exposed by `wlwl:std.collection`. Kept here as
/// a `const` slice so the eval side can iterate the same canonical list
/// (eval matches on `path == "wlwl:std.collection"` and walks
/// `BUILTINS`; `BUILTINS` must therefore enumerate every name in
/// `NAMES` in the same order — `names_match_builtins` locks it).
pub const NAMES: &[&str] = &[
    // Spec §10.5 order (lines 1239-1251). The eval-side BUILTINS
    // table mirrors this exactly; a re-order on one side forces a
    // failing `names_match_catalog` test on the other, so the two
    // files cannot silently drift.
    "MAP",
    "FILTER",
    "REDUCE",
    "SORT",
    "SORT_BY",
    "ZIP",
    "RANGE",
    "ANY",
    "ALL",
    "FIND",
    "ENUMERATE",
    "TAKE",
    "DROP",
    "FLAT",
    "UNIQ",
    "GROUP_BY",
    "JOIN",
];

/// `wlwl:std.collection` — name catalog. `functions` is intentionally
/// empty: see the module-level docs for the why. The eval side bypasses
/// this slice for this specific path; for all other std modules the
/// slice is the binding source of truth.
pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.collection",
    functions: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_path_is_wlwl_std_collection() {
        assert_eq!(SPEC.path, "wlwl:std.collection");
    }

    #[test]
    fn spec_functions_is_empty() {
        // The catalog shape: `wlwl_std::resolve` returns Some(&SPEC)
        // (so IMPORT passes), but the slice is empty because the eval
        // side handles the binding itself.
        assert!(
            SPEC.functions.is_empty(),
            "collection SPEC must be a name catalog (functions: &[]); \
             got {} entries",
            SPEC.functions.len()
        );
    }

    #[test]
    fn names_count_is_seventeen() {
        // Plan v0.2 §3 row says "16 个高阶函数"; §10.5 enumerates 17
        // when TAKE/DROP are split. Lock the count at the §10.5 value
        // so the eval-side BUILTINS table cannot silently grow/shrink
        // without a failing test (and a matching history/deviations note).
        assert_eq!(NAMES.len(), 17, "NAMES count drifted from §10.5 (17)");
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
            "duplicate name in NAMES; check §10.5 enumeration"
        );
    }

    #[test]
    fn names_match_spec_v0_4_section_10_5() {
        // The exact set from spec §10.5 (Phase B6 source of truth).
        // If you add a name here, update plan §3 / deviations.md too.
        let expected: &[&str] = &[
            "MAP", "FILTER", "REDUCE", "SORT", "SORT_BY", "ZIP", "RANGE",
            "ANY", "ALL", "FIND", "ENUMERATE", "TAKE", "DROP", "FLAT",
            "UNIQ", "GROUP_BY", "JOIN",
        ];
        assert_eq!(NAMES, expected, "NAMES drifted from spec §10.5");
    }
}