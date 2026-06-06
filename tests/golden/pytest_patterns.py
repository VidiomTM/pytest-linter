# Golden corpus: patterns from pytest's own test suite
# Each `# expect: RULE-ID` marks a line where the linter should report that rule.

import time

import pytest


# --- FLK-001: time.sleep in test ---
def test_slow_wait():  # expect: PYTEST-MNT-016
    time.sleep(2)  # expect: PYTEST-FLK-001
    assert True


# --- FLK-003: network import at module level ---
# expect: PYTEST-FLK-003
# expect: PYTEST-INF-001


# --- MNT-004: no assertion ---
def test_side_effect_only():
    do_thing()  # expect: PYTEST-MNT-004


# --- MNT-001: conditional logic ---
def test_conditional_branch():  # expect: PYTEST-MNT-001
    x = get_value()
    if x > 0:  # expect: PYTEST-MNT-001
        assert x > 0
    else:
        assert x <= 0


# --- MNT-006: assertion roulette (4+ asserts) ---
def test_many_asserts():
    assert 1 == 1  # expect: PYTEST-MNT-006
    assert 2 == 2
    assert 3 == 3
    assert 4 == 4


# --- MNT-007: bare try/except ---
def test_bare_catch():
    try:  # expect: PYTEST-MNT-007
        risky()
    except Exception:
        pass


# --- FIX-001: autouse fixture ---
@pytest.fixture(autouse=True)  # expect: PYTEST-FIX-001
def auto_setup():
    return 42


# --- BDD-001: missing Gherkin ---
def test_no_bdd_keywords():  # expect: PYTEST-BDD-001
    assert True


# --- DBC-001: happy-path only (no error/assertion_raises) ---
def test_happy_path():  # expect: PYTEST-DBC-001
    result = 1 + 1
    assert result == 2


# --- MNT-002: magic assert ---
def test_magic_boolean():
    assert True  # expect: PYTEST-MNT-002


# --- Clean test: no violation expected ---
# expect-clean: test_clean_addition
def test_clean_addition():
    """Given two numbers when added then sum is correct."""
    assert 1 + 1 == 2  # expect: PYTEST-DBC-001
    with pytest.raises(TypeError):
        1 + "a"
