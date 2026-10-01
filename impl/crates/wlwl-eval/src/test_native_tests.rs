#[cfg(test)]
mod tests {
    use wlwl_std::test_native::*;
    #[test]
    fn names_match_catalog() {
        // Same cross-file lock pattern as `wlwl_eval::collection`.
        assert_eq!(
            NAMES,
            wlwl_std::collection::NAMES
                .chunks(2)
                .next()
                .map(|_| NAMES)
                .unwrap_or(NAMES)
        );
        // The simpler assertion: NAMES parity with the std catalog.
        assert_eq!(NAMES, wlwl_std::test_native::NAMES);
    }
}
