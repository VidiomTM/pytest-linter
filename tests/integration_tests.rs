use pytest_linter::config::Config;
use pytest_linter::engine::LintEngine;
use pytest_linter::models::{Category, FixtureScope, Severity};
use pytest_linter::parser::PythonParser;
use std::path::{Path, PathBuf};

fn write_temp_file(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

fn parse_file(path: &Path) -> pytest_linter::models::ParsedModule {
    let mut parser = PythonParser::new().unwrap();
    parser.parse_file(path).unwrap()
}

fn lint_single_file(path: &Path) -> Vec<pytest_linter::models::Violation> {
    let engine = LintEngine::new(Config::default()).unwrap();
    engine.lint_paths(&[path.to_path_buf()]).unwrap()
}

fn find_violation<'a>(
    violations: &'a [pytest_linter::models::Violation],
    rule_id: &str,
) -> Option<&'a pytest_linter::models::Violation> {
    violations.iter().find(|v| v.rule_id == rule_id)
}

#[test]
fn test_time_sleep_triggers_flk001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_flaky.py",
        r#"
import time

def test_waits():
    time.sleep(2)
    assert True
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].uses_time_sleep);

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-001");
    assert!(v.is_some(), "Expected PYTEST-FLK-001 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "TimeSleepRule");
    assert_eq!(v.severity, Severity::Warning);
    assert_eq!(v.category, Category::Flakiness);
    assert!(v.message.contains("time.sleep"));
    assert!(v.suggestion.is_some());
}

#[test]
fn test_file_io_without_tmp_path_triggers_flk002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_file_io.py",
        r#"
def test_reads_file():
    f = open("data.txt")
    content = f.read()
    f.close()
    assert content
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].uses_file_io);
    assert!(!module.test_functions[0]
        .fixture_deps
        .iter()
        .any(|d| d == "tmp_path"));

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-002");
    assert!(v.is_some(), "Expected PYTEST-FLK-002 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "FileIoRule");
    assert_eq!(v.category, Category::Flakiness);
    assert!(v.message.contains("file I/O"));
}

#[test]
fn test_file_io_with_tmp_path_does_not_trigger_flk002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_file_io_safe.py",
        r#"
def test_reads_file(tmp_path):
    f = open(str(tmp_path / "data.txt"))
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-002");
    assert!(
        v.is_none(),
        "Should not trigger PYTEST-FLK-002 when tmp_path is used"
    );
}

#[test]
fn test_network_import_triggers_flk003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_network.py",
        r#"
import requests

def test_api():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.imports.iter().any(|imp| imp.contains("requests")));

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-003");
    assert!(v.is_some(), "Expected PYTEST-FLK-003 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "NetworkImportRule");
    assert_eq!(v.category, Category::Flakiness);
    assert!(v.message.contains("network"));
}

#[test]
fn test_conditional_logic_triggers_mnt001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_logic.py",
        r#"
def test_with_condition():
    x = 5
    if x > 3:
        assert True
    else:
        assert False
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].has_conditional_logic);

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-001");
    assert!(v.is_some(), "Expected PYTEST-MNT-001 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "TestLogicRule");
    assert_eq!(v.category, Category::Maintenance);
    assert!(v.message.contains("conditional logic"));
}

#[test]
fn test_no_assertions_triggers_mnt004() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_assert.py",
        r#"
def test_does_nothing():
    x = 42
    print(x)
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(!module.test_functions[0].has_assertions);

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-004");
    assert!(v.is_some(), "Expected PYTEST-MNT-004 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "NoAssertionRule");
    assert_eq!(v.severity, Severity::Error);
    assert_eq!(v.category, Category::Maintenance);
    assert!(v.message.contains("no assertions"));
}

#[test]
fn test_try_except_triggers_mnt007() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_try_except.py",
        r#"
def test_catches_exception():
    try:
        risky_operation()
    except ValueError:
        pass
    assert True
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].has_try_except);

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-007");
    assert!(v.is_some(), "Expected PYTEST-MNT-007 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "RawExceptionHandlingRule");
    assert!(v.message.contains("try/except"));
    assert!(v.suggestion.as_ref().unwrap().contains("pytest.raises"));
}

#[test]
fn test_autouse_fixture_triggers_fix001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_autouse.py",
        r#"
import pytest

@pytest.fixture(autouse=True)
def setup_env():
    os.environ["TEST"] = "1"
    yield
    del os.environ["TEST"]

def test_something():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 1);
    assert!(module.fixtures[0].is_autouse);

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-001");
    assert!(v.is_some(), "Expected PYTEST-FIX-001 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "AutouseFixtureRule");
    assert_eq!(v.category, Category::Fixture);
    assert!(v.message.contains("autouse"));
}

#[test]
fn test_many_assertions_triggers_mnt006() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_many_asserts.py",
        r#"
def test_many_things():
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3
    assert 4 == 4
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].assertion_count > 3);

    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-006");
    assert!(v.is_some(), "Expected PYTEST-MNT-006 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "AssertionRouletteRule");
    assert!(v.message.contains("assertion roulette"));
}

#[test]
fn test_few_assertions_does_not_trigger_mnt006() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_few_asserts.py",
        r#"
def test_simple():
    assert True
    assert False == False
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-006");
    assert!(
        v.is_none(),
        "Should not trigger PYTEST-MNT-006 with <= 3 assertions"
    );
}

#[test]
fn test_clean_file_has_no_error_violations() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_clean.py",
        r#"
def test_addition():
    assert 1 + 1 == 2

def test_subtraction():
    assert 3 - 1 == 2
"#,
    );
    let violations = lint_single_file(&path);
    let error_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .collect();
    assert!(
        error_violations.is_empty(),
        "Clean test file should not have Error-severity violations, got: {:?}",
        error_violations
    );
}

#[test]
fn test_engine_lint_paths_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_multi.py",
        r#"
import time
import requests

def test_has_sleep():
    time.sleep(1)
    assert True

def test_no_assert():
    x = 42

def test_many_asserts():
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3
    assert 4 == 4
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(!violations.is_empty(), "Expected multiple violations");

    let rule_ids: Vec<&str> = violations.iter().map(|v| v.rule_id.as_str()).collect();
    assert!(
        rule_ids.contains(&"PYTEST-FLK-001"),
        "Should detect time.sleep"
    );
    assert!(
        rule_ids.contains(&"PYTEST-FLK-003"),
        "Should detect network import"
    );
    assert!(
        rule_ids.contains(&"PYTEST-MNT-004"),
        "Should detect no assertion"
    );
    assert!(
        rule_ids.contains(&"PYTEST-MNT-006"),
        "Should detect assertion roulette"
    );
}

#[test]
fn test_violation_ordering() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_order.py",
        r#"
def test_a():
    assert True

def test_b():
    pass
"#,
    );
    let violations = lint_single_file(&path);
    for i in 1..violations.len() {
        assert!(
            violations[i - 1] <= violations[i],
            "Violations should be sorted: {:?} > {:?}",
            violations[i - 1],
            violations[i]
        );
    }
}

#[test]
fn test_parser_extracts_test_functions() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_parse.py",
        r#"
def helper():
    return 42

def test_one():
    assert helper() == 42

def test_two():
    assert True

def not_a_test():
    pass
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 2);
    let names: Vec<&str> = module
        .test_functions
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(names.contains(&"test_one"));
    assert!(names.contains(&"test_two"));
    assert!(!names.contains(&"helper"));
    assert!(!names.contains(&"not_a_test"));
}

#[test]
fn test_parser_extracts_imports() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_imports.py",
        r#"
import os
import sys
from pathlib import Path

def test_something():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.imports.iter().any(|imp| imp.contains("import os")));
    assert!(module.imports.iter().any(|imp| imp.contains("import sys")));
    assert!(module.imports.iter().any(|imp| imp.contains("pathlib")));
}

#[test]
fn test_parser_extracts_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fixtures_parse.py",
        r#"
import pytest

@pytest.fixture
def my_fixture():
    return 42

@pytest.fixture(scope="module")
def module_fixture():
    return {}

def test_uses_fixture(my_fixture):
    assert my_fixture == 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 2);

    let f1 = module
        .fixtures
        .iter()
        .find(|f| f.name == "my_fixture")
        .unwrap();
    assert_eq!(f1.scope, FixtureScope::Function);
    assert!(!f1.is_autouse);

    let f2 = module
        .fixtures
        .iter()
        .find(|f| f.name == "module_fixture")
        .unwrap();
    assert_eq!(f2.scope, FixtureScope::Module);
}

#[test]
fn test_non_test_file_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "helper.py",
        r#"
import time

def test_waits():
    time.sleep(2)
    assert True
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(violations.is_empty(), "Non-test files should be ignored");
}

#[test]
fn test_conftest_is_recognized_as_test_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture(autouse=True)
def auto_fixture():
    return 42
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    let v = find_violation(&violations, "PYTEST-FIX-001");
    assert!(
        v.is_some(),
        "conftest.py should be parsed and autouse detected"
    );
}

#[test]
fn test_mystery_guest_triggers_flk005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mystery.py",
        r#"
def test_reads_file():
    f = open("data.txt")
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-005");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-005 Mystery Guest violation"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "MysteryGuestRule");
}

#[test]
fn test_fixture_db_commit_no_cleanup_triggers_fix008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_db.py",
        r#"
import pytest

@pytest.fixture
def db_fixture():
    conn = get_connection()
    conn.commit()
    return conn

def test_db(db_fixture):
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-008");
    assert!(
        v.is_some(),
        "Expected PYTEST-FIX-008 for DB commit without rollback"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "FixtureDbCommitNoCleanupRule");
}

#[test]
fn test_violation_contains_correct_file_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_path.py",
        r#"
def test_empty():
    pass
"#,
    );
    let violations = lint_single_file(&path);
    assert!(!violations.is_empty());
    for v in &violations {
        assert_eq!(v.file_path, path);
    }
}

#[test]
fn test_parametrized_test_detection() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [1, 2, 3])
def test_values(val):
    assert val > 0
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].is_parametrized);
    assert!(module.test_functions[0].parametrize_count.is_some());
}

#[test]
fn test_async_test_detection() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_async.py",
        r#"
async def test_async_thing():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].is_async);
}

#[test]
fn test_run_linter_clean_file_returns_no_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_clean_lint.py",
        r#"
def test_ok():
    assert True
"#,
    );
    let has_errors =
        pytest_linter::engine::run_linter(&[path], "json", None, true, Config::default()).unwrap();
    assert!(!has_errors);
}

#[test]
fn test_run_linter_with_errors_returns_true() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_bad_lint.py",
        r#"
def test_bad():
    pass
"#,
    );
    let has_errors =
        pytest_linter::engine::run_linter(&[path], "json", None, true, Config::default()).unwrap();
    assert!(has_errors);
}

#[test]
fn test_run_linter_json_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_json_out.py",
        r#"
def test_ok():
    assert True
"#,
    );
    let output_path = dir.path().join("output.json");
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "json",
        Some(&output_path),
        true,
        Config::default(),
    )
    .unwrap();
    assert!(!has_errors, "Info-only violations should not be errors");
    let content = std::fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn test_run_linter_terminal_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_term_out.py",
        r#"
def test_bad():
    pass
"#,
    );
    let output_path = dir.path().join("output.txt");
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "terminal",
        Some(&output_path),
        true,
        Config::default(),
    )
    .unwrap();
    assert!(has_errors);
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("ERROR"));
    assert!(content.contains("Summary"));
}

#[test]
fn test_run_linter_terminal_with_info_violations() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_term_info.py",
        r#"
def test_ok():
    assert 1 + 1 == 2
"#,
    );
    let output_path = dir.path().join("info.txt");
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "terminal",
        Some(&output_path),
        true,
        Config::default(),
    )
    .unwrap();
    assert!(!has_errors, "Info-only violations should not be errors");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("Summary"));
    assert!(content.contains("info"));
}

#[test]
fn test_run_linter_no_test_files_no_violations() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("empty.txt");
    let has_errors = pytest_linter::engine::run_linter(
        &[dir.path().to_path_buf()],
        "terminal",
        Some(&output_path),
        true,
        Config::default(),
    )
    .unwrap();
    assert!(!has_errors);
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("No violations found"));
}

#[test]
fn test_run_linter_json_output_contains_violation() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_json_violation.py",
        r#"
def test_no_assert():
    x = 42
"#,
    );
    let output_path = dir.path().join("violations.json");
    pytest_linter::engine::run_linter(&[path], "json", Some(&output_path), true, Config::default())
        .unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty());
    let has_mnt004 = arr
        .iter()
        .any(|v| v["rule_id"].as_str().unwrap() == "PYTEST-MNT-004");
    assert!(has_mnt004);
}

#[test]
fn test_invalid_scope_triggers_fix003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_scope.py",
        r#"
