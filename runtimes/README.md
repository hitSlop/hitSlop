# Runtime files and release archives

This directory tracks sealed runtime checksums in [releases.json](releases.json).
The current runtime is built from source; its generated JS/WASM files are ignored
by Git and bundled into the distributed Mac app, native helper, and authoring CLI.
The installed app loads those files locally. It does not build or download runtimes.

## Where the files live

Paths below are relative to the repository root.

| Location | Purpose |
| --- | --- |
| `packages/document/src/` | Runtime source, the static `boot.js`/`headless.js` entries and `runtime-identity.json`; Loro JS/WASM comes from the pinned dependency. |
| `runtimes/releases.json` | Committed contract/revision identities and checksums of their sealed runtime directories. |
| `packages/cli/runtimes/<contract>/` | Generated runtime shipped in the CLI package for browser previews. |
| `apps/apple/Packages/HitSlopApple/Sources/HitSlopWasm/Resources/runtimes/<contract>/` | Generated SwiftPM resources bundled with the Mac app and native helper. |
| `generated/v1/runtime-releases/<contract>-<revision>/<contract>/` | Local preserved release bytes used for historical compatibility tests and release packaging. |
| `runtimes/<contract>/` | Immutable older-contract files copied into consumers when another contract is introduced; no such directory is needed while contract 2 is the only supported contract. |

Each runtime directory contains `index.js` (the internal engine module), `boot.js`
(visible sessions), `headless.js` (engine-only sessions), `identity.json`, and `loro/`
with the Loro JavaScript and WASM files. Generated consumer copies and
`generated/` are [Git-ignored](../.gitignore), so they do not appear in GitHub's
source browser.

## Building and loading

`bun run build` builds the current contract from source and copies identical
runtime catalogs into the CLI and native resource directories. Older contracts,
when present, are copied from this directory without rebuilding their bytes.
Ordinary builds do not seal releases or update the checksum ledger.

When opening a document, the native host reads `assets/runtime.json`, selects
the bundled contract, and checks its revision meets `minRuntimeRevision` before
opening storage. The WebView loads files through `slop://app/__runtime__/`, backed
by that selected directory. The host serves the page itself: visible sessions load
`boot.js`, which opens the document and mounts `assets/app.js` through the `ctx`
ABI; closed-document editing loads `headless.js` and never loads authored app code. Browser previews serve the CLI's runtime over localhost.
The `.slop` document contains its authored code and runtime requirements, not the
engine. See the [runtime reference](../docs/reference/runtime.md).

## Preserving and restoring releases

Explicit sealing with `bun scripts/v1/runtime-release.ts` preserves the generated
bytes under `generated/v1/runtime-releases/` and records their checksum in the
ledger. The Mac release workflow attaches `runtime-<contract>-<revision>.tar.gz`
and `runtime-releases.json` to [GitHub Releases](https://github.com/hitslop/hitslop/releases).
These are permanent release assets, not temporary CI artifacts.

On a fresh checkout, after installing dependencies, run:

```sh
bun run compatibility:restore
```

The command uses the GitHub CLI (`gh`) to download missing historical releases,
extracts them into `generated/v1/runtime-releases/`, and verifies their bytes
against the committed checksums. It reuses verified local copies and refuses
damaged ones. Historical runtimes are never regenerated from current source.
The current contract/revision is built from source instead of downloaded.

An app release does not automatically need a new runtime revision. Multiple app
releases can bundle the same sealed runtime and attach an archive of those same
bytes. Each consumer ships one revision per supported contract; historical
revisions remain available for compatibility testing. See [versioning](../docs/versioning.md)
for revision and contract rules, and [releasing](../docs/guides/releasing.md) for
the release procedure.

## Upgrading Loro

A Loro upgrade changes runtime bytes, so it is always a new revision (or a new
contract if it cannot keep the three contracts in [versioning](../docs/versioning.md)).

1. Bump the exact pin in the root and `packages/document` `package.json` files and
   `runtime-identity.json`'s `loroVersion`; increment `runtimeRevision`.
2. Row and tree `$id`s are application registers, so identity does not depend on Loro's
   container ID format. Still review Loro's changelog for encoding, import and
   mergeable-container changes.
3. `bun run test` must pass historical readers: every sealed revision of the contract
   reads state written by the candidate, and mixed-version collaboration converges in
   both directions (`collaboration.json`).
4. Never enable shallow snapshots or history pruning without a fixture exercising them
   and a sync-aware design.
5. Run the native tiers and `release:check`, then seal the new revision.
