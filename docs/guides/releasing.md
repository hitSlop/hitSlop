# Releasing hitSlop

The latest host must open, render, edit, save and reopen every document a public
release wrote. Downgrades are unsupported. Before the first public release, `dev`
fixtures are replaceable; the unpublished 1.0.0 capture is not a public baseline.

## Release flow

1. On `master`, merge the version/build changes and reviewed `docs/releases/X.Y.Z.md`.
   The workspace, npm package and Mac marketing version agree. Both semantic version
   and Mac build number advance every published release.
2. Run `gh workflow run macos-release.yml --ref master`. The workflow uses its own
   dispatch commit throughout. It builds all platform engines, captures this version's
   corpus outside Git, replays every published corpus plus the candidate, signs/notarizes the app and checks Gatekeeper and the packaged owner/evaluator.
3. Download `release-candidate-1` from that run. Install and hand-test its DMG
   using the checklist below. The workflow summary names the artifact and record hash.
4. Approve the `release` environment in that same run. Publication creates the tag,
   stages and checks every asset, publishes the immutable GitHub release without
   making it latest, publishes the exact npm tarball, promotes npm/GitHub latest. Publication never builds or captures anything.

A rehearsal is the same candidate workflow: reject its deployment after testing.
There is no separate dry-run implementation and no tag-push workflow. The approval
wait expires after 30 days; unused candidate artifacts expire after 90 days. If an
unpublished candidate expires or its build fails, start a new workflow dispatch and repeat
acceptance. Candidate builds run only on attempt 1; publication retries reuse that
accepted candidate. Rerunning all jobs is refused.

Only the latest release line ships. If master contains unfinished work, create a
short-lived `hotfix/X.Y.Z` branch from the latest public tag, apply the fix and bump
version/build, then dispatch the same workflow on that branch. Forward-port the fix
and version changes to master. Never release an older line with a higher Sparkle build.

## Frozen compatibility evidence