import pytest

@pytest.fixture
def func_scoped():
    return 42

@pytest.fixture(scope="session")
def session_scoped(func_scoped):
    return func_scoped

def test_thing(session_scoped):
    assert session_scoped == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-003");
    assert!(v.is_some(), "Expected PYTEST-FIX-003 for invalid scope");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "InvalidScopeRule");
    assert!(v.message.contains("session_scoped"));
    assert!(v.message.contains("func_scoped"));
}

#[test]
fn test_shadowed_fixture_triggers_fix004() {
    let dir = tempfile::tempdir().unwrap();
    let path1 = write_temp_file(
        dir.path(),
        "test_a.py",
        r#"
import pytest

@pytest.fixture
def shared_fix():
    return 42

def test_a(shared_fix):
    assert shared_fix == 42
"#,
    );
    let path2 = write_temp_file(
        dir.path(),
        "test_b.py",
        r#"
import pytest

@pytest.fixture
def shared_fix():
    return "hello"

def test_b(shared_fix):
    assert shared_fix == "hello"
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[path1, path2]).unwrap();
    let v = find_violation(&violations, "PYTEST-FIX-004");
    assert!(v.is_some(), "Expected PYTEST-FIX-004 for shadowed fixture");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "ShadowedFixtureRule");
    assert!(v.message.contains("shared_fix"));
    assert!(v.message.contains("2 different modules"));
}

#[test]
fn test_unused_fixture_triggers_fix005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_unused.py",
        r#"
import pytest

@pytest.fixture
def unused_fixture():
    return 42

def test_something():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-005");
    assert!(v.is_some(), "Expected PYTEST-FIX-005 for unused fixture");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "UnusedFixtureRule");
    assert!(v.message.contains("unused_fixture"));
}

#[test]
fn test_stateful_session_fixture_triggers_fix006() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_stateful.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def shared_list():
    return []

def test_uses_list(shared_list):
    shared_list.append(1)
    assert len(shared_list) == 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-006");
    assert!(
        v.is_some(),
        "Expected PYTEST-FIX-006 for stateful session fixture"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "StatefulSessionFixtureRule");
    assert!(v.message.contains("shared_list"));
    assert!(v.message.contains("mutable state"));
}

#[test]
fn test_db_commit_no_cleanup_with_yield_does_not_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_db_yield.py",
        r#"
import pytest

@pytest.fixture
def db_with_cleanup():
    conn = get_conn()
    conn.commit()
    yield conn
    conn.rollback()

def test_db(db_with_cleanup):
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-008");
    assert!(
        v.is_none(),
        "Should not trigger FIX-008 when fixture has yield"
    );
}

#[test]
fn test_mock_only_verify_triggers_mnt005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mock_verify.py",
        r#"
def test_mock_only(mock_obj):
    mock_obj.assert_called()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(v.is_some(), "Expected PYTEST-MNT-005 for mock-only verify");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "MockOnlyVerifyRule");
    assert!(v.message.contains("mock_only"));
    assert!(v.message.contains("mocks without checking state"));
}

#[test]
fn test_mock_with_state_assertions_does_not_trigger_mnt005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mock_state.py",
        r#"
def test_mock_and_state(mock_obj):
    mock_obj.assert_called()
    assert 1 == 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(
        v.is_none(),
        "Should not trigger MNT-005 when test has state assertions"
    );
}

#[test]
fn test_bdd_missing_scenario_triggers_bdd001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_bdd.py",
        r#"
def test_without_gherkin():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-BDD-001");
    assert!(
        v.is_some(),
        "Expected PYTEST-BDD-001 for missing BDD scenario"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "BddMissingScenarioRule");
    assert_eq!(v.severity, Severity::Info);
}

#[test]
fn test_bdd_with_gherkin_does_not_trigger_bdd001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_bdd_ok.py",
        r#"
def test_with_gherkin():
    """Given a setup when an action then a result."""
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-BDD-001");
    assert!(
        v.is_none(),
        "Should not trigger BDD-001 when docstring has Gherkin"
    );
}

#[test]
fn test_property_test_hint_triggers_pbt001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_pbt.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [1, 2, 3, 4])
def test_many(val):
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PBT-001");
    assert!(
        v.is_some(),
        "Expected PYTEST-PBT-001 for many parametrize cases"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "PropertyTestHintRule");
    assert!(v.message.contains("parametrized cases"));
    assert!(v.suggestion.as_ref().unwrap().contains("hypothesis"));
}

#[test]
fn test_property_test_hint_few_cases_does_not_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_pbt_few.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [1, 2, 3])
def test_few(val):
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PBT-001");
    assert!(v.is_none(), "Should not trigger PBT-001 with <= 3 cases");
}

#[test]
fn test_parametrize_empty_triggers_param001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param_empty.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [1])
def test_single(val):
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-001");
    assert!(
        v.is_some(),
        "Expected PYTEST-PARAM-001 for single parametrize case"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "ParametrizeEmptyRule");
    assert!(v.message.contains("only"));
}

#[test]
fn test_xdist_fixture_io_triggers_xdist002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_xdist.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def session_data():
    f = open("data.txt")
    data = f.read()
    f.close()
    return data

def test_data(session_data):
    assert session_data
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-XDIST-002");
    assert!(
        v.is_some(),
        "Expected PYTEST-XDIST-002 for session fixture with I/O"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "XdistFixtureIoRule");
    assert!(v.message.contains("session_data"));
    assert!(v.message.contains("file I/O"));
}

#[test]
fn test_xdist_fixture_io_function_scope_does_not_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_xdist_ok.py",
        r#"
import pytest

@pytest.fixture
def func_data():
    f = open("data.txt")
    data = f.read()
    f.close()
    return data

def test_data(func_data):
    assert func_data
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-XDIST-002");
    assert!(
        v.is_none(),
        "Should not trigger XDIST-002 for function-scoped fixture"
    );
}

#[test]
fn test_assertion_roulette_exactly_3_does_not_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_3_asserts.py",
        r#"
def test_three():
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions[0].assertion_count, 3);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-006");
    assert!(
        v.is_none(),
        "Should not trigger PYTEST-MNT-006 with exactly 3 assertions"
    );
}

#[test]
fn test_assertion_roulette_exactly_4_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_4_asserts.py",
        r#"
def test_four():
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3
    assert 4 == 4
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions[0].assertion_count, 4);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-006");
    assert!(
        v.is_some(),
        "Expected PYTEST-MNT-006 with exactly 4 assertions"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "AssertionRouletteRule");
}

#[test]
fn test_parser_parametrize_count_three() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_pcount.py",
        r#"
import pytest

@pytest.mark.parametrize("arg", [1, 2, 3])
def test_values(arg):
    assert arg > 0
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(module.test_functions[0].is_parametrized);
    assert_eq!(module.test_functions[0].parametrize_count, Some(3));
}

#[test]
fn test_parser_fixture_scope_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fscope.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def session_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 1);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Session);
}

#[test]
fn test_parser_fixture_scope_class() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_class_scope.py",
        r#"
import pytest

@pytest.fixture(scope="class")
def class_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Class);
}

#[test]
fn test_parser_fixture_scope_package() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_pkg_scope.py",
        r#"
import pytest

@pytest.fixture(scope="package")
def pkg_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Package);
}

#[test]
fn test_parser_fixture_has_yield() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_yield.py",
        r#"
import pytest

@pytest.fixture
def yield_fixture():
    yield 42
    cleanup()
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 1);
    assert!(module.fixtures[0].has_yield);
}

#[test]
fn test_parser_fixture_db_commit() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_db_commit.py",
        r#"
import pytest

@pytest.fixture
def db_fix():
    conn = get_conn()
    conn.commit()
    return conn
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 1);
    assert!(module.fixtures[0].has_db_commit);
    assert!(!module.fixtures[0].has_db_rollback);
}

#[test]
fn test_parser_fixture_db_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_db_rollback.py",
        r#"
import pytest

@pytest.fixture
def db_fix():
    conn = get_conn()
    conn.rollback()
    return conn
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].has_db_rollback);
    assert!(!module.fixtures[0].has_db_commit);
}

#[test]
fn test_parser_fixture_autouse() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_autouse_parse.py",
        r#"
import pytest

@pytest.fixture(autouse=True)
def auto_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 1);
    assert!(module.fixtures[0].is_autouse);
}

#[test]
fn test_parser_fixture_returns_mutable_list() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mutable_list.py",
        r#"
import pytest

@pytest.fixture
def mutable_fix():
    return []
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].returns_mutable);
}

#[test]
fn test_parser_fixture_returns_mutable_dict() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mutable_dict.py",
        r#"
import pytest

@pytest.fixture
def mutable_fix():
    return {}
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].returns_mutable);
}

#[test]
fn test_parser_fixture_not_mutable() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_not_mutable.py",
        r#"
import pytest

@pytest.fixture
def immutable_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert!(!module.fixtures[0].returns_mutable);
}

#[test]
fn test_stateful_session_dict_triggers_fix006() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_stateful_dict.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def shared_dict():
    return {}

def test_uses_dict(shared_dict):
    assert len(shared_dict) == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-006");
    assert!(
        v.is_some(),
        "Expected PYTEST-FIX-006 for session fixture returning dict"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "StatefulSessionFixtureRule");
}

#[test]
fn test_function_scope_mutable_does_not_trigger_fix006() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_func_mutable.py",
        r#"
import pytest

@pytest.fixture
def local_list():
    return []

def test_uses(local_list):
    assert len(local_list) == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-006");
    assert!(
        v.is_none(),
        "Should not trigger FIX-006 for function-scoped mutable fixture"
    );
}

#[test]
fn test_unused_fixture_with_autouse_does_not_trigger_fix005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_auto_unused.py",
        r#"
import pytest

@pytest.fixture(autouse=True)
def auto_setup():
    return 42

def test_something():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-005");
    assert!(
        v.is_none(),
        "Autouse fixtures should not trigger unused rule"
    );
}

#[test]
fn test_collect_all_fixtures_cross_module() {
    let dir = tempfile::tempdir().unwrap();
    let _path1 = write_temp_file(
        dir.path(),
        "test_fix_a.py",
        r#"
import pytest

@pytest.fixture
def fixture_a():
    return 1

def test_a(fixture_a):
    assert fixture_a == 1
"#,
    );
    let path2 = write_temp_file(
        dir.path(),
        "test_fix_b.py",
        r#"
import pytest

@pytest.fixture
def fixture_b(fixture_a):
    return fixture_a + 1

def test_b(fixture_b):
    assert fixture_b == 2
"#,
    );
    let module = parse_file(&path2);
    assert_eq!(module.fixtures.len(), 1);
    assert_eq!(module.fixtures[0].name, "fixture_b");
    assert!(module.fixtures[0]
        .dependencies
        .contains(&"fixture_a".to_string()));
}

#[test]
fn test_violation_has_suggestion_and_test_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_details.py",
        r#"
def test_waits():
    import time
    time.sleep(2)
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-001").unwrap();
    assert!(v.suggestion.is_some());
    assert_eq!(v.test_name.as_ref().unwrap(), "test_waits");
}

#[test]
fn test_violation_severity_and_category_consistency() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_consistency.py",
        r#"
def test_no_assert():
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-004").unwrap();
    assert_eq!(v.rule_id, "PYTEST-MNT-004");
    assert_eq!(v.rule_name, "NoAssertionRule");
    assert_eq!(v.severity, Severity::Error);
    assert_eq!(v.category, Category::Maintenance);
}

#[test]
fn test_discover_files_test_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "foo_test.py",
        r#"
def test_something():
    assert True
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(
        !violations.is_empty(),
        "foo_test.py should be recognized as test file"
    );
}

#[test]
fn test_discover_files_directory_mode() {
    let dir = tempfile::tempdir().unwrap();
    write_temp_file(
        dir.path(),
        "test_dir_example.py",
        r#"
def test_example():
    assert True
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[dir.path().to_path_buf()]).unwrap();
    assert!(
        !violations.is_empty(),
        "Should discover test files in directory"
    );
}

#[test]
fn test_discover_files_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("empty_subdir");
    std::fs::create_dir_all(&subdir).unwrap();
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[subdir]).unwrap();
    assert!(
        violations.is_empty(),
        "Empty directory should yield no violations"
    );
}

#[test]
fn test_discover_files_non_py_file_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test_example.txt");
    std::fs::write(&path, "def test_foo(): pass").unwrap();
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[path]).unwrap();
    assert!(violations.is_empty(), "Non-.py files should be ignored");
}

#[test]
fn test_run_linter_with_color() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_color.py",
        r#"
def test_bad():
    pass
"#,
    );
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "terminal",
        None::<&std::path::Path>,
        false,
        Config::default(),
    )
    .unwrap();
    assert!(has_errors);
}

