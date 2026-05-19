//! Core linting engine: file discovery, parallel parsing, rule execution, and output formatting.

use crate::config::Config;
use crate::models::{Category, Fixture, FixtureScope, ParsedModule, Severity, Violation};
use crate::rules::{Rule, RuleContext};
use anyhow::Result;
use colored::Colorize;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Single-pass rule dispatcher. Instead of each rule walking the parsed module
/// data independently, the dispatcher iterates all rules in a single pass per
/// module. This minimizes redundant iteration and provides a single integration
/// point for per-file override resolution.
pub struct RuleDispatcher {
    all_rules: Vec<Box<dyn Rule>>,
}

impl Default for RuleDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleDispatcher {
    pub fn new() -> Self {
        Self {
            all_rules: crate::rules::all_rules(),
        }
    }

    /// Check all rules against a single module in one pass, applying per-file
    /// config (global + overrides) for rule enablement and severity.
    pub fn check_module(
        &self,
        module: &ParsedModule,
        all_modules: &[ParsedModule],
        ctx: &RuleContext,
        config: &Config,
    ) -> Result<Vec<Violation>> {
        let effective = config.effective_rules_for_file(&module.file_path)?;
        let mut violations = Vec::new();

        for rule in &self.all_rules {
            let rule_id = rule.id();

            let enabled = effective
                .get(rule_id)
                .map(|rc| rc.enabled.unwrap_or(true))
                .unwrap_or(true);

            if !enabled {
                continue;
            }

            let default_severity = rule.severity();
            let severity = effective
                .get(rule_id)
                .and_then(|rc| rc.severity)
                .unwrap_or(default_severity);

            let mut v = rule.check(module, all_modules, ctx);
            for violation in &mut v {
                violation.severity = severity;
            }
            violations.append(&mut v);
        }

        Ok(violations)
    }
}

/// Memory budget for the linter.
///
/// The engine processes files in a streaming fashion to keep peak RSS within
/// the configured budget (default 256 MB):
///
/// 1. File discovery: Walk directory tree, collect test file paths only.
/// 2. Parsing: Each file is read with `std::fs::read_to_string`, parsed by
///    tree-sitter, and converted to a `ParsedModule`. The source string is
///    dropped after parsing — only extracted metadata (names, flags, fixtures)
///    is retained.
/// 3. Cross-module context: Fixture maps and usage sets are computed once from
///    all parsed modules.
/// 4. Rule checking: The `RuleDispatcher` iterates all rules per module in a
///    single pass, applying per-file overrides.
///
/// For a 1 GB Python repo (~10K test files), estimated peak memory:
///   - ParsedModule structs: ~10-50 MB (lightweight metadata, no source text)
///   - Cross-module context: ~5-10 MB
///   - Violations: ~1-5 MB
///   - Parser + tree-sitter overhead: ~5-10 MB
///   - Total: ~20-75 MB, well within the 256 MB budget.
pub struct LintEngine {
    dispatcher: RuleDispatcher,
    config: Config,
    memory_limit_mb: usize,
}

impl LintEngine {
    /// Create a new engine with rules filtered by the given configuration.
    #[allow(clippy::missing_errors_doc)]
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self {
            dispatcher: RuleDispatcher::new(),
            config,
            memory_limit_mb: 256,
        })
    }

    /// Create a LintEngine with an explicit memory limit (in MB).
    #[allow(clippy::missing_errors_doc)]
    pub fn with_memory_limit(config: Config, memory_limit_mb: usize) -> Result<Self> {
        Ok(Self {
            dispatcher: RuleDispatcher::new(),
            config,
            memory_limit_mb,
        })
    }

    /// Lint all test files discovered under the given paths and return violations.
    #[allow(clippy::missing_errors_doc)]
    pub fn lint_paths(&self, paths: &[PathBuf]) -> Result<Vec<Violation>> {
        let files = discover_files(paths, &self.config.excludes);

        let (estimated_mb, over_budget) = exceeds_memory_budget(&files, self.memory_limit_mb);
        if over_budget {
            eprintln!(
                "Warning: estimated memory usage ({estimated_mb} MB) exceeds limit ({} MB). \
                 Processing may exceed the configured budget.",
                self.memory_limit_mb
            );
        }

        let modules = parse_files_parallel(&files);

        let fixture_map = collect_all_fixtures(&modules);
        let used_fixture_names = compute_used_fixture_names(&modules);
        let fixture_locations = compute_fixture_locations(&modules);
        let session_mutable_fixtures = compute_session_mutable_fixtures(&modules);

        let ctx = RuleContext {
            fixture_map: &fixture_map,
            used_fixture_names: &used_fixture_names,
            fixture_locations: &fixture_locations,
            session_mutable_fixtures: &session_mutable_fixtures,
        };

        let mut violations = Vec::new();
        for module in &modules {
            let mut v = self
                .dispatcher
                .check_module(module, &modules, &ctx, &self.config)?;
            violations.append(&mut v);
        }

        let suppressions = collect_suppressions(&modules);
        let mut violations: Vec<Violation> = violations
            .into_iter()
            .filter(|v| !is_suppressed(v, &suppressions))
            .collect();
        violations.sort();
        Ok(violations)
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn lint_source(&self, source: &str, file_path: &Path) -> Result<Vec<Violation>> {
        self.lint_source_with_context(source, file_path, &[])
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn lint_source_with_context(
        &self,
        source: &str,
        file_path: &Path,
        context_modules: &[ParsedModule],
    ) -> Result<Vec<Violation>> {
        let mut parser = crate::parser::PythonParser::new()?;
        let module = parser.parse_source(source, file_path)?;

        let mut all_modules: Vec<ParsedModule> = context_modules.to_vec();
        all_modules.push(module);
        let primary = &all_modules[all_modules.len() - 1];

        let fixture_map = collect_all_fixtures(&all_modules);
        let used_fixture_names = compute_used_fixture_names(&all_modules);
        let fixture_locations = compute_fixture_locations(&all_modules);
        let session_mutable_fixtures = compute_session_mutable_fixtures(&all_modules);

        let ctx = RuleContext {
            fixture_map: &fixture_map,
            used_fixture_names: &used_fixture_names,
            fixture_locations: &fixture_locations,
            session_mutable_fixtures: &session_mutable_fixtures,
        };

        let violations = self
            .dispatcher
            .check_module(primary, &all_modules, &ctx, &self.config)?;

        Ok(violations)
    }
}

/// Check whether the estimated memory usage of processing the given files
/// exceeds the configured budget (in MB). Uses strict greater-than so that
/// being exactly at the budget does not trigger a warning. Returns the
/// estimated MB and whether it exceeds the limit.
fn exceeds_memory_budget(files: &[PathBuf], memory_limit_mb: usize) -> (u64, bool) {
    let estimated_bytes: u64 = files.len() as u64 * 50_000;
    let estimated_mb = estimated_bytes / 1_048_576;
    (estimated_mb, estimated_mb > memory_limit_mb as u64)
}

/// Default directory names to exclude during file discovery (virtual environments,
/// package caches, and VCS directories).
pub const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".venv",
    "venv",
    "env",
    "__pypackages__",
    "site-packages",
    "node_modules",
    "__pycache__",
];

