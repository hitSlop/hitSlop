# Release the Mac app and npm packages

Launch includes the signed/notarized Apple silicon Mac app and matching `@hitslop/schema`, `@hitslop/document`, and `@hitslop/cli` packages. Hosted template publication, catalog services, accounts, and collaboration remain deferred. npm publication is a separate maintainer-run action after the compatible Mac app is available. CLI-only releases may reuse an already shipped compatible app and SDK.

## CLI-only release

A CLI-only release keeps the shipped SDK packages and Mac app. Publish only the
CLI; keep SDK versions and Mac tags unchanged. This is supported only when the CLI
WASM core build ID is unchanged and matches the installed helper and live app. A core
change requires a matching Mac release. Identity equality does not replace the
installed-consumer smoke checks below.

1. Update the CLI version, lockfile, and user-facing commands. Generated projects
   must pin the CLI's exact document dependency independently of the CLI version.
   Confirm the npm version and `cli-vVERSION` tag are unused.
2. Commit the candidate, then run the complete `bun run release:check` on the clean
   commit. Additionally install the exact CLI tarball with registry SDK packages
   outside the checkout and exercise init/install/check/build/register, document
   editing, and PNG/PDF export using the already shipped compatible Mac app.
3. Push master and wait for `fast`, `native`, and the secret scan on that commit.
   Tag it `cli-vVERSION`. This tag does not trigger Mac signing/publication.
4. Create a GitHub Release retaining the tested CLI tarball, `SHA256SUMS`, gate
   report, and `release-record.json`. Record the commit/tag, CLI and dependency
   versions, compatible Mac version, page-shell digest, artifact hashes,
   and installed-app smoke results. Never attach new files to an old Mac release.
5. Verify the retained tarball checksum, then manually publish that exact file
   with `bun publish ./hitslop-cli-VERSION.tgz --access public --tag latest`.
   Do not repack or republish unchanged SDK packages. If publication is interrupted,
   inspect the registry before retrying; never overwrite a published version.
6. Verify fresh `bunx @hitslop/cli@VERSION init smoke --yes` and global `slop`
   consumers, successful generated-project installation without SDK overrides,
   and that default `bunx @hitslop/cli` resolves to the intended version. Retain
   registry integrity and consumer results with release evidence.

For coordinated Mac releases below, package manifests determine each tarball's
version. Release records include `packageVersions` and the page-shell digest. CLI
dependency pins must match the released SDK packages.

## Coordinated release sequence

1. Finish release preparation and commit a clean tree. Check package versions, dependency pins and Apple version/build together. Confirm the intended npm versions and Mac tag have not already shipped.
2. Run the complete local gate below on that final commit and record manual acceptance results. A report from a dirty checkout or another commit does not validate the release candidate.
3. Push `master` and wait for `fast`, `native`, and the full-history secret scan to pass for the exact commit. Optionally run the Release macOS workflow manually on `master` as a dry run of the complete gate. Tag that commit `macos-vVERSION` and push the tag; this triggers `.github/workflows/macos-release.yml`.
4. Monitor signing, notarization, Gatekeeper verification, and GitHub Release publication. Verify downloaded artifacts and complete signed-install/Sparkle acceptance. Retain the release record and checksums.
5. Download and verify the release's tested npm tarballs, then publish schema, document, and CLI in that order using the procedure below. Finish with fresh registry consumer checks.

If a gate fails, fix and validate the candidate before publication. Never move a published release tag or overwrite released artifacts. After partial npm publication, verify registry state and resume only missing packages.

## Validate one commit

Use the pinned Bun release, Xcode, and XcodeGen on macOS:

Build templates from a clean dependency installation. A frozen-lockfile install may leave old, undeclared links in existing workspace `node_modules` directories.

```sh
bun install --frozen-lockfile
bun install --cwd apps/landing --frozen-lockfile
bun run release:check
```

`bun run hygiene` runs only repository hygiene and does not establish release readiness.

