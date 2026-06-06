# Golden corpus: PYTEST-FLK-001 + PYTEST-MNT-016 async sleep hardening
# expect: PYTEST-FLK-001
# expect: PYTEST-MNT-016
# expect: PYTEST-BDD-001
# expect: PYTEST-DBC-001
# expect: PYTEST-MNT-002

import asyncio

import anyio
import pytest


def test_time_sleep():
    import time

    time.sleep(2)
    assert True


def test_asyncio_sleep_sync():
    asyncio.sleep(1)
    assert True


async def test_asyncio_sleep_async():
    await asyncio.sleep(0.5)
    assert True


def test_anyio_sleep():
    anyio.sleep(3)
    assert True


def test_sleep_zero():
    import time

    time.sleep(0)
    assert True


def test_no_sleep_clean():
    """Given two numbers when added then result is correct."""
    assert 1 + 1 == 2
    with pytest.raises(ZeroDivisionError):
        1 / 0