Each immutable GitHub release contains `compat-corpus-X.Y.Z.tar.gz`, its exact npm
writer tarball, `release-record.json`, and `SHA256SUMS`. The record binds source commit,
workflow run/attempt, all artifact hashes and `qualified` (every prior corpus version
and SHA-256 actually replayed). Frozen evidence is never imported into Git, edited,
deleted or recaptured. GitHub allows whole-release deletion even with immutability;
do not delete public releases. The secondary copy lives on the maintainer's Mac, under
`~/.cache/hitslop/compat-corpora/releases/`, and is retained by Time Machine. Keep this
folder when cleaning build caches. Recovery requires original retained bytes;
rebuilding from a tag creates new evidence. See [Local release backup](#local-release-backup).

`bun run compat:resolve` discovers published releases with pagination, verifies their
immutable status and history closure, downloads missing archives and verifies hashes,
then extracts each archive once under `~/.cache/hitslop/compat-corpora/trees/<sha>/`.
A verified snapshot of version links is activated atomically at `.hitslop/compat/current`.
Standalone Rust, Swift and TypeScript tests use that pointer; run `bun run compat:resolve`
before them on a fresh checkout and whenever published history changes. The verifier
passes a fixed snapshot through `HITSLOP_COMPAT_ROOT` so concurrent preparation cannot
change an in-progress run. Candidate snapshots never replace the published pointer.
An explicit root is checked against the required published inventory and original archive
manifests before replay. Every selected manifest, including the candidate, contributes to
the verifier's cache key; a receipt alone cannot stand in for missing evidence.
Archives, extracted manifests and file hashes are checked again on reuse. Extracted files
are read-only; replay makes only its disposable document copies writable. A corrupt
cache fails with the affected path; remove only that cache entry and resolve again. Historical writer binaries come from the npm tarball attached to the
same immutable release, without depending on npm's availability.

The verifier prepares this automatically for corpus-consuming tiers. Listings and
unrelated filtered Rust tests stay network-free. `HITSLOP_COMPAT_OFFLINE=1` explicitly
uses cached history for local work, announces the limitation, and cannot certify a
release or run in CI. CI and publication refresh discovery. A new corpus invalidates
local verification passes. Drafts do not block ordinary PR tests; incomplete
publications block starting another release.

Capture is automatic in the candidate job:

```sh
bun run compat:capture X.Y.Z --writer generated/engines/darwin-arm64/slop-engine --out dist/corpus
```

Before capture, the candidate checks the command prelude: unreleased ABIs must match
the bundle of their current source; released ABIs must match their frozen bytes.
If an unreleased ABI drifts, run `bun run compat:runner`, review the generated change
and start a new candidate from the committed result. Released preludes are never regenerated.
Capture requires clean source and a matching candidate writer. No corpus-only commit,
source fingerprint, import PR, engine rebuild for publication or freeze command exists.
Keep `tests/compat/dev` until the first public corpus passes cold-cache replay. In the
post-launch cleanup PR, pin that release's version and `corpus.sha256` from its retained
record in `scripts/compat/required-history.json`, then remove the prelaunch fixture and
its fallback together. The map is empty only before launch. Retain the first pin permanently;
it prevents a cold machine from accepting empty history if every remote release disappears.
Later releases do not require edits to this file; their recorded history provides the closure.

## Publication and recovery

Candidate builds may overlap; only the publish job is serialized. After approval it refreshes published history and
requires an exact match with `qualified`; a release published since qualification
requires a new candidate. It checks the candidate job succeeded, its run/attempt and
commit, the retained record hash and all artifact hashes. It also requires an approved
review for this run’s `release` environment before any publication writes. A retry excludes its own
already-published release from the history comparison while checking its original bytes.

Rerun **failed jobs** in the original workflow after a publication failure, within
GitHub's [30-day rerun window](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs). Artifact retention is not a longer retry window. Do not
rerun all jobs or dispatch a new version. Recovery restores exact bytes from the
complete draft/public release or the original candidate artifact, never a new build.
If that rerun window has expired after publication started, recovery is a maintainer
operation using the original retained release record and artifacts at the original
commit, with GitHub/npm authentication supplied separately. Do not create new
evidence or overwrite the release; this workflow does not automate expired-run recovery.
The next candidate remains blocked until npm integrity/latest and GitHub latest match
the last immutable release record. Completion does not depend on an Actions run or the
website. Existing tags must point at the same commit, assets must have the same SHA-256,
and an existing npm version must have the same SHA-512. A failure after npm publication
can resume promotion. GitHub and npm have no shared transaction; the retained evidence
and resumable steps make partial publication recoverable.

An unrelated bare `v*` tag is reported but is not release history. A tag matching a new
candidate blocks it. If publication created it, resume the original run. If it was an
accidental hand-pushed tag with no draft/public release or distributed artifacts, a
maintainer can temporarily permit deletion of that one tag, delete it, and restore the
rule. Never use that recovery for a published release. Actual drafts and incomplete
publications still block a new candidate.

GitHub publishes and locks the corpus and writer before npm distributes a runtime.
The first GitHub publication becomes a compatibility commitment even if npm later
fails. Do not delete it to restart. Signing/notarization secrets are available only to
candidate signing steps.

Archive and export both use manual signing with the installed **Developer ID Application**
certificate for direct DMG distribution. App Store Connect credentials are used only
for notarization; packaging does not create development certificates or provisioning profiles.

## Website deployment

`website.yml` checks, builds and deploys landing changes merged to `master`, or runs
manually on `master`. Deployments are serialized separately from host publication.
The site's download link follows GitHub's latest release. Its outage or failed deploy
does not block a host release; rerun the website workflow to recover. Website artifacts,
credentials and deployment state are outside the host release record. Public docs track
`master`, so they can describe changes ahead of the current shipped app; identify that
availability in the affected docs.

## Repository and npm setup

- Enable GitHub immutable releases; retain the rule preventing public tag changes.
- Protect the `release` environment with the release owner's required approval. A solo
  maintainer must be allowed to approve their own candidate. Restrict deployment branches
  to `master` and `hotfix/*`; do not allow administrator bypass of the approval.
- Configure npm trusted publishing for repository `hitSlop/hitSlop`, workflow
  `macos-release.yml`, environment `release`, with both direct publish and dist-tag
  permissions. The workflow uses npm 11.21.0. Configure a new publisher close to release
  because its first publish must occur within two days. See
  [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/).
- Confirm ownership of `hitslop`. If npm requires a bootstrap package before configuring
  trust, separately approve a metadata-only `0.0.0-bootstrap.0` under `bootstrap`.
  It contains no executable/runtime and creates no supported document format.
- Confirm Apple signing/notarization and Sparkle credentials, and Cloudflare credentials
  for the independent website workflow. Keep the dSYM
  upload phase. Only after the first release is verified should superseded packages
  be deprecated in favor of `hitslop`.

After publication, verify npm integrity/dist-tags, all GitHub assets and immutable
status, the Sparkle appcast, and a cold-cache corpus replay. Complete the local backup
below and confirm a Time Machine backup includes it. Check website health separately. Make
`native-rust` required alongside the existing required checks. Retire `release/1.0`
after its fixes are merged; new hotfix branches are temporary and may be deleted.

## Local release backup

After each publication, resolve history and download all original assets into the Mac's
cache. `compat:resolve` alone is insufficient: it stores corpus archives and parsed
records in `history.json`, but does not retain the original record bytes or fetch every
writer package. The additional release directory preserves all assets needed for recovery.
Replace `X.Y.Z` with the published version:

```sh
bun run compat:resolve
gh release download vX.Y.Z --repo hitSlop/hitSlop --skip-existing \
  --dir "$HOME/.cache/hitslop/compat-corpora/releases/vX.Y.Z"
(cd "$HOME/.cache/hitslop/compat-corpora/releases/vX.Y.Z" && shasum -a 256 -c SHA256SUMS)
tmutil isexcluded "$HOME/.cache/hitslop/compat-corpora/releases/vX.Y.Z"
```

Require every checksum to pass and the folder to report `Included`. `--skip-existing`
allows an interrupted download to resume; a mismatching existing file is an error to
investigate, not a reason to overwrite evidence. Confirm a completed Time Machine backup
contains the release directory; an inclusion setting alone does not establish a backup.
Apple documents [Time Machine exclusions](https://support.apple.com/guide/mac-help/mh15622/mac).
Restore original assets from this directory or its Time Machine copy, preserving their
filenames and checksums. Keep the release record and corpus together; never recapture.

## Release notes

Draft locally with your coding agent; CI publishes reviewed text and never calls an AI
provider. After the code changes are committed, collect the release's evidence:

```sh
bun run release:notes --from v1.0.0 --to HEAD
# First supported release: summarize current capabilities, not prelaunch history.
bun run release:notes --initial --to HEAD
```

The command prints agent instructions, resolved commit IDs, commit messages, changed
paths and a diff summary. `--to` defaults to `HEAD`. Supply the previous published
release explicitly: old local prelaunch tags are not a release baseline. The diff compares
the two snapshots directly, including releases on different branches. Initial mode includes product and
authoring sources from the candidate commit. Collection only reads committed content;
it does not generate prose or modify files itself.

Ask the agent to inspect relevant diffs and referenced PRs, then draft
`docs/releases/<version>.md`. Every claim should describe an observable, supported
capability or change; omit internal refactors and deferred work. Use this structure:

```markdown
# hitSlop 1.0.1

## Mac app

### Fixed

- Describe the change a person using the app will notice.

## Authoring

- Describe CLI or SDK changes for app authors.
```

`Mac app` is required and must contain text; omit `Authoring` if there are no authoring
changes. Use optional third-level New, Improved and Fixed headings when useful. For an
authoring-only release, write “No Mac app changes in this release.” in the Mac section.
Keep links absolute so they work outside GitHub. Review and edit the draft, run
`bun run release:notes --check`, then commit it before running the candidate workflow. `bun run release:check` also validates the current notes
before building. Regenerate or amend the draft if later code changes affect its claims.

Packaging writes `hitSlop-<version>.md` for Sparkle with only the Mac app section and
`release-notes.md` for GitHub with both sections, installation requirements and the
first-release compatibility notice when applicable. Both files are checksummed and
retained with the candidate. The appcast links to the versioned Markdown asset on
GitHub; the release body uses the retained GitHub notes. Change the committed source
before release, not a generated or published artifact.

The Sparkle package's `exactVersion` in `apps/apple/project.yml` also selects the
`generate_appcast` tool. There is no separate packaging version override.

## Validate what ships

`bun run release:check` runs all release tiers with the shipping `dist` Cargo profile.
The workflow uses `--skip-app` because it checks the actual signed Release app after
packaging. Reports and render evidence are retained with the workflow.

`CompatCorpusTests` runs the original compiled page and its own contract scenario.
The signed-app corpus smoke tests the packaged owner/evaluator: open every saved file,
compare saved data, replay recorded call-channel probes (including refusals) on fresh
copies, apply its recorded edit, verify the app remains the live owner, then stop it
and reopen through the engine to verify persistence. This does not replace page replay.

The installed npm consumer checks SDK exports, init/install/check/build, preview,
skills, registration, document edits and export outside the workspace. Packing verifies
that every engine comes from the candidate commit/core and that the captured writer
is byte-identical to the shipped darwin-arm64 engine.

## Manual Mac acceptance

Record the candidate run, record hash, version/build, OS and results. Test the exact
candidate DMG; do not substitute a local rebuild.

### Local sanity library

Keep a personal library under the ignored `release-sanity/` directory in this checkout.
After each public release, create about ten varied mini apps with that exact published
`hitslop@X.Y.Z` package, then use the matching released Mac app to fill, save and close
their documents. Pin the SDK dependency too; a versioned CLI alone does not pin the
app's SDK. Keep these projects outside the repository's npm workspace resolution and
confirm they use the installed release, not a workspace link or engine override.
Start this history with the first public release; local prelaunch builds are practice.

```text
release-sanity/
  1.0.0/
    originals/          # Closed, saved .slop documents; never edit these
    projects/           # Optional original source and lockfiles
    README.md           # Package/host versions, build, and checks for each example
    SHA256SUMS          # Hashes of originals/*.slop
  runs/
    1.1.0-candidate-RUN/
      1.0.0/            # Disposable writable copies of the 1.0.0 documents
      results.md        # Candidate run/hash, macOS, observations and failures
```

Random ideas are useful; choose examples with a mix of text, lists and reordering,
numbers, attachments, named commands, themes, custom shapes and exports. Give each a
short description of its expected appearance and one edit to try. Use synthetic data.
Once saved and closed, hash the original documents, make them read-only and retain
those exact bytes. Never rebuild or replace an older set with today's SDK.

Before hand acceptance, verify each set's checksums and copy its originals into a fresh
candidate run directory. Make only those copies writable. Open them with the exact
candidate app and check rendering, an edit, save/close/reopen and a representative
export. Keep the originals closed and recheck their hashes afterward. A failure blocks
acceptance; retain the failing copy and notes. Add new examples after publication.

This library supplements the automated frozen corpus with human review of real apps;
CI does not depend on it. Keep it when cleaning the checkout, and include it in the
Mac's Time Machine backup alongside the retained release assets.

### Every release

- Install from the downloaded DMG and confirm Gatekeeper launch and the expected version.
  The installed app must work without Node/Bun or an engine download.
- Open fresh copies from every version in the [local sanity library](#local-sanity-library).
  Check the page, edit, save, close and reopen; create one new document too. Until the
  first public release, use the prelaunch sanity set and record that limitation.
- Check Recents, a basic PNG/PDF export, and quit/relaunch with saved edits intact.
- Update from the previous supported signed release through Sparkle. Confirm publisher,
  resulting version and that its existing documents still work. For the first public
  release, record this as unavailable and exercise an isolated appcast instead.

### When the change touches the area

Run all of these for 1.0.0. Later releases select them from the diff; record what ran.

- **Packaging, templates or catalog:** fresh offline install, every bundled starter,
  external template install/removal, local discovery, categories, Mail template handling,
  sidebar GitHub/Discord links and a downloaded ZIP install.
- **Editing, ownership or save lifecycle:** IME commit, type then close, row reorder,
  multiple-document quit, failed-save retry and cancelled quit retaining ownership.
- **Page/session recovery:** kill WebContent and recover saved edits; retain the Retry
  Save sheet, check diagnostic details contain no document values, and test Get help.
  Wedge a page with a busy loop: after 15 seconds it must terminate, recover through
  Reopen, and allow Close/Quit without hanging.
- **Widgets or previews:** all widget sizes, rename and same-volume move, edits/saves,
  regeneration after a Sparkle update, Finder icon and Quick Look after AirDrop.
- **Rendering, native UI or platform dependencies:** PNG/PDF, keyboard focus, narrow
  windows, repeated open/close and menus on macOS 15.2 and the current supported macOS
  on Apple silicon.
- **Updater or release notes:** isolated reachable appcast; versioned Markdown loads
  with headings/bullets in Sparkle and authoring notes remain on GitHub.
- **Telemetry, signing or build configuration:** a disposable Release validation build
  sends representative Analytics events and a symbolicated Crashlytics test crash
  outside the debugger. Relaunch for upload, check version/build and retain the dSYM
  upload phase. Do not ship a crash trigger. Debug/tests do not prove delivery.
