//! Rules that detect test flakiness patterns: time.sleep, file I/O, network, random, subprocess.

use crate::engine::make_violation;
use crate::models::{Category, ParsedModule, Severity, Violation};
use crate::rules::{Rule, RuleContext};
use tree_sitter::Node;

/// Rule that detects use of `time.sleep` in tests, which causes flaky behavior.
pub struct TimeSleepRule;

impl Rule for TimeSleepRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-001"
    }
    fn name(&self) -> &'static str {
        "TimeSleepRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.uses_time_sleep {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' uses time.sleep which causes flaky tests",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Use pytest's time mocking or wait for a condition instead".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects file I/O without temporary fixtures.
pub struct FileIoRule;

impl Rule for FileIoRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-002"
    }
    fn name(&self) -> &'static str {
        "FileIoRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.uses_file_io {
                let has_tmp = test
                    .fixture_deps
                    .iter()
                    .any(|d| d == "tmp_path" || d == "tmpdir" || d == "tmp_path_factory");
                if !has_tmp {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' uses file I/O without tmp_path/tmpdir fixture",
                            test.name
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some("Use the tmp_path or tmpdir fixture for temporary files".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

/// Rule that detects imports of network libraries in test files.
pub struct NetworkImportRule;

impl Rule for NetworkImportRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-003"
    }
    fn name(&self) -> &'static str {
        "NetworkImportRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let network_modules = [
            "requests",
            "socket",
            "httpx",
            "aiohttp",
            "urllib",
            "urllib3",
            "pycurl",
            "tornado.httpclient",
            "grpc",
            "aiogrpc",
        ];
        let mock_layer_libs = [
            "pytest_httpx",
            "respx",
            "aioresponses",
            "responses",
            "requests_mock",
            "pytest_mock",
            "vcrpy",
            "betamax",
            "httmock",
        ];

        let has_network = module
            .imports
            .iter()
            .any(|imp| network_modules.iter().any(|nm| imp.contains(nm)));

        let has_mock_layer = module
            .imports
            .iter()
            .any(|imp| mock_layer_libs.iter().any(|ml| imp.contains(ml)));

        if has_network && !has_mock_layer {
            vec![make_violation(
                self.id(),
                self.name(),
                self.severity(),
                self.category(),
                "File imports network libraries which may cause flaky tests".to_string(),
                module.file_path.clone(),
                1,
                Some("Mock network calls or use pytest-localserver".to_string()),
                None,
            )]
        } else {
            vec![]
        }
    }
}

/// Rule that detects tests depending on the current working directory.
pub struct CwdDependencyRule;

impl Rule for CwdDependencyRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-004"
    }
    fn name(&self) -> &'static str {
        "CwdDependencyRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.uses_cwd_dependency {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' depends on the current working directory",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Use absolute paths or tmp_path fixture instead".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects mystery guest anti-pattern: file I/O without explicit temp fixtures.
pub struct MysteryGuestRule;