#[test]
fn test_run_linter_json_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_json_stdout.py",
        r#"
def test_ok():
    assert True
"#,
    );
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "json",
        None::<&std::path::Path>,
        true,
        Config::default(),
    )
    .unwrap();
    assert!(!has_errors);
}

#[test]
fn test_parse_file_only_comments() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_comments.py",
        r#"
# This is a comment
# Another comment
"#,
    );
    let mut parser = PythonParser::new().unwrap();
    let module = parser.parse_file(&path).unwrap();
    assert_eq!(module.test_functions.len(), 0);
    assert_eq!(module.fixtures.len(), 0);
}

#[test]
fn test_parse_non_test_functions_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mixed.py",
        r#"
def helper():
    return 42

class TestClass:
    pass

def test_real():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert_eq!(module.test_functions[0].name, "test_real");
}

#[test]
fn test_parse_test_uses_network() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_network_usage.py",
        r#"
import socket

def test_network():
    s = socket.socket()
    assert s
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_network);
    assert!(module.imports.iter().any(|imp| imp.contains("socket")));
}

#[test]
fn test_parse_test_no_network() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_network.py",
        r#"
def test_plain():
    assert 1 + 1 == 2
"#,
    );
    let module = parse_file(&path);
    assert!(!module.test_functions[0].uses_network);
}

#[test]
fn test_parse_multiple_parametrize() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_multi_param.py",
        r#"
import pytest

@pytest.mark.parametrize("x,y", [(1,2), (3,4), (5,6)])
def test_multi(x, y):
    assert x < y
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].is_parametrized);
    assert_eq!(module.test_functions[0].parametrize_count, Some(3));
}

#[test]
fn test_parse_parametrize_tuple_args() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_tuple_param.py",
        r#"
import pytest

@pytest.mark.parametrize("val", (1, 2, 3, 4))
def test_val(val):
    assert val > 0
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].is_parametrized);
    assert!(module.test_functions[0].parametrize_count.is_some());
    assert!(module.test_functions[0].parametrize_count.unwrap() >= 3);
}

#[test]
fn test_parse_parametrize_many_cases_triggers_explosion() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = r#"
import pytest

@pytest.mark.parametrize("val", ["#
        .to_string();
    for i in 0..25 {
        if i > 0 {
            content.push_str(", ");
        }
        content.push_str(&i.to_string());
    }
    content.push_str(
        r#"])
def test_many(val):
    assert val >= 0
"#,
    );
    let path = write_temp_file(dir.path(), "test_explosion.py", &content);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-003");
    assert!(v.is_some(), "Expected PYTEST-PARAM-003 for explosion");
}

#[test]
fn test_parse_fixture_no_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_body.py",
        r#"
import pytest

@pytest.fixture
def empty_fix():
    pass

def test_something():
    pass
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 1);
    assert!(!module.fixtures[0].returns_mutable);
    assert!(!module.fixtures[0].has_yield);
}

#[test]
fn test_parse_fixture_with_dict_call() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_dict_call.py",
        r#"
import pytest

@pytest.fixture
def dict_fix():
    return dict()
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].returns_mutable);
}

#[test]
fn test_parse_fixture_with_list_call() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_list_call.py",
        r#"
import pytest

@pytest.fixture
def list_fix():
    return list()
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].returns_mutable);
}

#[test]
fn test_parse_test_uses_file_io_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_write_io.py",
        r#"
def test_write():
    f = open("out.txt")
    f.write("hello")
    f.close()
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_file_io);
}

#[test]
fn test_parse_test_uses_time_sleep_import() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_sleep_import.py",
        r#"
def test_sleep():
    sleep(2)
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_time_sleep);
}

#[test]
fn test_parse_test_no_time_sleep() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_sleep.py",
        r#"
def test_noop():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(!module.test_functions[0].uses_time_sleep);
}

#[test]
fn test_parse_fixture_module_scope() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mod_scope.py",
        r#"
import pytest

@pytest.fixture(scope="module")
def mod_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Module);
}

#[test]
fn test_parse_fixture_no_scope() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_default_scope.py",
        r#"
import pytest

@pytest.fixture
def default_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Function);
}

#[test]
fn test_parse_fixture_file_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix_io.py",
        r#"
import pytest

@pytest.fixture
def io_fix():
    f = open("data.txt")
    return f.read()
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].uses_file_io);
}

#[test]
fn test_parse_fixture_no_file_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix_no_io.py",
        r#"
import pytest

@pytest.fixture
def noio_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert!(!module.fixtures[0].uses_file_io);
}

#[test]
fn test_fixture_scope_by_name_found() {
    use pytest_linter::engine::{collect_all_fixtures, fixture_scope_by_name};
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_scope_lookup.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def my_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    let modules = [module];
    let fixtures = collect_all_fixtures(&modules);
    let scope = fixture_scope_by_name(&fixtures, "my_fix");
    assert_eq!(scope, Some(FixtureScope::Session));
}

#[test]
fn test_fixture_scope_by_name_not_found() {
    use pytest_linter::engine::{collect_all_fixtures, fixture_scope_by_name};
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_fix.py",
        r#"
def test_ok():
    assert True
"#,
    );
    let module = parse_file(&path);
    let modules = [module];
    let fixtures = collect_all_fixtures(&modules);
    let scope = fixture_scope_by_name(&fixtures, "nonexistent");
    assert_eq!(scope, None);
}

#[test]
fn test_is_fixture_used_by_test() {
    use pytest_linter::engine::is_fixture_used_by_any_test_or_fixture;
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_used.py",
        r#"
import pytest

@pytest.fixture
def used_fix():
    return 42

def test_thing(used_fix):
    assert used_fix == 42
"#,
    );
    let module = parse_file(&path);
    assert!(is_fixture_used_by_any_test_or_fixture(
        "used_fix",
        &[module]
    ));
}

#[test]
fn test_is_fixture_not_used() {
    use pytest_linter::engine::is_fixture_used_by_any_test_or_fixture;
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_not_used.py",
        r#"
import pytest

@pytest.fixture
def unused_fix():
    return 42

def test_thing():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(!is_fixture_used_by_any_test_or_fixture(
        "unused_fix",
        &[module]
    ));
}

#[test]
fn test_is_fixture_used_by_other_fixture() {
    use pytest_linter::engine::is_fixture_used_by_any_test_or_fixture;
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix_dep.py",
        r#"
import pytest

@pytest.fixture
def base_fix():
    return 1

@pytest.fixture
def derived_fix(base_fix):
    return base_fix + 1
"#,
    );
    let module = parse_file(&path);
    assert!(is_fixture_used_by_any_test_or_fixture(
        "base_fix",
        &[module]
    ));
}

#[test]
fn test_file_io_with_tmpdir_does_not_trigger_flk002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_tmpdir.py",
        r#"
def test_reads_file(tmpdir):
    f = open(str(tmpdir / "data.txt"))
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-002");
    assert!(
        v.is_none(),
        "Should not trigger FLK-002 when tmpdir is used"
    );
}

#[test]
fn test_file_io_with_tmp_path_factory_does_not_trigger_flk002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_factory.py",
        r#"
def test_reads_file(tmp_path_factory):
    f = open(str(tmp_path_factory / "data.txt"))
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-002");
    assert!(v.is_none());
}

#[test]
fn test_network_import_httpx_triggers_flk003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_httpx.py",
        r#"
import httpx

def test_api():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-003");
    assert!(v.is_some());
}

#[test]
fn test_network_import_urllib_triggers_flk003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_urllib.py",
        r#"
import urllib.request

def test_api():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-003");
    assert!(v.is_some());
}

#[test]
fn test_no_network_import_does_not_trigger_flk003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_net.py",
        r#"
import os
import sys

def test_ok():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-003");
    assert!(v.is_none());
}

#[test]
fn test_mystery_guest_with_tmp_path_does_not_trigger_flk005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mystery_ok.py",
        r#"
def test_reads_file(tmp_path):
    f = open(str(tmp_path / "data.txt"))
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-005");
    assert!(v.is_none());
}

#[test]
fn test_bdd_with_when_does_not_trigger_bdd001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_bdd_when.py",
        r#"
def test_with_when():
    """When something happens."""
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-BDD-001");
    assert!(v.is_none());
}

#[test]
fn test_bdd_with_then_does_not_trigger_bdd001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_bdd_then.py",
        r#"
def test_with_then():
    """Then something is verified."""
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-BDD-001");
    assert!(v.is_none());
}

#[test]
fn test_property_test_hint_non_parametrized_does_not_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_param.py",
        r#"
def test_plain():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PBT-001");
    assert!(v.is_none());
}

#[test]
fn test_parametrize_empty_zero_count_triggers_param001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param_zero.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [])
def test_empty(val):
    assert val
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-001");
    assert!(v.is_some());
}

#[test]
fn test_parametrize_many_does_not_trigger_explosion() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param_ok_count.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
def test_vals(val):
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-003");
    assert!(v.is_none(), "10 cases should not trigger explosion");
}

#[test]
fn test_db_commit_with_rollback_does_not_trigger_fix008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_db_safe.py",
        r#"
import pytest

@pytest.fixture
def db_safe():
    conn = get_conn()
    conn.commit()
    conn.rollback()
    return conn
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-008");
    assert!(v.is_none(), "DB fixture with rollback should not trigger");
}

#[test]
fn test_session_fixture_no_mutable_does_not_trigger_fix006() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_session_immutable.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def immutable_fix():
    return 42

def test_uses(immutable_fix):
    assert immutable_fix == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-006");
    assert!(v.is_none());
}

#[test]
fn test_valid_scope_same_scope_does_not_trigger_fix003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_same_scope.py",
        r#"
import pytest

@pytest.fixture
def base():
    return 1

@pytest.fixture
def derived(base):
    return base + 1

def test_thing(derived):
    assert derived == 2
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-003");
    assert!(v.is_none(), "Same scope fixtures should not trigger");
}

#[test]
fn test_unused_fixture_used_by_test_does_not_trigger_fix005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_used_fix.py",
        r#"
import pytest

@pytest.fixture
def used_fix():
    return 42

def test_thing(used_fix):
    assert used_fix == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-005");
    assert!(v.is_none());
}

#[test]
fn test_format_terminal_with_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_warning_out.py",
        r#"
import time

def test_waits():
    time.sleep(1)
    assert True
"#,
    );
    let output_path = dir.path().join("warning_output.txt");
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "terminal",
        Some(&output_path),
        true,
        Config::default(),
    )
    .unwrap();
    assert!(!has_errors);
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("WARNING"));
    assert!(content.contains("Summary"));
}

#[test]
fn test_fixture_with_self_and_cls_params_excluded() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_class_fixtures.py",
        r#"
import pytest

@pytest.fixture
def my_fixture(self, cls, real_dep):
    return real_dep

def test_thing(my_fixture):
    assert my_fixture
"#,
    );
    let module = parse_file(&path);
    let fix = module
        .fixtures
        .iter()
        .find(|f| f.name == "my_fixture")
        .unwrap();
    assert_eq!(fix.dependencies.len(), 1);
    assert_eq!(fix.dependencies[0], "real_dep");
}

#[test]
fn test_fixture_with_scope_single_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_single_quotes.py",
        r#"
import pytest

@pytest.fixture(scope='session')
def session_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Session);
}

#[test]
fn test_fixture_class_scope_single_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_class_sq.py",
        r#"
import pytest

@pytest.fixture(scope='class')
def class_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Class);
}

#[test]
fn test_fixture_module_scope_single_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mod_sq.py",
        r#"
import pytest

@pytest.fixture(scope='module')
def mod_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Module);
}

#[test]
fn test_fixture_package_scope_single_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_pkg_sq.py",
        r#"
import pytest

@pytest.fixture(scope='package')
def pkg_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Package);
}

#[test]
fn test_fixture_session_scope_single_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_sess_sq.py",
        r#"
import pytest

@pytest.fixture(scope='session')
def sess_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Session);
}

#[test]
fn test_compute_used_fixture_names() {
    use pytest_linter::engine::compute_used_fixture_names;
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_used_names.py",
        r#"
import pytest

@pytest.fixture
def fix_a():
    return 1

@pytest.fixture
def fix_b(fix_a):
    return fix_a + 1

def test_thing(fix_b):
    assert fix_b == 2
"#,
    );
    let module = parse_file(&path);
    let used = compute_used_fixture_names(&[module]);
    assert!(used.contains("fix_b"));
    assert!(used.contains("fix_a"));
}

#[test]
fn test_collect_all_fixtures_empty() {
    use pytest_linter::engine::collect_all_fixtures;
    let modules: Vec<pytest_linter::models::ParsedModule> = vec![];
    let fixtures = collect_all_fixtures(&modules);
    assert!(fixtures.is_empty());
}

