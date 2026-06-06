# Golden corpus: PYTEST-MOC-002 MagicMockOnAsyncRule
# expect: PYTEST-MOC-002
# expect: PYTEST-BDD-001
# expect: PYTEST-MNT-004

from unittest.mock import MagicMock


async def test_async_with_magic_mock():
    mock = MagicMock()
    await mock.async_method()