impl Rule for MysteryGuestRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-005"
    }
    fn name(&self) -> &'static str {
        "MysteryGuestRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.uses_file_io {
                let has_tmp = test
                    .fixture_deps
                    .iter()
                    .any(|d| d == "tmp_path" || d == "tmpdir" || d == "tmp_path_factory");
                if !has_tmp {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' may be a Mystery Guest — uses file I/O without temp fixtures",
                            test.name
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some(
                            "Use tmp_path fixture and make test data explicit".to_string(),
                        ),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

/// Rule that detects session-scoped fixtures returning mutable state modified by tests.
pub struct XdistSharedStateRule;

impl Rule for XdistSharedStateRule {
    fn id(&self) -> &'static str {
        "PYTEST-XDIST-001"
    }
    fn name(&self) -> &'static str {
        "XdistSharedStateRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        if ctx.session_mutable_fixtures.is_empty() {
            return violations;
        }

        for test in &module.test_functions {
            for dep in &test.mutates_fixture_deps {
                if ctx.session_mutable_fixtures.contains(dep) {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Session-scoped fixture '{}' returns mutable state that is modified by test '{}' — unsafe for xdist",
                            dep, test.name
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some("Use function scope or return immutable values".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

pub struct SocketWithoutBindTimeoutRule;

impl Rule for SocketWithoutBindTimeoutRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-010"
    }
    fn name(&self) -> &'static str {
        "SocketWithoutBindTimeoutRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        let has_socket_import = module.imports.iter().any(|imp| imp.contains("socket"));
        if !has_socket_import {
            return violations;
        }
        for test in &module.test_functions {
            if test.uses_network {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' uses socket without proper bind and timeout setup",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some(
                        "Add socket.settimeout() or use timeout parameter in socket.socket()"
                            .to_string(),
                    ),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

pub struct DatetimeInAssertionRule;

impl Rule for DatetimeInAssertionRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-011"
    }
    fn name(&self) -> &'static str {
        "DatetimeInAssertionRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        let has_datetime_import = module.imports.iter().any(|imp| imp.contains("datetime"));
        if !has_datetime_import {
            return violations;
        }
        for test in &module.test_functions {
            if test.has_assertions {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' uses datetime functions near assertions — tests relying on real time are flaky",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some(
                        "Use freezegun or time mocking to make assertions deterministic".to_string(),
                    ),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects session-scoped fixtures performing file I/O.
pub struct XdistFixtureIoRule;

impl Rule for XdistFixtureIoRule {
    fn id(&self) -> &'static str {
        "PYTEST-XDIST-002"
    }
    fn name(&self) -> &'static str {
        "XdistFixtureIoRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for fixture in &module.fixtures {
            if fixture.scope == crate::models::FixtureScope::Session && fixture.uses_file_io {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Session-scoped fixture '{}' uses file I/O — may conflict with xdist workers",
                        fixture.name
                    ),
                    module.file_path.clone(),
                    fixture.line,
                    Some("Use tmp_path_factory or make I/O paths unique per worker".to_string()),
                    None,
                ));
            }
        }
        violations
    }
}

/// Rule that detects use of random functions without a fixed seed, reported per call site.
pub struct RandomWithoutSeedRule;

impl Rule for RandomWithoutSeedRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-008"
    }
    fn name(&self) -> &'static str {
        "RandomWithoutSeedRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.uses_random && !test.has_random_seed {
                let random_lines = collect_random_call_lines(test);
                for line in random_lines {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' uses random without fixed seed — causes flaky tests",
                            test.name
                        ),
                        module.file_path.clone(),
                        line,
                        Some(
                            "Call random.seed() at the start of the test or use a fixture"
                                .to_string(),
                        ),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

/// Rule that detects subprocess calls without a timeout, reported per call site.
pub struct SubprocessWithoutTimeoutRule;

impl Rule for SubprocessWithoutTimeoutRule {
    fn id(&self) -> &'static str {
        "PYTEST-FLK-009"
    }
    fn name(&self) -> &'static str {
        "SubprocessWithoutTimeoutRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Flakiness
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.uses_subprocess {
                let unguarded_lines = collect_unguarded_subprocess_calls(test);
                for line in unguarded_lines {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' uses subprocess without timeout — may hang indefinitely",
                            test.name
                        ),
                        module.file_path.clone(),
                        line,
                        Some("Add timeout parameter to subprocess calls".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

/// Collect line numbers of each `random.*` call in a test function body.
fn collect_random_call_lines(test: &crate::models::TestFunction) -> Vec<usize> {
    let source = match std::fs::read_to_string(&test.file_path) {
        Ok(s) => s,
        Err(_) => return vec![test.line],
    };
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return vec![test.line];
    }
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return vec![test.line],
    };
    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    let func_node = match find_function_node(&root, test.line) {
        Some(n) => n,
        None => return vec![test.line],
    };
    let body = match func_node.child_by_field_name("body") {
        Some(b) => b,
        None => return vec![test.line],
    };

    let mut lines = Vec::new();
    collect_random_calls(body, source_bytes, &mut lines);
    if lines.is_empty() {
        vec![test.line]
    } else {
        lines
    }
}

/// Check if a node is a call to a random function.
fn is_random_call(node: Node, source: &[u8]) -> bool {
    if node.kind() == "call" {
        if let Some(f) = node.child_by_field_name("function") {
            let text = f.utf8_text(source).unwrap_or_default();
            let random_fns = [
                "random.random",
                "random.randint",
                "random.choice",
                "random.shuffle",
                "random.uniform",
                "random.randrange",
                "random.sample",
                "random.gauss",
                "random.normalvariate",
            ];
            return random_fns.contains(&text)
                || (f.kind() == "attribute"
                    && f.child_by_field_name("object")
                        .is_some_and(|o| o.utf8_text(source).unwrap_or_default() == "random"));
        }
    }
    false
}

