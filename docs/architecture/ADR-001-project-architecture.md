---
id: ADR-001
kind: adr
title: "Project Architecture"
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

pytest-linter is a Rust CLI tool for linting Pytest test suites. It needs to parse Python test files, detect common anti-patterns and flakiness sources, and report violations in multiple formats. The tool must scale to large codebases (~10K test files) while keeping memory under 256 MB.

## Decision

### Architecture Overview

```
User input (paths) --> Config discovery --> File discovery --> Parallel parsing
    --> Cross-module context --> Rule dispatch --> Output formatting
```

### Components

| Component | File | Responsibility |
|---|---|---|
| `main.rs` | `src/main.rs` | CLI entry point, argument parsing via `clap` |
| `config.rs` | `src/config.rs` | Config discovery: `pyproject.toml` → `pytest-linter.toml` → defaults. Per-file override resolution via glob patterns. |
| `engine.rs` | `src/engine.rs` | `LintEngine`: orchestrates file discovery, parallel parsing, rule dispatch, suppression handling. `RuleDispatcher`: single-pass rule iteration. |
| `parser.rs` | `src/parser.rs` | Python source parsing via tree-sitter. Converts source to `ParsedModule` (lightweight metadata, no retained source text after extraction). |
| `models.rs` | `src/models.rs` | Core data types: `Violation`, `TestFunction`, `Fixture`, `ParsedModule`, `Severity`, `Category`, `FixtureScope`. |
| `rules/` | `src/rules/` | 49 lint rules across 5 categories (flakiness, maintenance, fixtures, mocking, infrastructure). Single `Rule` trait with `check()`. |
| `output/` | `src/output/` | Output formatters: terminal (colored), JSON, SARIF. |
| `lib.rs` | `src/lib.rs` | Public API surface for library consumers (LSP server, tests). |

### Data Flow

1. **Config discovery**: Walks up from target directory, merges `pyproject.toml [tool.pytest-linter]` then `pytest-linter.toml` (standalone takes priority). CLI args override all.
2. **File discovery**: `walkdir` traversal, filtering by `test_*`/`*_test.py`/`conftest.py` naming. Excludes venvs, `.git`, caches.
3. **Parallel parsing**: Rayon `par_iter` over files. Each file is read, parsed by tree-sitter, and reduced to a `ParsedModule` struct (fixtures, test functions, imports — source text dropped).
4. **Cross-module context**: Builds fixture maps (`collect_all_fixtures`), used-fixture sets (`compute_used_fixture_names`), session-mutable sets (`compute_session_mutable_fixtures`), and location maps (`compute_fixture_locations`).
5. **Rule dispatch**: `RuleDispatcher` iterates all enabled rules per module in a single pass, applying per-file severity/disable overrides.
6. **Suppression**: Collects `# noqa` comments, filters violations.
7. **Output**: Terminal (default), JSON, or SARIF.

### Memory Budget

Default 256 MB. Memory estimation: ~50 KB per file × file count. Peak RSS for 10K test files estimated at 20–75 MB. Warning emitted if estimation exceeds budget.

### Key Design Properties

- **Single-pass rule dispatch**: Rules do not walk the module independently — the dispatcher owns iteration, minimizing redundant work.
- **No retained source**: Source strings are dropped after parsing. Only extracted metadata survives in `ParsedModule`.
- **Parallel by default**: File parsing uses rayon `par_iter`. Rule checking is sequential per module (rules share the same module data).
- **Config priority**: CLI > pytest-linter.toml > pyproject.toml > defaults.
- **Baseline support**: Save/load known violations as JSON baseline to allow incremental adoption.

## Consequences

- **Positive**: Low memory footprint enables linting large monorepos. Single-pass dispatch avoids O(n_rules × n_files) overhead. Parallel parsing scales with cores.
- **Positive**: Clear separation between parsing, analysis, and presentation enables independent evolution of each layer.
- **Negative**: Source text is unavailable to rules (only metadata). Rules that need raw source context cannot exist in the current architecture — would require a separate source-retaining path.
- **Negative**: Cross-module analysis (e.g., "is this fixture used?") requires holding all `ParsedModule` structs in memory simultaneously, which trades peak memory for analytical power.
