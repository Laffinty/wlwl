//! `wlwl:std.fs` — `READ_FILE`, `WRITE_FILE`, `EXISTS` (v0.3 §15.3).
//!
//! Error code mapping (matches spec §14.4):
//!   - E0061 — file not found
//!   - E0062 — permission denied
//!   - E0060 — other I/O error (retryable)

use crate::{expect_string, arity_error, type_error, StdCtx, StdError, StdFn, StdValue, ModuleSpec};
use wlwl_error::ErrorCode;
use std::io::ErrorKind;

pub fn std_read_file(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    let path = expect_string("READ_FILE", &args, 0, 1)?;
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(StdValue::String(content)),
        Err(e) => Err(io_error_to_std("READ_FILE", e, path)),
    }
}

pub fn std_write_file(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if args.len() != 2 {
        return Err(arity_error("WRITE_FILE", args.len(), 2));
    }
    let path = expect_string("WRITE_FILE", &args, 0, 2)?;
    let content = match &args[1] {
        StdValue::String(s) => s.clone(),
        other => return Err(type_error("WRITE_FILE", "string", other)),
    };
    match std::fs::write(path, &content) {
        Ok(()) => Ok(StdValue::Null),
        Err(e) => Err(io_error_to_std("WRITE_FILE", e, path)),
    }
}

pub fn std_exists(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    let path = expect_string("EXISTS", &args, 0, 1)?;
    Ok(StdValue::Bool(std::fs::metadata(path).is_ok()))
}

/// Map a `std::io::Error` to the v0.3 §14.4 IO error codes. `E0060`
/// is the catch-all; `E0061` for NotFound; `E0062` for PermissionDenied.
fn io_error_to_std(fn_name: &str, e: std::io::Error, path: &str) -> StdError {
    let (code, label) = match e.kind() {
        ErrorKind::NotFound => (ErrorCode::E0061, "file not found"),
        ErrorKind::PermissionDenied => (ErrorCode::E0062, "permission denied"),
        _ => (ErrorCode::E0060, "I/O error"),
    };
    StdError {
        code,
        message: format!("{}: {} ({}): {}", fn_name, label, path, e),
    }
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.fs",
    functions: &[
        ("READ_FILE", std_read_file as StdFn),
        ("WRITE_FILE", std_write_file as StdFn),
        ("EXISTS", std_exists as StdFn),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmpfile(suffix: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let unique = format!(
            "wlwl_std_fs_test_{}_{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            suffix
        );
        p.push(unique);
        p
    }

    #[test]
    fn write_then_read_roundtrip() {
        let mut ctx = StdCtx::default();
        let path = tmpfile(".txt");
        let p_str = path.to_string_lossy().into_owned();

        std_write_file(
            &mut ctx,
            vec![StdValue::String(p_str.clone()), StdValue::String("hello\nworld".into())],
        )
        .unwrap();

        let v = std_read_file(&mut ctx, vec![StdValue::String(p_str.clone())]).unwrap();
        assert_eq!(v, StdValue::String("hello\nworld".into()));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_nonexistent_is_e0061() {
        let mut ctx = StdCtx::default();
        let err = std_read_file(
            &mut ctx,
            vec![StdValue::String(
                "Z:/this/path/should/definitely/not/exist/abc_xyz_123".into(),
            )],
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::E0061);
    }

    #[test]
    fn exists_true_for_real_and_false_for_missing() {
        let mut ctx = StdCtx::default();
        let path = tmpfile(".txt");
        std::fs::write(&path, b"x").unwrap();
        let p_str = path.to_string_lossy().into_owned();

        let v_true = std_exists(&mut ctx, vec![StdValue::String(p_str.clone())]).unwrap();
        assert_eq!(v_true, StdValue::Bool(true));

        let _ = std::fs::remove_file(&path);
        let v_false = std_exists(&mut ctx, vec![StdValue::String(p_str)]).unwrap();
        assert_eq!(v_false, StdValue::Bool(false));
    }

    #[test]
    fn arity_mismatch_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_read_file(&mut ctx, vec![]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn spec_lists_all_three() {
        assert_eq!(SPEC.path, "wlwl:std.fs");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["READ_FILE", "WRITE_FILE", "EXISTS"]);
    }
}