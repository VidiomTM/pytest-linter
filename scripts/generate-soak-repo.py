#!/usr/bin/env python3
"""Generate a synthetic repository with many test files for soak testing."""

import argparse
import os
import random

CLEAN_TEMPLATE = """\
def test_{name}():
    assert {a} + {b} == {c}
"""

FLK001_TEMPLATE = """\
import time

def test_{name}():
    time.sleep({delay})
    assert True
"""

FLK002_TEMPLATE = """\
def test_{name}():
    f = open("data.txt")
    content = f.read()
    f.close()
    assert content
"""

FLK003_TEMPLATE = """\
import requests

def test_{name}():
    assert True
"""

FLK004_TEMPLATE = """\
import os

def test_{name}():
    cwd = os.getcwd()
    assert cwd
"""

FLK005_TEMPLATE = """\
def test_{name}():
    f = open("fixture.json")
    data = f.read()
    f.close()
    assert data
"""

MNT001_TEMPLATE = """\
def test_{name}():
    x = {val}
    if x > 0:
        assert True
    else:
        assert False
"""

MNT002_TEMPLATE = """\
def test_{name}():
    assert True
"""

MNT003_TEMPLATE = """\
def test_{name}():
    assert len(items) == {n}
"""

MNT004_TEMPLATE = """\
def test_{name}():
    x = {val}
"""

MNT005_TEMPLATE = """\
def test_{name}(mock_obj):
    mock_obj.assert_called()
"""

MNT006_TEMPLATE = """\
def test_{name}():
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3
    assert 4 == 4
"""

MNT007_TEMPLATE = """\
def test_{name}():
    try:
        risky()
    except Exception:
        pass
"""

FIX001_TEMPLATE = """\
import pytest

@pytest.fixture(autouse=True)
def auto_{name}():
    return 42

def test_{name}():
    assert True
"""

FIX003_TEMPLATE = """\
import pytest

@pytest.fixture
def base_{name}():
    return 1

@pytest.fixture(scope="session")
def derived_{name}(base_{name}):
    return base_{name}

def test_{name}(derived_{name}):
    assert derived_{name}
"""

FIX005_TEMPLATE = """\
import pytest

@pytest.fixture
def unused_{name}():
    return 42

def test_{name}():
    assert True
"""

FIX006_TEMPLATE = """\
import pytest

@pytest.fixture(scope="session")
def shared_{name}():
    return []

def test_{name}(shared_{name}):
    assert len(shared_{name}) == 0
"""

FIX008_TEMPLATE = """\
import pytest

@pytest.fixture
def db_{name}():
    conn = get_conn()
    conn.commit()
    return conn

def test_{name}(db_{name}):
    assert db_{name}
"""

FIX009_TEMPLATE = """\
import pytest

@pytest.fixture(scope="session")
def simple_{name}():
    return 42

def test_{name}(simple_{name}):
    assert simple_{name} == 42
"""

BDD001_TEMPLATE = """\
def test_{name}():
    assert True
"""

DBC001_TEMPLATE = """\
def test_{name}():
    result = 1 + 1
    assert result == 2
"""

PBT001_TEMPLATE = """\
import pytest

@pytest.mark.parametrize("x", [1, 2, 3, 4, 5])
def test_{name}(x):
    assert x > 0
"""

PARAM001_TEMPLATE = """\
import pytest

@pytest.mark.parametrize("x", [42])
def test_{name}(x):
    assert x == 42
"""

PARAM002_TEMPLATE = """\
import pytest

@pytest.mark.parametrize("x", [1, 2, 2, 3])
def test_{name}(x):
    assert x > 0
"""

PARAM003_TEMPLATE = """\
import pytest

@pytest.mark.parametrize("val", {values})
def test_{name}(val):
    assert val >= 0
"""

XDIST001_TEMPLATE = """\
import pytest

@pytest.fixture(scope="session")
def shared_{name}():
    return []

def test_{name}(shared_{name}):
    shared_{name}.append(1)
    assert len(shared_{name}) == 1
"""

XDIST002_TEMPLATE = """\
import pytest

@pytest.fixture(scope="session")
def io_{name}():
    f = open("data.csv")
    data = f.read()
    f.close()
    return data

def test_{name}(io_{name}):
    assert io_{name}
"""