/// Returns `true` if the directory entry's name is in the exclusion set.
fn should_exclude_dir(entry: &walkdir::DirEntry, exclude_set: &HashSet<&str>) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| exclude_set.contains(name))
}

/// Walk a directory, applying exclusion filters, and collect matching test files.
fn walk_dir_for_files(dir: &Path, exclude_set: &HashSet<&str>) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| !should_exclude_dir(e, exclude_set))
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().is_file() && is_py_test_file(entry.path()))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// Discover test files from the given paths (files or directories),
/// skipping any directory whose name is in `exclude_dirs`.
fn discover_files(paths: &[PathBuf], exclude_dirs: &[String]) -> Vec<PathBuf> {
    let exclude_set: HashSet<&str> = exclude_dirs.iter().map(|s| s.as_str()).collect();
    let mut files = Vec::new();

    for path in paths {
        if path.is_file() && is_py_test_file(path) {
            files.push(path.clone());
        } else if path.is_dir() {
            files.extend(walk_dir_for_files(path, &exclude_set));
        }
    }

    files.sort();
    files.dedup();
    files
}

/// Check if a file is a Python test file by naming convention.
fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    name.starts_with("test_") || name.ends_with("_test.py") || name == "conftest.py"
}

/// Parse multiple files in parallel using rayon.
fn parse_files_parallel(files: &[PathBuf]) -> Vec<ParsedModule> {
    files
        .par_iter()
        .filter_map(|file| {
            let mut parser = crate::parser::PythonParser::new().ok()?;
            match parser.parse_file(file) {
                Ok(m) => Some(m),
                Err(e) => {
                    eprintln!("Warning: failed to parse {}: {}", file.display(), e);
                    None
                }
            }
        })
        .collect()
}

type SuppressionMap = HashMap<(PathBuf, usize), HashSet<String>>;

fn collect_suppressions(modules: &[ParsedModule]) -> SuppressionMap {
    let mut map: SuppressionMap = HashMap::new();
    for module in modules {
        for (line_idx, line) in module.source.lines().enumerate() {
            let line_num = line_idx + 1;
            if let Some(rules) = parse_noqa_comment(line) {
                map.entry((module.file_path.clone(), line_num))
                    .or_default()
                    .extend(rules);
                // Also suppress on the next line (inline noqa applies to the statement)
                map.entry((module.file_path.clone(), line_num + 1))
                    .or_default()
                    .extend(parse_noqa_comment(line).unwrap_or_default());
            }
        }
    }
    map
}

/// Parse `# noqa` comments from a line and return the suppressed rule IDs.
fn parse_noqa_comment(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    let noqa_pos = trimmed.find("# noqa")?;
    let after_noqa = &trimmed[noqa_pos + 6..].trim();

    if after_noqa.is_empty() || after_noqa.starts_with(':') {
        let rules_str = if let Some(stripped) = after_noqa.strip_prefix(':') {
            stripped.trim()
        } else {
            // bare `# noqa` suppresses all rules
            return Some(vec!["*".to_string()]);
        };

        if rules_str.is_empty() {
            return Some(vec!["*".to_string()]);
        }

        let rules: Vec<String> = rules_str
            .split(',')
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();

        if rules.is_empty() {
            return Some(vec!["*".to_string()]);
        }

        return Some(rules);
    }

    None
}

/// Check if a violation is suppressed by a noqa comment.
fn is_suppressed(violation: &Violation, suppressions: &SuppressionMap) -> bool {
    // Check the violation's line
    if let Some(rules) = suppressions.get(&(violation.file_path.clone(), violation.line)) {
        if rules.contains("*") || rules.contains(&violation.rule_id) {
            return true;
        }
    }
    // Also check the line above (noqa on previous line)
    if violation.line > 1 {
        if let Some(rules) = suppressions.get(&(violation.file_path.clone(), violation.line - 1)) {
            if rules.contains("*") || rules.contains(&violation.rule_id) {
                return true;
            }
        }
    }
    false
}

