# Golden corpus: PYTEST-INF-003 NonIdiomaticMonkeyPatchRule
# expect: PYTEST-INF-003
# expect: PYTEST-BDD-001
# expect: PYTEST-DBC-001
# expect: PYTEST-MNT-002


def test_monkeypatch_no_context(monkeypatch):
    monkeypatch.setattr("os.environ", {"TEST": "1"})
    assert True
