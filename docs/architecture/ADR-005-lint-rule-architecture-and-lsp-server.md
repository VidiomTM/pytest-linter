---
id: ADR-005
kind: adr
title: "Lint Rule Architecture + LSP Server Design"
status: draft
date: 2026-05-17
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

**Deciders:** Jonathangadeaharder  

---

## Context

The linter needs a rule system that is extensible, testable, and performant. The LSP server needs to reuse the linter engine to provide in-editor diagnostics.

## Decision

### Lint Rule Architecture

#### Rule Trait

All rules implement the `Rule` trait (`src/rules/mod.rs:14`):

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn severity(&self) -> Severity;
    fn category(&self) -> Category;
    fn check(&self, module: &ParsedModule, all_modules: &[ParsedModule], ctx: &RuleContext) -> Vec<Violation>;
}
```

- `Send + Sync` enables parallel rule execution (though current dispatch is sequential per module).
- `id()` returns a stable identifier (e.g., `PYTEST-FLK-001`).
- `check()` receives the current module, all modules (for cross-module analysis), and a `RuleContext` with pre-computed fixture maps.

#### Rule Categories (5)

| Category | Prefix | Count | Example Rules |
|---|---|---|---|
| Flakiness | `PYTEST-FLK-*`, `PYTEST-XDIST-*` | 9 | TimeSleep, FileIo, NetworkImport, RandomWithoutSeed |
| Maintenance | `PYTEST-MNT-*`, `PYTEST-BDD-*`, `PYTEST-PBT-*`, `PYTEST-PARAM-*`, `PYTEST-DBC-*` | 14 | TestLogic, MagicAssert, NoAssertion, ParametrizeDuplicate |
| Fixtures | `PYTEST-FIX-*` | 13 | AutouseFixture, InvalidScope, ShadowedFixture, UnusedFixture |
| Mocking | (in `mocking.rs`) | 4 | PatchTargetingDefinitionModule, MagicMockOnAsync |
| Infrastructure | (in `infrastructure.rs`) | 4 | NetworkBanMissing, LiveSuiteUnmarked |

Total: 49 rules (as of this writing).

#### Rule Registration

All rules registered in `all_rules()` (`src/rules/mod.rs:40`), returned as `Vec<Box<dyn Rule>>`.

#### Rule Dispatch

`RuleDispatcher` (`src/engine.rs:19`) iterates all enabled rules per module in a single pass:

1. Resolve per-file config (global + glob overrides).
2. Skip disabled rules.
3. Apply severity override from config.
4. Call `rule.check()`.
5. Collect violations.

This avoids N rule traversals of the module data.

#### RuleContext

Pre-computed cross-module data passed to all rules:

```rust
pub struct RuleContext<'a> {
    pub fixture_map: &'a HashMap<String, Vec<&'a Fixture>>,
    pub used_fixture_names: &'a HashSet<String>,
    pub fixture_locations: &'a HashMap<String, Vec<PathBuf>>,
    pub session_mutable_fixtures: &'a HashSet<String>,
}
```

#### Suppression

Inline `# noqa` comments suppress violations by rule ID or globally (`*`). Suppression applies to the comment line and the next line. Implemented in `engine.rs:302-371`.

### LSP Server Design

#### Architecture

- **Framework**: `tower-lsp` (v0.20) on `tokio` runtime.
- **Location**: `lsp-server/` workspace member (`pytest-linter-lsp`).
- **Dependency**: `pytest-linter = { path = ".." }` — reuses the library crate.

#### Server Capabilities

| Capability | Value |
|---|---|
| Text sync | `FULL` (send entire file on change) |
| Open/Close | Yes |
| Incremental changes | No (full resend) |

#### Diagnostic Flow

1. `did_open`: Client opens a file → full text sent → `LintEngine::lint_source()` called → diagnostics published.
2. `did_change`: Client edits file → full text sent → re-lint → diagnostics updated.
3. `lint_source` creates a fresh `PythonParser`, parses the source, optionally merges with context modules, runs rule dispatch, returns violations mapped to LSP `Diagnostic`.

#### Config Discovery

On `initialize`, discovers config from workspace root and stores in `Arc<RwLock<Config>>`. Config is read on each lint request (cheap clone via `Config::clone()`).

#### Key Implementation (`lsp-server/src/main.rs:90-131`)

```rust
fn lint_document(uri: &Url, text: &str, config: &Config) -> Vec<Diagnostic> {
    let engine = LintEngine::new(config.clone())?;
    let violations = engine.lint_source(text, &file_path)?;
    violations.into_iter().map(|v| Diagnostic { ... }).collect()
}
```

## Consequences

- **Positive**: Adding a new rule requires only implementing the `Rule` trait and registering in `all_rules()`.
- **Positive**: LSP server reuses the full linter engine with zero duplication — same config, same rules, same output logic.
- **Positive**: Single-pass dispatch ensures linear scaling with rule count.
- **Positive**: `RuleContext` gives rules access to cross-module data without each rule computing it independently.
- **Negative**: Full-text sync is wasteful for small edits. Mitigation: acceptable for lint-level file sizes (test files are typically small).
- **Negative**: Each `lint_source` call recreates a `PythonParser` (tree-sitter initialization overhead). Mitigation: negligible for interactive use; batch scanning uses the file-path path with parallel parsing.
- **Negative**: Rule trait has no access to raw AST nodes — only `ParsedModule` metadata. Rules needing AST-level patterns must add fields to `ParsedModule`/`TestFunction`.
