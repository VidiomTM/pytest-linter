# Golden corpus: patterns from pandas test suite
# Each `# expect: RULE-ID` marks a line where the linter should report that rule.

import os

import pytest


# --- FLK-004: cwd dependency ---
def test_cwd_dependent():
    cwd = os.getcwd()  # expect: PYTEST-FLK-004
    assert cwd


# --- FIX-003: invalid fixture scope (session depends on function) ---
@pytest.fixture
def func_scoped():
    return 1


@pytest.fixture(scope="session")
def session_dep(func_scoped):  # expect: PYTEST-FIX-003
    return func_scoped


# --- FIX-005: unused fixture ---
@pytest.fixture
def never_used():
    return 42  # expect: PYTEST-FIX-005


# --- FIX-006: stateful session fixture ---
@pytest.fixture(scope="session")
def shared_state():  # expect: PYTEST-FIX-006
    return []


# --- FIX-008: DB commit no cleanup ---
@pytest.fixture
def db_commit_no_cleanup():
    conn = get_conn()
    conn.commit()  # expect: PYTEST-FIX-008
    return conn


# --- FIX-009: overly broad scope ---
@pytest.fixture(scope="session")
def simple_val():  # expect: PYTEST-FIX-009
    return 42


# --- MNT-002: magic assert ---
def test_magic_bool():
    assert True  # expect: PYTEST-MNT-002


# --- MNT-003: suboptimal assert ---
def test_type_assert():
    assert type(result) == dict  # expect: PYTEST-MNT-003


# --- MNT-004: no assertion ---
def test_no_assert():
    do_setup()  # expect: PYTEST-MNT-004


# --- MNT-005: mock-only verify ---
def test_mock_only():
    mock_obj.assert_called()  # expect: PYTEST-MNT-005


# --- BDD-001: missing Gherkin ---
def test_plain():  # expect: PYTEST-BDD-001
    assert True


# --- DBC-001: happy-path only ---
def test_happy():  # expect: PYTEST-DBC-001
    assert 1 + 1 == 2


# --- XDIST-002: session fixture with I/O ---
@pytest.fixture(scope="session")
def session_io():
    f = open("data.csv")  # expect: PYTEST-XDIST-002
    data = f.read()
    f.close()
    return data