#[test]
fn test_parse_file_with_from_import() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_from_import.py",
        r#"
from os.path import join
from collections import OrderedDict

def test_ok():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.imports.iter().any(|imp| imp.contains("os.path")));
    assert!(module.imports.iter().any(|imp| imp.contains("collections")));
}

#[test]
fn test_parametrize_count_ast_empty_list_with_comma() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_empty_list.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [,])
def test_empty(val):
    assert val
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].is_parametrized);
}

#[test]
fn test_parse_decorator_no_node_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_bare_fixture.py",
        r#"
@fixture
def my_fix():
    return 42

def test_thing(my_fix):
    assert my_fix == 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures.len(), 1);
    assert_eq!(module.fixtures[0].name, "my_fix");
}

#[test]
fn test_parse_test_with_mock_call_count() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mock_count.py",
        r#"
def test_mock():
    mock_obj.call_count
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].has_mock_verifications);
    assert!(!module.test_functions[0].has_state_assertions);
}

#[test]
fn test_parse_test_with_mock_called() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mock_called.py",
        r#"
def test_mock():
    mock_obj.called
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].has_mock_verifications);
}

#[test]
fn test_parse_test_with_mock_assert_called_with_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mock_and_state.py",
        r#"
def test_both():
    mock_obj.assert_called()
    assert 1 == 1
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].has_mock_verifications);
    assert!(module.test_functions[0].has_state_assertions);
    assert!(module.test_functions[0].has_assertions);
}

#[test]
fn test_engine_multiple_files_lint() {
    let dir = tempfile::tempdir().unwrap();
    let path1 = write_temp_file(
        dir.path(),
        "test_multi_a.py",
        r#"
import time

def test_sleep():
    time.sleep(1)
    assert True
"#,
    );
    let path2 = write_temp_file(
        dir.path(),
        "test_multi_b.py",
        r#"
def test_no_assert():
    pass
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[path1, path2]).unwrap();
    let rule_ids: Vec<&str> = violations.iter().map(|v| v.rule_id.as_str()).collect();
    assert!(rule_ids.contains(&"PYTEST-FLK-001"));
    assert!(rule_ids.contains(&"PYTEST-MNT-004"));
}

#[test]
fn test_engine_non_existent_file_graceful() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nonexistent_test.py");
    let engine = LintEngine::new(Config::default()).unwrap();
    let result = engine.lint_paths(&[missing]);
    assert!(result.is_ok());
}

#[test]
fn test_format_json_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_json_stdout2.py",
        r#"
def test_bad():
    pass
"#,
    );
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "json",
        None::<&std::path::Path>,
        true,
        Config::default(),
    )
    .unwrap();
    assert!(has_errors);
}

#[test]
fn test_fixture_db_commit_no_rollback_no_yield_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_db_commit_only.py",
        r#"
import pytest

@pytest.fixture
def db_only_commit():
    conn.commit()
    return 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-008");
    assert!(v.is_some());
}

#[test]
fn test_fixture_db_rollback_present_does_not_trigger_fix008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_db_rollback_ok.py",
        r#"
import pytest

@pytest.fixture
def db_with_rollback():
    conn.commit()
    conn.rollback()
    return 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-008");
    assert!(v.is_none());
}

#[test]
fn test_network_import_aiohttp_triggers_flk003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_aiohttp.py",
        r#"
import aiohttp

def test_api():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-003");
    assert!(v.is_some());
}

#[test]
fn test_network_import_socket_triggers_flk003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_socket.py",
        r#"
import socket

def test_api():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-003");
    assert!(v.is_some());
}

#[test]
fn test_parse_parametrize_text_only_decorator() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_text_param.py",
        r#"
import pytest

@pytest.mark.parametrize(
    "x",
    [
        1,
        (2, 3),
    ],
)
def test_nested(x):
    assert x
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].is_parametrized);
}

#[test]
fn test_parse_parametrize_with_nested_brackets() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_nested_param.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [{"a": 1}, {"b": 2}])
def test_dict_val(val):
    assert val
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].is_parametrized);
    assert!(module.test_functions[0].parametrize_count.unwrap() >= 1);
}

#[test]
fn test_parse_fixture_with_scope_but_no_match() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_scope_nomatch.py",
        r#"
import pytest

@pytest.fixture(scope="invalid")
def weird_fix():
    return 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.fixtures[0].scope, FixtureScope::Function);
}

#[test]
fn test_parse_test_file_io_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_io_patterns.py",
        r#"
def test_io():
    f = open("file.txt")
    data = f.read()
    f.write("hello")
    f.close()
    assert data
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_file_io);
}

#[test]
fn test_parse_test_no_file_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_io.py",
        r#"
def test_plain():
    x = 42
    assert x == 42
"#,
    );
    let module = parse_file(&path);
    assert!(!module.test_functions[0].uses_file_io);
}

#[test]
fn test_parse_test_uses_file_io_open_call() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_open_call.py",
        r#"
def test_open():
    with open("data.txt") as f:
        assert f.read()
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_file_io);
}

#[test]
fn test_parse_test_try_except_in_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_try_body.py",
        r#"
def test_with_try():
    try:
        do_something()
    except Exception:
        pass
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].has_try_except);
}

#[test]
fn test_parse_test_no_try_except() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_try.py",
        r#"
def test_no_try():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(!module.test_functions[0].has_try_except);
}

#[test]
fn test_parse_test_with_if_in_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_if_body.py",
        r#"
def test_if():
    x = 5
    if x > 3:
        assert True
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].has_conditional_logic);
}

#[test]
fn test_parse_test_no_conditional() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_cond.py",
        r#"
def test_no_cond():
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(!module.test_functions[0].has_conditional_logic);
}

#[test]
fn test_parse_test_docstring_with_given() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_docstring_given.py",
        r#"
def test_given():
    """Given a setup."""
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-BDD-001");
    assert!(v.is_none());
}

#[test]
fn test_parse_test_no_docstring() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_docstring.py",
        r#"
def test_plain():
    x = 42
    assert x == 42
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].docstring.is_none());
}

#[test]
fn test_parse_test_with_assert_parens() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_assert_parens.py",
        r#"
def test_parens():
    assert(1 == 1)
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].has_assertions);
    assert_eq!(module.test_functions[0].assertion_count, 1);
}

#[test]
fn test_parse_fixture_with_db_rollback_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_rollback_only.py",
        r#"
import pytest

@pytest.fixture
def rollback_only():
    conn.rollback()
    return 42
"#,
    );
    let module = parse_file(&path);
    assert!(!module.fixtures[0].has_db_commit);
    assert!(module.fixtures[0].has_db_rollback);
}

#[test]
fn test_parse_fixture_with_commit_uppercase() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_commit_upper.py",
        r#"
import pytest

@pytest.fixture
def upper_commit():
    COMMIT
    return 42
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].has_db_commit);
}

#[test]
fn test_parse_fixture_with_rollback_uppercase() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_rollback_upper.py",
        r#"
import pytest

@pytest.fixture
def upper_rollback():
    ROLLBACK
    return 42
"#,
    );
    let module = parse_file(&path);
    assert!(module.fixtures[0].has_db_rollback);
}

#[test]
fn test_parse_test_uses_file_io_dot_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_dot_open.py",
        r#"
def test_dot_open():
    pathlib.Path("data.txt").open()
    assert True
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_file_io);
}

#[test]
fn test_fixture_autouse_false_does_not_trigger_fix001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_autouse_false.py",
        r#"
import pytest

@pytest.fixture(autouse=False)
def not_auto():
    return 42

def test_thing(not_auto):
    assert not_auto == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-001");
    assert!(v.is_none());
}

#[test]
fn test_fixture_scope_narrower_than_dep_does_not_trigger_fix003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_narrower_scope.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def base_fix():
    return 42

@pytest.fixture
def func_fix(base_fix):
    return base_fix

def test_thing(func_fix):
    assert func_fix == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-003");
    assert!(v.is_none(), "Narrower scope should not trigger");
}

#[test]
fn test_fixture_scope_wider_than_dep_triggers_fix003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_wider_scope.py",
        r#"
import pytest

@pytest.fixture(scope="module")
def mod_fix():
    return 42

@pytest.fixture(scope="session")
def sess_fix(mod_fix):
    return mod_fix

def test_thing(sess_fix):
    assert sess_fix == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-003");
    assert!(v.is_some());
}

#[test]
fn test_magic_assert_detected_in_assertions() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_magic.py",
        r#"
def test_magic():
    assert True
    assert 1
    assert 42 == 42
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    let test = &module.test_functions[0];
    assert_eq!(test.assertions.len(), 3);
    assert!(test.assertions[0].is_magic);
    assert!(test.assertions[1].is_magic);
    assert!(!test.assertions[2].is_magic);
    assert!(test.assertions[2].has_comparison);
}

#[test]
fn test_suboptimal_assert_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_suboptimal.py",
        r#"
def test_suboptimal():
    assert len(items) == 3
    assert type(x) == int
    assert result is not None
    assert value == expected
"#,
    );
    let module = parse_file(&path);
    let test = &module.test_functions[0];
    assert_eq!(test.assertions.len(), 4);
    assert!(
        test.assertions[0].is_suboptimal,
        "assert len(x)==N should be suboptimal"
    );
    assert!(
        test.assertions[1].is_suboptimal,
        "assert type(x)==Y should be suboptimal"
    );
    assert!(
        !test.assertions[2].is_suboptimal,
        "assert x is not None should NOT be suboptimal"
    );
    assert!(
        !test.assertions[3].is_suboptimal,
        "normal comparison should not be suboptimal"
    );
}

#[test]
fn test_cwd_dependency_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_cwd.py",
        r#"
import os

def test_cwd():
    cwd = os.getcwd()
    assert cwd
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_cwd_dependency);
}

#[test]
fn test_no_cwd_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_cwd.py",
        r#"
def test_normal():
    assert 1 == 1
"#,
    );
    let module = parse_file(&path);
    assert!(!module.test_functions[0].uses_cwd_dependency);
}

#[test]
fn test_pytest_raises_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_raises.py",
        r#"
import pytest

def test_error():
    with pytest.raises(ValueError):
        raise ValueError("bad")
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_pytest_raises);
}

#[test]
fn test_fixture_mutation_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mutation.py",
        r#"
def test_mutates(my_list):
    my_list.append(1)
    assert len(my_list) == 1
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0]
        .mutates_fixture_deps
        .contains(&"my_list".to_string()));
}

#[test]
fn test_no_fixture_mutation_on_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_readonly.py",
        r#"
def test_read_only(my_list):
    x = my_list[0]
    assert x
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].mutates_fixture_deps.is_empty());
}

#[test]
fn test_parametrize_values_extracted() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param_values.py",
        r#"
import pytest

@pytest.mark.parametrize("x", [1, 2, 2, 3])
def test_dup(x):
    assert x > 0
"#,
    );
    let module = parse_file(&path);
    let test = &module.test_functions[0];
    assert_eq!(test.parametrize_values.len(), 1);
    assert_eq!(test.parametrize_values[0], vec!["1", "2", "2", "3"]);
}

#[test]
fn test_fixture_scope_unknown_dep_does_not_trigger_fix003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_unknown_dep.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def unknown_dep(builtin_fixture):
    return builtin_fixture

def test_thing(unknown_dep):
    assert unknown_dep
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-003");
    assert!(v.is_none(), "Unknown deps should not trigger scope check");
}

#[test]
fn test_overly_broad_scope_triggers_fix009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def simple_value():
    return 42

@pytest.fixture(scope="module")
def another_simple():
    return "hello"
"#,
    );
    let violations = lint_single_file(&path);
    let v1 = find_violation(&violations, "PYTEST-FIX-009");
    assert!(
        v1.is_some(),
        "Expected PYTEST-FIX-009 for session-scoped simple fixture"
    );
}

#[test]
fn test_broad_scope_with_expensive_setup_does_not_trigger_fix009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def db_connection():
    conn = create_engine(DB_URL)
    yield conn
    conn.close()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-009");
    assert!(
        v.is_none(),
        "Session-scoped fixture with yield should not trigger FIX-009"
    );
}

#[test]
fn test_cwd_dependency_triggers_flk004() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_cwd_flaky.py",
        r#"
import os

def test_cwd():
    os.getcwd()
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-004");
    assert!(v.is_some(), "Expected PYTEST-FLK-004 violation");
    assert_eq!(v.unwrap().rule_name, "CwdDependencyRule");
}

#[test]
fn test_no_cwd_dependency_does_not_trigger_flk004() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_cwd_safe.py",
        r#"
def test_safe():
    assert 1 == 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-004");
    assert!(v.is_none());
}

#[test]
fn test_no_contract_hint_triggers_dbc001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_happy_path.py",
        r#"
