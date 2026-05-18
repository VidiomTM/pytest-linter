use pytest_linter::config::Config;
use pytest_linter::engine::LintEngine;
use std::collections::HashSet;
use std::path::PathBuf;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn parse_expected_violations(content: &str) -> HashSet<String> {
    let mut expected = HashSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(idx) = trimmed.find("# expect: ") {
            let rule_id = trimmed[idx + "# expect: ".len()..].trim();
            if rule_id.starts_with("PYTEST-") {
                expected.insert(rule_id.to_string());
            }
        }
    }
    expected
}

fn parse_expected_clean(content: &str) -> HashSet<String> {
    let mut clean = HashSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(idx) = trimmed.find("# expect-clean: ") {
            let test_name = trimmed[idx + "# expect-clean: ".len()..].trim();
            clean.insert(test_name.to_string());
        }
    }
    clean
}

fn run_golden_test(file_name: &str) {
    let src_path = golden_dir().join(file_name);
    let content = std::fs::read_to_string(&src_path)
        .unwrap_or_else(|e| panic!("Failed to read golden file {:?}: {}", src_path, e));
    let expected = parse_expected_violations(&content);
    let expected_clean = parse_expected_clean(&content);

    let dir = tempfile::tempdir().unwrap();
    let test_name = file_name.replace("_patterns", "");
    let dest = dir.path().join(format!("test_{test_name}"));
    std::fs::write(&dest, &content).unwrap();

    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[dest]).unwrap();

    let actual: HashSet<String> = violations.iter().map(|v| v.rule_id.clone()).collect();

    let missing: Vec<_> = expected.difference(&actual).collect();
    let extra: Vec<_> = actual.difference(&expected).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "Golden test mismatch for {}:\n  Missing violations (expected but not found): {:?}\n  Extra violations (found but not expected): {:?}\n  Expected: {:?}\n  Actual: {:?}",
        file_name,
        missing,
        extra,
        expected,
        actual,
    );

    if !expected_clean.is_empty() {
        for test_name in &expected_clean {
            let has_violation = violations
                .iter()
                .any(|v| v.test_name.as_ref().is_some_and(|t| t == test_name));
            assert!(
                !has_violation,
                "Clean test '{}' should have zero violations but got violations: {:?}",
                test_name,
                violations
                    .iter()
                    .filter(|v| v.test_name.as_ref().is_some_and(|t| t == test_name))
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn test_golden_pytest_patterns() {
    run_golden_test("pytest_patterns.py");
}

#[test]
fn test_golden_hypothesis_patterns() {
    run_golden_test("hypothesis_patterns.py");
}

#[test]
fn test_golden_pandas_patterns() {
    run_golden_test("pandas_patterns.py");
}

#[test]
fn test_golden_django_patterns() {
    run_golden_test("django_patterns.py");
}

#[test]
fn test_golden_fix_006_stateful_session() {
    run_golden_test("fix_006_stateful_session.py");
}

#[test]
fn test_golden_fix_007_chained_mutation() {
    run_golden_test("fix_007_chained_mutation.py");
}

#[test]
fn test_golden_flk_001_async_sleep() {
    run_golden_test("flk_001_async_sleep.py");
}

#[test]
fn test_golden_flk_003_network_httpx() {
    run_golden_test("flk_003_network_httpx.py");
}

#[test]
fn test_golden_flk_003_network_mock_clean() {
    run_golden_test("flk_003_network_mock_clean.py");
}

#[test]
fn test_golden_moc_001_patch_definition() {
    run_golden_test("moc_001_patch_definition.py");
}

#[test]
fn test_golden_moc_002_async_magic_mock() {
    run_golden_test("moc_002_async_magic_mock.py");
}

#[test]
fn test_golden_moc_003_patch_init() {
    run_golden_test("moc_003_patch_init.py");
}

#[test]
fn test_golden_moc_004_mock_ratio() {
    run_golden_test("moc_004_mock_ratio.py");
}

#[test]
fn test_golden_inf_001_network_ban_missing() {
    run_golden_test("inf_001_network_ban_missing.py");
}

#[test]
fn test_golden_inf_002_live_unmarked() {
    run_golden_test("inf_002_live_unmarked.py");
}

#[test]
fn test_golden_inf_003_monkeypatch_nonidiomatic() {
    run_golden_test("inf_003_monkeypatch_nonidiomatic.py");
}

#[test]
fn test_golden_inf_004_shutil_copy() {
    run_golden_test("inf_004_shutil_copy.py");
}

#[test]
fn test_golden_val_001_inline_schema() {
    run_golden_test("val_001_inline_schema.py");
}
