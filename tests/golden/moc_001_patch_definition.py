# Golden corpus: PYTEST-MOC-001 PatchTargetingDefinitionModuleRule
# expect: PYTEST-MOC-001
# expect: PYTEST-BDD-001
# expect: PYTEST-MNT-004

from unittest.mock import patch

from myapp.service import create_user


@patch("myapp.models.User.save")
def test_patches_definition_module():
    create_user()
