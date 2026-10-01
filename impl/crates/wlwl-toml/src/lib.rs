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
/// D11-026, spec §1.1: both the manifest and the lock are text inputs, and
/// neither `toml` nor `serde_json` accepts a leading BOM — on a Windows host
/// that is the difference between "your project loads" and "your project
/// silently loses its `[package]` block".
///
/// Exactly one, not `trim_start_matches`: a second `U+FEFF` is not a BOM, it
/// is an ordinary character, and silently eating it would change what the file
/// means. See §1.1 for why one is the only rule the source file can support
/// (P3-011 lets identifiers start with any non-ASCII letter, so the lexer
/// cannot tell a BOM from identifier content) — and why it is unconditionally
/// correct for the manifest and lock, whose first character is grammar-locked.
pub(crate) fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}
