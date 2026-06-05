# Configuration

pytest-linter is configured via `pyproject.toml` under the `[tool.pytest-linter]` section, or via a standalone `pytest-linter.toml` file.

## Config File Priority

1. CLI arguments (highest)
2. `pytest-linter.toml` (standalone, walks up directories)
3. `pyproject.toml` `[tool.pytest-linter]` (walks up directories)
4. Built-in defaults

## Basic Options

```toml
[tool.pytest-linter]
# Output format: terminal, json, sarif
format = "terminal"

# Write output to a file (empty string = stdout)
output = ""

# Additional directory names to exclude during file discovery
excludes = ["generated", "vendor"]

# Per-rule overrides
[tool.pytest-linter.rules.PYTEST-FLK-001]
enabled = false

[tool.pytest-linter.rules.PYTEST-MNT-004]
severity = "warning"
```

## Standalone Config File

A `pytest-linter.toml` file uses a flat structure (no `[tool]` prefix):

```toml
# pytest-linter.toml
format = "json"

[rules.PYTEST-FLK-001]
enabled = false
```

## Per-Rule Overrides

Override severity or disable individual rules:

```toml
[[tool.pytest-linter.overrides]]
path = "tests/integration/**"
rules = { PYTEST-FLK-001 = { severity = "info" } }

[[tool.pytest-linter.overrides]]
path = "tests/unit/**"
rules = { PYTEST-MNT-003 = { enabled = false } }
```

## Suppression

Suppress specific rules inline using `noqa` comments:

```python
def test_something():  # noqa: PYTEST-FLK-001
    time.sleep(1)
    assert True
```

Suppress multiple rules on one line:

```python
def test_something():  # noqa: PYTEST-FLK-001, PYTEST-MNT-004
    time.sleep(1)
```

## Pre-commit Integration

Add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/Jonathangadeaharder/pytest-linter
    rev: v0.1.0
    hooks:
      - id: pytest-linter
```

## CI Integration

### GitHub Actions

```yaml
- name: Lint tests
  run: |
    curl -sL https://github.com/Jonathangadeaharder/pytest-linter/releases/latest/download/pytest-linter-x86_64-unknown-linux-gnu.tar.gz | tar xz
    ./pytest-linter --format sarif --output pytest-linter.sarif tests/
```

### GitLab CI

```yaml
lint-tests:
  script:
    - curl -sL https://github.com/Jonathangadeaharder/pytest-linter/releases/latest/download/pytest-linter-x86_64-unknown-linux-gnu.tar.gz | tar xz
    - ./pytest-linter tests/
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | No Error-severity violations found |
| 1 | One or more Error-severity violations found |
