# Migration Guide

Migrating to pytest-linter from pylint-pytest, flake8-pytest-style, or other pytest linting tools.

## From flake8-pytest-style

flake8-pytest-style focuses on pytest style conventions. pytest-linter focuses on test quality and smell detection. They are **complementary** — you can run both.

### Rule Mapping

flake8-pytest-style rules that have pytest-linter equivalents:

| flake8-pytest-style | pytest-linter | Notes |
|--------------------|---------------|-------|
| PT015 (assertion always true) | PYTEST-MNT-002 (MagicAssertRule) | Similar detection, different scope |
| PT019 (fixture without value returned) | PYTEST-FIX-005 (UnusedFixtureRule) | pytest-linter checks cross-file usage |
| PT022 (no teardown in yield fixture) | PYTEST-FIX-008 (FixtureDbCommitNoCleanupRule) | pytest-linter is more specific to DB patterns |

flake8-pytest-style rules **not covered** by pytest-linter (style/formatting rules):

| PT001–PT014, PT016–PT018, PT020–PT021, PT023–PT027 | Use **ruff** for these |

### Config Migration

**Before** (`.flake8` or `setup.cfg`):
```ini
[flake8]
max-line-length = 88
pytest-socket-timeout = 5.0
pytest-fixture-no-parentheses = false
```

**After** (`pyproject.toml`):
```toml
[tool.pytest-linter]

# Disable specific rules
[tool.pytest-linter.rules.PYTEST-FLK-001]
enabled = false
```

Keep your flake8 config for style rules. pytest-linter doesn't replace style enforcement.

### Running Both

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/your-org/pytest-linter
    rev: v0.1.0
    hooks:
      - id: pytest-linter
  - repo: https://github.com/m-burst/flake8-pytest-style
    rev: v2.0.0
    hooks:
      - id: flake8-pytest-style
```

## From pylint-pytest

pylint-pytest provides minimal pytest-specific rules within the pylint ecosystem.

### Rule Mapping

| pylint-pytest | pytest-linter | Notes |
|--------------|---------------|-------|
| W0612 (unused fixture) | PYTEST-FIX-005 (UnusedFixtureRule) | pytest-linter does cross-file analysis |
| W0602 (autouse fixture) | PYTEST-FIX-001 (AutouseFixtureRule) | Same detection |

### Config Migration

**Before** (`.pylintrc`):
```ini
[MASTER]
load-plugins=pylint_pytest

[MESSAGES CONTROL]
disable=W0612
```

**After** (`pyproject.toml`):
```toml
[tool.pytest-linter]
# Disable specific rules
[tool.pytest-linter.rules.PYTEST-FIX-005]
enabled = false
```

### Running Both

pylint and pytest-linter can coexist:

```bash
pylint src/ tests/
pytest-linter tests/
```

## From pytest-flake8 (deprecated)

pytest-flake8 was a pytest plugin that ran flake8 on test files. It's now deprecated.

### Migration Steps

1. Remove `pytest-flake8` from dependencies
2. Remove `[pytest] flake8-ignore` config from `pytest.ini` / `pyproject.toml`
3. Install pytest-linter: `pip install pytest-linter`
4. Run: `pytest-linter tests/`

**Before** (`pytest.ini`):
```ini
[pytest]
flake8-ignore = E501 W503
flake8-max-line-length = 120
```

**After** (`pyproject.toml`):
```toml
[tool.pytest-linter]
# Disable specific rules that don't apply
[tool.pytest-linter.rules.PYTEST-FLK-001]
enabled = false
```

### What You Gain

pytest-flake8 only ran standard flake8 checks. pytest-linter adds:

- Flakiness detection (`time.sleep`, file I/O, network imports)
- Fixture quality analysis (scope, mutation, shadowing)
- Assertion quality (magic asserts, roulette, no-assert)
- BDD and PBT hints
- Parametrize quality checks

## Full Migration Checklist

- [ ] Install pytest-linter (`pip install pytest-linter` or download binary)
- [ ] Add `[tool.pytest-linter]` config to `pyproject.toml`
- [ ] Run `pytest-linter tests/` and review findings
- [ ] Suppress false positives with per-rule `enabled = false` config
- [ ] Add to pre-commit hooks
- [ ] Add to CI pipeline
- [ ] Keep complementary tools (ruff, flake8-pytest-style) for style rules