TEMPLATES = {
    "clean": CLEAN_TEMPLATE,
    "FLK001": FLK001_TEMPLATE,
    "FLK002": FLK002_TEMPLATE,
    "FLK003": FLK003_TEMPLATE,
    "FLK004": FLK004_TEMPLATE,
    "FLK005": FLK005_TEMPLATE,
    "MNT001": MNT001_TEMPLATE,
    "MNT002": MNT002_TEMPLATE,
    "MNT003": MNT003_TEMPLATE,
    "MNT004": MNT004_TEMPLATE,
    "MNT005": MNT005_TEMPLATE,
    "MNT006": MNT006_TEMPLATE,
    "MNT007": MNT007_TEMPLATE,
    "FIX001": FIX001_TEMPLATE,
    "FIX003": FIX003_TEMPLATE,
    "FIX005": FIX005_TEMPLATE,
    "FIX006": FIX006_TEMPLATE,
    "FIX008": FIX008_TEMPLATE,
    "FIX009": FIX009_TEMPLATE,
    "BDD001": BDD001_TEMPLATE,
    "DBC001": DBC001_TEMPLATE,
    "PBT001": PBT001_TEMPLATE,
    "PARAM001": PARAM001_TEMPLATE,
    "PARAM002": PARAM002_TEMPLATE,
    "PARAM003": PARAM003_TEMPLATE,
    "XDIST001": XDIST001_TEMPLATE,
    "XDIST002": XDIST002_TEMPLATE,
}

TEMPLATE_KEYS = list(TEMPLATES.keys())


def generate_file_content(rng: random.Random, idx: int) -> str:  # noqa: PLR0912
    num_functions = rng.randint(1, 20)
    parts: list[str] = []
    seen_fixtures: set[str] = set()
    used_imports: set[str] = set()

    for i in range(num_functions):
        tmpl_key = rng.choice(TEMPLATE_KEYS)
        name = f"fn_{idx}_{i}"
        params: dict[str, str] = {
            "name": name,
            "a": str(rng.randint(1, 100)),
            "b": str(rng.randint(1, 100)),
            "c": str(rng.randint(2, 200)),
            "delay": str(rng.choice([0.1, 0.5, 1, 2])),
            "val": str(rng.randint(0, 10)),
            "n": str(rng.randint(1, 10)),
            "values": str(list(range(25))),
        }

        content = TEMPLATES[tmpl_key].format(**params)

        if tmpl_key in ("FIX001", "FIX003", "FIX005", "FIX006", "FIX008", "FIX009"):
            fix_name = f"base_{name}"
            if tmpl_key == "FIX001":
                fix_name = f"auto_{name}"
            elif tmpl_key == "FIX005":
                fix_name = f"unused_{name}"
            elif tmpl_key == "FIX006":
                fix_name = f"shared_{name}"
            elif tmpl_key == "FIX008":
                fix_name = f"db_{name}"
            elif tmpl_key == "FIX009":
                fix_name = f"simple_{name}"
            elif tmpl_key == "FIX003":
                fix_name = f"derived_{name}"
            if fix_name in seen_fixtures:
                continue
            seen_fixtures.add(fix_name)

        if "import requests" in content:
            used_imports.add("requests")
        if "import time" in content:
            used_imports.add("time")
        if "import os" in content:
            used_imports.add("os")
        if "import pytest" in content:
            used_imports.add("pytest")

        parts.append(content)

    return "\n\n".join(parts) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate soak test repo")
    parser.add_argument("--num-files", type=int, default=10000)
    parser.add_argument("--output-dir", type=str, default="soak_repo")
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    if args.num_files <= 0:
        parser.error(f"--num-files must be positive, got {args.num_files}")

    if os.path.exists(args.output_dir):
        if not os.path.isdir(args.output_dir):
            parser.error(f"--output-dir {args.output_dir!r} exists but is not a directory")
        for f in os.listdir(args.output_dir):
            path = os.path.join(args.output_dir, f)
            if f.startswith("test_soak_") and f.endswith(".py") and os.path.isfile(path):
                os.remove(path)
    os.makedirs(args.output_dir, exist_ok=True)
    rng = random.Random(args.seed)

    for i in range(args.num_files):
        content = generate_file_content(rng, i)
        filepath = os.path.join(args.output_dir, f"test_soak_{i:06d}.py")
        with open(filepath, "w") as f:
            f.write(content)

    print(f"Generated {args.num_files} files in {args.output_dir}")


if __name__ == "__main__":
    main()
