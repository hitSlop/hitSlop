# Release hitSlop

The Mac app and the single `hitslop` npm package ship together under one `vX.Y.Z` tag.
Their shared version labels a release; file markers and the exact command protocol decide
compatibility. The first shared version is 1.0.0. Mac build numbers continue increasing.

## Prepare and freeze

1. Set the same version in root `package.json`, `packages/hitslop/package.json` and
   `apps/apple/project.yml`; increase `CURRENT_PROJECT_VERSION`. Update starter pins,
   public examples and the lockfile. Commit the candidate.
2. Run the Engines workflow on that commit. Its platform table is
   `scripts/build/platforms.json`: darwin-arm64, linux-x64 and linux-arm64. Download
   the artifacts into `generated/engines/<platform>/`. Each records its commit and core
   build; packing refuses mixed candidates.
3. Capture `bun run compat:capture VERSION --frozen` from the clean candidate. The
   default covers every bundled template, plus conformance fixtures, saved updates,
   attachments, page actions and stored commands. The entry records acceptance-rule
   hashes and producing inputs. Commit only the corpus. Frozen entries are permanent;
   the prelaunch `dev` entry may be replaced and is never frozen.
4. Run `bun install --frozen-lockfile`, `bun install --cwd apps/landing --frozen-lockfile`
   and `bun run release:check` on the final commit. Complete the manual acceptance below.
5. Push and wait for CI, then tag that exact commit `vVERSION`. Never move a public tag.
   A manual run of Release hitSlop checks the gate without publishing.

## Publication and recovery

`.github/workflows/macos-release.yml` serializes all release versions. It builds the
platform engines, checks the frozen corpus, builds/signs/notarizes the Mac artifacts,
verifies the installed app, and retains the tested npm tarball. The full candidate is
saved before upload. `release-record.json` names the tag, commit, originating workflow
run, Mac build, package version, shell digest and SHA-256 artifact hashes.

Publication stages the artifacts in a draft GitHub release, publishes the exact npm
bytes under a version-specific tag, promotes npm `latest` and the GitHub release
(Sparkle), then deploys the site. A previously published npm version must have the same
SHA-512 integrity. Existing GitHub assets must match their recorded hashes. A run with
an older Mac build cannot promote itself or redeploy the site over a newer release.

Rerun a failed workflow. Once a release record exists, it restores the recorded candidate
instead of rebuilding signed files. An interrupted upload can recover from the retained
`release-candidate` artifact (90 days). After all assets uploaded, the release itself is
the durable copy. Never replace a recorded artifact to get past a failure.

Before the first publication, configure ownership of `hitslop`, its npm trusted publisher
for `macos-release.yml` with both publish and dist-tag permissions, the existing Apple
signing/notarization/Sparkle secrets, and Cloudflare deployment credentials. The workflow
uses npm 11.21.0 for OIDC dist-tag support; see [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/).
These are deployment prerequisites, not part of local builds.

## Validate what ships

`release:check` runs every tier in the `dist` Cargo profile, including native rendering,
process-death recovery, historical document replay and the installed-tarball consumer
outside the workspace. Reports and render evidence live under `.hitslop/evidence`.
A dirty-tree or partial-tier result is not release acceptance.

The Mac app contains its linked core and `hitslop-native` rendering helper. The npm
package contains its own document engine. Release checks compare core identities and
protocols, render the packaged templates, and exercise creation/editing/export with a
system-only PATH. Engine and helper placement is independent. Development overrides
are `HITSLOP_ENGINE` and `HITSLOP_NATIVE_CLI`.

`bun run packages:pack` creates `generated/npm/hitslop-VERSION.tgz`. The installed-consumer
check covers SDK exports, init/install/check/build, preview, global and project skills,
and (in the native release tier) registration, document edits and PNG/PDF export. There
are no workspace dependencies in the tarball and no separate schema or shell packages.

## Manual Mac acceptance

Record commit, version/build, OS, and results. Test macOS 15.2 and the current supported macOS on Apple silicon:

- Fresh offline install: with networking unavailable, launch the installed app and
  exercise every selected starter, create a working copy, edit/save/close/reopen,
  Recents, local template discovery, and PNG/PDF export. Confirm no engine download
  is needed. Bundled-resource checks alone do not establish this acceptance.
- Install an external template file; confirm categories and selection update when it is removed. Invalid files report local issues without hiding valid templates. Open a template received by Mail: it offers to create a document. Check Quick Look's thumbnail and Space preview on a document sent by AirDrop.
- Type then close, reorder rows, commit IME, and quit with multiple documents.
- Failed save retains ownership; retry works and cancelled quit preserves other documents.
- Kill WebContent, reopen saved state, and continue editing.
- PNG/PDF, Finder preview/icon, keyboard focus, narrow windows, repeated open/close, and menu commands.
- GitHub and Discord remain in the native sidebar; verify Discord opens the configured community invite.
- Gatekeeper launch from downloaded DMG and ZIP; installed helper works without Node/Bun.
- Sparkle update from the previous signed release, verifying publisher and resulting version.

Use a disposable Release validation build to verify representative Analytics events and a symbolicated Crashlytics test crash outside the debugger. Relaunch after the crash for upload. Debug/tests do not upload telemetry. Do not ship a crash trigger. Retain the Release dSYM upload phase and verify reported version/build. Unit tests do not prove Firebase delivery.
