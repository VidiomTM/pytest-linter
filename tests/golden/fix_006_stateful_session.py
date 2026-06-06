# Golden corpus: PYTEST-FIX-006 StatefulSessionFixtureRule hardening
# expect: PYTEST-FIX-006
# expect: PYTEST-FIX-001
# expect: PYTEST-FIX-009
# expect: PYTEST-BDD-001
# expect: PYTEST-DBC-001
# expect: PYTEST-MNT-003
# expect: PYTEST-MNT-005
# test_uses_frozen and test_uses_int don't trigger FIX-006 but do trigger BDD-001/DBC-001

from dataclasses import dataclass

import pytest


@pytest.fixture(scope="session")
def session_dict():
    return {"key": "value"}


@pytest.fixture(scope="session")
def session_list():
    return [1, 2, 3]


class BrainState:
    def __init__(self):
        self.cache = {}


@pytest.fixture(scope="session")
def brain_state():
    state = BrainState()
    return state


@dataclass
class NonFrozenConfig:
    key: str = "default"


@pytest.fixture(scope="session")
def non_frozen_config():
    return NonFrozenConfig(key="value")


@pytest.fixture(scope="session", autouse=True)
def autouse_session_set():
    return {1, 2, 3}


@pytest.fixture(scope="session")
def session_int_fixture():
    return 42


@dataclass(frozen=True)
class FrozenConfig:
    key: str = "default"


@pytest.fixture(scope="session")
def frozen_session_fixture():
    return FrozenConfig(key="frozen")


def test_uses_dict(session_dict):
    assert "key" in session_dict


def test_uses_list(session_list):
    assert len(session_list) == 3


def test_uses_brain(brain_state):
    assert brain_state is not None


def test_uses_non_frozen(non_frozen_config):
    assert non_frozen_config.key == "value"


def test_uses_set(autouse_session_set):
    assert 1 in autouse_session_set


def test_uses_frozen(frozen_session_fixture):
    assert frozen_session_fixture.key == "frozen"


def test_uses_int(session_int_fixture):
    assert session_int_fixture == 42