def test_happy_only():
    result = 1 + 2
    assert result == 3
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-DBC-001");
    assert!(
        v.is_some(),
        "Expected PYTEST-DBC-001 for happy-path-only test"
    );
}

#[test]
fn test_with_pytest_raises_does_not_trigger_dbc001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_with_error.py",
        r#"
import pytest

def test_error():
    with pytest.raises(ValueError):
        raise ValueError("bad")
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-DBC-001");
    assert!(
        v.is_none(),
        "Test with pytest.raises should not trigger DBC-001"
    );
}

#[test]
fn test_parametrized_does_not_trigger_dbc001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param_dbc.py",
        r#"
import pytest

@pytest.mark.parametrize("x", [1, 2, 3])
def test_values(x):
    assert x > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-DBC-001");
    assert!(v.is_none(), "Parametrized tests should not trigger DBC-001");
}

#[test]
fn test_fixture_mutation_triggers_fix007() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture
def items():
    return [1, 2, 3]
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_mutate.py",
        r#"
def test_mutates(items):
    items.append(4)
    assert len(items) == 4
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-FIX-007");
    assert!(v.is_some(), "Expected PYTEST-FIX-007 violation");
}

#[test]
fn test_no_fixture_mutation_does_not_trigger_fix007() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture
def items():
    return [1, 2, 3]
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_safe_fix.py",
        r#"
def test_read_only(items):
    assert len(items) == 3
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-FIX-007");
    assert!(
        v.is_none(),
        "Read-only fixture usage should not trigger FIX-007"
    );
}

#[test]
fn test_magic_assert_triggers_mnt002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_magic_assert_mnt.py",
        r#"
def test_magic():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-002");
    assert!(v.is_some(), "Expected PYTEST-MNT-002 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "MagicAssertRule");
    assert!(v.message.contains("Magic assertion"));
}

#[test]
fn test_normal_assert_does_not_trigger_mnt002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_normal_assert.py",
        r#"
def test_normal():
    assert 1 + 2 == 3
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-002");
    assert!(v.is_none());
}

#[test]
fn test_suboptimal_assert_triggers_mnt003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_suboptimal_mnt.py",
        r#"
def test_subopt():
    assert len(items) == 3
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-003");
    assert!(v.is_some(), "Expected PYTEST-MNT-003 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "SuboptimalAssertRule");
    assert!(v.message.contains("Suboptimal"));
}

#[test]
fn test_good_assert_does_not_trigger_mnt003() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_good_assert_mnt.py",
        r#"
def test_good():
    x = 1 + 2
    assert x == 3
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-003");
    assert!(v.is_none());
}

#[test]
fn test_xdist_shared_state_triggers_xdist001() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def shared_list():
    return []
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_shared_xdist.py",
        r#"
def test_mutates_shared(shared_list):
    shared_list.append(1)
    assert len(shared_list) == 1
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-XDIST-001");
    assert!(v.is_some(), "Expected PYTEST-XDIST-001 violation");
}

#[test]
fn test_function_scope_mutable_does_not_trigger_xdist001() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture
def local_list():
    return []
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_local_xdist.py",
        r#"
def test_mutates_local(local_list):
    local_list.append(1)
    assert len(local_list) == 1
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-XDIST-001");
    assert!(
        v.is_none(),
        "Function-scoped fixture should not trigger XDIST-001"
    );
}

#[test]
fn test_parametrize_duplicate_triggers_param002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_dup_param.py",
        r#"
import pytest

@pytest.mark.parametrize("x", [1, 2, 2, 3])
def test_dup(x):
    assert x > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-002");
    assert!(v.is_some(), "Expected PYTEST-PARAM-002 violation");
    assert!(v.unwrap().message.contains("duplicate"));
}

#[test]
fn test_parametrize_no_duplicate_does_not_trigger_param002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_dup_param.py",
        r#"
import pytest

@pytest.mark.parametrize("x", [1, 2, 3])
def test_unique(x):
    assert x > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-002");
    assert!(v.is_none());
}

#[test]
fn test_noqa_suppresses_specific_rule() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_specific.py",
        r#"
def test_no_assert():  # noqa: PYTEST-MNT-004
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-004");
    assert!(v.is_none(), "noqa should suppress PYTEST-MNT-004");
}

#[test]
fn test_noqa_suppresses_all_rules() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_all.py",
        r#"
def test_no_assert():  # noqa
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-004");
    assert!(v.is_none(), "bare noqa should suppress all rules");
}

#[test]
fn test_noqa_does_not_suppress_other_rules() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_other.py",
        r#"
def test_no_assert():  # noqa: PYTEST-FLK-001
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-004");
    assert!(v.is_some(), "noqa for FLK-001 should not suppress MNT-004");
}

#[test]
fn test_noqa_multiple_rules() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_multi.py",
        r#"
def test_no_assert():  # noqa: PYTEST-MNT-004, PYTEST-BDD-001
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v1 = find_violation(&violations, "PYTEST-MNT-004");
    let v2 = find_violation(&violations, "PYTEST-BDD-001");
    assert!(v1.is_none(), "noqa should suppress PYTEST-MNT-004");
    assert!(v2.is_none(), "noqa should suppress PYTEST-BDD-001");
}

#[test]
fn test_noqa_on_previous_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_prev.py",
        r#"
# noqa: PYTEST-MNT-004
def test_no_assert():
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-004");
    assert!(
        v.is_none(),
        "noqa on previous line should suppress violation"
    );
}

#[test]
fn test_save_and_load_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let baseline_path = dir.path().join("baseline.json");

    let violations = vec![
        pytest_linter::models::Violation {
            rule_id: "PYTEST-MNT-004".to_string(),
            rule_name: "NoAssertionRule".to_string(),
            severity: Severity::Error,
            category: Category::Maintenance,
            message: "no assertions".to_string(),
            file_path: PathBuf::from("test_foo.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        },
        pytest_linter::models::Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "TimeSleepRule".to_string(),
            severity: Severity::Warning,
            category: Category::Flakiness,
            message: "time.sleep".to_string(),
            file_path: PathBuf::from("test_bar.py"),
            line: 10,
            col: None,
            suggestion: None,
            test_name: None,
        },
    ];

    pytest_linter::engine::save_baseline(&violations, &baseline_path).unwrap();
    let loaded = pytest_linter::engine::load_baseline(&baseline_path).unwrap();

    assert_eq!(loaded.len(), 2);
    assert!(loaded.contains(&("test_foo.py".to_string(), 5, "PYTEST-MNT-004".to_string())));
    assert!(loaded.contains(&("test_bar.py".to_string(), 10, "PYTEST-FLK-001".to_string())));
}

#[test]
fn test_load_baseline_invalid_json() {
    let dir = tempfile::tempdir().unwrap();
    let baseline_path = dir.path().join("bad_baseline.json");
    std::fs::write(&baseline_path, "not valid json{{{").unwrap();

    let result = pytest_linter::engine::load_baseline(&baseline_path);
    assert!(result.is_err(), "Should fail on invalid JSON");
}

#[test]
fn test_load_baseline_missing_file() {
    let result = pytest_linter::engine::load_baseline(Path::new("/nonexistent/baseline.json"));
    assert!(result.is_err(), "Should fail on missing file");
}

#[test]
fn test_filter_new_violations_empty_baseline() {
    let violations = vec![pytest_linter::models::Violation {
        rule_id: "PYTEST-MNT-004".to_string(),
        rule_name: "NoAssertionRule".to_string(),
        severity: Severity::Error,
        category: Category::Maintenance,
        message: "no assertions".to_string(),
        file_path: PathBuf::from("test_foo.py"),
        line: 5,
        col: None,
        suggestion: None,
        test_name: None,
    }];

    let baseline = std::collections::HashSet::new();
    let filtered = pytest_linter::engine::filter_new_violations(&violations, &baseline);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].rule_id, "PYTEST-MNT-004");
}

#[test]
fn test_filter_new_violations_with_baseline_match() {
    let violations = vec![
        pytest_linter::models::Violation {
            rule_id: "PYTEST-MNT-004".to_string(),
            rule_name: "NoAssertionRule".to_string(),
            severity: Severity::Error,
            category: Category::Maintenance,
            message: "no assertions".to_string(),
            file_path: PathBuf::from("test_foo.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        },
        pytest_linter::models::Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "TimeSleepRule".to_string(),
            severity: Severity::Warning,
            category: Category::Flakiness,
            message: "time.sleep".to_string(),
            file_path: PathBuf::from("test_bar.py"),
            line: 10,
            col: None,
            suggestion: None,
            test_name: None,
        },
    ];

    let mut baseline = std::collections::HashSet::new();
    baseline.insert(("test_foo.py".to_string(), 5, "PYTEST-MNT-004".to_string()));

    let filtered = pytest_linter::engine::filter_new_violations(&violations, &baseline);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].rule_id, "PYTEST-FLK-001");
}

#[test]
fn test_filter_new_violations_all_in_baseline() {
    let violations = vec![pytest_linter::models::Violation {
        rule_id: "PYTEST-MNT-004".to_string(),
        rule_name: "NoAssertionRule".to_string(),
        severity: Severity::Error,
        category: Category::Maintenance,
        message: "no assertions".to_string(),
        file_path: PathBuf::from("test_foo.py"),
        line: 5,
        col: None,
        suggestion: None,
        test_name: None,
    }];

    let mut baseline = std::collections::HashSet::new();
    baseline.insert(("test_foo.py".to_string(), 5, "PYTEST-MNT-004".to_string()));

    let filtered = pytest_linter::engine::filter_new_violations(&violations, &baseline);
    assert!(filtered.is_empty());
}

#[test]
fn test_save_baseline_empty_violations() {
    let dir = tempfile::tempdir().unwrap();
    let baseline_path = dir.path().join("empty_baseline.json");

    pytest_linter::engine::save_baseline(&[], &baseline_path).unwrap();
    let content = std::fs::read_to_string(&baseline_path).unwrap();
    assert_eq!(content.trim(), "[]");
}

#[test]
fn test_collect_violations_function() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_collect.py",
        r#"
def test_no_assert():
    pass
"#,
    );
    let violations = pytest_linter::engine::collect_violations(&[path], Config::default()).unwrap();
    assert!(!violations.is_empty());
    assert!(violations.iter().any(|v| v.rule_id == "PYTEST-MNT-004"));
}

#[test]
fn test_format_json_output_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("out.json");
    let violations = vec![pytest_linter::models::Violation {
        rule_id: "PYTEST-MNT-004".to_string(),
        rule_name: "NoAssertionRule".to_string(),
        severity: Severity::Error,
        category: Category::Maintenance,
        message: "no assertions".to_string(),
        file_path: PathBuf::from("test.py"),
        line: 1,
        col: None,
        suggestion: Some("add assert".to_string()),
        test_name: Some("test_foo".to_string()),
    }];

    pytest_linter::engine::format_json_output(&violations, Some(&output_path)).unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[test]
fn test_format_sarif_output_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("out.sarif");
    let violations = vec![pytest_linter::models::Violation {
        rule_id: "PYTEST-MNT-004".to_string(),
        rule_name: "NoAssertionRule".to_string(),
        severity: Severity::Error,
        category: Category::Maintenance,
        message: "no assertions".to_string(),
        file_path: PathBuf::from("test.py"),
        line: 1,
        col: None,
        suggestion: None,
        test_name: None,
    }];

    pytest_linter::engine::format_sarif_output(&violations, Some(&output_path)).unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("$schema").is_some());
}

#[test]
fn test_format_terminal_output_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("out.txt");
    let violations = vec![pytest_linter::models::Violation {
        rule_id: "PYTEST-MNT-004".to_string(),
        rule_name: "NoAssertionRule".to_string(),
        severity: Severity::Error,
        category: Category::Maintenance,
        message: "no assertions".to_string(),
        file_path: PathBuf::from("test.py"),
        line: 1,
        col: Some(5),
        suggestion: Some("add assert".to_string()),
        test_name: Some("test_foo".to_string()),
    }];

    pytest_linter::engine::format_terminal_output(&violations, Some(&output_path), true).unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("ERROR"));
    assert!(content.contains("PYTEST-MNT-004"));
    assert!(content.contains("Summary"));
}

#[test]
fn test_format_terminal_output_with_col_none() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("out2.txt");
    let violations = vec![pytest_linter::models::Violation {
        rule_id: "PYTEST-FLK-001".to_string(),
        rule_name: "TimeSleepRule".to_string(),
        severity: Severity::Warning,
        category: Category::Flakiness,
        message: "time.sleep".to_string(),
        file_path: PathBuf::from("test.py"),
        line: 3,
        col: None,
        suggestion: None,
        test_name: None,
    }];

    pytest_linter::engine::format_terminal_output(&violations, Some(&output_path), true).unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("WARNING"));
    assert!(content.contains("-"));
}

