/// Identify test/benchmark/spec modules by analyzing file path fragments.
pub fn is_test_like_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_lowercase();
    normalized.contains("/tests/")
        || normalized.contains("/test/")
        || normalized.contains("_test.")
        || normalized.contains(".test.")
        || normalized.contains("/spec/")
        || normalized.contains("/specs/")
        || normalized.contains("_spec.")
        || normalized.contains("tests.rs")
        || normalized.contains("test_bench")
        || normalized.contains("_test.rs")
        || normalized.contains("_tests.rs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_test_and_spec_paths() {
        assert!(is_test_like_path("src/tests/foo.rs"));
        assert!(is_test_like_path("src/foo_test.rs"));
        assert!(is_test_like_path("spec/models_spec.rb"));
        assert!(!is_test_like_path("src/main.rs"));
    }
}
