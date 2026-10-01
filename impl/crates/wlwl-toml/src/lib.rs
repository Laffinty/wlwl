//! WLWL package manifest + lock file (v0.3 §13.8).
//!
//! ## Scope
//!
//! - `manifest` — parse + validate a `wlwl.toml`. The on-disk schema
//!   follows v0.3 §13.8: a `[package]` block, a `[dependencies]`
//!   map keyed by `<namespace>:<name>`, an optional `[namespaces]`
//!   override map, and an optional `[features]` block.
//!
//! - `lock` — generate / read a `wlwl.lock` JSON file that records
//!   the resolved paths / versions / source hashes for every
//!   dependency. JSON chosen over TOML so the lock stays stable
//!   even if the manifest schema evolves.
//!
//! ## Why a separate crate
//!
//! `wlwl-eval`'s `ModuleLoader` needs the manifest to resolve
//! `IMPORT("myteam:utils", ...)` paths; the `wlwl` CLI also needs it
//! to honour `[package].entry` and to emit `wlwl.lock`. Pulling this
//! into its own crate keeps parsing / validation independent of the
//! tree-walking interpreter, and avoids growing `wlwl-eval` further
//! (it is already 2,400+ lines).

pub mod lock;
pub mod manifest;
pub mod mvs;

/// Strip **at most one** leading UTF-8 BOM (`U+FEFF`).
///
/// D11-026: both the manifest and the lock are text inputs, and neither
/// `toml` nor `serde_json` accepts a leading BOM — on a Windows host that is
/// the difference between "your project loads" and "your project silently
/// loses its `[package]` block".
///
/// Exactly one, not `trim_start_matches`: a second `U+FEFF` is not a BOM, it
/// is an ordinary character, and silently eating it would change what the file
/// means.
///
/// The language spec covers BOM only for `.wll` (§1.1); the manifest and the
/// lock are **not** covered (§9.4 puts the lock outside the spec entirely), so
/// this leniency is a tool choice with no normative backing. Rationale, the
/// per-file map and the test layout live in
/// `docs/spec/wlwl-agent-spec-v0.11.md` §2.
pub(crate) fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}
