use crate::engine::make_violation;
use crate::models::{Category, ParsedModule, Severity, Violation};
use crate::rules::{Rule, RuleContext};

fn find_patch_target_def_module(target: &str) -> Option<String> {
    let parts: Vec<&str> = target.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let class_idx = parts
        .iter()
        .position(|p| p.chars().next().is_some_and(|c| c.is_uppercase()))?;
    if class_idx == 0 {
        return None;
    }
    Some(parts[..class_idx].join("."))
}

pub struct PatchTargetingDefinitionModuleRule;

impl Rule for PatchTargetingDefinitionModuleRule {
    fn id(&self) -> &'static str {
        "PYTEST-MOC-001"
    }
    fn name(&self) -> &'static str {
        "PatchTargetingDefinitionModuleRule"
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
            for target in &test.patch_targets {
                if let Some(def_module) = find_patch_target_def_module(target) {
                    if module.imports.iter().any(|imp| imp.contains(&def_module)) {
                        violations.push(make_violation(
                            self.id(),
                            self.name(),
                            self.severity(),
                            self.category(),
                            format!(
                                "Test '{}' patches definition module '{}' — patch the consumer instead",
                                test.name, target
                            ),
                            module.file_path.clone(),
                            test.line,
                            Some("Patch where the target is used, not where it is defined".to_string()),
                            Some(test.name.clone()),
                        ));
                    }
                }
            }
        }
        violations
    }
}

pub struct MagicMockOnAsyncRule;

impl Rule for MagicMockOnAsyncRule {
    fn id(&self) -> &'static str {
        "PYTEST-MOC-002"
    }
    fn name(&self) -> &'static str {
        "MagicMockOnAsyncRule"
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
            if test.is_async && test.has_magic_mock {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Async test '{}' uses MagicMock instead of AsyncMock",
                        test.name
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Use unittest.mock.AsyncMock for async functions".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}

pub struct PatchInitBypassRule;

impl Rule for PatchInitBypassRule {
    fn id(&self) -> &'static str {
        "PYTEST-MOC-003"
    }
    fn name(&self) -> &'static str {
        "PatchInitBypassRule"
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
            for target in &test.patch_targets {
                if target.ends_with(".__init__") {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' patches __init__ on '{}' — bypasses constructor validation",
                            test.name, target
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some("Patch the class itself or use a factory fixture instead".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            }
        }
        violations
    }
}

pub struct MockRatioBudgetRule;

impl Rule for MockRatioBudgetRule {
    fn id(&self) -> &'static str {
        "PYTEST-MOC-004"
    }
    fn name(&self) -> &'static str {
        "MockRatioBudgetRule"
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
            if test.mock_count > 0 && test.assertion_count > 0 {
                let ratio = test.mock_count as f64 / test.assertion_count as f64;
                if ratio > 3.0 {
                    violations.push(make_violation(
                        self.id(),
                        self.name(),
                        self.severity(),
                        self.category(),
                        format!(
                            "Test '{}' has mock-to-assertion ratio of {:.1}:1 ({} mocks, {} assertions) — over budget",
                            test.name, ratio, test.mock_count, test.assertion_count
                        ),
                        module.file_path.clone(),
                        test.line,
                        Some("Reduce mock count or add more state assertions".to_string()),
                        Some(test.name.clone()),
                    ));
                }
            } else if test.mock_count > 3 && test.assertion_count == 0 {
                violations.push(make_violation(
                    self.id(),
                    self.name(),
                    self.severity(),
                    self.category(),
                    format!(
                        "Test '{}' has {} mock operations but zero assertions — over budget",
                        test.name, test.mock_count
                    ),
                    module.file_path.clone(),
                    test.line,
                    Some("Add assertions to verify actual outcomes".to_string()),
                    Some(test.name.clone()),
                ));
            }
        }
        violations
    }
}
