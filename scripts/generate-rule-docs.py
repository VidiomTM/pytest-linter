#!/usr/bin/env python3
"""Generate per-rule documentation pages from Rust source code."""

from pathlib import Path

RULES = [
    {
        "id": "PYTEST-FLK-001",
        "name": "TimeSleepRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' uses time.sleep which causes flaky tests",
        "suggestion": "Use pytest's time mocking or wait for a condition instead",
        "rationale": "`time.sleep()` introduces implicit timing dependencies that vary across machines and CI environments. Tests become flaky because they rely on wall-clock time rather than synchronization.",
        "bad": [
            "def test_retry():\n    time.sleep(5)  # waits an arbitrary duration\n    assert service.is_ready()",
        ],
        "good": [
            "def test_retry():\n    with pytest.mock.patch('time.sleep'):\n        service.trigger()\n    assert service.is_ready()",
        ],
    },
    {
        "id": "PYTEST-FLK-002",
        "name": "FileIoRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' uses file I/O without tmp_path/tmpdir fixture",
        "suggestion": "Use the tmp_path or tmpdir fixture for temporary files",
        "rationale": "Direct file I/O without temporary directory fixtures can leave residual files, cause path conflicts between tests, or fail in parallel execution.",
        "bad": [
            "def test_save():\n    with open('output.txt', 'w') as f:\n        f.write('data')",
        ],
        "good": [
            "def test_save(tmp_path):\n    out = tmp_path / 'output.txt'\n    out.write_text('data')",
        ],
    },
    {
        "id": "PYTEST-FLK-003",
        "name": "NetworkImportRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "File imports network libraries which may cause flaky tests",
        "suggestion": "Mock network calls or use pytest-localserver",
        "rationale": "Importing `requests`, `socket`, `httpx`, `aiohttp`, or `urllib` indicates potential network dependencies. Network calls are inherently non-deterministic and cause flaky tests.",
        "bad": [
            "import requests\nimport httpx\n\ndef test_api():\n    resp = requests.get('https://api.example.com')\n    assert resp.status_code == 200",
        ],
        "good": [
            "import requests\nfrom unittest.mock import patch\n\ndef test_api():\n    with patch('requests.get') as mock_get:\n        mock_get.return_value.status_code = 200\n        resp = requests.get('https://api.example.com')\n        assert resp.status_code == 200",
        ],
    },
    {
        "id": "PYTEST-FLK-004",
        "name": "CwdDependencyRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' depends on the current working directory",
        "suggestion": "Use absolute paths or tmp_path fixture instead",
        "rationale": "Tests that depend on `os.getcwd()` or relative paths are sensitive to execution order and working directory, leading to failures when run from different locations or in parallel.",
        "bad": [
            "def test_load():\n    data = open('data/config.json').read()\n    assert data",
        ],
        "good": [
            "from pathlib import Path\n\ndef test_load(tmp_path):\n    config = tmp_path / 'config.json'\n    config.write_text('{}')\n    data = config.read_text()\n    assert data",
        ],
    },
    {
        "id": "PYTEST-FLK-005",
        "name": "MysteryGuestRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' may be a Mystery Guest — uses file I/O without temp fixtures",
        "suggestion": "Use tmp_path fixture and make test data explicit",
        "rationale": "A Mystery Guest is a test that uses external data (files, databases) that isn't visible in the test itself. This makes tests hard to understand and debug.",
        "bad": [
            "def test_parse():\n    with open('fixtures/data.csv') as f:\n        result = parse(f)\n    assert result",
        ],
        "good": [
            "def test_parse(tmp_path):\n    csv = tmp_path / 'data.csv'\n    csv.write_text('a,b\\n1,2')\n    result = parse(csv.read_text())\n    assert result",
        ],
    },
    {
        "id": "PYTEST-XDIST-001",
        "name": "XdistSharedStateRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Session-scoped fixture '{fixture}' returns mutable state that is modified by test '{test}' — unsafe for xdist",
        "suggestion": "Use function scope or return immutable values",
        "rationale": "When using `pytest-xdist` for parallel test execution, session-scoped fixtures with mutable state can be corrupted by concurrent test modifications, causing flaky failures.",
        "bad": [
            "@pytest.fixture(scope='session')\ndef shared_list():\n    return []\n\ndef test_a(shared_list):\n    shared_list.append(1)\n    assert len(shared_list) == 1",
        ],
        "good": [
            "@pytest.fixture\ndef fresh_list():\n    return []\n\ndef test_a(fresh_list):\n    fresh_list.append(1)\n    assert len(fresh_list) == 1",
        ],
    },
    {
        "id": "PYTEST-XDIST-002",
        "name": "XdistFixtureIoRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Session-scoped fixture '{fixture}' uses file I/O — may conflict with xdist workers",
        "suggestion": "Use tmp_path_factory or make I/O paths unique per worker",
        "rationale": "Session-scoped fixtures that perform file I/O can race with each other when tests run in parallel under xdist, causing file corruption or missing data.",
        "bad": [
            "@pytest.fixture(scope='session')\ndef db_file():\n    with open('test.db', 'w') as f:\n        f.write('schema')\n    return 'test.db'",
        ],
        "good": [
            "@pytest.fixture(scope='session')\ndef db_file(tmp_path_factory):\n    db = tmp_path_factory.mktemp('db') / 'test.db'\n    db.write_text('schema')\n    return db",
        ],
    },
    {
        "id": "PYTEST-MNT-001",
        "name": "TestLogicRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Test '{test}' contains conditional logic (if statements)",
        "suggestion": "Split into separate tests or use parametrize",
        "rationale": "Conditional logic in tests makes them harder to understand and debug. Each branch should be a separate test case so failures are isolated and traceable.",
        "bad": [
            "def test_user():\n    if user.is_admin:\n        assert dashboard.shows_admin_panel()\n    else:\n        assert not dashboard.shows_admin_panel()",
        ],
        "good": [
            "@pytest.mark.parametrize('role,expected', [\n    ('admin', True),\n    ('user', False),\n])\ndef test_admin_panel(role, expected):\n    user = create_user(role)\n    assert dashboard.shows_admin_panel(user) == expected",
        ],
    },
    {
        "id": "PYTEST-MNT-002",
        "name": "MagicAssertRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Magic assertion at line {line}: '{expr}' — this always passes/fails",
        "suggestion": "Replace with a meaningful comparison",
        "rationale": "Magic assertions are assertions that always pass or always fail regardless of the code under test, providing no real verification. Examples include `assert True` or `assert False`.",
        "bad": [
            "def test_status():\n    result = get_status()\n    assert True  # always passes",
        ],
        "good": [
            "def test_status():\n    result = get_status()\n    assert result == 'ok'",
        ],
    },
    {
        "id": "PYTEST-MNT-003",
        "name": "SuboptimalAssertRule",
        "severity": "Info",
        "category": "Enhancement",
        "message": "Suboptimal assertion at line {line}: '{expr}'",
        "suggestion": "Use a more direct assertion pattern",
        "rationale": "Some assertion patterns produce unclear failure messages. For example, `assert len(items) > 0` doesn't show the actual count. Using `assert items` or `assert items == [...]` is clearer.",
        "bad": [
            "def test_items():\n    items = get_items()\n    assert len(items) > 0",
        ],
        "good": [
            "def test_items():\n    items = get_items()\n    assert items  # shows actual list on failure",
        ],
    },
    {
        "id": "PYTEST-MNT-004",
        "name": "NoAssertionRule",
        "severity": "Error",
        "category": "Maintenance",
        "message": "Test '{test}' has no assertions",
        "suggestion": "Add assertions to verify expected behavior",
        "rationale": "A test without assertions can never fail, making it useless as a verification tool. Every test should assert at least one expected behavior.",
        "bad": [
            "def test_process():\n    result = process(data)\n    # no assertion",
        ],
        "good": [
            "def test_process():\n    result = process(data)\n    assert result.status == 'success'",
        ],
    },
    {
        "id": "PYTEST-MNT-005",
        "name": "MockOnlyVerifyRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Test '{test}' only verifies mocks without checking state",
        "suggestion": "Add state assertions to verify actual outcomes",
        "rationale": "Tests that only verify mock interactions but never check actual state are brittle — they confirm the code calls something but not that it produces correct results.",
        "bad": [
            "def test_send_email(mocker):\n    send_welcome_email(user)\n    mocker.assert_called_once()  # only mock check",
        ],
        "good": [
            "def test_send_email(mocker):\n    result = send_welcome_email(user)\n    mocker.assert_called_once()\n    assert result.status == 'sent'  # state assertion",
        ],
    },
    {
        "id": "PYTEST-MNT-006",
        "name": "AssertionRouletteRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Test '{test}' has {count} assertions (assertion roulette)",
        "suggestion": "Split into smaller, focused tests",
        "rationale": "When a test has many assertions (>3), it's hard to tell which one failed and why. Smaller, focused tests provide clearer failure messages and better isolation.",
        "bad": [
            "def test_user_full():\n    user = create_user('Alice')\n    assert user.name == 'Alice'\n    assert user.email == 'alice@example.com'\n    assert user.age == 30\n    assert user.active is True\n    assert user.role == 'admin'",
        ],
        "good": [
            "def test_user_name():\n    user = create_user('Alice')\n    assert user.name == 'Alice'\n\ndef test_user_email():\n    user = create_user('Alice')\n    assert user.email == 'alice@example.com'",
        ],
    },
    {
        "id": "PYTEST-MNT-007",
        "name": "RawExceptionHandlingRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Test '{test}' uses try/except instead of pytest.raises",
        "suggestion": "Use pytest.raises() for exception testing",
        "rationale": "`pytest.raises()` provides clearer intent, better failure messages, and integrates with pytest's reporting. Raw `try/except` is verbose and error-prone.",
        "bad": [
            "def test_divide_by_zero():\n    try:\n        divide(1, 0)\n    except ZeroDivisionError:\n        pass",
        ],
        "good": [
            "def test_divide_by_zero():\n    with pytest.raises(ZeroDivisionError):\n        divide(1, 0)",
        ],
    },
    {
        "id": "PYTEST-BDD-001",
        "name": "BddMissingScenarioRule",
        "severity": "Info",
        "category": "Enhancement",
        "message": "Test '{test}' lacks a Gherkin-style docstring scenario",
        "suggestion": "Add a docstring with Given/When/Then structure",
        "rationale": "Tests with Gherkin-style docstrings (Given/When/Then) serve as living documentation and make test intent clear without reading implementation details.",
        "bad": [
            "def test_login():\n    user = login('admin', 'pass')\n    assert user.authenticated",
        ],
        "good": [
            'def test_login():\n    """Given a valid user\n    When logging in\n    Then the user is authenticated"""\n    user = login(\'admin\', \'pass\')\n    assert user.authenticated',
        ],
    },
    {
        "id": "PYTEST-PBT-001",
        "name": "PropertyTestHintRule",
        "severity": "Info",
        "category": "Enhancement",
        "message": "Test '{test}' has {count} parametrized cases — consider property-based testing",
        "suggestion": "Consider using hypothesis for property-based testing",
        "rationale": "When a parametrize decorator has many cases (>3), property-based testing with Hypothesis can cover more edge cases with less boilerplate and find bugs manual cases miss.",
        "bad": [
            "@pytest.mark.parametrize('val', [1, 2, 3, 4, 5, 6, 7, 8])\ndef test_abs(val):\n    assert abs(val) >= 0",
        ],
        "good": [
            "from hypothesis import given, strategies as st\n\n@given(st.integers())\ndef test_abs(val):\n    assert abs(val) >= 0",
        ],
    },
    {
        "id": "PYTEST-PARAM-001",
        "name": "ParametrizeEmptyRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Test '{test}' is parametrized with only {count} case(s)",
        "suggestion": "Add more test cases or remove parametrize",
        "rationale": "A parametrize with 0 or 1 cases is either dead code or adds unnecessary complexity. Either add meaningful cases or remove the parametrize decorator.",
        "bad": [
            "@pytest.mark.parametrize('x', [1])\ndef test_double(x):\n    assert x * 2 == 2",
        ],
        "good": [
            "@pytest.mark.parametrize('x,expected', [(1, 2), (2, 4), (0, 0), (-1, -2)])\ndef test_double(x, expected):\n    assert x * 2 == expected",
        ],
    },
    {
        "id": "PYTEST-PARAM-002",
        "name": "ParametrizeDuplicateRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Parametrize in test '{test}' has duplicate values: {values}",
        "suggestion": "Remove duplicate parametrize values",
        "rationale": "Duplicate parametrize values waste test execution time and make the test suite slower without adding any verification value.",
        "bad": [
            "@pytest.mark.parametrize('x', [1, 2, 3, 2, 1])\ndef test_positive(x):\n    assert x > 0",
        ],
        "good": [
            "@pytest.mark.parametrize('x', [1, 2, 3])\ndef test_positive(x):\n    assert x > 0",
        ],
    },
    {
        "id": "PYTEST-PARAM-003",
        "name": "ParametrizeExplosionRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Test '{test}' has {count} parametrized cases — combinatorial explosion",
        "suggestion": "Reduce test cases or use hypothesis",
        "rationale": "When parametrize generates >20 test cases (especially with multiple `@pytest.mark.parametrize` decorators), the test suite becomes slow. Property-based testing covers edge cases more efficiently.",
        "bad": [
            "@pytest.mark.parametrize('a', range(10))\n@pytest.mark.parametrize('b', range(10))\ndef test_add(a, b):\n    assert (a + b) >= a",
        ],
        "good": [
            "from hypothesis import given, strategies as st\n\n@given(st.integers(), st.integers())\ndef test_add_commutative(a, b):\n    assert a + b == b + a",
        ],
    },
    {
        "id": "PYTEST-FIX-001",
        "name": "AutouseFixtureRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Fixture '{fixture}' uses autouse=True",
        "suggestion": "Explicitly declare fixture dependencies instead",
        "rationale": "`autouse=True` fixtures run implicitly for every test, making dependencies invisible. This hurts readability and makes it hard to understand what setup a test relies on.",
        "bad": [
            "@pytest.fixture(autouse=True)\ndef setup_db():\n    db.create_tables()",
        ],
        "good": [
            "@pytest.fixture\ndef db_tables():\n    db.create_tables()\n    return db\n\ndef test_query(db_tables):\n    assert db_tables.query('SELECT 1')",
        ],
    },
    {
        "id": "PYTEST-FIX-003",
        "name": "InvalidScopeRule",
        "severity": "Error",
        "category": "Fixture",
        "message": "Fixture '{fixture}' (scope={scope}) depends on '{dep}' (scope={dep_scope}) — fixture scope must not exceed dependency scope",
        "suggestion": "Reduce scope of '{fixture}' to match or be narrower than '{dep}'",
        "rationale": "A fixture with a broader scope than its dependency will fail because the dependency may be torn down before the dependent fixture is done. Scope hierarchy: function < class < module < package < session.",
        "bad": [
            "@pytest.fixture(scope='function')\ndef config():\n    return load_config()\n\n@pytest.fixture(scope='session')\ndef db(config):\n    return Database(config)",
        ],
        "good": [
            "@pytest.fixture(scope='session')\ndef config():\n    return load_config()\n\n@pytest.fixture(scope='session')\ndef db(config):\n    return Database(config)",
        ],
    },
    {
        "id": "PYTEST-FIX-004",
        "name": "ShadowedFixtureRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Fixture '{fixture}' is defined in {count} different modules (shadowed)",
        "suggestion": "Rename or consolidate fixture definitions",
        "rationale": "When the same fixture name is defined in multiple files, pytest's resolution order can lead to surprising behavior. Tests may use a different fixture than expected depending on file location.",
        "bad": [
            "# conftest.py\n@pytest.fixture\ndef user():\n    return User('default')\n\n# tests/conftest.py\n@pytest.fixture\ndef user():\n    return User('test')",
        ],
        "good": [
            "# conftest.py\n@pytest.fixture\ndef default_user():\n    return User('default')\n\n# tests/conftest.py\n@pytest.fixture\ndef test_user():\n    return User('test')",
        ],
    },
    {
        "id": "PYTEST-FIX-005",
        "name": "UnusedFixtureRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Fixture '{fixture}' is not used by any test or fixture",
        "suggestion": "Remove the unused fixture or reference it explicitly from tests/other fixtures",
        "rationale": "Unused fixtures add dead code to the test suite, making it harder to maintain. They may also perform unnecessary setup/teardown work.",
        "bad": [
            "@pytest.fixture\ndef legacy_db():\n    # no test uses this\n    return Database('legacy')",
        ],
        "good": [
            "# Remove the fixture entirely, or add:\ndef test_legacy(legacy_db):\n    assert legacy_db.is_connected()",
        ],
    },
    {
        "id": "PYTEST-FIX-006",
        "name": "StatefulSessionFixtureRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Session-scoped fixture '{fixture}' returns mutable state",
        "suggestion": "Return immutable data or use a factory pattern",
        "rationale": "Session-scoped fixtures that return mutable objects (lists, dicts) can accumulate state across tests, causing order-dependent failures.",
        "bad": [
            "@pytest.fixture(scope='session')\ndef cache():\n    return {}",
        ],
        "good": [
            "@pytest.fixture(scope='session')\ndef cache_factory():\n    def _cache():\n        return {}\n    return _cache\n\ndef test_a(cache_factory):\n    c = cache_factory()\n    c['key'] = 'value'",
        ],
    },
    {
        "id": "PYTEST-FIX-007",
        "name": "FixtureMutationRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Test '{test}' mutates fixture '{fixture}' which may affect other tests",
        "suggestion": "Create a fresh copy of the fixture value before modifying it",
        "rationale": "When a test mutates a fixture's return value (especially shared-scope fixtures), subsequent tests may see the modified state, leading to order-dependent failures.",
        "bad": [
            "@pytest.fixture\ndef config():\n    return {'debug': True}\n\ndef test_a(config):\n    config['debug'] = False\n    assert not config['debug']",
        ],
        "good": [
            "@pytest.fixture\ndef config():\n    return {'debug': True}\n\ndef test_a(config):\n    test_config = config.copy()\n    test_config['debug'] = False\n    assert not test_config['debug']",
        ],
    },
    {
        "id": "PYTEST-FIX-008",
        "name": "FixtureDbCommitNoCleanupRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Fixture '{fixture}' commits to DB without rollback or cleanup (no yield)",
        "suggestion": "Use yield to provide cleanup or wrap in a transaction",
        "rationale": "Database fixtures that commit without cleanup leave residual data that can contaminate subsequent test runs, causing mysterious failures.",
        "bad": [
            "@pytest.fixture\ndef db_record():\n    record = db.insert({'name': 'test'})\n    db.commit()\n    return record",
        ],
        "good": [
            "@pytest.fixture\ndef db_record():\n    record = db.insert({'name': 'test'})\n    db.commit()\n    yield record\n    db.delete(record)\n    db.commit()",
        ],
    },
    {
        "id": "PYTEST-FIX-009",
        "name": "FixtureOverlyBroadScopeRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Fixture '{fixture}' has scope '{scope}' but no expensive setup — consider using function scope for better isolation",
        "suggestion": "Change fixture scope to 'function'",
        "rationale": "Broad-scoped fixtures (module, session) are intended for expensive setup (DB connections, large fixtures). Without expensive setup, function scope provides better test isolation at negligible cost.",
        "bad": [
            "@pytest.fixture(scope='module')\ndef simple_value():\n    return 42",
        ],
        "good": [
            "@pytest.fixture\ndef simple_value():\n    return 42",
        ],
    },
    {
        "id": "PYTEST-DBC-001",
        "name": "NoContractHintRule",
        "severity": "Info",
        "category": "Enhancement",
        "message": "Test '{test}' only tests the happy path — consider adding error/edge case coverage",
        "suggestion": "Add tests for error conditions using pytest.raises",
        "rationale": "Design-by-contract testing suggests covering both happy paths and error/edge cases. Tests that only assert positive outcomes miss important failure modes.",
        "bad": [
            "def test_parse():\n    result = parse('valid json')\n    assert result.success",
        ],
        "good": [
            "def test_parse_valid():\n    result = parse('valid json')\n    assert result.success\n\ndef test_parse_invalid():\n    with pytest.raises(ParseError):\n        parse('invalid json')",
        ],
    },
    {
        "id": "PYTEST-FLK-008",
        "name": "RandomWithoutSeedRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' uses random without fixed seed — causes flaky tests",
        "suggestion": "Call random.seed() at the start of the test or use a fixture",
        "rationale": "Using `random` without a fixed seed produces non-deterministic output. Tests that depend on random values will fail intermittently across different runs because the random state varies.",
        "bad": [
            "import random\n\ndef test_shuffle():\n    items = list(range(10))\n    random.shuffle(items)\n    assert items[0] != 0  # may pass or fail unpredictably",
        ],
        "good": [
            "import random\n\ndef test_shuffle():\n    random.seed(42)\n    items = list(range(10))\n    random.shuffle(items)\n    assert items[0] == 7  # deterministic",
        ],
    },
    {
        "id": "PYTEST-FLK-009",
        "name": "SubprocessWithoutTimeoutRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' uses subprocess without timeout — may hang indefinitely",
        "suggestion": "Add timeout parameter to subprocess calls",
        "rationale": "Subprocess calls without a `timeout` parameter can hang indefinitely if the child process stalls. In CI environments this causes builds to time out at the job level rather than failing fast.",
        "bad": [
            "import subprocess\n\ndef test_cli():\n    result = subprocess.run(['my-cli', 'serve'])\n    assert result.returncode == 0",
        ],
        "good": [
            "import subprocess\n\ndef test_cli():\n    result = subprocess.run(['my-cli', 'serve'], timeout=10)\n    assert result.returncode == 0",
        ],
    },
    {
        "id": "PYTEST-FLK-010",
        "name": "SocketWithoutBindTimeoutRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' uses socket without proper bind and timeout setup",
        "suggestion": "Add socket.settimeout() or use socket.create_connection() with a timeout",
        "rationale": "Socket operations without timeout configuration can block indefinitely on connect/accept/recv. Tests that create sockets should always set timeouts to avoid hanging the test suite.",
        "bad": [
            "import socket\n\ndef test_server():\n    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)\n    s.connect(('localhost', 8080))\n    data = s.recv(1024)",
        ],
        "good": [
            "import socket\n\ndef test_server():\n    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)\n    s.settimeout(5)\n    s.connect(('localhost', 8080))\n    data = s.recv(1024)",
        ],
    },
    {
        "id": "PYTEST-FLK-011",
        "name": "DatetimeInAssertionRule",
        "severity": "Warning",
        "category": "Flakiness",
        "message": "Test '{test}' uses datetime functions near assertions — tests relying on real time are flaky",
        "suggestion": "Use freezegun or time mocking to make assertions deterministic",
        "rationale": "Assertions involving `datetime.now()`, `datetime.today()`, or similar functions produce different values on each run. These tests fail when run around midnight, across timezones, or under load.",
        "bad": [
            "from datetime import datetime\n\ndef test_created_at():\n    obj = create_record()\n    assert obj.created_at == datetime.now()",
        ],
        "good": [
            "from freezegun import freeze_time\nfrom datetime import datetime\n\n@freeze_time('2025-01-01 12:00:00')\ndef test_created_at():\n    obj = create_record()\n    assert obj.created_at == datetime(2025, 1, 1, 12, 0, 0)",
        ],
    },
    {
        "id": "PYTEST-MNT-014",
        "name": "ConditionalLogicInTestRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Parametrized test '{test}' contains conditional logic (if/elif/else/for/while) — use separate parameter cases instead of branching",
        "suggestion": "Split into separate tests or use pytest.mark.parametrize",
        "rationale": "Conditional logic inside parametrized tests defeats the purpose of parametrization. Each branch should be a separate parameter case for clearer failure isolation and better test reporting.",
        "bad": [
            "@pytest.mark.parametrize('role', ['admin', 'user'])\ndef test_access(role):\n    if role == 'admin':\n        assert has_admin_access()\n    else:\n        assert not has_admin_access()",
        ],
        "good": [
            "@pytest.mark.parametrize('role,expected', [\n    ('admin', True),\n    ('user', False),\n])\ndef test_access(role, expected):\n    assert has_admin_access(role) == expected",
        ],
    },
    {
        "id": "PYTEST-MNT-015",
        "name": "DuplicateTestBodiesRule",
        "severity": "Info",
        "category": "Maintenance",
        "message": "Test '{test}' has identical body to {count} other test(s): {peers} (shared body hash)",
        "suggestion": "Consolidate or differentiate the test bodies",
        "rationale": "Duplicate test bodies provide no additional verification value and increase maintenance burden. Either the tests are redundant (remove them) or they should test different scenarios (differentiate them).",
        "bad": [
            "def test_add_positive():\n    result = add(2, 3)\n    assert result == 5\n\ndef test_add_positive_two():\n    result = add(2, 3)\n    assert result == 5",
        ],
        "good": [
            "def test_add_positive():\n    result = add(2, 3)\n    assert result == 5\n\ndef test_add_negative():\n    result = add(-1, -2)\n    assert result == -3",
        ],
    },
    {
        "id": "PYTEST-MNT-016",
        "name": "SleepWithValueRule",
        "severity": "Warning",
        "category": "Maintenance",
        "message": "Test '{test}' uses time.sleep() with value > 0.1s — slows test suite",
        "suggestion": "Use mocking, async waits, or reduce sleep duration",
        "rationale": "Using `time.sleep()` with values > 0.1s unnecessarily slows the test suite. In large projects, even small sleeps compound. Use mocking, `pytest-asyncio` waits, or reduce the sleep to the minimum needed.",
        "bad": [
            "import time\n\ndef test_debounce():\n    trigger_event()\n    time.sleep(2)\n    assert is_debounced()",
        ],
        "good": [
            "import time\nfrom unittest.mock import patch\n\ndef test_debounce():\n    with patch('time.sleep'):\n        trigger_event()\n    assert is_debounced()",
        ],
    },
    {
        "id": "PYTEST-MNT-017",
        "name": "TestNameLengthRule",
        "severity": "Info",
        "category": "Maintenance",
        "message": "Test name '{test}' exceeds 80 characters ({count} chars)",
        "suggestion": "Shorten the test name to be more concise",
        "rationale": "Overly long test names reduce readability in test reports, IDE test runners, and CI logs. Names > 80 characters usually contain implementation details that belong in the test body or parametrize parameters instead.",
        "bad": [
            "def test_user_registration_with_valid_email_and_password_creates_account_and_sends_welcome_email_and_redirects_to_dashboard():\n    ...",
        ],
        "good": [
            "@pytest.mark.parametrize('email,password', [\n    ('valid@example.com', 'strongpass'),\n])\ndef test_user_registration(email, password):\n    ...",
        ],
    },
    {
        "id": "PYTEST-FIX-010",
        "name": "ModuleScopeFixtureMutatedRule",
        "severity": "Error",
        "category": "Fixture",
        "message": "Test '{test}' mutates module/session-scoped fixture '{fixture}' — causes cross-test contamination",
        "suggestion": "Use function-scoped fixture or copy the value before mutation",
        "rationale": "Mutating a module or session-scoped fixture causes state to leak between tests. Test B sees the state left by test A, creating order-dependent failures that are hard to debug.",
        "bad": [
            "@pytest.fixture(scope='module')\ndef config():\n    return {'debug': True}\n\ndef test_a(config):\n    config['debug'] = False\n\ndef test_b(config):\n    # config['debug'] is now False — contaminated by test_a",
        ],
        "good": [
            "@pytest.fixture(scope='module')\ndef config():\n    return {'debug': True}\n\ndef test_a(config):\n    test_cfg = config.copy()\n    test_cfg['debug'] = False\n    assert not test_cfg['debug']",
        ],
    },
    {
        "id": "PYTEST-FIX-011",
        "name": "YieldWithoutTryFinallyRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Fixture '{fixture}' uses yield without try/finally cleanup",
        "suggestion": "Wrap yield in try/finally to ensure cleanup runs even on failure",
        "rationale": "While pytest runs teardown code after `yield` even if a test fails, using `try/finally` is essential for fixtures that acquire multiple resources. It ensures that if an error occurs during the setup of one resource, previously acquired resources are still cleaned up.",
        "bad": [
            "@pytest.fixture\ndef db_connection():\n    conn = create_connection()\n    yield conn\n    conn.close()  # may be skipped if setup fails before yield",
        ],
        "good": [
            "@pytest.fixture\ndef db_connection():\n    conn = create_connection()\n    try:\n        yield conn\n    finally:\n        conn.close()  # always runs",
        ],
    },
    {
        "id": "PYTEST-FIX-012",
        "name": "FixtureNameShadowsBuiltinRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Fixture '{fixture}' shadows a Python builtin or pytest hook",
        "suggestion": "Rename the fixture to avoid shadowing built-in names",
        "rationale": "Fixture names that shadow Python builtins (`list`, `dict`, `id`, `type`, `open`) or pytest hooks (`tmp_path`, `capsys`, `request`) cause confusing errors and make the test harder to understand.",
        "bad": [
            "@pytest.fixture\ndef list():\n    return [1, 2, 3]",
        ],
        "good": [
            "@pytest.fixture\ndef item_list():\n    return [1, 2, 3]",
        ],
    },
    {
        "id": "PYTEST-FIX-013",
        "name": "AutouseCascadeDepthRule",
        "severity": "Warning",
        "category": "Fixture",
        "message": "Autouse fixture '{fixture}' has dependency cascade depth of {depth} (> 3)",
        "suggestion": "Reduce fixture dependency chain or remove autouse",
        "rationale": "Deep fixture dependency chains with `autouse=True` create hidden, complex setup graphs that are hard to debug. When an autouse fixture depends on other fixtures that depend on more fixtures, understanding test setup requires tracing many levels.",
        "bad": [
            "@pytest.fixture(autouse=True)\ndef base():\n    return {'base': True}\n\n@pytest.fixture(autouse=True)\ndef layer1(base):\n    return {**base, 'l1': True}\n\n@pytest.fixture(autouse=True)\ndef layer2(layer1):\n    return {**layer1, 'l2': True}\n\n@pytest.fixture(autouse=True)\ndef layer3(layer2):\n    return {**layer2, 'l3': True}",
        ],
        "good": [
            "@pytest.fixture\ndef env():\n    return {'base': True, 'l1': True, 'l2': True, 'l3': True}\n\ndef test_env(env):\n    assert env['base']",
        ],
    },
]


