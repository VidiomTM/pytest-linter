# Rules Overview

pytest-linter includes **49 rules** across four categories (Flakiness, Maintenance, Fixture, Enhancement), with Infrastructure as a documentation grouping for rules related to test infrastructure.

## Flakiness

| Rule ID | Name | Severity |
|---------|------|----------|
| [PYTEST-FLK-001](./PYTEST-FLK-001.md) | TimeSleepRule | Warning |
| [PYTEST-FLK-002](./PYTEST-FLK-002.md) | FileIoRule | Warning |
| [PYTEST-FLK-003](./PYTEST-FLK-003.md) | NetworkImportRule | Warning |
| [PYTEST-FLK-004](./PYTEST-FLK-004.md) | CwdDependencyRule | Warning |
| [PYTEST-FLK-005](./PYTEST-FLK-005.md) | MysteryGuestRule | Warning |
| [PYTEST-FLK-008](./PYTEST-FLK-008.md) | RandomWithoutSeedRule | Warning |
| [PYTEST-FLK-009](./PYTEST-FLK-009.md) | SubprocessWithoutTimeoutRule | Warning |
| [PYTEST-FLK-010](./PYTEST-FLK-010.md) | SocketWithoutBindTimeoutRule | Warning |
| [PYTEST-FLK-011](./PYTEST-FLK-011.md) | DatetimeInAssertionRule | Warning |
| [PYTEST-XDIST-001](./PYTEST-XDIST-001.md) | XdistSharedStateRule | Warning |
| [PYTEST-XDIST-002](./PYTEST-XDIST-002.md) | XdistFixtureIoRule | Warning |

## Infrastructure

| Rule ID | Name | Severity |
|---------|------|----------|
| [PYTEST-INF-001](./PYTEST-INF-001.md) | NetworkBanMissingRule | Warning |
| [PYTEST-INF-002](./PYTEST-INF-002.md) | LiveSuiteUnmarkedRule | Warning |
| [PYTEST-INF-003](./PYTEST-INF-003.md) | NonIdiomaticMonkeyPatchRule | Info |
| [PYTEST-INF-004](./PYTEST-INF-004.md) | MacOsCopyArtefactRule | Warning |

## Maintenance

| Rule ID | Name | Severity |
|---------|------|----------|
| [PYTEST-MNT-001](./PYTEST-MNT-001.md) | TestLogicRule | Warning |
| [PYTEST-MNT-002](./PYTEST-MNT-002.md) | MagicAssertRule | Warning |
| [PYTEST-MNT-004](./PYTEST-MNT-004.md) | NoAssertionRule | Error |
| [PYTEST-MNT-005](./PYTEST-MNT-005.md) | MockOnlyVerifyRule | Warning |
| [PYTEST-MNT-006](./PYTEST-MNT-006.md) | AssertionRouletteRule | Warning |
| [PYTEST-MNT-007](./PYTEST-MNT-007.md) | RawExceptionHandlingRule | Warning |
| [PYTEST-MNT-014](./PYTEST-MNT-014.md) | ConditionalLogicInTestRule | Warning |
| [PYTEST-MNT-015](./PYTEST-MNT-015.md) | DuplicateTestBodiesRule | Info |
| [PYTEST-MNT-016](./PYTEST-MNT-016.md) | SleepWithValueRule | Warning |
| [PYTEST-MNT-017](./PYTEST-MNT-017.md) | TestNameLengthRule | Info |
| [PYTEST-PARAM-001](./PYTEST-PARAM-001.md) | ParametrizeEmptyRule | Warning |
| [PYTEST-PARAM-002](./PYTEST-PARAM-002.md) | ParametrizeDuplicateRule | Warning |
| [PYTEST-PARAM-003](./PYTEST-PARAM-003.md) | ParametrizeExplosionRule | Warning |
| [PYTEST-MOC-001](./PYTEST-MOC-001.md) | PatchTargetingDefinitionModuleRule | Warning |
| [PYTEST-MOC-002](./PYTEST-MOC-002.md) | MagicMockOnAsyncRule | Error |
| [PYTEST-MOC-003](./PYTEST-MOC-003.md) | PatchInitBypassRule | Warning |

## Fixture

| Rule ID | Name | Severity |
|---------|------|----------|
| [PYTEST-FIX-001](./PYTEST-FIX-001.md) | AutouseFixtureRule | Warning |
| [PYTEST-FIX-003](./PYTEST-FIX-003.md) | InvalidScopeRule | Error |
| [PYTEST-FIX-004](./PYTEST-FIX-004.md) | ShadowedFixtureRule | Warning |
| [PYTEST-FIX-005](./PYTEST-FIX-005.md) | UnusedFixtureRule | Warning |
| [PYTEST-FIX-006](./PYTEST-FIX-006.md) | StatefulSessionFixtureRule | Warning |
| [PYTEST-FIX-007](./PYTEST-FIX-007.md) | FixtureMutationRule | Warning |
| [PYTEST-FIX-008](./PYTEST-FIX-008.md) | FixtureDbCommitNoCleanupRule | Warning |
| [PYTEST-FIX-009](./PYTEST-FIX-009.md) | FixtureOverlyBroadScopeRule | Warning |
| [PYTEST-FIX-010](./PYTEST-FIX-010.md) | ModuleScopeFixtureMutatedRule | Error |
| [PYTEST-FIX-011](./PYTEST-FIX-011.md) | YieldWithoutTryFinallyRule | Warning |
| [PYTEST-FIX-012](./PYTEST-FIX-012.md) | FixtureNameShadowsBuiltinRule | Warning |
| [PYTEST-FIX-013](./PYTEST-FIX-013.md) | AutouseCascadeDepthRule | Warning |

## Enhancement

| Rule ID | Name | Severity |
|---------|------|----------|
| [PYTEST-MNT-003](./PYTEST-MNT-003.md) | SuboptimalAssertRule | Info |
| [PYTEST-BDD-001](./PYTEST-BDD-001.md) | BddMissingScenarioRule | Info |
| [PYTEST-PBT-001](./PYTEST-PBT-001.md) | PropertyTestHintRule | Info |
| [PYTEST-DBC-001](./PYTEST-DBC-001.md) | NoContractHintRule | Info |
| [PYTEST-VAL-001](./PYTEST-VAL-001.md) | InlineSchemaRedeclaredRule | Info |
| [PYTEST-MOC-004](./PYTEST-MOC-004.md) | MockRatioBudgetRule | Info |
