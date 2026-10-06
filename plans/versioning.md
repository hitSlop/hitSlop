# Unified versioning and releases

Status: implemented, 2026-10-06. The exact command protocol, tolerant discovery and
candidate-writer corpus work is preserved. The development corpus is refreshed; freezing
the first public corpus and publication remain release steps.

## Contract

- One `hitslop` npm package, one Mac/npm release version and one `vX.Y.Z` tag.
  The first shared version is 1.0.0; Mac build numbers continue increasing.
- Versions label releases. Protocol and file markers decide compatibility; independently
  updated installations need not have identical release numbers.
- Preserve old documents, including their embedded JavaScript and command bundles.
  Updating hitSlop never replaces an existing document's app.
- Read older storage in place. Migrate only inside a necessary write under the writer
  lock, validating and committing data and markers atomically. Display reads and export
  rendering do not migrate their source; flushing pending edits remains a save.
- Storage, layout, package format and runtime ABI remain separate requirements. Raise
  only the marker a change needs. Unsupported markers refuse without writes.
- Frozen manifest acceptance and app limits belong to each package format; persistence
  limits belong to each storage version. New authoring restrictions cannot reject old files.
- Format readers normalize into the host model; that model does not reapply current
  authoring restrictions. Document-app upgrades and replica upgrades remain deferred.

## Implementation

1. Finish compatibility protection: versioned acceptance and limits, a normalized host
   manifest, write-time migration infrastructure, and a permanent small socket preflight.
   Keep the existing refusal shape, discovery fields and checks on the actual request.
2. Consolidate into `packages/hitslop`: `src/{cli,sdk,schema,shell}`, generated contracts,
   templates and skills. Public exports are `.`, `./svelte`, `./embed`, `./package.json`.
   Enforce schema → SDK → shell import direction; CLI may consume all three.
3. Project scripts run pinned authoring tools. Explicit `--project=DIR` can delegate;
   never scan arbitrary directory arguments or switch document commands based on cwd.
   Global authoring with a mismatched project install directs users to project scripts.
4. Ship the engine only in npm (darwin-arm64, linux-x64, linux-arm64). Keep the app's
   linked core and native rendering helper. Remove sibling-engine selection and retain
   development overrides. Project skill links go through unresolved node_modules paths.
5. One resumable, serialized release pipeline: build engines and Mac artifacts, replay
   the corpus with the packaged engine, publish npm, promote Sparkle, then deploy docs.
   Existing artifacts must match recorded hashes; older runs never supersede newer builds.
6. Integrate [minimal commands](slop-commands.md), restore all bundled-template corpus
   coverage, rehearse packaging and freeze the first public corpus only after final APIs.

## Verification

Every historical document reads, renders, edits, saves, exports and reopens. Test stricter
new authoring rules, repeat opens, preserved IDs and attachments, and unsupported markers
without writes. There is no historical migration before launch; the first one must add
interruption and losslessness tests. Protocol preflight must handle payload limits
changing between versions. Test one installed tarball outside the checkout, project
pinning, global document commands, skills and Linux engines. Run `bun run verify` per
stage, native verification for native changes, and `bun run release:check` before release.

## Existing evidence and external setup

- [Marker fire drill](../docs/evidence/marker-drill-2026-10-06.md)
- [One-package installation](../docs/evidence/one-package-install-2026-10-06.md)

Registry ownership/trusted publishing and deployment credentials must be available before
publishing. Local implementation and rehearsal do not require publishing a release.
