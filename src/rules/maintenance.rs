//! Rules that detect test maintenance issues: magic asserts, missing assertions, conditional logic.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::engine::make_violation;
use crate::models::{Category, ParsedModule, Severity, TestFunction, Violation};
use crate::rules::{Rule, RuleContext};

fn stable_hash(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Rule that detects conditional logic inside test functions.
pub struct TestLogicRule;

impl Rule for TestLogicRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-001"
    }
    fn name(&self) -> &'static str {
        "TestLogicRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.has_conditional_logic {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' contains conditional logic (if statements)",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Split into separate tests or use parametrize".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects magic values in assertions.
pub struct MagicAssertRule;

impl Rule for MagicAssertRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-002"
    }
    fn name(&self) -> &'static str {
        "MagicAssertRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            for assertion in &test.assertions {
                if assertion.is_magic {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Magic assertion at line {}: '{}' — this always passes/fails",
                            assertion.line, assertion.expression_text
                        ),
                        module.file_path.clone(),
                        assertion.line,
                        Some("Replace with a meaningful comparison".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

/// Rule that detects suboptimal assertion patterns.
pub struct SuboptimalAssertRule;

impl Rule for SuboptimalAssertRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-003"
    }
    fn name(&self) -> &'static str {
        "SuboptimalAssertRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn category(&self) -> Category {
        Category::Enhancement
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            for assertion in &test.assertions {
                if assertion.is_suboptimal {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Suboptimal assertion at line {}: '{}'",
                            assertion.line, assertion.expression_text
                        ),
                        module.file_path.clone(),
                        assertion.line,
                        Some("Use a more direct assertion pattern".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

/// Rule that detects tests with no assertions.
pub struct NoAssertionRule;

impl Rule for NoAssertionRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-004"
    }
    fn name(&self) -> &'static str {
        "NoAssertionRule"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if !test.has_assertions {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!("Test '{}' has no assertions", test.name),
                    module.file_path.clone(),
                    test.line,
                    Some("Add assertions to verify expected behavior".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects tests that only verify mock calls without real assertions.
pub struct MockOnlyVerifyRule;

impl Rule for MockOnlyVerifyRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-005"
    }
    fn name(&self) -> &'static str {
        "MockOnlyVerifyRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.has_mock_verifications && !test.has_state_assertions {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' only verifies mocks without checking state",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Add state assertions to verify actual outcomes".to_string()),
                    Some(test.name.clone()),
                ));
            }
            if test.mocks_stdlib_module {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' mocks stdlib module(s): {} — use dependency injection or test doubles instead",
                        test.name,
                        test.mocked_stdlib_targets.join(", ")
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Refactor to inject dependencies instead of patching stdlib internals".to_string()),
                    Some(test.name.clone()),
                ));
            }
            if test.has_weak_assertions && !test.weak_assertion_details.is_empty() {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' uses weak assertion patterns: {}",
                        test.name,
                        test.weak_assertion_details.join(", ")
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some(
                        "Use value-level assertions instead of type/existence/key-presence checks"
                            .to_string(),
                    ),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects assertion roulette: too many assertions without messages.
pub struct AssertionRouletteRule;

impl Rule for AssertionRouletteRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-006"
    }
    fn name(&self) -> &'static str {
        "AssertionRouletteRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.assertion_count > 3 && !test.is_parametrized {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' has {} assertions (assertion roulette)",
                        test.name, test.assertion_count
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Split into smaller, focused tests".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects bare except or overly broad exception handling.
pub struct RawExceptionHandlingRule;

impl Rule for RawExceptionHandlingRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-007"
    }
    fn name(&self) -> &'static str {
        "RawExceptionHandlingRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.has_try_except {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' uses try/except instead of pytest.raises",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Use pytest.raises() for exception testing".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that detects BDD-style tests missing Given/When/Then scenario structure.
pub struct BddMissingScenarioRule;