#[test]
fn test_format_terminal_output_info_severity() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("info.txt");
    let violations = vec![pytest_linter::models::Violation {
        rule_id: "PYTEST-BDD-001".to_string(),
        rule_name: "BddMissingScenarioRule".to_string(),
        severity: Severity::Info,
        category: Category::Enhancement,
        message: "missing BDD".to_string(),
        file_path: PathBuf::from("test.py"),
        line: 1,
        col: None,
        suggestion: None,
        test_name: None,
    }];

    pytest_linter::engine::format_terminal_output(&violations, Some(&output_path), true).unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("INFO"));
}

#[test]
fn test_format_terminal_empty_violations() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("empty.txt");

    pytest_linter::engine::format_terminal_output(&[], Some(&output_path), true).unwrap();
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("No violations found"));
}

#[test]
fn test_run_linter_sarif_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_sarif.py",
        r#"
def test_bad():
    pass
"#,
    );
    let output_path = dir.path().join("output.sarif");
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "sarif",
        Some(&output_path),
        true,
        Config::default(),
    )
    .unwrap();
    assert!(has_errors);
    let content = std::fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("$schema").is_some());
}

#[test]
fn test_run_linter_sarif_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_sarif_stdout.py",
        r#"
def test_ok():
    assert True
"#,
    );
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "sarif",
        None::<&std::path::Path>,
        true,
        Config::default(),
    )
    .unwrap();
    assert!(!has_errors);
}

#[test]
fn test_get_changed_files_invalid_base() {
    let result = pytest_linter::engine::get_changed_files("nonexistent_ref_12345");
    assert!(result.is_err(), "Should fail with invalid git ref");
}

#[test]
fn test_run_linter_terminal_no_color_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_color.py",
        r#"
def test_ok():
    assert True
"#,
    );
    let has_errors = pytest_linter::engine::run_linter(
        &[path],
        "terminal",
        None::<&std::path::Path>,
        false,
        Config::default(),
    )
    .unwrap();
    assert!(!has_errors);
}

#[test]
fn test_random_without_seed_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random.py",
        r#"
import random

def test_random_value():
    val = random.randint(1, 100)
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-008 for random without seed"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "RandomWithoutSeedRule");
    assert!(v.message.contains("random"));
}

#[test]
fn test_random_with_seed_does_not_trigger_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_seed.py",
        r#"
import random

def test_seeded_random():
    random.seed(42)
    val = random.randint(1, 100)
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(
        v.is_none(),
        "Should not trigger FLK-008 when random.seed is called"
    );
}

#[test]
fn test_random_choice_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_choice.py",
        r#"
import random

def test_random_choice():
    val = random.choice([1, 2, 3])
    assert val in [1, 2, 3]
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "Expected PYTEST-FLK-008 for random.choice");
}

#[test]
fn test_random_shuffle_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_shuffle.py",
        r#"
import random

def test_random_shuffle():
    items = [1, 2, 3]
    random.shuffle(items)
    assert len(items) == 3
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "Expected PYTEST-FLK-008 for random.shuffle");
}

#[test]
fn test_random_uniform_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_uniform.py",
        r#"
import random

def test_random_uniform():
    val = random.uniform(0.0, 1.0)
    assert 0 <= val <= 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "Expected PYTEST-FLK-008 for random.uniform");
}

#[test]
fn test_random_sample_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_sample.py",
        r#"
import random

def test_random_sample():
    val = random.sample(range(100), 5)
    assert len(val) == 5
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "Expected PYTEST-FLK-008 for random.sample");
}

#[test]
fn test_subprocess_without_timeout_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_subprocess.py",
        r#"
import subprocess

def test_subprocess_run():
    result = subprocess.run(["echo", "hello"], capture_output=True)
    assert result.returncode == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-009 for subprocess without timeout"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "SubprocessWithoutTimeoutRule");
    assert!(v.message.contains("timeout"));
}

#[test]
fn test_subprocess_with_timeout_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_subprocess_timeout.py",
        r#"
import subprocess

def test_subprocess_with_timeout():
    result = subprocess.run(["echo", "hello"], capture_output=True, timeout=30)
    assert result.returncode == 0
"#,
    );
    let module = parse_file(&path);
    assert!(module.test_functions[0].uses_subprocess);
    assert!(
        !module.test_functions[0].has_subprocess_timeout,
        "has_subprocess_timeout should be false when timeout is provided"
    );
}

#[test]
fn test_subprocess_popen_without_timeout_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_popen.py",
        r#"
import subprocess

def test_popen():
    proc = subprocess.Popen(["echo", "hello"])
    proc.wait()
    assert proc.returncode == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-009 for Popen without timeout"
    );
}

#[test]
fn test_subprocess_check_output_without_timeout_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_check_output.py",
        r#"
import subprocess

def test_check_output():
    output = subprocess.check_output(["echo", "hello"])
    assert output
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-009 for check_output without timeout"
    );
}

#[test]
fn test_subprocess_call_without_timeout_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_subprocess_call.py",
        r#"
import subprocess

def test_subprocess_call():
    ret = subprocess.call(["echo", "hello"])
    assert ret == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-009 for subprocess.call without timeout"
    );
}

#[test]
fn test_subprocess_check_call_without_timeout_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_check_call.py",
        r#"
import subprocess

def test_check_call():
    subprocess.check_call(["echo", "hello"])
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-009 for check_call without timeout"
    );
}

#[test]
fn test_no_random_does_not_trigger_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_random.py",
        r#"
def test_no_random():
    assert 1 + 1 == 2
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_none());
}

#[test]
fn test_no_subprocess_does_not_trigger_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_subprocess.py",
        r#"
def test_no_subprocess():
    assert 1 + 1 == 2
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(v.is_none());
}

#[test]
fn test_random_in_nested_function_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_nested_random.py",
        r#"
import random

def test_with_helper():
    def inner():
        return random.random()
    val = inner()
    assert 0 <= val <= 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-008 for random in nested function"
    );
}

#[test]
fn test_subprocess_in_nested_function_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_nested_subprocess.py",
        r#"
import subprocess

def test_with_helper():
    def inner():
        return subprocess.run(["echo", "hi"])
    result = inner()
    assert result.returncode == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-009 for subprocess in nested function"
    );
}

#[test]
fn test_random_qualified_call_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_qualified.py",
        r#"
import random

def test_random_random():
    val = random.random()
    assert 0 <= val <= 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "Expected PYTEST-FLK-008 for random.random()");
}

#[test]
fn test_subprocess_qualified_call_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_subprocess_qualified.py",
        r#"
import subprocess

def test_subprocess_run():
    result = subprocess.run(["echo", "hello"])
    assert result.returncode == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(v.is_some(), "Expected PYTEST-FLK-009 for subprocess.run()");
}

#[test]
fn test_discover_files_test_prefix_in_subdir() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    std::fs::create_dir_all(&subdir).unwrap();
    write_temp_file(
        &subdir,
        "test_sub.py",
        r#"
def test_in_subdir():
    assert True
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[dir.path().to_path_buf()]).unwrap();
    assert!(
        !violations.is_empty(),
        "Should discover test files in subdirectories"
    );
}

#[test]
fn test_make_violation_function() {
    use pytest_linter::engine::make_violation;
    let v = make_violation(
        "TEST-001",
        "TestRule",
        Severity::Warning,
        Category::Flakiness,
        "test message".to_string(),
        PathBuf::from("test.py"),
        42,
        Some("suggestion".to_string()),
        Some("test_name".to_string()),
    );
    assert_eq!(v.rule_id, "TEST-001");
    assert_eq!(v.rule_name, "TestRule");
    assert_eq!(v.severity, Severity::Warning);
    assert_eq!(v.category, Category::Flakiness);
    assert_eq!(v.message, "test message");
    assert_eq!(v.file_path, PathBuf::from("test.py"));
    assert_eq!(v.line, 42);
    assert_eq!(v.col, None);
    assert_eq!(v.suggestion.as_deref(), Some("suggestion"));
    assert_eq!(v.test_name.as_deref(), Some("test_name"));
}

#[test]
fn test_noqa_with_empty_rules_after_colon() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_empty.py",
        r#"
def test_no_assert():  # noqa:
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-004");
    assert!(v.is_none(), "noqa: with empty rules should suppress all");
}

#[test]
fn test_noqa_with_whitespace_rules() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_ws.py",
        r#"
def test_no_assert():  # noqa:   PYTEST-MNT-004  ,  PYTEST-BDD-001
    pass
"#,
    );
    let violations = lint_single_file(&path);
    let v1 = find_violation(&violations, "PYTEST-MNT-004");
    let v2 = find_violation(&violations, "PYTEST-BDD-001");
    assert!(v1.is_none());
    assert!(v2.is_none());
}

#[test]
fn test_noqa_suppresses_flk001_in_integration() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_flk001.py",
        r#"
import time

def test_sleep():  # noqa: PYTEST-FLK-001
    time.sleep(1)
"#,
    );
    let violations = lint_single_file(&path);
    let flk001 = find_violation(&violations, "PYTEST-FLK-001");
    assert!(flk001.is_none(), "noqa should suppress PYTEST-FLK-001");
}

#[test]
fn test_noqa_bare_suppresses_all() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_noqa_bare.py",
        r#"
import os

def test_cwd():  # noqa
    cwd = os.getcwd()
    assert cwd
"#,
    );
    let violations = lint_single_file(&path);
    let flk004 = find_violation(&violations, "PYTEST-FLK-004");
    assert!(
        flk004.is_none(),
        "bare # noqa should suppress all violations on that line"
    );
}

#[test]
fn test_lint_source_with_context_finds_fixture_issues() {
    let dir = tempfile::tempdir().unwrap();
    let ctx_path = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture
def shared_dep():
    return 42
"#,
    );
    let mut parser = PythonParser::new().unwrap();
    let ctx_module = parser.parse_file(&ctx_path).unwrap();

    let engine = LintEngine::new(Config::default()).unwrap();
    let source = r#"
import pytest

@pytest.fixture(scope="session")
def session_val():
    return 1
"#;
    let violations = engine
        .lint_source_with_context(source, Path::new("test_ctx.py"), &[ctx_module])
        .unwrap();
    let broad = find_violation(&violations, "PYTEST-FIX-009");
    assert!(
        broad.is_some(),
        "Should detect session-scoped fixture without expensive setup"
    );
}

#[test]
fn test_fixture_mutation_rule_name_fix007() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture
def items():
    return [1, 2, 3]
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_mutate_fix007.py",
        r#"
def test_mutates(items):
    items.append(4)
    assert len(items) == 4
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-FIX-007");
    assert!(v.is_some(), "Expected PYTEST-FIX-007 violation");
    assert_eq!(v.unwrap().rule_name, "FixtureMutationRule");
}

#[test]
fn test_overly_broad_scope_rule_name_fix009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def simple_value():
    return 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-009");
    assert!(v.is_some(), "Expected PYTEST-FIX-009 violation");
    assert_eq!(v.unwrap().rule_name, "FixtureOverlyBroadScopeRule");
}

#[test]
fn test_module_scope_fixture_mutated_triggers_fix010() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix010.py",
        r#"
import pytest

@pytest.fixture(scope="module")
def mod_list():
    return []

def test_mutates(mod_list):
    mod_list.append(1)
    assert len(mod_list) == 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-010");
    assert!(v.is_some(), "Expected PYTEST-FIX-010 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-FIX-010");
    assert_eq!(v.rule_name, "ModuleScopeFixtureMutatedRule");
    assert!(v.message.contains("module/session-scoped fixture"));
}

#[test]
fn test_function_scope_same_name_no_fix010() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture(scope="module")
def mod_list():
    return []
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_same_name_fix010.py",
        r#"
import pytest

@pytest.fixture
def mod_list():
    return []

def test_mutates(mod_list):
    mod_list.append(1)
    assert len(mod_list) == 1
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-FIX-010");
    assert!(
        v.is_none(),
        "Function-scoped same-file fixture should not trigger FIX-010"
    );
}

#[test]
fn test_yield_without_try_finally_triggers_fix011() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix011.py",
        r#"
import pytest

@pytest.fixture
def bare_yield():
    yield 42

def test_uses(bare_yield):
    assert bare_yield == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-011");
    assert!(v.is_some(), "Expected PYTEST-FIX-011 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-FIX-011");
    assert_eq!(v.rule_name, "YieldWithoutTryFinallyRule");
    assert!(v.message.contains("yield without try/finally"));
}

#[test]
fn test_fixture_shadows_builtin_list_triggers_fix012() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix012.py",
        r#"
import pytest

@pytest.fixture
def list():
    return [1, 2, 3]

def test_uses(list):
    assert len(list) == 3
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-012");
    assert!(v.is_some(), "Expected PYTEST-FIX-012 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-FIX-012");
    assert_eq!(v.rule_name, "FixtureNameShadowsBuiltinRule");
    assert!(v.message.contains("shadows a Python builtin"));
}

