# Runtime versioning

## The promise

After any hitSlop update, supported documents keep opening, editing and exporting
correctly. That promise is kept by preserving three contracts and testing them with
frozen artifacts. It is not a promise that arbitrary JavaScript runs forever on every
future WebKit or macOS release; platform or security exceptions need an explicit
product decision.

## Launch baseline

The 2026-09 pre-launch reset retired contract 1 and its fixtures; no documents from
that era need to open; [runtime reset](runtime-reset.md) explains the reasons and changes. The launch baseline is **runtime contract 2, revision 1** with
SDK 2.0.0. `runtimes/releases.json` receives its seal when the launch release is cut
(see [releasing](guides/releasing.md)). From then on, every record is immutable.

## What a slop contains and what the app supplies

A `.slop` owns its compiled app, schema and state. The app supplies the engine and the
page. `assets/runtime.json` declares `runtimeContract` and `minRuntimeRevision`; SDK,
Loro and protocol versions are provenance only. Missing or invalid requirements are
refused before storage opens; unsupported contracts and insufficient revisions ask for
an app update.

| Immutable app content | Mutable document content | Ancillary |
|---|---|---|
| `manifest.json`, `assets/runtime.json` | `state/document.sqlite` (Loro log, checkpoint, `doc_id`) | `QuickLook/` |
| `state.schema.json`, creation-only `initial.json` | `state/theme.json` overrides | `.agents/` guidance |
| `assets/app.js`, `assets/app.css`, fonts, local assets, `assets/theme.json` | `state/attachments/<sha256>` | |

Missing or stale ancillary files never make saved work unreadable.

## The three contracts

Everything a frozen slop depends on falls in one of these. Each has a named gate.

1. **App module.** The runtime serves the page (packages have no `app.html`), opens the
   document, imports `/assets/app.js` and calls `default.mount(ctx, target)`. Also
   covered: `/assets/*` resolution, the mount target and CSP, `--slop-*` theme variables,
   capture behavior and package validation. Apps import nothing from `/__runtime__/`;
   the build rejects engine imports, bridge access and remote boot resources.
2. **The `ctx` interface** ([abi.ts](../packages/document/src/abi.ts)). Behavioral:
   arguments, results, timing and errors, not only names. It only grows; new
   capabilities are optional and listed in `ctx.capabilities`.
3. **Data.** Descriptor format 1 and its frozen schema-key canonicalization (sorted keys,
   array order kept, no whitespace, JavaScript JSON numbers); the Loro layout (`data`
   root, `$id` registers, mergeable containers only for shared fields); SQLite format 1;
   theme override and attachment reference formats.

| Contract | Gate |
|---|---|
| App module + `ctx` | Sealed consumers `2-1` (hand-written plain JS) and `2-1-svelte` (the real adapter) run their self-tests in WebKit: `test:render --fixtures`, `RuntimeCompatibilityTests` |
| Data | Bun replay of every sealed fixture, exact `issues.json`, scenarios, checkpoint/update phases, historical readers and mixed-version collaboration |
| Package history | `compatibility-history.ts` rejects changed ledger records or fixture bytes against the base commit |
| Shipped templates | `check:sealed-templates` in `release:check` |

## May change freely

The Swift↔JS bridge and `globalThis.__slop` are private between the runtime and the
host, which always ship together. Boot internals, the runtime's `index.js` exports
(beyond the harness contract in `compatibility-worker.ts`), CLI internals, and Loro
versions that pass historical readers and mixed-version collaboration may change in any
release.

## Identities

| Identity | Meaning | Rules |
|---|---|---|
| `$id` | A row or tree node | Random 128-bit application register. Immutable through authored operations; survives moves, reopen and matching JSON import. Never a Loro container ID. Rows with a missing, invalid or duplicate ID read a derived `x-` ID (a frozen hash of internal identity, never written); ownership of a duplicated ID never depends on position. |
| `doc_id` | A logical document | Minted with the database; renewed by Duplicate; copied by a plain file copy, so it never authorizes synchronization. |
| Loro peer ID | A writing session | Random per session; never persisted or copied. |

Schema compatibility (the schema key) is not app identity: unrelated apps may share a
schema.

## Reads and writes

Decodable state always opens. Semantic anomalies from merges are preserved and reported
through `ctx.document.issues`; representable values keep their stored value, unusable
ones read as a documented fallback (a non-finite counter reads as `null`) and refuse
edits beneath them. Nothing is repaired on open. Undecodable bytes, missing
dependencies, unsupported formats or schema keys and resource limits still fail.
Validation stays strict for local writes. Live edits are synchronous; saving measures the
actual full snapshot before acknowledging durability. A capacity failure retains live
work, reports `save-failed` and `full`, and blocks close/export. Further edits and retry
remain available. `full` clears after successful saving or explicit discard. Discard
reloads the latest durable state and clears drafts while retaining the writer lock;
a failed reload preserves unsaved work. Every accepted local commit is emitted once
on the outbound update stream, independently of persistence; imported peer updates
are persisted without re-emission. Projected counter types include `null` for overflow;
creation inputs and local writes still require finite numbers.

## Revisions and contracts

App versions and runtime identities are separate; several app releases may ship the
same sealed runtime. Compatible byte changes need a new revision; changes that cannot
preserve the three contracts need a new contract. Each consumer bundles one revision
per supported contract. Within a contract, state written by a new revision must stay
readable by every earlier revision, because documents keep their minimum revision.
Writers keep full history; readers accept shallow checkpoints, but safe pruning still needs a separate design
for peer catch-up and shared history. SQLite format 1 is owned by Swift; a format change
needs its own compatibility design.

`bun scripts/v1/runtime-release.ts` seals the current runtime and emits a release
directory; `bun run compatibility:restore` restores sealed releases for cross-revision
tests. Ordinary builds never change the ledger or fixtures. New capabilities get new
fixtures, authored with `bun scripts/v1/author-fixtures.ts`, which never replaces an
existing one.

## Independent CLI releases

The CLI package version may advance independently when its SDK and runtime
requirements are unchanged. `init` pins the project's SDK to the CLI's exact
`@hitslop/document` dependency, and builds compare the complete SDK identity.