impl Rule for BddMissingScenarioRule {
    fn id(&self) -> &'static str {
        "PYTEST-BDD-001"
    }
    fn name(&self) -> &'static str {
        "BddMissingScenarioRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn category(&self) -> Category {
        Category::Enhancement
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            // Parametrized tests provide structured coverage via data variants,
            // so BDD-style docstrings are not required.
            if test.is_parametrized {
                continue;
            }
            let has_gherkin = test.docstring.as_ref().is_some_and(|ds| {
                let lower = ds.to_lowercase();
                lower.contains("given") || lower.contains("when") || lower.contains("then")
            });
            if !has_gherkin {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' lacks a Gherkin-style docstring scenario",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Add a docstring with Given/When/Then structure".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

/// Rule that suggests using property-based testing for suitable tests.
pub struct PropertyTestHintRule;

impl Rule for PropertyTestHintRule {
    fn id(&self) -> &'static str {
        "PYTEST-PBT-001"
    }
    fn name(&self) -> &'static str {
        "PropertyTestHintRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn category(&self) -> Category {
        Category::Enhancement
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.is_parametrized {
                if let Some(count) = test.parametrize_count {
                    if count > 3 {
                        violations.push(make_violation(
                            self.id(),
                            self.name(),
                            self.severity(),
                            self.category(),
                            format!(
                                "Test '{}' has {} parametrized cases — consider property-based testing",
                                test.name, count
                            ),
                            module.file_path.clone(),
                            test.line,
                            Some("Consider using hypothesis for property-based testing".to_string()),
                            Some(test.name.clone()),
                        ));
                    }
                }
            }
        }
        violations
    }
}

/// Rule that detects parametrize decorators with empty value lists.
pub struct ParametrizeEmptyRule;

impl Rule for ParametrizeEmptyRule {
    fn id(&self) -> &'static str {
        "PYTEST-PARAM-001"
    }
    fn name(&self) -> &'static str {
        "ParametrizeEmptyRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.is_parametrized {
                if let Some(count) = test.parametrize_count {
                    if count <= 1 {
                        violations.push(make_violation(
                            self.id(),
                            self.name(),
                            self.severity(),
                            self.category(),
                            format!(
                                "Test '{}' is parametrized with only {} case(s)",
                                test.name, count
                            ),
                            module.file_path.clone(),
                            test.line,
                            Some("Add more test cases or remove parametrize".to_string()),
                            Some(test.name.clone()),
                        ));
                    }
                }
            }
        }
        violations
    }
}

/// Rule that detects duplicate values in parametrize lists.
pub struct ParametrizeDuplicateRule;

impl Rule for ParametrizeDuplicateRule {
    fn id(&self) -> &'static str {
        "PYTEST-PARAM-002"
    }
    fn name(&self) -> &'static str {
        "ParametrizeDuplicateRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            for values in &test.parametrize_values {
                let mut seen = std::collections::HashSet::new();
                let mut duplicates = std::collections::HashSet::new();
                for val in values {
                    if !seen.insert(val) {
                        duplicates.insert(val.as_str());
                    }
                }
                if !duplicates.is_empty() {
                    let mut dup_str: Vec<&str> = duplicates.into_iter().collect();
                    dup_str.sort_unstable();
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Parametrize in test '{}' has duplicate values: {}",
                            test.name,
                            dup_str.join(", ")
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some("Remove duplicate parametrize values".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

/// Rule that detects parametrize decorators with too many combinations.
pub struct ParametrizeExplosionRule;

impl Rule for ParametrizeExplosionRule {
    fn id(&self) -> &'static str {
        "PYTEST-PARAM-003"
    }
    fn name(&self) -> &'static str {
        "ParametrizeExplosionRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if let Some(count) = test.parametrize_count {
                if count > 20 {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' has {} parametrized cases — combinatorial explosion",
                            test.name, count
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some("Reduce test cases or use hypothesis".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

pub struct ConditionalLogicInTestRule;

impl Rule for ConditionalLogicInTestRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-014"
    }
    fn name(&self) -> &'static str {
        "ConditionalLogicInTestRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.is_parametrized && test.has_conditional_logic {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Parametrized test '{}' contains conditional logic (if/elif/else/for/while) — use separate parameter cases instead of branching",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Split into separate tests or use pytest.mark.parametrize".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

pub struct DuplicateTestBodiesRule;

impl Rule for DuplicateTestBodiesRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-015"
    }
    fn name(&self) -> &'static str {
        "DuplicateTestBodiesRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        let tests = &module.test_functions;

        let mut hash_map: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, test) in tests.iter().enumerate() {
            if let Some(hash) = test.body_hash {
                hash_map.entry(hash).or_default().push(i);
            }
        }

        let mut reported = std::collections::HashSet::new();
        for indices in hash_map.values() {
            if indices.len() < 2 {
                continue;
            }
            let names: Vec<&str> = indices.iter().map(|i| tests[*i].name.as_str()).collect();
            for &i in indices {
                let test = &tests[i];
                if reported.contains(&test.name) {
                    continue;
                }
                let peers: Vec<&str> = names.iter().filter(|n| **n != test.name).copied().collect();
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' has identical body to {} other test(s): {} (shared body hash)",
                        test.name,
                        peers.len(),
                        peers.join(", ")
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Consolidate or differentiate the test bodies".to_string()),
                    Some(test.name.clone()),
                ));
                reported.insert(test.name.clone());
            }
        }
        violations
    }
}

