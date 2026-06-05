# CLI Reference

## Usage

```bash
pytest-linter [OPTIONS] <PATHS>...
```

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<PATHS>...` | Yes | Files or directories to lint |

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `--format <FORMAT>` | `terminal` | Output format: `terminal`, `json`, `sarif` |
| `--output <OUTPUT>` | stdout | Write output to file instead of stdout |
| `--memory-limit <MB>` | `256` | Soft memory limit in MB |
| `--no-color` | off | Disable colored output |
| `--incremental` | off | Only lint files changed since `--base` |
| `--base <BASE>` | `HEAD` | Git ref for incremental mode |
| `--exclude <DIR>` | — | Additional directory names to exclude (repeatable) |
| `--baseline <FILE>` | — | Save violations to baseline file |
| `--check-baseline <FILE>` | — | Compare against baseline, fail on new violations |
| `-h`, `--help` | — | Print help |

## Output Formats

### Terminal (default)

Colored, human-readable output:

```text
tests/test_api.py:12 PYTEST-FLK-001 [warning] Test 'test_timeout' uses time.sleep which causes flaky tests
  → Use pytest's time mocking or wait for a condition instead

tests/test_models.py:45 PYTEST-MNT-004 [error] Test 'test_create' has no assertions
  → Add assertions to verify expected behavior
```

### JSON

Structured output for tooling integration:

```bash
pytest-linter --format json tests/
```

```json
[
  {
    "rule_id": "PYTEST-FLK-001",
    "severity": "warning",
    "message": "Test 'test_timeout' uses time.sleep which causes flaky tests",
    "file": "tests/test_api.py",
    "line": 12,
    "suggestion": "Use pytest's time mocking or wait for a condition instead"
  }
]
```

### SARIF

Static Analysis Results Interchange Format for GitHub Code Scanning:

```bash
pytest-linter --format sarif --output results.sarif tests/
```

Upload to GitHub:

```yaml
- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

## Incremental Mode

Lint only changed files:

```bash
# Compare against main branch
pytest-linter --incremental --base origin/main tests/

# Compare against last commit (default)
pytest-linter --incremental tests/
```

## Baseline Mode

Track violations over time:

```bash
# Create baseline
pytest-linter --baseline violations.json tests/

# Check for new violations
pytest-linter --check-baseline violations.json tests/
```
