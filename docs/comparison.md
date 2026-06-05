# Comparison with Other Tools

How pytest-linter compares to other Python linting tools with pytest-specific rules.

## Tool Overview

| Feature | pytest-linter | ruff | pylint-pytest | flake8-pytest-style |
|---------|:---:|:---:|:---:|:---:|
| Language | Rust | Rust | Python | Python |
| Speed | Fast | Fast | Slow | Medium |
| AST Parser | tree-sitter | RustPython | ast | ast |
| Output formats | terminal, JSON, SARIF | terminal, JSON | terminal | terminal |
| Pre-commit hook | Yes | Yes | Yes | Yes |
| Config file | pyproject.toml | pyproject.toml | pyproject.toml / .pylintrc | setup.cfg / pyproject.toml |
| Fixture analysis | Deep | None | Basic | None |
| Cross-file analysis | Yes | No | No | No |

## Rule Comparison

### Flakiness Rules

| Rule | pytest-linter | ruff | pylint-pytest | flake8-pytest-style |
|------|:---:|:---:|:---:|:---:|
| `time.sleep` in tests | **PYTEST-FLK-001** | — | — | — |
| File I/O without tmp fixture | **PYTEST-FLK-002** | — | — | — |
| Network library imports | **PYTEST-FLK-003** | — | — | — |
| CWD dependency | **PYTEST-FLK-004** | — | — | — |
| Mystery Guest pattern | **PYTEST-FLK-005** | — | — | — |
| xdist shared mutable state | **PYTEST-XDIST-001** | — | — | — |
| xdist session fixture I/O | **PYTEST-XDIST-002** | — | — | — |

### Maintenance Rules

| Rule | pytest-linter | ruff | pylint-pytest | flake8-pytest-style |
|------|:---:|:---:|:---:|:---:|
| Conditional logic in tests | **PYTEST-MNT-001** | — | — | — |
| Magic assertions | **PYTEST-MNT-002** | — | — | — |
| Suboptimal assertions | **PYTEST-MNT-003** | — | — | — |
| No assertions | **PYTEST-MNT-004** | — | — | — |
| Mock-only verification | **PYTEST-MNT-005** | — | — | — |
| Assertion roulette | **PYTEST-MNT-006** | — | — | — |
| Raw try/except vs pytest.raises | **PYTEST-MNT-007** | PT017 | — | PT017 |
| Missing BDD scenario | **PYTEST-BDD-001** | — | — | — |
| Property-based test hint | **PYTEST-PBT-001** | — | — | — |
| Empty parametrize | **PYTEST-PARAM-001** | — | — | — |
| Duplicate parametrize values | **PYTEST-PARAM-002** | — | — | — |
| Parametrize explosion | **PYTEST-PARAM-003** | — | — | — |

### Fixture Rules

| Rule | pytest-linter | ruff | pylint-pytest | flake8-pytest-style |
|------|:---:|:---:|:---:|:---:|
| autouse=True fixture | **PYTEST-FIX-001** | — | W0602 | — |
| Invalid fixture scope | **PYTEST-FIX-003** | — | — | — |
| Shadowed fixture | **PYTEST-FIX-004** | — | — | — |
| Unused fixture | **PYTEST-FIX-005** | — | — | — |
| Mutable session fixture | **PYTEST-FIX-006** | — | — | — |
| Fixture mutation | **PYTEST-FIX-007** | — | — | — |
| DB commit without cleanup | **PYTEST-FIX-008** | — | — | — |
| Overly broad fixture scope | **PYTEST-FIX-009** | — | — | — |
| Missing error path tests | **PYTEST-DBC-001** | — | — | — |

### ruff pytest rules (not in pytest-linter)

| ruff Rule | Description |
|-----------|-------------|
| PT001 | Use `@pytest.fixture()` over `@pytest.fixture` |
| PT002 | `pytest.fixture()` config takes no positional args |
| PT003 | `scope='function'` is implied by `@pytest.fixture()` |
| PT004 | Fixture returning `None` should not have underscore prefix |
| PT005 | Fixture names should not have underscore prefix |
| PT006 | Parametrize names should be a tuple |
| PT007 | Parametrize values should be a list of tuples |
| PT008 | Use `return` instead of `yield` in fixture |
| PT009 | Use `assertTrue` instead of `assert expr` |
| PT010 | `pytest.raises()` should have `match=` parameter |
| PT011 | `pytest.raises()` should match specific exception |
| PT012 | `pytest.raises()` block should contain a single statement |
| PT013 | Incorrect import of `pytest` |
| PT014 | Duplicate parametrize test IDs |
| PT015 | Assertion always true |
| PT016 | Fail without message |
| PT017 | Assert in `except` block instead of `pytest.raises()` |
| PT018 | Composite assertion |
| PT019 | Fixture without value injected as parameter |
| PT020 | Deprecated `@pytest.yield_fixture` |
| PT021 | Fixture `params` without IDs |
| PT022 | No teardown in `yield` fixture |
| PT023 | Incorrect `@pytest.mark` usage |
| PT024 | `pytest.mark.usefixtures` without fixture args |
| PT025 | `pytest.mark.usefixtures` with `@pytest.fixture` |
| PT026 | Deprecated `pytest.mark.parametrize` marks |
| PT027 | `pytest.raises` without `match` |

### flake8-pytest-style rules (not in pytest-linter)

| Rule | Description |
|------|-------------|
| PT001 | Use `@pytest.fixture()` over `@pytest.fixture` |
| PT002 | Fixture positional args |
| PT003 | `scope='function'` is implied |
| PT004 | Fixture returning None underscore prefix |
| PT005 | Fixture name underscore prefix |
| PT006 | Parametrize names format |
| PT007 | Parametrize values format |
| PT008 | Patch target as first arg |
| PT009 | Use `assertTrue` |
| PT010 | `pytest.raises()` `match=` |
| PT011 | `pytest.raises()` specific exception |
| PT012 | Single statement in `pytest.raises()` |
| PT013 | Correct import of `pytest` |
| PT015 | Assertion always true |
| PT016 | Fail without message |
| PT017 | Assert in except block |
| PT018 | Composite assertion |
| PT019 | Fixture without value injected as parameter |
| PT020 | Deprecated `@pytest.yield_fixture` |
| PT021 | Fixture `params` without IDs |
| PT022 | No teardown in `yield` fixture |
| PT023 | Incorrect `@pytest.mark` usage |
| PT024 | `pytest.mark.usefixtures` without fixture args |
| PT025 | `pytest.mark.usefixtures` with `@pytest.fixture` |
| PT026 | Deprecated `pytest.mark.parametrize` marks |
| PT027 | `pytest.raises` without `match` |

## Summary

**pytest-linter** is unique in its focus on:

1. **Test smell detection** — Not style enforcement. Detects patterns that cause flaky or low-quality tests.
2. **Cross-file fixture analysis** — Understands fixture relationships across `conftest.py` files and test modules.
3. **Flakiness detection** — The only tool with dedicated rules for `time.sleep`, file I/O, network imports, and xdist compatibility.
4. **Maintenance quality** — Rules for assertion roulette, conditional logic, and mock-only verification go beyond what other tools offer.

**ruff** and **flake8-pytest-style** focus on pytest style conventions (fixture decoration style, parametrize formatting, mark usage). They are complementary to pytest-linter rather than competing.

**pylint-pytest** provides basic pytest support for pylint but has minimal pytest-specific rules.