#[test]
fn test_no_contract_hint_rule_name_dbc001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_dbc001_name.py",
        r#"
def test_happy():
    result = 1 + 2
    assert result == 3
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-DBC-001");
    assert!(v.is_some(), "Expected PYTEST-DBC-001 violation");
    assert_eq!(v.unwrap().rule_name, "NoContractHintRule");
}

#[test]
fn test_cascade_depth_deep_triggers_fix013() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix013_deep.py",
        r#"
import pytest

@pytest.fixture
def dep_a():
    return 1

@pytest.fixture
def dep_b(dep_a):
    return dep_a + 1

@pytest.fixture
def dep_c(dep_b):
    return dep_b + 1

@pytest.fixture(autouse=True)
def deep_auto(dep_c):
    return dep_c + 1

def test_something():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-013");
    assert!(v.is_some(), "Expected PYTEST-FIX-013 for cascade depth > 3");
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-FIX-013");
    assert_eq!(v.rule_name, "AutouseCascadeDepthRule");
    assert!(
        v.message.contains("cascade depth of 4"),
        "Expected depth 4, got: {}",
        v.message
    );
}

#[test]
fn test_cascade_depth_three_no_trigger_fix013() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_fix013_ok.py",
        r#"
import pytest

@pytest.fixture
def dep_a():
    return 1

@pytest.fixture
def dep_b(dep_a):
    return dep_a + 1

@pytest.fixture(autouse=True)
def three_auto(dep_b):
    return dep_b + 1

def test_something():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FIX-013");
    assert!(v.is_none(), "Cascade depth of 3 should not trigger FIX-013");
}

#[test]
fn test_cascade_depth_cross_file_fix013() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture
def shared():
    return 1
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_cross_cascade.py",
        r#"
import pytest

@pytest.fixture
def dep_a():
    return 1

@pytest.fixture
def dep_b(dep_a):
    return dep_b + 1

@pytest.fixture
def dep_c(dep_b):
    return dep_b + 1

@pytest.fixture
def shared(dep_c):
    return dep_c + 1

@pytest.fixture(autouse=True)
def auto_cross(shared):
    return shared + 1

def test_something():
    assert True
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-FIX-013");
    assert!(
        v.is_some(),
        "Expected PYTEST-FIX-013 for cross-file cascade"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "AutouseCascadeDepthRule");
    assert!(
        v.message.contains("cascade depth of 5"),
        "Expected depth 5, got: {}",
        v.message
    );
}

#[test]
fn test_socket_without_timeout_triggers_flk010() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_socket_flk010.py",
        r#"
import socket

def test_socket_no_timeout():
    s = socket.socket()
    assert s
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-010");
    assert!(v.is_some(), "Expected PYTEST-FLK-010 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "SocketWithoutBindTimeoutRule");
    assert_eq!(v.severity, Severity::Warning);
    assert_eq!(v.category, Category::Flakiness);
    assert!(v.message.contains("socket"));
    assert!(v.message.contains("timeout"));
    assert!(v.suggestion.is_some());
    assert_eq!(v.test_name.as_ref().unwrap(), "test_socket_no_timeout");
}

#[test]
fn test_no_socket_import_does_not_trigger_flk010() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_socket.py",
        r#"
def test_no_network():
    assert 1 + 1 == 2
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-010");
    assert!(
        v.is_none(),
        "Should not trigger FLK-010 without socket import"
    );
}

#[test]
fn test_socket_import_no_network_usage_does_not_trigger_flk010() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_socket_not_used.py",
        r#"
import socket

def test_no_socket_usage():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-010");
    assert!(
        v.is_none(),
        "Should not trigger FLK-010 when socket imported but not used in test"
    );
}

#[test]
fn test_datetime_in_assertion_triggers_flk011() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_datetime_flk011.py",
        r#"
import datetime

def test_datetime_now():
    now = datetime.now()
    assert now is not None
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-011");
    assert!(v.is_some(), "Expected PYTEST-FLK-011 violation");
    let v = v.unwrap();
    assert_eq!(v.rule_name, "DatetimeInAssertionRule");
    assert_eq!(v.severity, Severity::Warning);
    assert_eq!(v.category, Category::Flakiness);
    assert!(v.message.contains("datetime"));
    assert!(v.message.contains("assertion"));
    assert!(v.suggestion.is_some());
    assert_eq!(v.test_name.as_ref().unwrap(), "test_datetime_now");
}

#[test]
fn test_no_datetime_import_does_not_trigger_flk011() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_datetime.py",
        r#"
def test_no_datetime():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-011");
    assert!(
        v.is_none(),
        "Should not trigger FLK-011 without datetime import"
    );
}

#[test]
fn test_datetime_no_assertions_does_not_trigger_flk011() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_datetime_no_assert.py",
        r#"
import datetime

def test_datetime_no_assert():
    now = datetime.now()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-011");
    assert!(v.is_none(), "Should not trigger FLK-011 when no assertions");
}

#[test]
fn test_xdist_shared_state_verifies_details_xdist001() {
    let dir = tempfile::tempdir().unwrap();
    let conftest = write_temp_file(
        dir.path(),
        "conftest.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def shared_list():
    return []
"#,
    );
    let test_path = write_temp_file(
        dir.path(),
        "test_xdist_details.py",
        r#"
def test_mutates_shared(shared_list):
    shared_list.append(1)
    assert len(shared_list) == 1
"#,
    );
    let engine = LintEngine::new(Config::default()).unwrap();
    let violations = engine.lint_paths(&[conftest, test_path]).unwrap();
    let v = find_violation(&violations, "PYTEST-XDIST-001").unwrap();
    assert_eq!(v.rule_name, "XdistSharedStateRule");
    assert_eq!(v.severity, Severity::Warning);
    assert_eq!(v.category, Category::Flakiness);
    assert!(v.message.contains("shared_list"));
    assert!(v.message.contains("mutable state"));
    assert!(v.message.contains("xdist"));
    assert!(v.suggestion.is_some());
    assert_eq!(v.test_name.as_ref().unwrap(), "test_mutates_shared");
}

#[test]
fn test_random_line_number_exact_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_line.py",
        r#"
import random

def test_random_line():
    val = random.randint(1, 100)
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008").unwrap();
    assert_eq!(
        v.line, 5,
        "Violation should be at random call line 5, not function def line 4"
    );
}

#[test]
fn test_subprocess_line_number_exact_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_subprocess_line.py",
        r#"
import subprocess

def test_subprocess_line():
    result = subprocess.run(["echo", "hello"])
    assert result.returncode == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009").unwrap();
    assert_eq!(
        v.line, 5,
        "Violation should be at subprocess call line 5, not function def line 4"
    );
}

#[test]
fn test_random_randrange_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_randrange.py",
        r#"
import random

def test_randrange():
    val = random.randrange(10)
    assert val >= 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "Expected PYTEST-FLK-008 for random.randrange");
}

#[test]
fn test_random_gauss_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_gauss.py",
        r#"
import random

def test_gauss():
    val = random.gauss(0, 1)
    assert isinstance(val, float)
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "Expected PYTEST-FLK-008 for random.gauss");
}

#[test]
fn test_random_normalvariate_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_normalvariate.py",
        r#"
import random

def test_normalvariate():
    val = random.normalvariate(0, 1)
    assert isinstance(val, float)
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(
        v.is_some(),
        "Expected PYTEST-FLK-008 for random.normalvariate"
    );
}

#[test]
fn test_multiple_random_calls_multiple_violations_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_multi_random.py",
        r#"
import random

def test_multi_random():
    a = random.randint(1, 10)
    b = random.choice([1, 2, 3])
    assert a != b
"#,
    );
    let violations = lint_single_file(&path);
    let flk008: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == "PYTEST-FLK-008")
        .collect();
    assert_eq!(
        flk008.len(),
        2,
        "Expected 2 FLK-008 violations for 2 random calls"
    );
    let lines: Vec<usize> = flk008.iter().map(|v| v.line).collect();
    assert!(lines.contains(&5), "First random call at line 5");
    assert!(lines.contains(&6), "Second random call at line 6");
}

#[test]
fn test_multiple_subprocess_calls_multiple_violations_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_multi_subprocess.py",
        r#"
import subprocess

def test_multi_subprocess():
    subprocess.run(["echo", "a"])
    subprocess.call(["echo", "b"])
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let flk009: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == "PYTEST-FLK-009")
        .collect();
    assert_eq!(
        flk009.len(),
        2,
        "Expected 2 FLK-009 violations for 2 subprocess calls"
    );
    let lines: Vec<usize> = flk009.iter().map(|v| v.line).collect();
    assert!(lines.contains(&5), "First subprocess call at line 5");
    assert!(lines.contains(&6), "Second subprocess call at line 6");
}

#[test]
fn test_random_parametrized_line_number_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param_random_line.py",
        r#"
import random
import pytest

@pytest.mark.parametrize("x", [1, 2])
def test_param_random(x):
    val = random.randint(1, 100)
    assert val > x
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008").unwrap();
    assert_eq!(
        v.line, 7,
        "Violation should be at random call line 7 for parametrized test"
    );
}

#[test]
fn test_subprocess_parametrized_line_number_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_param_subprocess_line.py",
        r#"
import subprocess
import pytest

@pytest.mark.parametrize("cmd", [["echo", "a"], ["echo", "b"]])
def test_param_subprocess(cmd):
    subprocess.run(cmd)
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009").unwrap();
    assert_eq!(
        v.line, 7,
        "Violation should be at subprocess call line 7 for parametrized test"
    );
}

#[test]
fn test_socket_multiple_tests_multiple_violations_flk010() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_multi_socket.py",
        r#"
import socket

def test_socket_a():
    s = socket.socket()
    assert s

def test_socket_b():
    s2 = socket.socket()
    assert s2
"#,
    );
    let violations = lint_single_file(&path);
    let flk010: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == "PYTEST-FLK-010")
        .collect();
    assert_eq!(
        flk010.len(),
        2,
        "Expected 2 FLK-010 violations for 2 tests with network"
    );
}

#[test]
fn test_datetime_multiple_tests_multiple_violations_flk011() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_multi_datetime.py",
        r#"
import datetime

def test_dt_a():
    now = datetime.now()
    assert now

def test_dt_b():
    later = datetime.now()
    assert later
"#,
    );
    let violations = lint_single_file(&path);
    let flk011: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == "PYTEST-FLK-011")
        .collect();
    assert_eq!(
        flk011.len(),
        2,
        "Expected 2 FLK-011 violations for 2 tests with datetime+assertions"
    );
}

#[test]
fn test_xdist_no_mutable_fixtures_no_trigger_xdist001() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_no_xdist.py",
        r#"
def test_plain():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-XDIST-001");
    assert!(
        v.is_none(),
        "Should not trigger XDIST-001 without mutable fixtures"
    );
}

#[test]
fn test_xdist_fixture_io_non_session_does_not_trigger_xdist002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_xdist_module.py",
        r#"
import pytest

@pytest.fixture(scope="module")
def module_data():
    f = open("data.txt")
    data = f.read()
    f.close()
    return data

def test_data(module_data):
    assert module_data
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-XDIST-002");
    assert!(
        v.is_none(),
        "Should not trigger XDIST-002 for module-scoped fixture"
    );
}

#[test]
fn test_xdist_fixture_io_no_file_io_does_not_trigger_xdist002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_xdist_no_io.py",
        r#"
import pytest

@pytest.fixture(scope="session")
def session_val():
    return 42

def test_val(session_val):
    assert session_val == 42
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-XDIST-002");
    assert!(
        v.is_none(),
        "Should not trigger XDIST-002 for session fixture without file I/O"
    );
}

#[test]
fn test_random_all_functions_line_numbers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_all_random.py",
        r#"
import random

def test_all_random():
    a = random.random()
    b = random.randint(1, 10)
    c = random.choice([1])
    d = random.shuffle([1, 2])
    e = random.uniform(0, 1)
    f = random.randrange(5)
    g = random.sample([1, 2], 1)
    h = random.gauss(0, 1)
    i = random.normalvariate(0, 1)
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let flk008: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == "PYTEST-FLK-008")
        .collect();
    assert_eq!(
        flk008.len(),
        9,
        "Expected 9 FLK-008 violations for all random functions"
    );
    let lines: Vec<usize> = flk008.iter().map(|v| v.line).collect();
    for expected_line in 5..=13 {
        assert!(
            lines.contains(&expected_line),
            "Expected violation at line {expected_line}, got lines {lines:?}"
        );
    }
}

#[test]
fn test_mystery_guest_with_tmpdir_does_not_trigger_flk005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mystery_tmpdir.py",
        r#"
