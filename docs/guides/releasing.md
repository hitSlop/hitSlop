# Release hitSlop

The Mac app and the single `hitslop` npm package ship together under one `vX.Y.Z` tag.
Their shared version labels a release; file markers and the exact command protocol decide
compatibility. The first shared version is 1.0.0. Mac build numbers continue increasing.

## Release branches and hotfixes

Development continues on `master`. When a candidate is ready to stabilize, cut
`release/X.Y` from that candidate commit (for example, `release/1.0`). Keep only the latest
release line maintained by default. A release branch accepts stabilization and patch fixes
through PRs; it is not a second feature-development branch.

PRs into a release branch and every push to it run all required CI tiers, including the
full native suite. Before tagging, run **Release hitSlop** manually on the final branch
commit and wait for the full release dry run to pass. That workflow builds matching Engines
artifacts and validates the release gate. Any later commit requires another dry run;
record the accepted SHA and tag that exact commit. Branch pushes and manual runs do not
sign or publish. Only `v*` tags enter publication.

For an urgent fix to `v1.0.0`, branch the fix from `release/1.0`, merge it through a PR,
prepare version `1.0.1` and a higher Mac build number, and follow the same freeze and
release acceptance steps below. This excludes unfinished changes on `master`. Forward-port
the fix through a PR into `master` so the next release keeps it; do not overwrite master's
development version or copy release-only version bumps back blindly. Frozen corpus entries
remain permanent and must also be retained on master. If the release branch has been
retired, recreate it from the latest tag in that line before preparing the fix.

## GitHub rules rollout

Install these workflows on the default branch **before** activating release-branch rules.
The attribution validator always executes the default branch's trusted code, so changing
only a release branch cannot install that check. The workflow trigger and validator both
must recognize `release/*`.

The reviewed ruleset in [release-ruleset.json](../../.github/release-ruleset.json) is staged
with enforcement disabled. It targets `refs/heads/release/*`, requires PRs and resolved
review threads, blocks force pushes/deletion, and requires `fast`, `native`, `linux-smoke`,
`Gitleaks` and `Attribution` from GitHub Actions. It permits initial branch creation without
checks on that new ref; subsequent changes require current checks. Create branches from
the chosen candidate or released tag, never as a way to bypass validation before tagging.

After the workflow PR merges, confirm a release-target PR reports every required check,
then activate the staged ruleset in repository Settings → Rules → Rulesets. Ensure master's
required checks also include `Attribution` after that workflow is installed. Keep the
existing release-tag protection. Branch protection controls merges; the release dry run
and immutable tag workflow still control acceptance and publication.

## Merge-ready candidate

Finish cleanup and commit the candidate on its working branch. Run `bun run verify`
and `bun run verify --native`, then obtain Engines artifacts for that exact commit and
run the frozen-lockfile installs and `bun run release:check` below without setting
`HITSLOP_RELEASE_TAG`. Retain the commit and evidence report. This validates a candidate;
it does not freeze 1.0.0 or prove signing, publishing, deployment or manual acceptance.

After merge into the release branch, any new commit needs matching Engines artifacts again. Branch cleanup,
npm bootstrap/deprecations and public release changes are separate release work.

## After merge: prepare and freeze

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
   the prelaunch `dev` entry may be replaced and is never frozen. Each public version
   gets its own entry even when all format markers remain at 1. Keep the original
   `.slop` bytes and expectations in the test repository, not the installed app.
   Required affected PR checks replay all entries through Rust and the native host
   (open, render, edit, save, reopen); Playwright qualification is separate.
4. Run Engines again on the corpus-only commit and replace the downloaded artifacts
   with that commit's outputs. The corpus records producing source inputs, which a
   corpus-only commit does not change; engine packaging requires the exact final commit.
5. Run `bun install --frozen-lockfile`, `bun install --cwd apps/landing --frozen-lockfile`
   and `bun run release:check` on the final commit. Complete the manual acceptance below.
6. Push and wait for CI, then tag that exact commit `vVERSION`. Never move a public tag.
   Before tagging, run Release hitSlop manually on the final candidate. It checks the
   gate but skips signing, notarization, publication and deployment; it cannot validate
   their credentials or npm permissions.

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
Configure a new trusted publisher close to publication: npm requires its first
successful publish within two days. These are deployment prerequisites, not local builds.

First-publication checklist, after merge:

- Confirm ownership of `hitslop`. If a bootstrap publication is needed to configure
  trusted publishing, publish it under a bootstrap dist-tag and explicitly accept that
  its version remains part of npm history.
- Configure the exact repository/workflow with both direct publish and dist-tag access;
  confirm Apple, Sparkle and Cloudflare credentials independently of the dry run.
- Retain the previous signed app and reachable appcast for Sparkle acceptance before
  cleaning obsolete prerelease releases or tags. Keep Git history and increasing Mac builds.
- Complete and record the manual acceptance below. Publish only the accepted commit.
- Once `hitslop` is available, deprecate the superseded `@hitslop/*` packages with a
  message pointing to it. Unpublication is not required for this release.

## Validate what ships

`release:check` runs release acceptance in the `dist` Cargo profile, including native rendering,
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