/// Recursively collect line numbers of random function calls.
fn collect_random_calls(node: Node, source: &[u8], lines: &mut Vec<usize>) {
    if is_random_call(node, source) {
        lines.push(node.start_position().row + 1);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_random_calls(child, source, lines);
    }
}

/// Collect line numbers of subprocess calls that lack a timeout argument.
fn collect_unguarded_subprocess_calls(test: &crate::models::TestFunction) -> Vec<usize> {
    let source = match std::fs::read_to_string(&test.file_path) {
        Ok(s) => s,
        Err(_) => return vec![test.line],
    };
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return vec![test.line];
    }
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return vec![test.line],
    };
    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    let func_node = match find_function_node(&root, test.line) {
        Some(n) => n,
        None => return vec![test.line],
    };
    let body = match func_node.child_by_field_name("body") {
        Some(b) => b,
        None => return vec![test.line],
    };

    let mut lines = Vec::new();
    collect_subprocess_calls_without_timeout(body, source_bytes, &mut lines);
    if lines.is_empty() {
        vec![test.line]
    } else {
        lines
    }
}

/// Check if a node is a call to a subprocess function.
fn is_subprocess_call(node: Node, source: &[u8]) -> bool {
    if node.kind() == "call" {
        if let Some(f) = node.child_by_field_name("function") {
            let text = f.utf8_text(source).unwrap_or_default();
            let subprocess_fns = [
                "subprocess.Popen",
                "subprocess.run",
                "subprocess.call",
                "subprocess.check_output",
                "subprocess.check_call",
            ];
            return subprocess_fns.contains(&text)
                || (f.kind() == "attribute"
                    && f.child_by_field_name("object")
                        .is_some_and(|o| o.utf8_text(source).unwrap_or_default() == "subprocess"));
        }
    }
    false
}

/// Recursively collect line numbers of subprocess calls missing a timeout keyword arg.
fn collect_subprocess_calls_without_timeout(node: Node, source: &[u8], lines: &mut Vec<usize>) {
    if is_subprocess_call(node, source) && !call_has_timeout(node, source) {
        lines.push(node.start_position().row + 1);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_subprocess_calls_without_timeout(child, source, lines);
    }
}

