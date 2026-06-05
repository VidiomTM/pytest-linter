# pytest-linter

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Fast, tree-sitter-powered test smell detector for **pytest** (Python), written in Rust.

## Quick Start

```bash
# Install from source
cargo install --path .

# Run
pytest-linter /path/to/tests

# JSON output
pytest-linter --format json /path/to/tests

# Incremental mode (only changed files)
pytest-linter --incremental /path/to/tests

# Baseline mode
pytest-linter --baseline violations.json /path/to/tests
pytest-linter --check-baseline violations.json /path/to/tests
```

## Rules (49)

**Flakiness (11):**

| Rule ID | Name | Severity |
|---------|------|----------|
| PYTEST-FLK-001 | TimeSleepRule | Warning |
| PYTEST-FLK-002 | FileIoRule | Warning |
| PYTEST-FLK-003 | NetworkImportRule | Warning |
| PYTEST-FLK-004 | CwdDependencyRule | Warning |
| PYTEST-FLK-005 | MysteryGuestRule | Warning |
| PYTEST-FLK-008 | RandomWithoutSeedRule | Warning |
| PYTEST-FLK-009 | SubprocessWithoutTimeoutRule | Warning |
| PYTEST-FLK-010 | SocketWithoutBindTimeoutRule | Warning |
| PYTEST-FLK-011 | DatetimeInAssertionRule | Warning |
| PYTEST-XDIST-001 | XdistSharedStateRule | Warning |
| PYTEST-XDIST-002 | XdistFixtureIoRule | Warning |

**Infrastructure (4):**

| Rule ID | Name | Severity |
|---------|------|----------|
| PYTEST-INF-001 | NetworkBanMissingRule | Warning |
| PYTEST-INF-002 | LiveSuiteUnmarkedRule | Warning |
| PYTEST-INF-003 | NonIdiomaticMonkeyPatchRule | Info |
| PYTEST-INF-004 | MacOsCopyArtefactRule | Warning |

**Maintenance (16):**

| Rule ID | Name | Severity |
|---------|------|----------|
| PYTEST-MNT-001 | TestLogicRule | Warning |
| PYTEST-MNT-002 | MagicAssertRule | Warning |
| PYTEST-MNT-004 | NoAssertionRule | Error |
| PYTEST-MNT-005 | MockOnlyVerifyRule | Warning |
| PYTEST-MNT-006 | AssertionRouletteRule | Warning |
| PYTEST-MNT-007 | RawExceptionHandlingRule | Warning |
| PYTEST-MNT-014 | ConditionalLogicInTestRule | Warning |
| PYTEST-MNT-015 | DuplicateTestBodiesRule | Info |
| PYTEST-MNT-016 | SleepWithValueRule | Warning |
| PYTEST-MNT-017 | TestNameLengthRule | Info |
| PYTEST-PARAM-001 | ParametrizeEmptyRule | Warning |
| PYTEST-PARAM-002 | ParametrizeDuplicateRule | Warning |
| PYTEST-PARAM-003 | ParametrizeExplosionRule | Warning |
| PYTEST-MOC-001 | PatchTargetingDefinitionModuleRule | Warning |
| PYTEST-MOC-002 | MagicMockOnAsyncRule | Error |
| PYTEST-MOC-003 | PatchInitBypassRule | Warning |

**Enhancement (6):**

| Rule ID | Name | Severity |
|---------|------|----------|
| PYTEST-MNT-003 | SuboptimalAssertRule | Info |
| PYTEST-VAL-001 | InlineSchemaRedeclaredRule | Info |
| PYTEST-BDD-001 | BddMissingScenarioRule | Info |
| PYTEST-PBT-001 | PropertyTestHintRule | Info |
| PYTEST-MOC-004 | MockRatioBudgetRule | Info |
| PYTEST-DBC-001 | NoContractHintRule | Info |

**Fixtures (12):**

| Rule ID | Name | Severity |
|---------|------|----------|
| PYTEST-FIX-001 | AutouseFixtureRule | Warning |
| PYTEST-FIX-003 | InvalidScopeRule | Error |
| PYTEST-FIX-004 | ShadowedFixtureRule | Warning |
| PYTEST-FIX-005 | UnusedFixtureRule | Warning |
| PYTEST-FIX-006 | StatefulSessionFixtureRule | Warning |
| PYTEST-FIX-007 | FixtureMutationRule | Warning |
| PYTEST-FIX-008 | FixtureDbCommitNoCleanupRule | Warning |
| PYTEST-FIX-009 | FixtureOverlyBroadScopeRule | Warning |
| PYTEST-FIX-010 | ModuleScopeFixtureMutatedRule | Error |
| PYTEST-FIX-011 | YieldWithoutTryFinallyRule | Warning |
| PYTEST-FIX-012 | FixtureNameShadowsBuiltinRule | Warning |
| PYTEST-FIX-013 | AutouseCascadeDepthRule | Warning |

## CLI Options

```
Usage: pytest-linter [OPTIONS] <PATHS>...

Arguments:
  <PATHS>...                     Files or directories to lint (required)

Options:
  --format <FORMAT>              Output format: terminal, json, sarif
  --output <OUTPUT>              Write output to file instead of stdout
  --memory-limit <MB>            Soft memory limit in MB [default: 256]
  --no-color                     Disable colored output
  --incremental                  Only lint files changed since --base
  --base <BASE>                  Git ref for incremental mode [default: HEAD]
  --exclude <DIR>                Additional directory names to exclude (repeatable)
  --baseline <FILE>              Save violations to baseline file
  --check-baseline <FILE>        Compare against baseline, fail on new violations
  -h, --help                     Print help
```

Exit code: **1** if any `Error` severity violations found, **0** otherwise.

## Configuration

Add to your `pyproject.toml`:

```toml
[tool.pytest-linter]
format = "json"

[tool.pytest-linter.rules.PYTEST-FLK-001]
enabled = false

[tool.pytest-linter.rules.PYTEST-MNT-004]
severity = "warning"
```

## Suppression

Suppress specific rules inline:

```python
def test_something():  # noqa: PYTEST-FLK-001
    time.sleep(1)
    assert True
```

## Architecture

- **tree-sitter** for AST parsing (no regex)
- **Rule trait** with `check(module, all_modules, ctx) -> Vec<Violation>`
- **Engine** discovers test files, parses them, runs all rules
- **CLI** via clap with terminal/JSON/SARIF output
- **Parallel** file parsing and rule checking via rayon


## LSP Server

A `tower-lsp`-based LSP server ships as the `pytest-linter-lsp` binary. It provides real-time diagnostics in any LSP-aware editor. Scope: diagnostics only (code actions and hover are future work).

### Install

```bash
cargo install --path lsp-server
```

### Neovim (native LSP)

```lua
-- init.lua
vim.lsp.config('pytest_linter', {
  cmd = { 'pytest-linter-lsp' },
  filetypes = { 'python' },
  root_markers = { 'pyproject.toml', 'pytest-linter.toml' },
})
vim.lsp.enable('pytest_linter')
```

### VS Code (launch.json)

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Attach to pytest-linter LSP",
      "type": "extensionHost",
      "request": "launch",
      "runtimeExecutable": "${workspaceFolder}/target/debug/pytest-linter-lsp",
      "runtimeArgs": ["--stdio"]
    }
  ]
}
```

## License

MIT