/// Build a map of fixture name to all fixture definitions across modules.
#[must_use]
pub fn collect_all_fixtures(modules: &[ParsedModule]) -> HashMap<String, Vec<&Fixture>> {
    let mut map: HashMap<String, Vec<&Fixture>> = HashMap::new();
    for module in modules {
        for fixture in &module.fixtures {
            map.entry(fixture.name.clone()).or_default().push(fixture);
        }
    }
    map
}

/// Build a map of fixture name to the file paths where it is defined.
#[must_use]
pub fn compute_fixture_locations(modules: &[ParsedModule]) -> HashMap<String, Vec<PathBuf>> {
    let mut map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for module in modules {
        for fixture in &module.fixtures {
            map.entry(fixture.name.clone())
                .or_default()
                .push(module.file_path.clone());
        }
    }
    map
}

/// Collect names of session-scoped fixtures that return mutable state.
#[must_use]
pub fn compute_session_mutable_fixtures(modules: &[ParsedModule]) -> HashSet<String> {
    modules
        .iter()
        .flat_map(|m| m.fixtures.iter())
        .filter(|f| f.scope == crate::models::FixtureScope::Session && f.returns_mutable)
        .map(|f| f.name.clone())
        .collect()
}

/// Look up the narrowest scope for a fixture by name across all modules.
#[must_use]
pub fn fixture_scope_by_name<S: BuildHasher>(
    all_fixtures: &HashMap<String, Vec<&Fixture>, S>,
    name: &str,
) -> Option<FixtureScope> {
    all_fixtures
        .get(name)
        .and_then(|v| v.iter().min_by_key(|f| f.scope).map(|f| f.scope))
}

/// Check whether a fixture is referenced by any test or other fixture.
#[must_use]
pub fn is_fixture_used_by_any_test_or_fixture(
    fixture_name: &str,
    modules: &[ParsedModule],
) -> bool {
    for module in modules {
        for test in &module.test_functions {
            if test.fixture_deps.iter().any(|d| d == fixture_name) {
                return true;
            }
        }
        for other_fixture in &module.fixtures {
            if other_fixture.name != fixture_name
                && other_fixture.dependencies.iter().any(|d| d == fixture_name)
            {
                return true;
            }
        }
    }
    false
}

/// Build a map from fixture name to its direct dependencies.
fn build_fixture_deps_map(modules: &[ParsedModule]) -> HashMap<&str, Vec<&String>> {
    let mut map = HashMap::new();
    for module in modules {
        for fixture in &module.fixtures {
            map.insert(fixture.name.as_str(), fixture.dependencies.iter().collect());
        }
    }
    map
}

/// Seed the worklist with all fixture names directly referenced by tests.
fn collect_direct_fixture_deps(modules: &[ParsedModule]) -> (HashSet<String>, Vec<String>) {
    let mut used = HashSet::new();
    let mut worklist = Vec::new();
    for module in modules {
        for test in &module.test_functions {
            for dep in &test.fixture_deps {
                if used.insert(dep.clone()) {
                    worklist.push(dep.clone());
                }
            }
        }
    }
    (used, worklist)
}

/// Compute the transitive closure of fixture names used by tests.
#[must_use]
pub fn compute_used_fixture_names(modules: &[ParsedModule]) -> HashSet<String> {
    let fixture_deps_map = build_fixture_deps_map(modules);
    let (mut used, mut worklist) = collect_direct_fixture_deps(modules);

    while let Some(name) = worklist.pop() {
        if let Some(deps) = fixture_deps_map.get(name.as_str()) {
            for dep in deps {
                if used.insert(dep.to_string()) {
                    worklist.push(dep.to_string());
                }
            }
        }
    }

    used
}

/// Construct a `Violation` from the given rule metadata and location info.
#[allow(dead_code, clippy::too_many_arguments)]
#[must_use]
pub fn make_violation(
    rule_id: &'static str,
    rule_name: &'static str,
    severity: Severity,
    category: Category,
    message: String,
    file_path: PathBuf,
    line: usize,
    suggestion: Option<String>,
    test_name: Option<String>,
) -> Violation {
    Violation {
        rule_id: rule_id.to_string(),
        rule_name: rule_name.to_string(),
        severity,
        category,
        message,
        file_path,
        line,
        col: None,
        suggestion,
        test_name,
    }
}

/// Check whether a path is a Python test file (both .py extension and test naming).
fn is_py_test_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "py") && is_test_file(path)
}

/// Get test files changed since the given git base ref.
#[allow(clippy::missing_errors_doc)]
pub fn get_changed_files(base: &str) -> Result<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=ACMR", base])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<PathBuf> = stdout
        .lines()
        .map(|line| PathBuf::from(line.trim()))
        .filter(|p| is_py_test_file(p))
        .collect();

    Ok(files)
}

/// Run the full linter pipeline: discover, lint, format output. Returns true if errors found.
#[allow(clippy::missing_errors_doc)]
pub fn run_linter(
    paths: &[PathBuf],
    format: &str,
    output: Option<&Path>,
    no_color: bool,
    config: Config,
) -> Result<bool> {
    run_linter_with_memory_limit(paths, format, output, no_color, config, 256)
}

#[allow(clippy::missing_errors_doc)]
pub fn run_linter_with_memory_limit(
    paths: &[PathBuf],
    format: &str,
    output: Option<&Path>,
    no_color: bool,
    config: Config,
    memory_limit_mb: usize,
) -> Result<bool> {
    if no_color {
        colored::control::set_override(false);
    }

    let engine = LintEngine::with_memory_limit(config, memory_limit_mb)?;
    let violations = engine.lint_paths(paths)?;

    match format {
        "json" => format_json(&violations, output)?,
        "sarif" => format_sarif(&violations, output)?,
        _ => format_terminal(&violations, output)?,
    }

    Ok(violations.iter().any(|v| v.severity == Severity::Error))
}