pub struct SleepWithValueRule;

impl Rule for SleepWithValueRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-016"
    }
    fn name(&self) -> &'static str {
        "SleepWithValueRule"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn category(&self) -> Category {
        Category::Maintenance
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
                let exceeds_threshold = test.sleep_value.is_some_and(|v| v > 0.1);
                if exceeds_threshold {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' uses time.sleep() with value > 0.1s — slows test suite",
                            test.name
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some("Use mocking, async waits, or reduce sleep duration".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

pub struct TestNameLengthRule;

impl Rule for TestNameLengthRule {
    fn id(&self) -> &'static str {
        "PYTEST-MNT-017"
    }
    fn name(&self) -> &'static str {
        "TestNameLengthRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn category(&self) -> Category {
        Category::Maintenance
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();
        for test in &module.test_functions {
            if test.name.chars().count() > 80 {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test name '{}' exceeds 80 characters ({} chars)",
                        test.name,
                        test.name.chars().count()
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Shorten the test name to be more concise".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

pub struct InlineSchemaRedeclaredRule;

fn extract_dict_content(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(eq_pos) = trimmed.find("= {") {
        let rest = &trimmed[eq_pos + 2..].trim();
        if rest.starts_with('{') && rest.ends_with('}') && rest.len() > 20 && rest.contains(':') {
            Some(rest.to_string())
        } else {
            None
        }
    } else if trimmed.starts_with('{')
        && trimmed.ends_with('}')
        && trimmed.len() > 20
        && trimmed.contains(':')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn collect_schema_hashes(tests: &[TestFunction], source: &str) -> HashMap<u64, Vec<String>> {
    let mut schema_hashes: HashMap<u64, Vec<String>> = HashMap::new();
    let source_lines: Vec<&str> = source.lines().collect();
    for test in tests {
        let start = test.line.saturating_sub(1);
        let len = test.end_line.saturating_sub(test.line).max(1);
        for line in source_lines.iter().skip(start).take(len) {
            if let Some(content) = extract_dict_content(line) {
                let hash = stable_hash(&content);
                schema_hashes
                    .entry(hash)
                    .or_default()
                    .push(test.name.clone());
            }
        }
    }
    schema_hashes
}

fn build_schema_violations(
    schema_hashes: &HashMap<u64, Vec<String>>,
    rule: &InlineSchemaRedeclaredRule,
    module: &ParsedModule,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut reported = HashSet::new();
    for (hash, names) in schema_hashes {
        if names.len() >= 2 && !reported.contains(hash) {
            reported.insert(*hash);
            let unique_names: HashSet<&str> = names.iter().map(|n| n.as_str()).collect();
            if unique_names.len() >= 2 {
                let test_names: Vec<&str> = unique_names.into_iter().collect();
                violations.push(make_violation(
                    rule.id(),
                    rule.name(),
                    rule.severity(),
                    rule.category(),
                    format!(
                        "Inline schema redeclared across {} tests: {} — extract to a fixture",
                        test_names.len(),
                        test_names.join(", ")
                    ),
                    module.file_path.clone(),
                    1,
                    Some("Extract shared test data into a fixture or conftest".to_string()),
                    None,
                ));
            }
        }
    }
    violations
}

impl Rule for InlineSchemaRedeclaredRule {
    fn id(&self) -> &'static str {
        "PYTEST-VAL-001"
    }
    fn name(&self) -> &'static str {
        "InlineSchemaRedeclaredRule"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn category(&self) -> Category {
        Category::Enhancement
    }
    fn check(
        &self,
        module: &ParsedModule,
        _all_modules: &[ParsedModule],
        _ctx: &RuleContext,
    ) -> Vec<Violation> {
        if module.test_functions.len() < 2 {
            return Vec::new();
        }
        let schema_hashes = collect_schema_hashes(&module.test_functions, &module.source);
        build_schema_violations(&schema_hashes, self, module)
    }
}
