# Golden corpus: patterns from Django test suite
# Each `# expect: RULE-ID` marks a line where the linter should report that rule.

import time

import pytest


# --- FLK-001: time.sleep ---
def test_slow():  # expect: PYTEST-MNT-016
    time.sleep(5)  # expect: PYTEST-FLK-001
    assert True


# --- FLK-003: network import at module level ---
# expect: PYTEST-FLK-003
# expect: PYTEST-INF-001


# --- FLK-002: file I/O without tmp_path ---
def test_file_read():
    with open("fixture.json") as f:  # expect: PYTEST-FLK-002
        data = f.read()
    assert data


# --- FLK-005: mystery guest (file I/O) ---
# expect: PYTEST-FLK-005


# --- MNT-001: conditional logic ---
def test_conditional():  # expect: PYTEST-MNT-001
    result = call_api()
    if result.ok:  # expect: PYTEST-MNT-001
        assert result.json()
    else:
        assert False


# --- MNT-002: magic assert ---
def test_magic_boolean():
    assert True  # expect: PYTEST-MNT-002


# --- MNT-003: suboptimal assert ---
def test_type_check():
    assert type(result) == dict  # expect: PYTEST-MNT-003


# --- MNT-004: no assertion ---
def test_setup_only():
    create_user("admin")  # expect: PYTEST-MNT-004


# --- MNT-006: assertion roulette ---
def test_model_fields():
    assert True  # expect: PYTEST-MNT-006
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3


# --- MNT-007: raw try/except ---
def test_error_handling():
    try:  # expect: PYTEST-MNT-007
        send_email()
    except Exception:
        pass


# --- FIX-001: autouse fixture ---
@pytest.fixture(autouse=True)  # expect: PYTEST-FIX-001
def setup_db():  # expect: PYTEST-FIX-011
    init_db()
    yield
    teardown_db()


# --- BDD-001: missing Gherkin in docstring ---
def test_response_status():  # expect: PYTEST-BDD-001
    assert get_response().status_code == 200


# --- DBC-001: happy-path only ---
def test_create_user_happy():  # expect: PYTEST-DBC-001
    user = create_user("alice")
    assert user.name == "alice"


# --- Clean test ---
def test_with_raises():
    """Given invalid input when creating user then ValueError raised."""
    with pytest.raises(ValueError):
        create_user("")