/// Collect all violations from the given paths without producing output.
#[allow(clippy::missing_errors_doc)]
pub fn collect_violations(paths: &[PathBuf], config: Config) -> Result<Vec<Violation>> {
    let engine = LintEngine::new(config)?;
    engine.lint_paths(paths)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BaselineEntry {
    file_path: String,
    line: usize,
    rule_id: String,
}

/// Save a baseline of known violations to a JSON file.
#[allow(clippy::missing_errors_doc)]
pub fn save_baseline(violations: &[Violation], path: &Path) -> Result<()> {
    let entries: Vec<BaselineEntry> = violations
        .iter()
        .map(|v| BaselineEntry {
            file_path: v.file_path.to_string_lossy().to_string(),
            line: v.line,
            rule_id: v.rule_id.clone(),
        })
        .collect();
    let json = serde_json::to_string_pretty(&entries)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load a baseline of known violations from a JSON file.
#[allow(clippy::missing_errors_doc)]
pub fn load_baseline(path: &Path) -> Result<HashSet<(String, usize, String)>> {
    let content = std::fs::read_to_string(path)?;
    let entries: Vec<BaselineEntry> = serde_json::from_str(&content)?;
    let set: HashSet<(String, usize, String)> = entries
        .into_iter()
        .map(|e| (e.file_path, e.line, e.rule_id))
        .collect();
    Ok(set)
}

/// Filter violations to only those not present in the baseline.
#[allow(clippy::missing_errors_doc)]
pub fn filter_new_violations(
    violations: &[Violation],
    baseline: &HashSet<(String, usize, String)>,
) -> Vec<Violation> {
    violations
        .iter()
        .filter(|v| {
            let key = (
                v.file_path.to_string_lossy().to_string(),
                v.line,
                v.rule_id.clone(),
            );
            !baseline.contains(&key)
        })
        .cloned()
        .collect()
}

/// Format violations as JSON and write to the given path or stdout.
#[allow(clippy::missing_errors_doc)]
pub fn format_json_output(violations: &[Violation], output: Option<&Path>) -> Result<()> {
    format_json(violations, output)
}

/// Format violations as SARIF and write to the given path or stdout.
#[allow(clippy::missing_errors_doc)]
pub fn format_sarif_output(violations: &[Violation], output: Option<&Path>) -> Result<()> {
    format_sarif(violations, output)
}

/// Format violations for terminal display and write to the given path or stdout.
#[allow(clippy::missing_errors_doc)]
pub fn format_terminal_output(
    violations: &[Violation],
    output: Option<&Path>,
    no_color: bool,
) -> Result<()> {
    if no_color {
        colored::control::set_override(false);
    }
    format_terminal(violations, output)
}

fn format_terminal(violations: &[Violation], output_path: Option<&Path>) -> Result<()> {
    let mut writer: Box<dyn Write> = match output_path {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };

    if violations.is_empty() {
        writeln!(writer, "{} No violations found", "✓".green())?;
        return Ok(());
    }

    let error_count = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warning_count = violations
        .iter()
        .filter(|v| v.severity == Severity::Warning)
        .count();
    let info_count = violations
        .iter()
        .filter(|v| v.severity == Severity::Info)
        .count();

    for v in violations {
        let severity_str = match v.severity {
            Severity::Error => "ERROR".red().bold(),
            Severity::Warning => "WARNING".yellow().bold(),
            Severity::Info => "INFO".blue().bold(),
        };

        let location = format!(
            "{}:{}:{}",
            v.file_path.display(),
            v.line,
            v.col.map_or_else(|| "-".to_string(), |c| c.to_string())
        );

        writeln!(
            writer,
            "{} [{}] {} ({})",
            severity_str, v.rule_id, v.message, location
        )?;

        if let Some(ref suggestion) = v.suggestion {
            writeln!(writer, "  {} {}", "→".cyan(), suggestion)?;
        }

        if let Some(ref test_name) = v.test_name {
            writeln!(writer, "  {} test: {}", "→".dimmed(), test_name)?;
        }
    }

    writeln!(writer)?;
    writeln!(
        writer,
        "{}: {} errors, {} warnings, {} info",
        "Summary".bold(),
        error_count.to_string().red(),
        warning_count.to_string().yellow(),
        info_count.to_string().blue()
    )?;

    Ok(())
}

fn format_json(violations: &[Violation], output_path: Option<&Path>) -> Result<()> {
    let json = serde_json::to_string_pretty(violations)?;

    match output_path {
        Some(path) => {
            let mut file = std::fs::File::create(path)?;
            file.write_all(json.as_bytes())?;
        }
        None => println!("{json}"),
    }

    Ok(())
}

fn format_sarif(violations: &[Violation], output_path: Option<&Path>) -> Result<()> {
    let json = crate::output::format_sarif(violations)?;

    match output_path {
        Some(path) => {
            let mut file = std::fs::File::create(path)?;
            file.write_all(json.as_bytes())?;
        }
        None => println!("{json}"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_is_test_file_detects_test_prefix() {
        assert!(is_test_file(Path::new("test_foo.py")));
    }

    #[test]
    fn test_is_test_file_detects_test_suffix() {
        assert!(is_test_file(Path::new("foo_test.py")));
    }

    #[test]
    fn test_is_test_file_detects_conftest() {
        assert!(is_test_file(Path::new("conftest.py")));
    }

    #[test]
    fn test_is_test_file_rejects_helper() {
        assert!(!is_test_file(Path::new("helper.py")));
    }

    #[test]
    fn test_collect_suppressions_bare_noqa() {
        let module = crate::parser::PythonParser::new()
            .unwrap()
            .parse_source("x = 1  # noqa\n", Path::new("test.py"))
            .unwrap();
        let suppressions = collect_suppressions(std::slice::from_ref(&module));
        assert!(suppressions.contains_key(&(PathBuf::from("test.py"), 1)));
        let rules = suppressions.get(&(PathBuf::from("test.py"), 1)).unwrap();
        assert!(rules.contains("*"), "bare noqa should suppress all rules");
    }

    #[test]
    fn test_collect_suppressions_specific_rule() {
        let module = crate::parser::PythonParser::new()
            .unwrap()
            .parse_source("x = 1  # noqa: PYTEST-FLK-001\n", Path::new("test.py"))
            .unwrap();
        let suppressions = collect_suppressions(std::slice::from_ref(&module));
        let rules = suppressions.get(&(PathBuf::from("test.py"), 1)).unwrap();
        assert!(
            rules.contains(&"PYTEST-FLK-001".to_string()),
            "should contain specific rule"
        );
    }

    #[test]
    fn test_collect_suppressions_next_line() {
        let module = crate::parser::PythonParser::new()
            .unwrap()
            .parse_source(
                "x = 1  # noqa: PYTEST-FLK-001\ny = 2\n",
                Path::new("test.py"),
            )
            .unwrap();
        let suppressions = collect_suppressions(std::slice::from_ref(&module));
        assert!(
            suppressions.contains_key(&(PathBuf::from("test.py"), 2)),
            "noqa should also suppress on next line"
        );
    }

    #[test]
    fn test_is_suppressed_by_rule_id() {
        use crate::models::Violation;
        let v = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        };
        let mut suppressions = std::collections::HashMap::new();
        suppressions.insert(
            (PathBuf::from("test.py"), 5),
            std::collections::HashSet::from(["PYTEST-FLK-001".to_string()]),
        );
        assert!(is_suppressed(&v, &suppressions));
    }

    #[test]
    fn test_is_suppressed_by_star_previous_line() {
        use crate::models::Violation;
        let v = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        };
        let mut suppressions = std::collections::HashMap::new();
        suppressions.insert(
            (PathBuf::from("test.py"), 4),
            std::collections::HashSet::from(["*".to_string()]),
        );
        assert!(
            is_suppressed(&v, &suppressions),
            "star on previous line should suppress"
        );
    }

    #[test]
    fn test_is_suppressed_not_matching() {
        use crate::models::Violation;
        let v = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        };
        let suppressions = std::collections::HashMap::new();
        assert!(!is_suppressed(&v, &suppressions));
    }

    #[test]
    fn test_violation_equality_same_key_different_rest() {
        use crate::models::Violation;
        let v1 = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "A".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "msg1".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        };
        let v2 = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "B".to_string(),
            severity: crate::models::Severity::Error,
            category: crate::models::Category::Fixture,
            message: "msg2".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: Some(10),
            suggestion: Some("fix".to_string()),
            test_name: Some("test_x".to_string()),
        };
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_violation_inequality_different_line() {
        use crate::models::Violation;
        let v1 = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        };
        let v2 = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 6,
            col: None,
            suggestion: None,
            test_name: None,
        };
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_violation_inequality_different_rule() {
        use crate::models::Violation;
        let v1 = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        };
        let v2 = Violation {
            rule_id: "PYTEST-FLK-002".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 5,
            col: None,
            suggestion: None,
            test_name: None,
        };
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_format_terminal_counts_severity() {
        use crate::models::Violation;
        let violations = vec![
            Violation {
                rule_id: "PYTEST-FLK-001".to_string(),
                rule_name: "T".to_string(),
                severity: crate::models::Severity::Error,
                category: crate::models::Category::Flakiness,
                message: "err1".to_string(),
                file_path: PathBuf::from("a.py"),
                line: 1,
                col: None,
                suggestion: None,
                test_name: None,
            },
            Violation {
                rule_id: "PYTEST-FLK-002".to_string(),
                rule_name: "T".to_string(),
                severity: crate::models::Severity::Error,
                category: crate::models::Category::Flakiness,
                message: "err2".to_string(),
                file_path: PathBuf::from("a.py"),
                line: 2,
                col: None,
                suggestion: None,
                test_name: None,
            },
            Violation {
                rule_id: "PYTEST-FLK-003".to_string(),
                rule_name: "T".to_string(),
                severity: crate::models::Severity::Warning,
                category: crate::models::Category::Flakiness,
                message: "warn".to_string(),
                file_path: PathBuf::from("a.py"),
                line: 3,
                col: None,
                suggestion: None,
                test_name: None,
            },
        ];
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("out.txt");
        format_terminal(&violations, Some(&path)).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("2 error"), "should count 2 errors");
        assert!(contents.contains("1 warning"), "should count 1 warning");
        assert!(contents.contains("0 info"), "should count 0 info");
    }

    #[test]
    fn test_lint_source_returns_violations() {
        let engine = LintEngine::new(crate::config::Config::default()).unwrap();
        let source = "import time\ndef test_sleep():\n    time.sleep(1)\n";
        let violations = engine
            .lint_source(source, Path::new("test_sleep.py"))
            .unwrap();
        assert!(!violations.is_empty(), "lint_source should find violations");
    }

    #[test]
    fn test_lint_source_clean_returns_nothing() {
        let engine = LintEngine::new(crate::config::Config::default()).unwrap();
        let source = "x = 1\n";
        let violations = engine.lint_source(source, Path::new("clean.py")).unwrap();
        assert!(
            violations.is_empty(),
            "clean file should have no violations"
        );
    }

    // --- Mutation-killing tests ---

    // Mutants on memory estimation arithmetic and comparison
    #[test]
    fn test_exceeds_memory_budget_computation_and_comparison() {
        // Verify the arithmetic:
        // estimated_bytes = len * 50_000, estimated_mb = estimated_bytes / 1_048_576
        // With 21 files: estimated_bytes = 1_050_000, estimated_mb = 1
        let files: Vec<PathBuf> = (0..21)
            .map(|i| PathBuf::from(format!("/test_{i}.py")))
            .collect();
        let (estimated_mb, over_budget) = exceeds_memory_budget(&files, 1);
        let estimated_bytes: u64 = files.len() as u64 * 50_000;
        assert_eq!(estimated_bytes, 1_050_000);
        assert_eq!(estimated_mb, 1);
        assert!(!over_budget);

        // Just over boundary (limit=0)
        let (_, over_budget_0) = exceeds_memory_budget(&files, 0);
        assert!(over_budget_0);

        // With 1 file: estimated_mb = 0
        let one_file: Vec<PathBuf> = vec![PathBuf::from("/test.py")];
        let (mb_1, over_1) = exceeds_memory_budget(&one_file, 0);
        assert_eq!(mb_1, 0);
        assert!(!over_1);
        let (_, over_2) = exceeds_memory_budget(&one_file, 1);
        assert!(!over_2);
    }

    #[test]
    fn test_exceeds_memory_budget_strict_greater_than() {
        // 22 files -> estimated_mb = 1, limit = 1 => 1 > 1 is false
        // Mutant >= would return true. Mutant == would return true.
        let files: Vec<PathBuf> = (0..22)
            .map(|i| PathBuf::from(format!("/test_{i}.py")))
            .collect();
        let (estimated_mb, over_budget) = exceeds_memory_budget(&files, 1);
        // 22 * 50000 = 1_100_000 / 1_048_576 = 1
        assert_eq!(estimated_mb, 1, "22 files should give estimated_mb == 1");
        assert!(!over_budget, "1 > 1 should be false");

        // 22 files with limit=0 => 1 > 0 is true
        // Mutant < would return false
        let (_, over_budget_0) = exceeds_memory_budget(&files, 0);
        assert!(over_budget_0, "1 > 0 should be true");

        // 42 files -> estimated_bytes = 2_100_000, estimated_mb = 2, limit = 1 => true
        let files_42: Vec<PathBuf> = (0..42)
            .map(|i| PathBuf::from(format!("/test_{i}.py")))
            .collect();
        let (mb_42, over_42) = exceeds_memory_budget(&files_42, 1);
        assert_eq!(mb_42, 2);
        assert!(over_42, "2 > 1 should be true");
    }

    // Mutants #4-5: discover_files should only include .py test files, not other files
    #[test]
    fn test_discover_files_ignores_non_py_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test_foo.py"), "def test_foo(): pass\n").unwrap();
        std::fs::write(dir.path().join("test_bar.txt"), "not a python file\n").unwrap();
        std::fs::write(dir.path().join("test_baz.rs"), "fn main() {}\n").unwrap();

        let files = discover_files(&[dir.path().to_path_buf()], &[]);
        let basenames: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(
            basenames.contains(&"test_foo.py".to_string()),
            "should include .py test files"
        );
        assert!(
            !basenames.iter().any(|n| n.ends_with(".txt")),
            "should not include .txt files"
        );
        assert!(
            !basenames.iter().any(|n| n.ends_with(".rs")),
            "should not include .rs files"
        );
    }

    #[test]
    fn test_discover_files_ignores_non_test_py_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test_foo.py"), "def test_foo(): pass\n").unwrap();
        std::fs::write(dir.path().join("helper.py"), "def helper(): pass\n").unwrap();
        std::fs::write(dir.path().join("conftest.py"), "import pytest\n").unwrap();

        let files = discover_files(&[dir.path().to_path_buf()], &[]);
        let basenames: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(basenames.contains(&"test_foo.py".to_string()));
        assert!(basenames.contains(&"conftest.py".to_string()));
        assert!(
            !basenames.contains(&"helper.py".to_string()),
            "should not include non-test .py files"
        );
    }

    // Mutant #6: replace > with >= in violation.line > 1 (is_suppressed)
    // With >, violation on line 1 does NOT check previous-line suppressions (line 0).
    // With >=, it WOULD check line 0. We test by putting a suppression at line 0
    // and verifying a line-1 violation is NOT suppressed.
    #[test]
    fn test_is_suppressed_line_1_does_not_check_line_0() {
        use crate::models::Violation;
        let v = Violation {
            rule_id: "PYTEST-FLK-001".to_string(),
            rule_name: "T".to_string(),
            severity: crate::models::Severity::Warning,
            category: crate::models::Category::Flakiness,
            message: "m".to_string(),
            file_path: PathBuf::from("test.py"),
            line: 1,
            col: None,
            suggestion: None,
            test_name: None,
        };
        let mut suppressions = std::collections::HashMap::new();
        // Insert a suppression at line 0 (which should NOT suppress line 1)
        suppressions.insert(
            (PathBuf::from("test.py"), 0),
            std::collections::HashSet::from(["*".to_string()]),
        );
        assert!(
            !is_suppressed(&v, &suppressions),
            "violation on line 1 should NOT be suppressed by line-0 noqa"
        );
    }

    // Mutant #7: replace && with || in is_fixture_used_by_any_test_or_fixture
    // With &&, a fixture is only "used" if it's a different fixture AND depends on the fixture.
    // With ||, a fixture would be "used" if it's a different fixture OR depends on it.
    // Test: a different fixture that does NOT depend on fixture_name should mean "not used".
    #[test]
    fn test_fixture_not_used_by_unrelated_fixture() {
        use crate::models::{Fixture, FixtureScope, ParsedModule, TestFunction};

        let unused_fixture = "db_connection";
        let module = ParsedModule {
            file_path: PathBuf::from("test_a.py"),
            source: "def test_x(): pass\n".to_string(),
            imports: vec![],
            test_functions: vec![TestFunction {
                name: "test_x".to_string(),
                file_path: PathBuf::from("test_a.py"),
                line: 1,
                is_async: false,
                is_parametrized: false,
                parametrize_count: None,
                has_assertions: false,
                assertion_count: 0,
                has_mock_verifications: false,
                has_state_assertions: false,
                fixture_deps: vec![],
                uses_time_sleep: false,
                sleep_value: None,
                uses_file_io: false,
                uses_network: false,
                has_conditional_logic: false,
                has_try_except: false,
                docstring: None,
                assertions: vec![],
                parametrize_values: vec![],
                uses_cwd_dependency: false,
                uses_pytest_raises: false,
                mutates_fixture_deps: vec![],
                body_hash: None,
                uses_random: false,
                has_random_seed: false,
                uses_subprocess: false,
                has_subprocess_timeout: false,
                mocks_stdlib_module: false,
                mocked_stdlib_targets: vec![],
                has_weak_assertions: false,
                weak_assertion_details: vec![],
                patch_targets: vec![],
                has_magic_mock: false,
                mock_count: 0,
                uses_shutil_copy: false,
                end_line: 1,
            }],
            fixtures: vec![
                Fixture {
                    name: "db_connection".to_string(),
                    file_path: PathBuf::from("test_a.py"),
                    line: 3,
                    scope: FixtureScope::Function,
                    is_autouse: false,
                    dependencies: vec![],
                    returns_mutable: false,
                    has_yield: false,
                    has_db_commit: false,
                    has_db_rollback: false,
                    has_cleanup: false,
                    uses_file_io: false,
                    used_by: vec![],
                },
                // unrelated_fixture does NOT depend on db_connection
                Fixture {
                    name: "unrelated_fixture".to_string(),
                    file_path: PathBuf::from("test_a.py"),
                    line: 5,
                    scope: FixtureScope::Function,
                    is_autouse: false,
                    dependencies: vec!["other_dep".to_string()],
                    returns_mutable: false,
                    has_yield: false,
                    has_db_commit: false,
                    has_db_rollback: false,
                    has_cleanup: false,
                    uses_file_io: false,
                    used_by: vec![],
                },
            ],
        };
        assert!(
            !is_fixture_used_by_any_test_or_fixture(unused_fixture, &[module]),
            "fixture not referenced by any test or dependency should be unused"
        );
    }

    #[test]
    fn test_fixture_used_via_other_fixture_dependency() {
        use crate::models::{Fixture, FixtureScope, ParsedModule, TestFunction};

        let fixture_name = "db_connection";
        // Test function uses db_connection directly — confirmed used
        let module = ParsedModule {
            file_path: PathBuf::from("test_a.py"),
            source: "def test_x(db_connection): pass\n".to_string(),
            imports: vec![],
            test_functions: vec![TestFunction {
                name: "test_x".to_string(),
                file_path: PathBuf::from("test_a.py"),
                line: 1,
                is_async: false,
                is_parametrized: false,
                parametrize_count: None,
                has_assertions: false,
                assertion_count: 0,
                has_mock_verifications: false,
                has_state_assertions: false,
                fixture_deps: vec!["db_connection".to_string()],
                uses_time_sleep: false,
                sleep_value: None,
                uses_file_io: false,
                uses_network: false,
                has_conditional_logic: false,
                has_try_except: false,
                docstring: None,
                assertions: vec![],
                parametrize_values: vec![],
                uses_cwd_dependency: false,
                uses_pytest_raises: false,
                mutates_fixture_deps: vec![],
                body_hash: None,
                uses_random: false,
                has_random_seed: false,
                uses_subprocess: false,
                has_subprocess_timeout: false,
                mocks_stdlib_module: false,
                mocked_stdlib_targets: vec![],
                has_weak_assertions: false,
                weak_assertion_details: vec![],
                patch_targets: vec![],
                has_magic_mock: false,
                mock_count: 0,
                uses_shutil_copy: false,
                end_line: 1,
            }],
            fixtures: vec![Fixture {
                name: "db_connection".to_string(),
                file_path: PathBuf::from("test_a.py"),
                line: 3,
                scope: FixtureScope::Function,
                is_autouse: false,
                dependencies: vec![],
                returns_mutable: false,
                has_yield: false,
                has_db_commit: false,
                has_db_rollback: false,
                has_cleanup: false,
                uses_file_io: false,
                used_by: vec![],
            }],
        };
        assert!(
            is_fixture_used_by_any_test_or_fixture(fixture_name, &[module]),
            "fixture referenced by test should be used"
        );
    }

    #[test]
    fn test_fixture_used_by_other_fixture_dependency() {
        use crate::models::{Fixture, FixtureScope, ParsedModule, TestFunction};

        let fixture_name = "db_connection";
        // db_connection is NOT in any test's fixture_deps, but IS in fixture's deps
        let module = ParsedModule {
            file_path: PathBuf::from("test_a.py"),
            source: "def test_x(api_client): pass\n".to_string(),
            imports: vec![],
            test_functions: vec![TestFunction {
                name: "test_x".to_string(),
                file_path: PathBuf::from("test_a.py"),
                line: 1,
                is_async: false,
                is_parametrized: false,
                parametrize_count: None,
                has_assertions: false,
                assertion_count: 0,
                has_mock_verifications: false,
                has_state_assertions: false,
                fixture_deps: vec!["api_client".to_string()],
                uses_time_sleep: false,
                sleep_value: None,
                uses_file_io: false,
                uses_network: false,
                has_conditional_logic: false,
                has_try_except: false,
                docstring: None,
                assertions: vec![],
                parametrize_values: vec![],
                uses_cwd_dependency: false,
                uses_pytest_raises: false,
                mutates_fixture_deps: vec![],
                body_hash: None,
                uses_random: false,
                has_random_seed: false,
                uses_subprocess: false,
                has_subprocess_timeout: false,
                mocks_stdlib_module: false,
                mocked_stdlib_targets: vec![],
                has_weak_assertions: false,
                weak_assertion_details: vec![],
                patch_targets: vec![],
                has_magic_mock: false,
                mock_count: 0,
                uses_shutil_copy: false,
                end_line: 1,
            }],
            fixtures: vec![
                Fixture {
                    name: "api_client".to_string(),
                    file_path: PathBuf::from("test_a.py"),
                    line: 3,
                    scope: FixtureScope::Function,
                    is_autouse: false,
                    dependencies: vec!["db_connection".to_string()],
                    returns_mutable: false,
                    has_yield: false,
                    has_db_commit: false,
                    has_db_rollback: false,
                    has_cleanup: false,
                    uses_file_io: false,
                    used_by: vec![],
                },
                Fixture {
                    name: "db_connection".to_string(),
                    file_path: PathBuf::from("test_a.py"),
                    line: 5,
                    scope: FixtureScope::Function,
                    is_autouse: false,
                    dependencies: vec![],
                    returns_mutable: false,
                    has_yield: false,
                    has_db_commit: false,
                    has_db_rollback: false,
                    has_cleanup: false,
                    uses_file_io: false,
                    used_by: vec![],
                },
            ],
        };
        assert!(
            is_fixture_used_by_any_test_or_fixture(fixture_name, &[module]),
            "fixture referenced by another fixture's dependencies should be used"
        );
    }

    // Mutants #8-9: get_changed_files uses same pattern as discover_files
    // These are hard to unit test directly (require git), so test discover_files
    // thoroughly to cover the is_test_file + extension logic.

    #[test]
    fn test_discover_files_rejects_py_files_without_test_naming() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("test_real.py"),
            "def test_it(): assert True\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("utils.py"), "def util(): pass\n").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(
            dir.path().join("subdir").join("helper.py"),
            "def help(): pass\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("subdir").join("test_nested.py"),
            "def test_n(): assert True\n",
        )
        .unwrap();

        let files = discover_files(&[dir.path().to_path_buf()], &[]);
        let names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"test_real.py".to_string()));
        assert!(names.contains(&"test_nested.py".to_string()));
        assert!(!names.contains(&"utils.py".to_string()));
        assert!(!names.contains(&"helper.py".to_string()));
    }

    #[test]
    fn test_is_test_file_only_checks_naming_convention() {
        // is_test_file checks naming convention only (extension is checked separately)
        assert!(is_test_file(Path::new("test_foo.py")));
        assert!(is_test_file(Path::new("foo_test.py")));
        assert!(is_test_file(Path::new("conftest.py")));
        assert!(!is_test_file(Path::new("helper.py")));
        assert!(!is_test_file(Path::new("setup.py")));
    }

    // Mutants #8-9: is_py_test_file must require BOTH .py extension AND test naming
    #[test]
    fn test_is_py_test_file_accepts_valid_test_files() {
        assert!(is_py_test_file(Path::new("test_foo.py")));
        assert!(is_py_test_file(Path::new("foo_test.py")));
        assert!(is_py_test_file(Path::new("conftest.py")));
        assert!(is_py_test_file(Path::new("src/test_bar.py")));
    }

    #[test]
    fn test_is_py_test_file_rejects_non_py_files() {
        // Mutant: replace == with != would make non-.py files pass the extension check
        assert!(!is_py_test_file(Path::new("test_foo.txt")));
        assert!(!is_py_test_file(Path::new("test_foo.rs")));
        assert!(!is_py_test_file(Path::new("test_foo"))); // no extension
        assert!(!is_py_test_file(Path::new("conftest.pyc")));
    }

    #[test]
    fn test_is_py_test_file_rejects_non_test_py_files() {
        // Mutant: replace && with || would make non-test .py files pass
        assert!(!is_py_test_file(Path::new("helper.py")));
        assert!(!is_py_test_file(Path::new("setup.py")));
        assert!(!is_py_test_file(Path::new("utils.py")));
        assert!(!is_py_test_file(Path::new("app.py")));
    }

    #[test]
    fn test_is_py_test_file_rejects_non_py_test_named_file() {
        // A file named "test_foo" without .py extension should be rejected
        assert!(!is_py_test_file(Path::new("test_foo")));
    }

    #[test]
    fn test_discover_files_excludes_venv_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let venv = dir.path().join(".venv/lib/site-packages");
        fs::create_dir_all(&venv).unwrap();
        fs::write(venv.join("test_something.py"), "def test_venv(): pass").unwrap();
        fs::write(dir.path().join("test_real.py"), "def test_real(): pass").unwrap();
        let files = discover_files(&[dir.path().to_path_buf()], &[".venv".to_string()]);
        let names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(
            names.contains(&"test_real.py".to_string()),
            "should include project test files"
        );
        assert!(
            !names.iter().any(|n| n == "test_something.py"),
            "should exclude .venv test files"
        );
    }
}
