# Minimal, extensible slop commands

Status: implemented for v1, 2026-10-06. Checklist, Hourglass and the init starter use
shared commands. The development corpus replays stored command bundles; freeze the
first public corpus with the final release candidate.

## Public behavior

Storage stays in `schema.ts`; named domain actions are exported from `commands.ts`.
`definition.command({description, args, run})` creates a typed callable command. TypeBox
schemas describe JSON arguments. Synchronous execution receives immutable `current`,
collecting `tx`, and host-provided `now` and `random`. It returns JSON or throws a refusal.
Page buttons and CLI calls use the same authored behavior.

Keep handles, bindings, `doc.change()` and generic CLI operations. Commands expose useful
verbs; incidental UI batches need not be public commands. Add optional field descriptions,
`slop describe PATH [--json]`, and `slop call PATH NAME --args JSON`. Describe includes
schema, allowed operations, commands, values, IDs and snapshot version.

## Execution and storage

- Web pages execute their commands in WebKit. CLI commands evaluate in a restricted child
  process given only bundle, snapshot, arguments, clock and seed. It has no document
  handles, owner sockets, filesystem/network authority or second document engine.
- Bound execution time, memory, input and output. Parent owns file access and routing.
  Rust validates every returned intent and atomically applies one version-guarded batch.
- A definite stale rejection rereads and retries once, using the same invocation clock
  and random seed. A second conflict returns stale_base. Never retry unknown outcomes.
- One successful command is one named undo step. Page completion follows publication;
  CLI success follows the existing durable-write contract.
- Metadata and JavaScript use reserved immutable assets, excluded from page asset serving.
  No additional SQLite columns. Prelaunch markers remain 1; later incompatible command
  storage changes raise packageFormat and host behavior changes raise runtimeABI.
- Frozen command bundles replay through compatible host context without rebuilding.
  Pure command evaluation produces intents; it never becomes the document validity boundary.

## Implementation

1. Descriptions and a shared Rust describe representation, with CLI text and JSON output.
2. Named command exports and TypeBox argument inference/validation. Keep change().
3. Build metadata and runner bundle, strip author paths, reject unsupported schemas,
   foreign definitions and non-command exports. Protect reserved assets from page reads.
4. Restricted QuickJS child, version guard, bounded retry and command undo boundaries.
5. Checklist and Hourglass domain actions, author/document skills and compatibility replay.

Exact checked integer counters remain appropriate for the single-writer v1. Correct stale
contribution-map documentation; a future mergeable representation requires a lossless
layout migration. Do not introduce floating-point counters for this release.

## Verification and extension

Test page/runner parity, typed and runtime argument failures, refusal atomicity, resource
limits, concurrency and retries, unknown outcomes, command undo alongside typing, and
replaying stored bundles. Include commands in the launch corpus and restore Hourglass
page actions. Run verify per stage and native verification before completion.

Defer whole-command CRDT merging, queries, effects, MCP, Shortcuts, typed row references
and output schemas. Add capabilities when needed while retaining released command
semantics. Correctness, isolation and compatibility gate release; richer automation does not.