/// Check if a call node has a `timeout` keyword argument.
fn call_has_timeout(call_node: Node, source: &[u8]) -> bool {
    if let Some(a) = call_node.child_by_field_name("arguments") {
        let mut cursor = a.walk();
        for child in a.children(&mut cursor) {
            if child.kind() == "keyword_argument" {
                if let Some(n) = child.child_by_field_name("name") {
                    if n.utf8_text(source).unwrap_or_default() == "timeout" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Search inside a decorated_definition node for a function_definition at the target line.
#[allow(clippy::manual_find)]
fn find_in_decorated_definition(node: Node, target_line: usize) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_definition"
            && child.start_position().row + 1 == target_line
        {
            return Some(child);
        }
    }
    None
}

/// Find the function_definition node at the given 1-indexed line number.
fn find_function_node<'tree>(root: &'tree Node<'tree>, target_line: usize) -> Option<Node<'tree>> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_definition" if child.start_position().row + 1 == target_line => {
                return Some(child);
            }
            "decorated_definition" => {
                if let Some(found) = find_in_decorated_definition(child, target_line) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod debug_timeout {
    use super::*;

    #[test]
    fn debug_call_has_timeout() {
        let source = r#"import subprocess

def test_subprocess_safe():
    result = subprocess.run(["echo", "hello"], timeout=30)
    assert result.returncode == 0
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let bytes = source.as_bytes();

        // Find the call node
        fn find_call<'a>(
            node: &tree_sitter::Node<'a>,
            source: &'a [u8],
        ) -> Option<tree_sitter::Node<'a>> {
            if node.kind() == "call" {
                if let Some(f) = node.child_by_field_name("function") {
                    let text = f.utf8_text(source).unwrap_or_default();
                    if text.contains("subprocess") {
                        return Some(*node);
                    }
                }
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(c) = find_call(&child, source) {
                    return Some(c);
                }
            }
            None
        }

        let call = find_call(&root, bytes).expect("should find subprocess call");
        let func_text = call
            .child_by_field_name("function")
            .unwrap()
            .utf8_text(bytes)
            .unwrap();
        eprintln!("call kind: {}", call.kind());
        eprintln!("call text: {}", call.utf8_text(bytes).unwrap());
        eprintln!("function: {}", func_text);

        let args = call.child_by_field_name("arguments");
        eprintln!("arguments node: {:?}", args.as_ref().map(|a| a.kind()));

        if let Some(a) = args {
            let mut cursor = a.walk();
            for (i, child) in a.children(&mut cursor).enumerate() {
                eprintln!(
                    "  arg[{}] kind={} text={}",
                    i,
                    child.kind(),
                    child.utf8_text(bytes).unwrap()
                );
                if child.kind() == "keyword_argument" {
                    let name_field = child.child_by_field_name("name");
                    eprintln!(
                        "    name_field: {:?}",
                        name_field.as_ref().map(|n| n.utf8_text(bytes).unwrap())
                    );
                }
            }
        }

        let result = call_has_timeout(call, bytes);
        eprintln!("call_has_timeout result: {}", result);
        assert!(
            result,
            "call_has_timeout should return true for subprocess.run with timeout=30"
        );
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_find_function_node_finds_top_level() {
        let source = "def test_foo():\n    pass\n";
        let tree = parse(source);
        let root = tree.root_node();
        let node = find_function_node(&root, 1);
        assert!(node.is_some(), "should find function on line 1");
    }

    #[test]
    fn test_find_function_node_finds_in_decorated() {
        let source = "@decorator\ndef test_bar():\n    pass\n";
        let tree = parse(source);
        let root = tree.root_node();
        let node = find_function_node(&root, 2);
        assert!(
            node.is_some(),
            "should find function on line 2 inside decorated_definition"
        );
        assert_eq!(node.unwrap().kind(), "function_definition");
    }

    #[test]
    fn test_find_function_node_wrong_line_returns_none() {
        let source = "def test_foo():\n    pass\n";
        let tree = parse(source);
        let root = tree.root_node();
        let node = find_function_node(&root, 99);
        assert!(node.is_none(), "should not find function on wrong line");
    }

    #[test]
    fn test_collect_random_calls_detects_random_module_attribute() {
        let source = "import random\nx = random.unknown_func()\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_random_calls(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(
            !lines.is_empty(),
            "random.<unknown> should be detected via attribute object check"
        );
    }

    #[test]
    fn test_collect_random_calls_detects_randint() {
        let source = "import random\nx = random.randint(1, 10)\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_random_calls(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(!lines.is_empty(), "random.randint should be detected");
    }

    #[test]
    fn test_collect_random_calls_no_false_positive() {
        let source = "x = deterministic_func()\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_random_calls(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(lines.is_empty(), "non-random call should not be detected");
    }

    #[test]
    fn test_collect_random_calls_no_false_positive_attribute() {
        let source = "x = math.sqrt(4)\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_random_calls(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(
            lines.is_empty(),
            "math.sqrt (non-random attribute) should not be detected as random"
        );
    }

    #[test]
    fn test_collect_subprocess_calls_detects_subprocess_attribute() {
        let source = "import subprocess\nr = subprocess.unknown_func()\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_subprocess_calls_without_timeout(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(
            !lines.is_empty(),
            "subprocess.<unknown> should be detected via attribute object check"
        );
    }

    #[test]
    fn test_collect_subprocess_calls_detects_run() {
        let source = "import subprocess\nr = subprocess.run(['ls'])\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_subprocess_calls_without_timeout(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(
            !lines.is_empty(),
            "subprocess.run without timeout should be detected"
        );
    }

    #[test]
    fn test_find_function_node_decorated_wrong_line_returns_none() {
        let source = "@decorator\ndef test_bar():\n    pass\n";
        let tree = parse(source);
        let root = tree.root_node();
        let node = find_function_node(&root, 99);
        assert!(
            node.is_none(),
            "decorated function on wrong line should return None"
        );
    }

    #[test]
    fn test_collect_subprocess_calls_no_false_positive_attribute() {
        let source = "r = os.system('ls')\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_subprocess_calls_without_timeout(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(
            lines.is_empty(),
            "os.system (non-subprocess attribute) should not be detected as subprocess"
        );
    }

    #[test]
    fn test_collect_subprocess_calls_no_false_positive() {
        let source = "x = other_func()\n";
        let tree = parse(source);
        let mut lines = Vec::new();
        collect_subprocess_calls_without_timeout(tree.root_node(), source.as_bytes(), &mut lines);
        assert!(
            lines.is_empty(),
            "non-subprocess call should not be detected"
        );
    }
}
