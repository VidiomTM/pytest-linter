---
id: SPEC-CORE-RULE-EN
kind: spec
title: "SPEC: Core Rule Engine"
status: draft
authors: []
reviewers: []
tags: []
supersedes: []
superseded_by: []
depends_on: []
blocks: []
implements: []
related: []
external: []
project: pytest-linter
---

**Date:** 2026-05-17  

---

## 1. Overview

The core rule engine is the heart of pytest-linter. It orchestrates file discovery, parallel parsing, cross-module analysis, single-pass rule dispatch, suppression handling, and output formatting. This SPEC covers the engine's design, data flow, and extension points.

## 2. Data Flow

```
                          ┌─────────────────┐
                          │  Config (CLI +   │
                          │  file discovery) │
                          └────────┬────────┘
                                   │
                          ┌────────▼────────┐
                          │  File Discovery │
                          │  (walkdir)      │
                          └────────┬────────┘
                                   │
                          ┌────────▼────────┐
                          │  Parallel Parse │
                          │  (rayon par_iter)│
                          └────────┬────────┘
                                   │
                          ┌────────▼────────┐
                          │ Cross-module    │
                          │ Context Build   │
                          └────────┬────────┘
                                   │
                    ┌──────────────┼──────────────┐
                    │              │              │
            ┌───────▼───────┐ ┌───▼────┐ ┌──────▼──────┐
            │ RuleDispatch  │ │ Noqa   │ │ Output      │
            │ per module    │ │ filter │ │ Format      │
            └───────────────┘ └────────┘ └─────────────┘
```

## 3. Key Components

### 3.1 Config (`Config`)

- **Discovery order**: CLI args > `pytest-linter.toml` > `pyproject.toml [tool.pytest-linter]` > built-in defaults.
- **Per-file overrides**: Glob patterns map paths to rule enable/severity overrides.
- **Merge strategy**: Later sources override earlier sources. `None` values never override explicit `Some(value)`.

### 3.2 File Discovery (`discover_files`)

- Accepts a list of file or directory paths.
- Filters by Python test naming: `test_*`, `*_test.py`, `conftest.py`.
- Excludes directories matching `DEFAULT_EXCLUDED_DIRS` (`.git`, `venv`, `node_modules`, etc.) plus user-configured `excludes`.
- Returns sorted, deduplicated list of `PathBuf`.

### 3.3 Parallel Parsing (`parse_files_parallel`)

```
fn parse_files_parallel(files: &[PathBuf]) -> Vec<ParsedModule>
```

- Uses `rayon::par_iter().filter_map()`.
- Each file: read → tree-sitter parse → extract metadata → drop source text.
- Failed parses produce a warning and are excluded.

### 3.4 Cross-Module Context

Built once from all parsed modules before rule dispatch:

| Function | Output | Purpose |
|---|---|---|
| `collect_all_fixtures` | `HashMap<String, Vec<&Fixture>>` | All fixtures by name |
| `compute_used_fixture_names` | `HashSet<String>` | Transitive closure of fixture deps from tests |
| `compute_fixture_locations` | `HashMap<String, Vec<PathBuf>>` | File paths per fixture name |
| `compute_session_mutable_fixtures` | `HashSet<String>` | Session-scoped fixtures returning mutable state |

### 3.5 Rule Dispatch (`RuleDispatcher`)

```
impl RuleDispatcher {
    fn check_module(&self, module, all_modules, ctx, config) -> Vec<Violation>
}
```

Algorithm per module:
1. Resolve per-file effective config via `config.effective_rules_for_file()`.
2. For each registered rule:
   a. Check if enabled in effective config (default: enabled).
   b. Determine severity (override or default).
   c. Call `rule.check(module, all_modules, ctx)`.
   d. Apply severity override to returned violations.
   e. Collect.

### 3.6 Suppression (`# noqa`)

- Bare `# noqa` suppresses all rules on current and next line.
- `# noqa: PYTEST-FLK-001, PYTEST-MNT-001` suppresses specific rules.
- Implemented as `SuppressionMap: HashMap<(PathBuf, usize), HashSet<String>>`.

### 3.7 Output Formatting

| Format | Function | Use Case |
|---|---|---|
| Terminal | `format_terminal` | CLI default, colored output |
| JSON | `format_json` | Machine consumption, tooling |
| SARIF | `format_sarif` (via `output/`) | GitHub Code Scanning, CI |

### 3.8 Memory Management

- **Budget**: 256 MB default, configurable.
- **Estimation**: `files.len() * 50_000 bytes / 1_048_576`.
- **Warning**: Emitted if estimation exceeds budget (strict `>` comparison).
- **Streaming**: Files parsed and metadata extracted before moving to next phase.

## 4. Rule Architecture

### 4.1 Rule Trait

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;       // e.g., "PYTEST-FLK-001"
    fn name(&self) -> &'static str;     // e.g., "time-sleep-in-test"
    fn severity(&self) -> Severity;      // default severity
    fn category(&self) -> Category;      // Flakiness, Maintenance, Fixture, etc.
    fn check(&self, module, all_modules, ctx) -> Vec<Violation>;
}
```

### 4.2 Adding a New Rule

1. Create a new struct in the appropriate category module (or a new module).
2. Implement `Rule` trait.
3. Register in `all_rules()` in `rules/mod.rs`.
4. Update `all_rules_count` test expectations.
5. Add to `expected_rule_ids_present` test.

### 4.3 Rule Categorization

Rules are grouped by module:

| Module | Category | Examples |
|---|---|---|
| `rules/flakiness.rs` | Flakiness | `TimeSleepRule`, `FileIoRule`, `NetworkImportRule`, `RandomWithoutSeedRule`, `DatetimeInAssertionRule` |
| `rules/maintenance.rs` | Maintenance | `TestLogicRule`, `MagicAssertRule`, `NoAssertionRule`, `ParametrizeDuplicateRule`, `SleepWithValueRule` |
| `rules/fixtures.rs` | Fixture | `AutouseFixtureRule`, `InvalidScopeRule`, `ShadowedFixtureRule`, `StatefulSessionFixtureRule` |
| `rules/mocking.rs` | Mocking | `PatchTargetingDefinitionModuleRule`, `MagicMockOnAsyncRule` |
| `rules/infrastructure.rs` | Infrastructure | `NetworkBanMissingRule`, `LiveSuiteUnmarkedRule`, `MacOsCopyArtefactRule` |

## 5. LSP Integration

The LSP server (`lsp-server/`) wraps `LintEngine` to provide in-editor diagnostics:

```
did_open/did_change
  → Backend::lint_document(uri, text, config)
    → LintEngine::lint_source(text, file_path)
      → PythonParser::parse_source()
      → RuleDispatcher::check_module()
      → violations → Vec<Diagnostic>
```

## 6. Extension Points

| Extension | What to Modify |
|---|---|
| New lint rule | `rules/<module>.rs` + `rules/mod.rs` |
| New output format | `src/output/` + `run_linter` format match |
| New config source | `Config::discover()` + new `from_*()` method |
| New parser backend | Replace `PythonParser` implementation while keeping `ParsedModule` interface |
| New LSP capability | `lsp-server/src/main.rs` `ServerCapabilities` |

## 7. Open Questions

- Should the engine support incremental re-parsing (only changed files)?
- Should `RuleContext` be extended with a source-text query API for rules that need line-level context?
- Should suppression support file-level `# noqa` (entire file)?
