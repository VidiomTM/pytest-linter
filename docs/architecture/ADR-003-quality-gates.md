---
id: ADR-003
kind: adr
title: "Quality Gates"
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

The project needs automated quality gates to ensure code quality, security, and correctness before merging changes.

## Decision

### Workflow: CI (`ci.yml`)

Triggered on pushes and PRs to `main`/`master`/`develop`. Runs on self-hosted macOS runner. Jobs are independent (except dogfood which depends on test):

| Job | What it does | Required for merge |
|---|---|---|
| **fmt** | `cargo fmt --check` | Yes |
| **clippy** | `cargo +nightly clippy --lib -- -W clippy::all -W clippy::pedantic -W clippy::nursery` | Yes |
| **test** | `cargo test` (all unit + integration tests) | Yes |
| **coverage** | `cargo +nightly llvm-cov --cobertura --output-path coverage/cobertura.xml --fail-under-lines 90` + SonarCloud scan | Yes |
| **bench** | `cargo bench -- --test` (smoke, single iteration) | No |
| **audit** | `cargo audit` (dependency vulnerabilities) | Yes |
| **dogfood** | Build release, run on synthetic test file, verify >0 violations found | Yes |

### Additional Workflows

| Workflow | File | Purpose |
|---|---|---|
| AI codebase review | `ai-codebase-review.yml` | AI-driven PR review |
| Codebase review | `codebase-review.yml` | Code review automation |
| Docs | `docs.yml` | Documentation build/deploy |
| Mutation testing | `mutation-testing.yml` | Periodic mutation testing |
| PR-Agent | `pr-agent.yml` | AI code review agent (slash-command only: `/review`, `/describe`, `/improve`) |
| Release | `release.yml` | Release automation |

### Quality Mandate

- **Coverage**: Minimum 90% line coverage enforced via `--fail-under-lines 90`.
- **Linting**: Zero clippy warnings (all categories and pedantic/nursery enabled).
- **Formatting**: Zero formatting diffs.
- **Security**: Zero cargo-audit vulnerabilities (high/medium severity blocks merge).
- **Dogfood**: The linter must detect at least one violation in its own synthetic test file.
- **SonarCloud**: Quality gate enforced. Project key: `Jonathangadeaharder_pytest-linter`.

### Branch Protection (Target)

- `main` and `master` require `Required Checks (PR)` status check.
- Requires PR review approval (1 reviewer minimum).
- Dismisses stale reviews on new pushes.

## Consequences

- **Positive**: Comprehensive CI catches issues across 7 dimensions (format, lint, test, coverage, security, dogfood, SonarCloud).
- **Positive**: Dogfooding ensures the linter works on real Python test code.
- **Positive**: Self-hosted runner gives fast, consistent execution environment.
- **Negative**: 7 serial CI jobs create long pipeline times. Mitigation: independent jobs run in parallel (all jobs except dogfood).
- **Negative**: Self-hosted runner must maintain Rust toolchain, sonar-scanner, and brew dependencies.
