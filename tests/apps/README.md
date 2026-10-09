# Authored infrastructure fixtures

These small apps exercise SDK, authoring, and host contracts. They are not templates
and are never bundled with hitSlop. Tests may copy and vary them in disposable folders.
Keep their controls and assets minimal; assertions describe platform behavior, not an
example's design or workflow. Stored-layout fixtures stay in `tests/fixtures`, and
the ABI consumers and frozen compatibility corpus retain their separate contracts.