def test_reads_file(tmpdir):
    f = open(str(tmpdir / "data.txt"))
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-005");
    assert!(
        v.is_none(),
        "Should not trigger FLK-005 when tmpdir is used"
    );
}

#[test]
fn test_mystery_guest_with_tmp_path_factory_does_not_trigger_flk005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mystery_factory.py",
        r#"
def test_reads_file(tmp_path_factory):
    f = open(str(tmp_path_factory / "data.txt"))
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-005");
    assert!(
        v.is_none(),
        "Should not trigger FLK-005 when tmp_path_factory is used"
    );
}

#[test]
fn test_random_betavariate_line_number_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_beta.py",
        r#"
import random

def test_beta():
    val = random.betavariate(1, 2)
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008").unwrap();
    assert_eq!(
        v.line, 5,
        "Violation should be at betavariate call line 5, not function def line 4"
    );
}

#[test]
fn test_subprocess_getoutput_line_number_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_getoutput.py",
        r#"
import subprocess

def test_getoutput():
    output = subprocess.getoutput("echo hi")
    assert output
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009").unwrap();
    assert_eq!(
        v.line, 5,
        "Violation should be at getoutput call line 5, not function def line 4"
    );
}

#[test]
fn test_random_multi_function_line_number_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_multi_fn_random.py",
        r#"
import random

def test_first():
    assert True

def test_second():
    val = random.randint(1, 100)
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008").unwrap();
    assert_eq!(
        v.line, 8,
        "Violation should be at random call line 8 in second function, not line 7"
    );
}

#[test]
fn test_subprocess_mixed_calls_count_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_mixed_subprocess.py",
        r#"
import subprocess

def test_mixed():
    print("starting")
    result = subprocess.run(["echo", "hi"])
    assert result.returncode == 0
"#,
    );
    let violations = lint_single_file(&path);
    let flk009: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == "PYTEST-FLK-009")
        .collect();
    assert_eq!(
        flk009.len(),
        1,
        "Expected exactly 1 FLK-009 violation for subprocess call only, got {}",
        flk009.len()
    );
}

#[test]
fn test_file_io_with_tmpdir_does_not_trigger_flk005() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_flk005_tmpdir.py",
        r#"
def test_reads_file(tmpdir):
    f = open(str(tmpdir / "data.txt"))
    content = f.read()
    f.close()
    assert content
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-002");
    assert!(
        v.is_none(),
        "Should not trigger FLK-002 when tmpdir is used"
    );
}

#[test]
fn test_long_test_name_triggers_mnt017() {
    let dir = tempfile::tempdir().unwrap();
    let long_name = format!("test_{}", "a".repeat(76));
    assert!(
        long_name.chars().count() > 80,
        "name is {} chars",
        long_name.chars().count()
    );
    let content = format!(
        r#"
def {}():
    assert True
"#,
        long_name
    );
    let path = write_temp_file(dir.path(), "test_long_name.py", &content);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-017");
    assert!(v.is_some(), "Expected PYTEST-MNT-017 for long test name");
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-MNT-017");
    assert_eq!(v.rule_name, "TestNameLengthRule");
    assert_eq!(v.severity, Severity::Info);
    assert!(v.message.contains("exceeds 80 characters"));
    assert!(v.suggestion.is_some());
}

#[test]
fn test_short_test_name_no_mnt017() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_short_name.py",
        r#"
def test_short():
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-017");
    assert!(
        v.is_none(),
        "Should not trigger MNT-017 for short test name"
    );
}

#[test]
fn test_name_exactly_80_chars_no_mnt017() {
    let dir = tempfile::tempdir().unwrap();
    let name_80 = format!("test_{}", "b".repeat(75));
    assert_eq!(name_80.chars().count(), 80);
    let content = format!(
        r#"
def {}():
    assert True
"#,
        name_80
    );
    let path = write_temp_file(dir.path(), "test_80_chars.py", &content);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-017");
    assert!(v.is_none(), "Exactly 80 chars should not trigger MNT-017");
}

#[test]
fn test_name_81_chars_triggers_mnt017() {
    let dir = tempfile::tempdir().unwrap();
    let name_81 = format!("test_{}", "c".repeat(76));
    assert_eq!(name_81.chars().count(), 81);
    let content = format!(
        r#"
def {}():
    assert True
"#,
        name_81
    );
    let path = write_temp_file(dir.path(), "test_81_chars.py", &content);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-017");
    assert!(v.is_some(), "81 chars should trigger MNT-017");
}

#[test]
fn test_duplicate_test_bodies_triggers_mnt015() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_dup_bodies.py",
        r#"
def test_first():
    x = 1
    assert x == 1

def test_second():
    x = 1
    assert x == 1
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-015");
    assert!(
        v.is_some(),
        "Expected PYTEST-MNT-015 for duplicate test bodies"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-MNT-015");
    assert_eq!(v.rule_name, "DuplicateTestBodiesRule");
    assert_eq!(v.severity, Severity::Info);
    assert!(v.message.contains("identical body"));
    assert!(v.message.contains("test_first"));
    assert!(v.message.contains("test_second"));
}

#[test]
fn test_duplicate_test_bodies_different_no_mnt015() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_diff_bodies.py",
        r#"
def test_alpha():
    x = 1
    assert x == 1

def test_beta():
    y = 2
    assert y == 2
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-015");
    assert!(v.is_none(), "Different bodies should not trigger MNT-015");
}

#[test]
fn test_sleep_above_threshold_triggers_mnt016() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_sleep_high.py",
        r#"
import time

def test_slow():
    time.sleep(2)
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-016");
    assert!(v.is_some(), "Expected PYTEST-MNT-016 for sleep > 0.1s");
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-MNT-016");
    assert_eq!(v.rule_name, "SleepWithValueRule");
    assert_eq!(v.severity, Severity::Warning);
    assert!(v.message.contains("> 0.1s"));
}

#[test]
fn test_sleep_exactly_0_1_no_mnt016() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_sleep_0_1.py",
        r#"
import time

def test_exact():
    time.sleep(0.1)
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-016");
    assert!(
        v.is_none(),
        "sleep == 0.1 should not trigger MNT-016 (threshold is > not >=)"
    );
}

#[test]
fn test_conditional_logic_in_parametrized_triggers_mnt014() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_cond_param.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [1, 2])
def test_branching(val):
    x = val
    if x > 0:
        assert x > 0
    else:
        assert x <= 0
"#,
    );
    let module = parse_file(&path);
    assert_eq!(module.test_functions.len(), 1);
    assert!(
        module.test_functions[0].is_parametrized,
        "should be parametrized"
    );
    assert!(
        module.test_functions[0].has_conditional_logic,
        "should have conditional logic"
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-014");
    assert!(
        v.is_some(),
        "Expected PYTEST-MNT-014 for parametrized test with conditional"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-MNT-014");
    assert_eq!(v.rule_name, "ConditionalLogicInTestRule");
    assert!(v.message.contains("conditional logic"));
    assert!(v.suggestion.is_some());
}

#[test]
fn test_conditional_logic_non_parametrized_no_mnt014() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_cond_no_param.py",
        r#"
def test_plain_condition():
    if True:
        assert True
    else:
        assert False
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-014");
    assert!(
        v.is_none(),
        "Non-parametrized test with condition should not trigger MNT-014"
    );
}

#[test]
fn test_parametrize_duplicate_values_triggers_param002() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_dup_param.py",
        r#"
import pytest

@pytest.mark.parametrize("val", [1, 2, 1])
def test_dup(val):
    assert val > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-002");
    assert!(
        v.is_some(),
        "Expected PYTEST-PARAM-002 for duplicate parametrize values"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_id, "PYTEST-PARAM-002");
    assert_eq!(v.rule_name, "ParametrizeDuplicateRule");
    assert!(v.message.contains("duplicate values"));
}

#[test]
fn test_parametrize_explosion_exactly_21_triggers_param003() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = r#"
import pytest

@pytest.mark.parametrize("val", ["#
        .to_string();
    for i in 0..21 {
        if i > 0 {
            content.push_str(", ");
        }
        content.push_str(&i.to_string());
    }
    content.push_str(
        r#"])
def test_many(val):
    assert val >= 0
"#,
    );
    let path = write_temp_file(dir.path(), "test_21_cases.py", &content);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-003");
    assert!(
        v.is_some(),
        "Expected PYTEST-PARAM-003 for exactly 21 cases (> 20)"
    );
    let v = v.unwrap();
    assert_eq!(v.rule_name, "ParametrizeExplosionRule");
}

#[test]
fn test_parametrize_explosion_exactly_20_no_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = r#"
import pytest

@pytest.mark.parametrize("val", ["#
        .to_string();
    for i in 0..20 {
        if i > 0 {
            content.push_str(", ");
        }
        content.push_str(&i.to_string());
    }
    content.push_str(
        r#"])
def test_exactly_20(val):
    assert val >= 0
"#,
    );
    let path = write_temp_file(dir.path(), "test_20_cases.py", &content);
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-PARAM-003");
    assert!(v.is_none(), "Exactly 20 cases should not trigger PARAM-003");
}

#[test]
fn test_subprocess_run_triggers_flk009() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_subprocess_timeout_lint.py",
        r#"
import subprocess

def test_subprocess_call():
    result = subprocess.run(["echo", "hello"], timeout=30)
    assert result.returncode == 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "subprocess.run should trigger FLK-009 (keyword timeout detection not supported)"
    );
}

#[test]
fn test_random_module_call_triggers_flk008_via_object() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_random_obj.py",
        r#"
import random

def test_random_custom():
    x = random.custom_func()
    assert x
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(
        v.is_some(),
        "random.<anything> should trigger FLK-008 via object check"
    );
}

#[test]
fn test_subprocess_module_call_triggers_flk009_via_object() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_subprocess_obj.py",
        r#"
import subprocess

def test_subprocess_custom():
    result = subprocess.custom_func()
    assert result
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-009");
    assert!(
        v.is_some(),
        "subprocess.<anything> should trigger FLK-009 via object check"
    );
}

#[test]
fn test_decorated_test_still_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_decorated.py",
        r#"
import time
import pytest

@pytest.mark.slow
def test_decorated():
    time.sleep(1)
    assert True
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-001");
    assert!(
        v.is_some(),
        "time.sleep in decorated test should trigger FLK-001"
    );
}

#[test]
fn test_random_randint_triggers_flk008() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_randint.py",
        r#"
import random

def test_rand():
    x = random.randint(1, 10)
    assert x > 0
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-FLK-008");
    assert!(v.is_some(), "random.randint should trigger FLK-008");
}

// ── PYTEST-MNT-005: stdlib mock detection ──

#[test]
fn test_mnt005_mock_subprocess_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_stdlib_mock.py",
        r#"
from unittest.mock import patch

@patch("subprocess.run")
def test_subprocess(mock_run):
    mock_run.return_value.returncode = 0
    mock_run.assert_called_once()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(v.is_some(), "mocking subprocess should trigger MNT-005");
}

#[test]
fn test_mnt005_mock_os_path_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_os_path_mock.py",
        r#"
from unittest.mock import patch

@patch("os.path.exists")
def test_path(mock_exists):
    mock_exists.return_value = True
    mock_exists.assert_called_once()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(v.is_some(), "mocking os.path should trigger MNT-005");
}

#[test]
fn test_mnt005_mock_builtins_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_builtins_mock.py",
        r#"
from unittest.mock import patch

@patch("builtins.open")
def test_open(mock_open):
    mock_open.return_value.__enter__ = lambda s: s
    mock_open.assert_called_once()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(v.is_some(), "mocking builtins should trigger MNT-005");
}

#[test]
fn test_mnt005_mock_socket_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_socket_mock.py",
        r#"
from unittest.mock import patch

@patch("socket.socket")
def test_socket(mock_sock):
    mock_sock.return_value.recv.return_value = b"data"
    mock_sock.assert_called_once()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(v.is_some(), "mocking socket should trigger MNT-005");
}

#[test]
fn test_mnt005_mock_time_triggers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_time_mock.py",
        r#"
from unittest.mock import patch

@patch("time.time")
def test_time(mock_time):
    mock_time.return_value = 1000.0
    mock_time.assert_called_once()
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(v.is_some(), "mocking time should trigger MNT-005");
}

#[test]
fn test_mnt005_non_stdlib_mock_no_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_file(
        dir.path(),
        "test_nonstdlib_mock.py",
        r#"
from unittest.mock import patch

@patch("myapp.service.fetch_data")
def test_fetch(mock_fetch):
    mock_fetch.return_value = {"ok": True}
    result = mock_fetch()
    assert result == {"ok": True}
"#,
    );
    let violations = lint_single_file(&path);
    let v = find_violation(&violations, "PYTEST-MNT-005");
    assert!(
        v.is_none(),
        "mocking non-stdlib module should NOT trigger MNT-005"
    );
}
