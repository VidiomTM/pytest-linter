---
id: ADR-004
kind: adr
title: "Testing Strategy"
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

The project needs a testing strategy that ensures correctness, prevents regressions, and measures test effectiveness.

## Decision

### Unit Testing

- **Framework**: Built-in `cargo test` with `#[cfg(test)]` modules co-located with source.
- **Mocking**: Minimal — tests use real `PythonParser` and real tree-sitter parsing. In-memory temp files (`tempfile` crate) for file-system-dependent tests.
- **Target**: All public functions in `engine.rs`, `models.rs`, `config.rs`, `rules/`, `parser.rs`.
- **Pattern**: Inline test modules within each source file (e.g., `src/engine.rs` tests at bottom of file).

Key test areas:
  - `Violation` equality and ordering (partial equality on file+line+rule_id)
  - `Severity` / `Category` / `FixtureScope` display and ordering
  - `Config` discovery, merging, overrides, effective rules for file
  - File discovery: test naming conventions, non-PY exclusion, directory exclusion
  - `is_suppressed`: star noqa, specific rule noqa, previous-line suppression, line-1 edge case
  - `is_fixture_used_by_any_test_or_fixture`: direct test dep, transitive fixture dep, unrelated fixture
  - Memory budget estimation and strict-greater-than comparison
  - `lint_source`: clean file returns no violations, smell file returns violations

### Coverage

- **Tool**: `cargo-llvm-cov` with `--cobertura` output.
- **Threshold**: Minimum 90% line coverage on `src/`.
- **CI Enforcement**: `cargo +nightly llvm-cov --fail-under-lines 90`.
- **Reporting**: Cobertura XML uploaded as CI artifact and to SonarCloud.

### Mutation Testing

- **Tool**: `cargo-mutants` via `mutants.toml`.
- **Coverage**: Dedicated `mutation-testing.yml` workflow.
- **Tests targetting mutation coverage**: Specific tests in `engine.rs` verify:
  - Memory budget comparison operator (`>` vs `>=`)
  - File discovery extension filtering (`.py` only, test naming only)
  - Suppression line-1 boundary (`violation.line > 1` vs `>= 1`)
  - Fixture usage condition (`&&` vs `||`)
  - Discover files non-PY file exclusion
  - Non-test PY file exclusion

### Benchmarking

- **Framework**: `criterion` with `html_reports`.
- **Location**: `benches/engine_bench.rs`.
- **CI**: Smoke test with `cargo bench -- --test` (single iteration).
- **Purpose**: Track performance regressions in file discovery, parsing, and rule dispatch.

### SonarCloud Integration

- **Project Key**: `Jonathangadeaharder_pytest-linter`
- **Coverage Report**: Cobertura XML at `coverage/cobertura.xml`
- **Static Analysis**: Bugs, vulnerabilities, code smells, security hotspots
- **Duplication**: Source directories `src/` and `lsp-server/src/`. CPD exclusions for `src/rules/` (rule implementations are structurally similar by design).

## Consequences

- **Positive**: Co-located tests stay close to code, reducing maintenance overhead.
- **Positive**: Real parsing (no mock ASTs) ensures tests validate the actual tree-sitter integration.
- **Positive**: Mutation testing validates test quality, not just coverage metrics.
- **Positive**: SonarCloud provides continuous static analysis alongside coverage.
- **Negative**: Real parsing makes tests slower than pure-unit tests (tree-sitter initialization per test).
- **Negative**: Mutation testing is slow (requires re-running test suite for each mutant) — run as separate workflow, not in PR gate.
