# Marker fire drill (2026-10-06)

Spike A of [the versioning plan](../../plans/versioning.md#spike-a-marker-fire-drill-gates-step-2).
No marker had ever been raised. Does today's format support the first bump of each one,
while changing the format is still free? Run on the development M1. Data:
[marker-drill-2026-10-06.json](marker-drill-2026-10-06.json).

The drill ran in a throwaway worktree (`../hitslop-spike-markers`, branch `spike/markers`),
built from `0f05fe6f` plus the uncommitted tree, and treated `tests/compat/dev` as the
released corpus. It raised three markers for real:

- **Storage 2** adds a table (`local`, the per-replica table `s.local` would need).
- **Format 2** adds a required manifest field (`locale`).
- **ABI 2** adds one context member (`ctx.platform`).

"The older build" is today's release engine: storage 1, format 1, ABI 1, build
`30ef53620cbe276c`. The diff is kept locally in `spikes/markers/`.

## Result

All three bumps work with small changes, and the drill settles two design questions for
step 2: how read-only paths read an old storage version, and when migration runs.

| Check | Result |
| --- | --- |
| `hitslop-core` suite, including the corpus replay (storage-1, format-1 documents read, edited through a migrating save, closed and reopened) | 203 passed |
| `inspect`, `schema` and a closed `get` on a storage-1 file | Byte-identical afterwards, still storage 1 |
| The first write to a storage-1 file | Migrates it to storage 2 (the `local` table and the marker) in the save's own transaction |
| The older build on a migrated file | `get` replies `{ok: false, code: "rejected", reason: "requires_update", error}`; `inspect` exits 1 with `requires_update`. File byte-identical |
| A crash inside the migration after pages reached the file (it grew from 160 KB to 1.1 MB before the abort) | Reopened by the new build **or the older one**: rolled back to storage 1, bytes identical to the original, journal gone |
| A crash inside the migration before any page reached the file | Both builds read storage 1, bytes identical. The journal isn't hot and stays until the next write |
| A new document from a storage-1 template | The document is storage 2; the template is unchanged |
| A template built by the new CLI | Format 2, ABI 2, storage 2, `locale: "en"`. The older engine refuses it (`requires_update`, on the storage version, which it checks first) |
| Raising `PACKAGE_FORMAT` without a reader | Fails to compile at the assertion in `manifest.rs`, as intended |
| Format 1's manifest | Checked by the frozen format-1 schema, translated (`locale: "en"`), and the translation checked by the current schema. Format 2 without `locale` is refused, and format 1's rules still refuse a `locale` (they never loosen) |
| Swift, on its own | Today's `SlopManifest` cannot decode a raw format-1 manifest, so the risk is real |
| Swift through the core | `SlopFile` gets the current model (`locale` `en`) and `runtimeABI` 1 |
| Native page replay (`CompatCorpusTests`, 7 scenarios) | Old ABI-1 apps edit their saved documents in WebKit under the dispatching shell; the saves migrate; each reopens to its recorded result |
| A display read through a migrated in-memory copy, at the launch limits (132 MiB) | 107 to 262 ms and about 132 MiB of memory per open, against 0.4 ms read in place |

## What this changes in step 2

- **Read-only paths use a reader for each storage version, never a migrated copy.**
  - Each version has its own table set (`expected_tables(version)`), and reads allow for
    tables an older version lacks.
  - An in-memory copy costs a Quick Look preview or a catalog scan up to 262 ms and
    132 MiB per file.
- **Migration runs inside the first write's transaction, not at open.**
  - One `begin_write` starts every write: the save, attachments, artwork, reclaiming, the
    clean copy, and new files made by `copy`.
  - It migrates when needed, then runs every open's checks on the result before the
    commit.
  - A closed `get` opens the writer owner but writes nothing, so it never migrates. It
    needs no new route, and the rule in `AGENTS.md` ("only a write under the writer lock
    migrates a file") holds as written.
- **Format readers are a frozen schema plus a translation.**
  - Each released format keeps its JSON schema and a function that brings its manifest to
    the current model; the result is checked by the current schema.
  - Hosts read only the translated manifest (`OpenedApp.manifest`): the FFI's
    `manifest_json`, `inspect` and the Rust catalog all switched to it.
- **The `runtimeABI` route** is six small edits: `OpenedFile.runtime_abi`,
  `SlopFile.runtimeABI`, the page `config`, and `contextFor(abi)` in `boot.ts`. As planned,
  it lands with the change that raises the ABI.
- **Raising a format updates the test fixtures**, which are stamped at the current format.
  `tests/support`'s manifest needed `locale`. That's expected work, not a defect.

## Also found

- **A fresh install can't build fixtures that live outside the workspaces.**
  - `tests/abi/owner-svelte`, and the temporary project of the CLI's build test, resolve
    `@hitslop/document` only through root `node_modules` links left over from an older
    install (dated 2026-09-19 in the main checkout).
  - CI installs fresh, and run 37410524077 fails with the same error. The fix belongs with
    the one-package step: fixtures declare the SDK they use, or the build resolves it
    through the CLI's own copy.
- **A new worktree needs the pinned code generators** (`generated/core-tools`), which are
  ignored by git and were copied from the main checkout. `scripts/build/core.ts` prints the
  install command.

## Not run

- **A4, layout 2.** Under the sync rules, layout 2 is reached only when sharing begins,
  from one shared starting snapshot. That's a value-preserving rewrite of the kind the
  2026-10-03 mergeable layout already did
  ([evidence](mergeable-layout-2026-10-03.json)). Run it with the collaboration work.
- The Quick Look extension itself, which opens through the same `open_file` path as
  `inspect`. Linux.