The complete gate checks hygiene; builds the page shell, helper, and all active templates; verifies generated contracts, types, and skills; runs JS and native tests with presentation fixtures; exercises relocated helper editing/export and storage crashes; packs/tests npm artifacts outside the checkout without Node; checks/builds the public site; builds/verifies the Apple app; and exercises native process death. Any failure stops the gate. Partial or non-macOS checks are not a complete release gate.

Bundled selection comes from `examples/slops/bundled.json`. Every selected package must be present in the app with no unexpected stale starters. Every package is checked for matching build bytes, valid manifest/schema/initial data, immutable contents, and preview/icon artwork. Installed create/schema/get/reopen/PNG/PDF checks run on Quick Checklist. Set `HITSLOP_TEMPLATE_EXHAUSTIVE=1` to run those installed checks on every bundled template. Schema-specific mutation/crash probes use known fixtures separately. Packed consumer tests also compile the public getting-started tutorial; its code is an executable contract.

### CI tiers and caches

The tiers are listed in [testing](../testing.md#ci); the full `release:check` runs once, in the Release macOS workflow, on a tag before signing or manually as a dry run.

CI restores SwiftPM `.build` by toolchain, lockfile and source identity, with a compatible restore prefix. The template cache reuses complete, fingerprinted packages; damaged entries rebuild, compiler/SDK/page-shell/renderer inputs, copied document guidance, and inherited TypeScript configuration invalidate entries, CLI routing and unrelated scripts do not, and removed templates are pruned. Misses log their changed inputs. Master pushes keep the release template cache warm for tags. Local and CI template builds default to `.hitslop/template-cache`; `HITSLOP_TEMPLATE_CACHE_DIR` can override the location. `build` prepares the page shell/helper, while `build:templates` prepares the complete artwork corpus. Native PNG/QuickLook rendering remains outside the fast job.

Bundled templates are black boxes: every one compiles, opens and reopens in Bun, then renders PNG/PDF and reopens in the native smoke. Conformance fixtures and native boundary tests cover platform edits, picker cancellation, persistence and exports. Application-specific walkthroughs are removed. The gate retains PNG/PDF evidence without visual snapshot comparison.

`release:check` writes `.hitslop/evidence/release-check.json`, including failed stages. Review the report and render artifacts; a partial report is not release approval.

Validate the final commit, push master and wait for CI, then tag that exact commit. The release workflow rejects a tag that disagrees with the Apple project version.

## Package the Mac app

The deployment target is macOS 15.2+ on Apple silicon. App version/build values live in `apps/apple/project.yml`. Existing TCA features, local catalog, native windows/toolbar, Analytics/Crashlytics, Sparkle, and NativeCLI remain part of release.

```sh
scripts/install-macos-release.sh
scripts/package-macos-release.sh
bun scripts/release-artifact.ts /path/to/hitSlop.app
```

Embedding ships the native executable and its HitSlopDocument resource bundle. Verification checks matching host/helper runtime catalogs and exercises installed helpers with a system-only PATH, including PNG/PDF. Native editing needs no checkout, Node, or Bun. `HITSLOP_NATIVE_CLI` selects an explicit matching helper for authoring verification.

Developer ID, notarization, provisioning, App Store Connect, and Sparkle private keys remain outside Git. The tagged GitHub workflow runs the gate once (`release:check --skip-app`), then archives one Release app. `package-macos-release.sh` runs host-crash acceptance on that signed app before notarizing (it needs the Debug helper from `bun run build`; `HITSLOP_SKIP_ACCEPTANCE=1` skips it). The workflow then verifies DMG/ZIP artifacts and publishes the Mac release. It does not publish npm packages. Release the exact tested commit.

The workflow checks these repository secrets before installing/building: `MACOS_CERTIFICATE`, `MACOS_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`, `ASC_API_KEY_ID`, `ASC_API_ISSUER_ID`, `ASC_API_KEY_P8`, and `SPARKLE_PRIVATE_KEY`. Their presence does not establish certificate validity or account access; signing and notarization must succeed. Temporary certificate/keychain files are cleaned up even after failure. The `release-evidence` workflow artifact retains the gate report, render evidence, packaging log, and any completed release record/checksums.

## Manual Mac acceptance

Record commit, version/build, OS, and results. Test macOS 15.2 and the current supported macOS on Apple silicon:

- Fresh offline install: with networking unavailable, launch the installed app and
  exercise every selected starter, create a working copy, edit/save/close/reopen,
  Recents, local template discovery, and PNG/PDF export. Confirm no engine download
  is needed. Bundled-resource checks alone do not establish this acceptance.
- Install an unpacked external template; confirm categories and selection update when it is removed. Invalid packages report local issues without hiding valid templates.
- Type then close, reorder rows, commit IME, and quit with multiple documents.
- Failed save retains ownership; retry works and cancelled quit preserves other documents.
- Kill WebContent, reopen saved state, and continue editing.
- PNG/PDF, Finder preview/icon, keyboard focus, narrow windows, repeated open/close, and menu commands.
- GitHub and Discord remain in the native sidebar; verify Discord opens the configured community invite.
- Gatekeeper launch from downloaded DMG and ZIP; installed helper works without Node/Bun.
- Sparkle update from the previous signed release, verifying publisher and resulting version.

Use a disposable Release validation build to verify representative Analytics events and a symbolicated Crashlytics test crash outside the debugger. Relaunch after the crash for upload. Debug/tests do not upload telemetry. Do not ship a crash trigger. Retain the Release dSYM upload phase and verify reported version/build. Unit tests do not prove Firebase delivery.

## Publish the tested npm artifacts

`bun run packages:pack` writes tarballs under `generated/npm`. `bun run test:packed --native` installs those exact artifacts into a temporary directory with spaces and verifies initialization, authoring, registration, themes, exports, preview resources, and durable installed skill links after package-cache removal. It derives versions from package manifests. Consumer-only overrides connect unpublished tarballs; shipped manifests contain registry versions, never workspace/file dependencies.

Keep the tested artifacts from the release commit. Confirm package versions and dependency pins agree, and make the compatible signed Mac app available first. Download the three npm tarballs, `SHA256SUMS`, and `release-record.json` from the matching GitHub Release. Verify each tarball against its checksum and confirm the record identifies the tagged commit. The workflow retains the exact artifacts exercised by `test:packed --native`; do not repack them locally.

Authenticate locally with `npm login --registry=https://registry.npmjs.org`, then check the account with `npm whoami`. Keep credentials in your user configuration outside the repository. Publication may require an interactive 2FA challenge; Bun supports browser authentication and `--otp` for supported OTP challenges. See [npm authentication](https://docs.npmjs.com/accessing-npm-using-2fa/) and [Bun publishing](https://bun.sh/docs/pm/cli/publish). Never put tokens or OTPs in Git.

Publish the downloaded files in order (substitute the tested version and download directory):

```sh
bun publish ./generated/npm/hitslop-schema-VERSION.tgz --access public
bun publish ./generated/npm/hitslop-document-VERSION.tgz --access public
bun publish ./generated/npm/hitslop-cli-VERSION.tgz --access public
```

Wait for each package to be available before publishing dependents. Never overwrite a published version or rebuild a different artifact between verification and publication. If publication partially succeeds, inspect registry state and resume only the missing packages; do not blindly replay the sequence.

From a fresh directory, verify `bunx @hitslop/cli@VERSION init smoke`, install the generated project, and run check/dev/build/register with the compatible Mac app. Separately verify `bun install -g @hitslop/cli@VERSION` and the `slop` entry point. Confirm the default `bunx @hitslop/cli` resolves to the intended launch release. Record artifact checksums and smoke results with the release record. Packing, tests, and this cleanup never publish automatically.
