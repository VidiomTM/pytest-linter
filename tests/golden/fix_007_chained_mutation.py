# Golden corpus: PYTEST-FIX-007 FixtureMutationRule hardening
# expect: PYTEST-FIX-007
# expect: PYTEST-BDD-001
# expect: PYTEST-DBC-001
# expect: PYTEST-MNT-005

import pytest


@pytest.fixture
def mutable_state():
    return {"cache": {}, "items": []}


@pytest.fixture
def complex_obj():
    class State:
        def __init__(self):
            self.data = {}
            self.tags = []

    return State()


def test_chained_subscript_attr(complex_obj):
    complex_obj.data["key"] = 123
    assert complex_obj.data["key"] == 123


def test_chained_append(complex_obj):
    complex_obj.tags.append("new")
    assert "new" in complex_obj.tags


def test_chained_update(mutable_state):
    mutable_state["cache"].update({"x": 1})
    assert mutable_state["cache"]["x"] == 1


def test_deep_chained(complex_obj):
    complex_obj.data["nested"]["deep"] = "value"
    assert "deep" in complex_obj.data["nested"]


def test_direct_append(mutable_state):
    mutable_state["items"].append(42)
    assert 42 in mutable_state["items"]


def test_read_only(mutable_state):
    """Given a mutable fixture when read-only accessed then no mutation detected."""
    x = mutable_state["cache"]
    assert isinstance(x, dict)
    with pytest.raises(KeyError):
        mutable_state["cache"]["nonexistent"]


def test_read_only_attr(complex_obj):
    """Given a complex fixture when read-only attr accessed then no mutation detected."""
    x = complex_obj.data
    assert x is not None
    with pytest.raises(AttributeError):
        complex_obj.nonexistent
