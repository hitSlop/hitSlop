# Build acceptance and shared restricted runner

2026-10-06. Changes staged without commits.

`file::build::accept` checks the explicit `BuildInput` inventory and owns all bytes
that the new packer will write. It checks markers before filesystem access, fixed
program roles, resource kinds, byte budgets, media signatures and content hashes.
Paths are walked from a held directory descriptor with `openat` and `O_NOFOLLOW` on
every component. Symlinks, traversal, devices and pipes are refused. Unlisted files
are irrelevant. Skins and artwork use the full PNG checker.

Initial values now round-trip through their Loro checkpoint before acceptance.
The regression `a_seed_that_loro_would_round_is_refused` failed before the fix:
9007199254740993 in an `s.number` field silently became a different value. Acceptance
now refuses that loss while allowing numerically equal JSON spellings such as 1.0.
Nested row identities and deterministic checkpoint generation remain covered.

The existing restricted QuickJS child moved into `hitslop-runner`. The engine keeps
its private evaluator entrypoint, backed by the shared crate; a standalone
`hitslop-evaluator` executable is ready for app integration. Every evaluation gets a
fresh process and runtime, with the existing memory, time, input/output and OS sandbox
bounds. The process launcher accepts a host-configured absolute executable path.
Tests cover the engine entrypoint, sandbox access refusal, deadlines, atomic failure,
and module state starting fresh on every invocation.

Verification:

- Seven new build-input tests and the existing file tests passed.
- The extracted evaluator and engine suites passed.
- Workspace Clippy with all targets passed.
- `bun run verify` passed in 332.6 seconds: 266 Rust tests (4 skipped), 152 Bun tests
  and hygiene/build checks. Unchanged tiers used their existing receipts.

This is acceptance and evaluator infrastructure. The production SQLite layout and
default compiler still use the current pack path. Owner-routed commands, the signed
app helper, and their native integration tests remain to be connected. Linux remains
deferred to the final platform gate.