def generate_rule_page(rule: dict) -> str:
    bad_examples = "\n".join(f"```python\n{ex}\n```" for ex in rule["bad"])
    good_examples = "\n".join(f"```python\n{ex}\n```" for ex in rule["good"])
    return f"""# {rule["id"]} — {rule["name"]}

| Property | Value |
|----------|-------|
| **ID** | `{rule["id"]}` |
| **Name** | {rule["name"]} |
| **Severity** | {rule["severity"]} |
| **Category** | {rule["category"]} |

## Message

> {rule["message"]}

## Rationale

{rule["rationale"]}

## Suggestion

{rule["suggestion"]}

## Examples

### ❌ Bad

{bad_examples}

### ✅ Good

{good_examples}
"""


def generate_rules_index() -> str:
    lines = ["# Rules Overview\n", f"pytest-linter includes **{len(RULES)} rules** across four categories.\n"]

    categories = {}
    for r in RULES:
        cat = r["category"]
        if cat not in categories:
            categories[cat] = []
        categories[cat].append(r)

    for cat_name in ["Flakiness", "Maintenance", "Fixture", "Enhancement"]:
        if cat_name not in categories:
            continue
        rules = categories[cat_name]
        lines.append(f"## {cat_name}\n")
        lines.append("| Rule ID | Name | Severity |")
        lines.append("|---------|------|----------|")
        for r in rules:
            lines.append(f"| [{r['id']}](./{r['id']}.md) | {r['name']} | {r['severity']} |")
        lines.append("")

    return "\n".join(lines)


def main():
    script_dir = Path(__file__).parent
    docs_rules_dir = script_dir.parent / "docs" / "rules"
    docs_rules_dir.mkdir(parents=True, exist_ok=True)

    for rule in RULES:
        page = generate_rule_page(rule)
        out_path = docs_rules_dir / f"{rule['id']}.md"
        out_path.write_text(page)
        print(f"Generated {out_path}")

    index_path = docs_rules_dir / "index.md"
    index_path.write_text(generate_rules_index())
    print(f"Generated {index_path}")

    print(f"\nDone. Generated {len(RULES)} rule pages + index.")


if __name__ == "__main__":
    main()
