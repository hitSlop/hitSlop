# Worktree consolidation

2026-10-06 · main branch `ready-ship`

There is one working checkout after consolidation. Every side worktree's unique
source and evidence was committed on its existing branch before that checkout was
removed. No branch was deleted, and no spike's build outputs were copied into main.

| Preserved branch | Checkpoint | Disposition |
| --- | --- | --- |
| `app-definition-refactor` | `47fbf217` (compiler), `460ed064` (evidence) | Compiler ported to the main working tree; plan/evidence reconciled there. |
| `spike/native-dev-owner` | `10316748` | Feasibility report and benchmark retained on main. Port lifetime/HMR behavior when the owner coordinator lands; discard the callback argument, TypeBox preview frame and engine-local coordinator. |
| `spike/schema` | `cce96d55` | Preserve serde/ts-rs/UniFFI and `s.*` experiments for selective porting. The ignored `archive/plans/schema-authoring-spike.md` report was explicitly added to this commit. Do not merge its old generated contracts or Specta probes wholesale. |
| `spike/commands` | `bc634a72` | Preserve multi-engine prototypes and evidence. Main already has the restricted QuickJS runner and bounded stale retry; the earlier alternate engines and retry policy do not replace them. |
| `spike/markers` | `43d48cb9` | Preserve the migration drill. Do not merge hypothetical version-2 formats into the pre-launch format-1 reset. The drill remains evidence for later migration work. |

At the user's request, the consolidation and implementation commits on `ready-ship`
were undone while retaining every working-tree change. Main remains based at
`6557240e`; completed work is staged after verification, with no new commits. The
preserved spike branches above are unchanged.

## What moved to main

- The two-build Vite compiler, command-body removal and UI stubs, restricted definition
  evaluation, dependency inventory, and real-bundle WebKit gate.
- The native-dev feasibility evidence, with Linux deferred.
- The revised packaging plan: seven lifecycle tables, an app-row asset seal, fixed
  bundle keys, optional stored command program, Rust page routing and media-type
  ownership, and no speculative storage-2 replay implementation.
- A runnable SQLite seal check, [check-app-seal.ts](../../scripts/dev/check-app-seal.ts),
  and its [result](app-seal-2026-10-06.json). It reads the plan's actual DDL, refuses all
  13 app/asset mutation attempts, and exercises the permitted document, attachment and
  artwork writes. Production storage has not switched yet.

The shared command runner remains the approved destination for both page and CLI
calls. Commands remain part of the released-document compatibility promise. The
proposed experimental CLI-only reversal was not adopted.

## Cleanup and recovery

Only clean, preserved checkouts were retired. The schema checkout also contained an
untracked `generated` symlink into main; it was an output reference, not unique source.
Ignored dependency/build caches, generated native resources and temporary test
documents were discarded with the checkouts. No process referenced a retired checkout
when cleanup began. Main's build and dependency directories resolve inside main.

Use `git show spike/schema:archive/plans/schema-authoring-spike.md` to read the schema
report, or `git show <checkpoint>:<path>` for any preserved file. A future isolated
experiment can use `git worktree add <new-path> <preserved-branch>`; give it independent
build outputs. Routine implementation continues in `ready-ship`.

## Verification and limits

- `git worktree list --porcelain` lists only the main checkout.
- `bun run verify` passes on main: 152 Bun tests, 219 Rust tests (4 skipped), four
  installed-package tests (one skipped), contract drift, hygiene and type checks.
- The focused compiler/WebKit gate passes: five tests and 46 assertions.
- Apps without commands omit the stored program; nonempty `public/` folders fail
  explicitly. Both added tests reproduced the old behavior before the fix.
- No Swift or FFI surface changed in this checkpoint. Native migration verification
  remains required when those surfaces move.

This consolidates the implementation checkpoint; it does not claim the full packaging
migration is complete. The subsequent engine wire checkpoint is recorded in the plan's
implementation progress, followed by the native-helper and `BuildInput` checkpoints.
