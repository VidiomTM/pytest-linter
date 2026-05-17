---
id: ADR-002
kind: adr
title: "Toolchain"
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

The project requires a consistent, reproducible development toolchain for building, formatting, linting, and type-checking Rust code.

## Decision

### Primary Toolchain

| Tool | Purpose | Version/Config |
|---|---|---|
| `cargo` | Build, test, bench | Rust edition 2021 |
| `rustfmt` | Code formatting | Stable channel. Checked via `cargo fmt --check`. |
| `clippy` | Linting | Nightly channel. Flags: `-W clippy::all -W clippy::pedantic -W clippy::nursery`. |
| `cargo-llvm-cov` | Code coverage | Nightly channel, `llvm-tools-preview` component. Cobertura XML output. |
| `cargo-audit` | Dependency security audit | Stable channel. |
| `cargo-mutants` | Mutation testing | Via `mutants.toml`. |
| `criterion` | Benchmarks | Dev dependency with `html_reports` feature. |

### GitHub Actions Infrastructure

All workflows run on **self-hosted macOS runners**. Standard actions used:

- `actions/checkout@v4` — checkout
- `dtolnay/rust-toolchain@stable` or `@nightly` — Rust toolchain installation
- `Swatinem/rust-cache@v2` — dependency caching
- `taiki-e/install-action@*` — tool installation (`cargo-llvm-cov`, `cargo-audit`)

### Build Configuration

Workspace with two members:

```toml
[workspace]
members = [".", "lsp-server"]
```

The `lsp-server` crate (`pytest-linter-lsp`) depends on the main crate via path dependency.

### Key Dependencies

| Crate | Purpose |
|---|---|
| `tree-sitter = "0.24"` | General-purpose parser framework |
| `tree-sitter-python = "0.23"` | Python grammar for tree-sitter |
| `clap = "4"` | CLI argument parsing (derive API) |
| `serde + serde_json` | Serialization for config, JSON output, SARIF |
| `tower-lsp = "0.20"` | LSP server framework (lsp-server crate only) |
| `tokio = "1"` | Async runtime (lsp-server crate only) |
| `rayon = "1"` | Parallel file parsing |
| `walkdir = "2"` | Directory traversal |
| `colored = "2"` | Terminal output coloring |
| `toml = "0.8"` | Config file parsing |
| `glob = "0.3"` | Glob pattern matching for per-file overrides |

## Consequences

- **Positive**: Consistent toolchain across CI and local development.
- **Positive**: Nightly clippy catches nursery-level issues proactively.
- **Positive**: Self-hosted runners avoid macOS CI minute costs and provide consistent hardware.
- **Negative**: Nightly clippy may introduce new warnings on toolchain upgrades.
- **Negative**: Self-hosted runners require maintenance (brew updates, toolchain installation).
