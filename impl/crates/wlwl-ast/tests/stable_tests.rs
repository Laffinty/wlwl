//! Phase E3 stable node-ID tree tests (spec v0.4 §16.4.1).
//!
//! Integration tests (not unit tests inside the lib) so that the
//! `wlwl_parser` dev-dep and this crate unify to one `wlwl_ast`
//! instance — a unit test inside `src/stable.rs` would compile two
//! copies of `wlwl_ast` (lib-under-test vs. parser's dependency) and
//! the `Expr` types would not match.

use wlwl_ast::stable::{stable_tree, StableNode};

fn parse_src(src: &str) -> wlwl_ast::Expr {
    wlwl_parser::parse(src, "t.wl").expect("parse failed")
}

fn collect(n: &StableNode, out: &mut Vec<String>) {
    out.push(n.node_id.clone());
    for c in &n.children {
        collect(c, out);
    }
}

#[test]
fn root_id_uses_module_and_body() {
    let e = parse_src("LET(x, 1); PRINT(x);");
    let tree = stable_tree(&e, "t.wl");
    assert_eq!(tree.node_id, "t.wl:fn:<top>/body");
    assert_eq!(tree.parent_id, None);
    assert_eq!(tree.kind, "Block");
    assert!(tree.hash.starts_with("sha256:"));
}

#[test]
fn node_ids_are_structural_and_unique() {
    let e = parse_src("LET(x, 1); PRINT(x);");
    let tree = stable_tree(&e, "t.wl");
    let mut ids = Vec::new();
    collect(&tree, &mut ids);
    assert_eq!(ids.len(), 5); // body + LET + Literal + PRINT-call + Var
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "ids must be unique: {:?}", ids);
    // Structural stability: the PRINT call is stmt:1.
    assert!(ids.iter().any(|i| i.ends_with("/stmt:1")), "ids: {:?}", ids);
}

#[test]
fn ids_stable_across_line_shifts() {
    // node_id + hash must not depend on line numbers. (The `span`
    // field naturally differs — it exists precisely to locate the
    // node in today's source.)
    let collect_id_hash = |src: &str| -> Vec<(String, String)> {
        let tree = stable_tree(&parse_src(src), "t.wl");
        let mut out = Vec::new();
        fn walk(n: &StableNode, out: &mut Vec<(String, String)>) {
            out.push((n.node_id.clone(), n.hash.clone()));
            for c in &n.children {
                walk(c, out);
            }
        }
        walk(&tree, &mut out);
        out
    };
    let a = collect_id_hash("LET(x, 1); PRINT(x);");
    let b = collect_id_hash("\n\nLET(x, 1);\n\nPRINT(x);");
    assert_eq!(a, b, "ids+hashes must not depend on line numbers");
}

#[test]
fn fun_boundary_switches_fn_context() {
    let e = parse_src("LET(f, FUN(double(x), *(x, 2)));");
    let tree = stable_tree(&e, "t.wl");
    let mut ids = Vec::new();
    collect(&tree, &mut ids);
    assert!(
        ids.iter().any(|i| i.contains("/fn:double/body:")),
        "body descendants must be addressed via the fn segment: {:?}",
        ids
    );
}

#[test]
fn anonymous_fun_uses_anon_context() {
    let e = parse_src("LET(f, FUN((x), x));");
    let tree = stable_tree(&e, "t.wl");
    let mut ids = Vec::new();
    collect(&tree, &mut ids);
    assert!(
        ids.iter().any(|i| i.contains("/fn:<anon>/body:")),
        "{:?}",
        ids
    );
}

#[test]
fn hash_changes_when_content_changes() {
    let a = stable_tree(&parse_src("LET(x, 1);"), "t.wl");
    let b = stable_tree(&parse_src("LET(x, 2);"), "t.wl");
    assert_ne!(a.hash, b.hash);
    // Whitespace-only changes do NOT change the content hash:
    let c = stable_tree(&parse_src("LET( x,  1 );"), "t.wl");
    assert_eq!(a.hash, c.hash, "spans must not affect content hash");
}

#[test]
fn parent_ids_link_the_tree() {
    // A single-statement program parses to the bare Let (blocks with
    // one statement are unwrapped by the parser), so the root IS the
    // Let node.
    let e = parse_src("LET(x, 1);");
    let tree = stable_tree(&e, "t.wl");
    assert_eq!(tree.kind, "Let");
    assert_eq!(tree.children.len(), 1);
    let literal = &tree.children[0];
    assert_eq!(literal.kind, "Literal");
    assert_eq!(literal.parent_id.as_deref(), Some(tree.node_id.as_str()));
}

#[test]
fn hash_is_sha256_hex() {
    let e = parse_src("PRINT(1);");
    let tree = stable_tree(&e, "t.wl");
    let hex = tree.hash.strip_prefix("sha256:").unwrap();
    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn deep_ids_embed_fun_and_paths() {
    // AI-tool scenario from §16.4.1: a call inside a function body is
    // addressable as `file:fn:NAME/body/.../call:N`.
    let e = parse_src("LET(summarize, FUN(summarize(x),\n    LET(y, ASK(x));\n    y\n));");
    let tree = stable_tree(&e, "t.wl");
    let mut under_fun = Vec::new();
    fn walk(n: &StableNode, out: &mut Vec<(String, String)>) {
        if n.node_id.contains("/fn:summarize/body:") {
            out.push((n.node_id.clone(), n.kind.clone()));
        }
        for c in &n.children {
            walk(c, out);
        }
    }
    walk(&tree, &mut under_fun);
    assert!(
        under_fun.iter().any(|(_, kind)| kind == "Call"),
        "expected a Call node under fn:summarize, got: {:?}",
        under_fun
    );
}
