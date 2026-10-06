# Versioning and distribution: one package, one version, one tag

Status: proposal, fourth revision on 2026-10-06. The third revision added packaging, release
automation and docs. The fourth adopts an outside review: it freezes the rules for saving as
well as opening, widens the permanent protocol boundary, promotes Sparkle only after npm
publishes, and moves freezing ahead of the package change. Nothing is implemented. The
`AGENTS.md` edits in steps 1, 3 and 5 need the user's sign-off.

## The question

hitSlop has `.slop` format versions, a CLI version and a Mac app version, and they're coupled
in ways that make the CLI, the app and releases more complicated than they need to be. How far
can we decouple them, given that users can be asked to update?

The second question is how simple distributing the app, the npm packages and the docs can
become, while every newer app and CLI keeps opening, editing, saving and reopening every older
slop. Breaking package changes are welcome before launch.

## What exists today

| # | Version | Where | Behavior today |
|---|---|---|---|
| 1 | Storage version | SQLite `user_version` (`crates/hitslop-core/src/file/mod.rs`, `STORAGE_VERSION`) | Only version 1 opens. A newer file gets `requires_update`; any other version is refused as invalid. Forward migration is a promise for later, not code |
| 2 | Layout | `meta.layout` inside the Loro document (`crates/hitslop-core/src/lib.rs`, `LAYOUT`) | Same as row 1 |
| 3 | `packageFormat` | The file's `app` row; `PackageFormat` in `@hitslop/schema` | An older app answers `requires_update` and offers **Update hitSlop…** |
| 4 | `runtimeABI` | The `app` row, stamped from the project's SDK | Same as row 3. `slop build` also refuses an SDK newer than the CLI |
| 5 | Command protocol `{version, minimum}` | `HelperProtocol` in `@hitslop/schema`; the engine, the helper and every socket request | Each side promises to keep serving every protocol from `minimum`. The release gate runs every released CLI against the new app |
| 6 | Core build ID | `crates/hitslop-core/build.rs` | The engine, helper and app must match exactly inside one bundle; npm engines are checked when packing |
| 7 | Page shell digest | Host and helper | Must match inside one bundle |
| 8 | Mac app version | `MARKETING_VERSION` 1.3.0, build 30 (`apps/apple/project.yml`) | A release label |
| 9 | npm versions | `@hitslop/cli`, `@hitslop/document`, `@hitslop/schema`, all 3.0.0 in the tree; npm has published up to 2.0.0 | A release label |

There are three gaps:

- **What an open or a save accepts isn't frozen.**
  - `crates/hitslop-core/src/manifest.rs` validates stored manifests against the latest
    generated `packages/schema/generated/manifest.schema.json`.
  - The open-time limits in `file/mod.rs` come from `wire.generated.rs`, so they're shared
    with authoring. So do the limits on every save: `STORAGE_BYTES` and `STORAGE_ROWS` in
    `store.rs` `within()`, the SQLite length limit that `file::connect` sets before any
    marker is read, and the attachment limits.
  - After Rust accepts a manifest, the FFI hands Swift the stored JSON
    (`hitslop-core-ffi/src/lib.rs`, `manifest_json`), and `SlopFile.swift` decodes it into
    today's `SlopManifest`. A field a later format makes required, or a changed enum, would
    reject an older file there, even with the Rust rules frozen.
  - The only guard is a compile-time assertion that fires when `PACKAGE_FORMAT` is raised.
    Tightening the TypeBox manifest or a limit without raising the format would silently
    reject files that opened before.
  - `docs/roadmap.md` already notes this, and `docs/guides/releasing.md` handles it with a
    manual copy at the first public release.
- **Programs are coupled.** Nearly all the coupling between the products comes from two
  things: row 5's promise that old programs keep working with new ones, and the second copy of
  `slop-engine` inside the app. Both protect programs from each other; neither protects files.
- **Distribution has many moving parts.**
  - The three npm packages are published by hand, in order, with 2FA.
  - CLI-only releases have their own procedure.
  - The site (`apps/landing`, Cloudflare) is deployed by hand.
  - A project pins `@hitslop/cli` and `@hitslop/document` separately, and they can disagree.
  - `slop init` copies the agent guides into the project. The copies drift when the
    project upgrades, and never reach agents that read their own folder, such as
    `.claude/skills`.

## The principle

**Support old documents forever. Require installed tools to update when they don't match.**

Old files depend on two things:

- the core's readers and limits: the app's, keyed by `packageFormat`, and the saved state's,
  keyed by the storage version and the layout;
- the shell's `ctx`, keyed by `runtimeABI`.

The app and the npm package compile the same core from one commit, and the frozen corpus
gates it. So "a newer build opens and saves every older slop" is one property of one core.
Programs only have to match each other.

| Boundary | Rule |
|---|---|
| Mac app and the `hitslop` package | One release version |
| CLI ↔ live owner and helper | One protocol number; the older side updates. Its refusal path is permanent |
| `packageFormat` | Readers for every released value are kept, each with its app limits, and each returns the current host-facing manifest |
| `runtimeABI` | Released behavior is preserved through the host context |
| Storage and layout | Readers are kept, or a lossless forward migration runs |
| Persistence limits | Keyed by the storage version and the layout. Never lowered for a released storage version, and raised only together with that marker |

- Release numbers never stand for compatibility. Protocol numbers and file markers do, so a
  compatible patch release forces no update.
- **An accepted edit stays readable under the markers its save writes.** Tightening an
  authoring limit never stops an existing valid document from being edited or saved.
- Updating programs doesn't remove the need to support what each `.slop` carries: its
  JavaScript, its descriptor and its saved state. Step 2 is what backs that promise.

What people install, and how each part updates:

| Who | Installs | Updates by |
|---|---|---|
| A person | hitSlop.app | Sparkle, or the **Update hitSlop…** button |
| A terminal or an agent | `bun install -g hitslop`, which gives `slop` and the global agent skills | The update notifier, then `bun install -g hitslop@latest`, which an agent can run itself |
| An author's project | `hitslop` as a devDependency, with its agent skills linked by Crust's project scope | `bun add -d hitslop@latest`. The links follow, because their path doesn't change |

## One version

- The release version lives in the root `package.json`. Release preparation writes it into
  `packages/hitslop/package.json` and `MARKETING_VERSION`, and increments
  `CURRENT_PROJECT_VERSION`, which Sparkle compares.
- Semver follows the author API, because `hitslop` imports are the only thing that reads
  version ranges:
  - a breaking SDK change is a major release;
  - new capability anywhere is a minor release;
  - fixes are patches.

  Protocol and marker bumps are independent of these.
- The first shared version is 3.0.0. It's above the Mac app's 1.3.0, it's what the tree
  already says, and nothing has been published under it. `hitslop` is a new name on npm
  (unclaimed on 2026-10-06), so nothing older competes with it there.
- **Cost:** a CLI-only fix ships an app update through Sparkle, and an app-only fix
  republishes the package. One tag automates both (step 5).
- **Why it's still worth it:** one answer to "which CLI goes with which app", one release
  procedure, and no table mapping CLI versions to app versions. Compatibility is gated by
  protocol and markers, never by the release number, so this can be relaxed later without
  touching compatibility.

## Why the engine can leave the app

Exactly one hop crosses versions either way. Dropping the app's engine only moves that hop:

| | Engine in both (today) | Engine in the CLI only |
|---|---|---|
| Edit a document open in the app | CLI → app's engine (different version) → app's owner (same build) | CLI → CLI's engine (same release) → app's owner over its socket (different version) |
| Edit a closed document | The app's engine runs the owner with the app's core | The CLI's engine runs the owner with the CLI's core |
| Export, open a window, native artwork | App's engine → app's helper (same build) | CLI's engine → app's helper (different version) |
| Build a slop | CLI's engine | CLI's engine |

So the app's copy only buys this: closed-document edits use the app's core, and a newer CLI
never upgrades a document past what the installed app opens. Under the principle, that case is
just an update prompt:

- **CLI newer than the app:** the app shows **Update hitSlop…**, as it already does for a
  document from a newer machine.
- **App newer than the CLI:** the CLI refuses documents the app has upgraded and says how to
  update.

Both only happen when a release raises a file marker, and with one release train only until
the other side updates.

## Alternatives considered

- **Engine only in the app.** `validate-app` and `pack` are the second half of the CLI's own
  build and name no protocol, so building would break whenever the CLI and the app differ.
  Linux has no app.
- **napi-rs (load the engine into Bun).** These reasons still hold:
  - The engine holds the writer lock and runs the owner's save worker. As a separate process
    it flushes and exits, and `process.ts` can kill it if it hangs. In-process, a hang or
    abort takes the CLI down while it holds the lock.
  - A static binary depends on no Bun ABI.
  - Each command runs once and the engine starts in under 10 ms, so there's nothing to speed
    up.
  - `compat_writers.rs` runs released engines as programs.
- **WASM for building.** The contract keeps WASM to `slop dev` and tests, and `pack` writes
  SQLite.
- **Per-platform npm packages for the engines.** They shrink the download, but add package
  resolution, publish ordering and install edge cases that have nothing to do with keeping old
  files working. Deferred; see step 5.
- **Separate `@hitslop/cli` and `@hitslop/document`.** They let a project upgrade its CLI
  without its SDK. But they need two pins, the "SDK newer than CLI" check, ordered publishing,
  and a rule for which SDK each CLI expects. One package removes all four. The cost is that an
  old project upgrades its SDK to reach a newer protocol, as with any framework's CLI.
- **The CLI inside the app, or a compiled single binary.** `dev`, `check` and `build` need
  real installs of Vite (with native Rolldown), svelte-check and TypeScript, and Linux has no
  app.
- **Guide copies in each project.** They work before `bun install`, but drift on upgrade and
  miss agent-specific folders. Crust's project scope links to the project's own install
  instead (step 6).
- **Capturing and shipping the same engine executable.** The corpus would need its capture to
  run in the tag workflow. Testing the shipped engine against the corpus before publishing
  gives the same assurance with a local capture (steps 1 and 5).

## Plan

### 1. Contract and compatibility evidence

**Protocol.**

- `{version, minimum}` becomes `{version}`. A side serves its own protocol, keeps no adapters
  for older programs, and refuses any other before touching a document, naming the older side
  to update.
- `HelperProtocol` in `packages/schema/src/constants.ts`, `--protocol` in
  `crates/slop-engine/src/main.rs`, and `HitSlopNativeCLI/NativeCLI.swift` all change to match.

**The permanent boundary.** Everything an old program needs in order to *receive* a refusal
never changes, so tools of any age can explain a mismatch to each other:

- **The helper:** `--client-protocol N` is the first argument of every command that reaches
  a document or the helper. A mismatch exits with status 2 and prints one line on stderr
  naming the side to update.
- **Live discovery:** the record's location (`~/.hitslop/live/`, named by the file's device
  and inode) and the two fields a client reads, `socket` and `documentPath`. Clients ignore
  any other field and never refuse one. Today `command.rs` `discovery()` checks the record
  against the strict `SocketDiscovery` schema, so an added field would stop an old client
  before it reaches the socket.
- **Framing:** one newline-terminated JSON request and one reply line, with the request limit
  the socket reads before parsing (`MAX_REQUEST_BYTES`).
- **Order of checks:** `protocol` is read from the parsed JSON before any envelope or method
  validation, as `command.rs` `parse()` does today, so a request in another protocol with an
  unknown method still gets the refusal.
- **The refusal:** `{ok: false, result: {code: "rejected", reason: "requires_update",
  error}}`. It counts as not applied. Nothing was written, so an update never requires
  replaying an edit whose outcome is unknown.
- **Its text** names the side, generically: "update hitSlop" for the app, "update the hitSlop
  CLI" for the CLI. The CLI turns the second into the exact command for the copy that ran
  (step 3).
- **Cross-version refusal tests** stay after old-program compatibility is dropped:
  - a socket request with protocol N+1 and an unknown method gets the refusal;
  - a discovery record with extra fields still connects;
  - the helper, given another `--client-protocol`, exits 2 with one stderr line;
  - the document is byte-for-byte unchanged.

**New marker rule.** A format change that an older build can't read correctly raises a
marker. An additive change without a marker is allowed only when older readers handle it.

**Corpus and release gate.** Old files are checked; old programs are not.

- **Drop:**
  - running each release's installed CLI against the new app (`HITSLOP_COMPAT_INSTALLED` in
    `tests/native/compat-replay.native.test.ts`, `scripts/verify.ts` and
    `scripts/release/package-macos-release.sh`);
  - the CLI command transcripts (`cli/transcript.json`, written by
    `scripts/compat/capture.ts`), which would freeze CLI syntax forever;
  - each entry's frozen install (`cli/install/` and its lockfile).
- **Keep:** templates, saved documents, attachments, expected state and explicit page
  interactions (`scenarios/`). Archived apps are never rebuilt during replay.
- **The entry stores the candidate writer**, not npm tarballs: the darwin-arm64 `slop-engine`
  built from the candidate, with its build ID, commit and hash recorded. That's about 3.7 MB
  in git per release, instead of a CLI tarball of about 12 MB.
  - `crates/hitslop-core/tests/compat_writers.rs` runs each archived writer on its own, then
    checks that this core reads, edits, saves and reopens what it wrote.
  - Request builders for older protocols live in that test harness, as test-only adapters.
  - Capture builds the writer locally on the Mac, so it doesn't need the Linux engines.
  - Native executables differ by machine even from the same source (`scripts/compat/
    integrity.ts` says so), so the writer isn't the shipped engine. Step 5 tests the shipped
    one against the corpus before publishing, and the release record keeps both hashes.
- The exact tested npm archive stays on each GitHub Release, as `scripts/release/bundle.ts`
  retains it today.
- Recapture `tests/compat/dev`, which is allowed before launch.

**Docs:**

- `docs/engineering-contract.md`:
  - the command-protocol row;
  - the permanent boundary;
  - the marker rule;
  - "the engine, rendering helper and live owner ship in one Mac app bundle" becomes the
    helper and the app only.
- `AGENTS.md` Compatibility:
  - the command protocol leaves "Public boundaries grow additively". Only its permanent
    boundary stays fixed;
  - "The engine and the helper ship in one bundle … meet only through the command protocol"
    becomes: the helper ships in the app bundle with the app's core; the CLI's engine and the
    app meet only through the command protocol; a mismatch names the older side.
- `docs/testing.md`: the compatibility corpus section.

### 2. Freeze what an open and a save accept (essential before launch)

This comes before the package change, because it protects the central promise on its own.
It replaces the manual step at the first public release in `docs/guides/releasing.md`.

- **Manifest.**
  - Generate the manifest schema from TypeBox under a name that carries its format, and have
    `manifest.rs` read the schema for the file's `packageFormat`. The generator lives in
    `scripts/build/generate.ts`.
  - **One host-facing manifest:** each released format's reader returns the current manifest
    model, with Rust filling defaults and translating older fields. The FFI's `manifest_json`
    and the engine's outputs carry that model, never the stored JSON, so `SlopFile.swift` and
    the CLI keep decoding only the current model.
- **App limits** are per-`packageFormat` constants: manifest, app text, theme defaults, asset
  paths, counts and sizes, and artwork and image sizes.
- **Persistence limits** are per-storage-version constants: `STORAGE_BYTES`, `STORAGE_ROWS`,
  the SQLite length limit `file::connect` sets, and the attachment file, total, count and name
  limits. They govern every save as well as every open.
  - They're never lowered for a released storage version.
  - Raising one raises the storage version, with a forward migration under the writer lock,
    so an older build answers `requires_update` instead of refusing an oversized file as
    invalid.
  - Keeping them apart from `packageFormat` stops mutable saved state from being tied to the
    immutable app.
- **Authoring limits** stay in `wire.generated.rs` and their TypeBox source, used only by
  building and validating new apps. Authoring may tighten them freely.
- **When a format freezes.** The first frozen corpus entry whose `markers` name a
  `packageFormat` or storage version locks that schema and those limits. A hygiene check
  refuses changing or deleting either. Until then they're regenerated freely.
- **Code paths** that can't be frozen as data are guarded by boundary documents in the
  corpus: the descriptor parser, shape parsing and the theme checks. Each document holds limits
  at their maximums, every descriptor kind and every window shape form. This is the roadmap
  item, now in scope.
- **Embedded apps.** Old ones keep running through the host context
  (`packages/shell/src/boot.ts`, `createContext`). Add an adapter only when behavior actually
  diverges, and share compatible code otherwise. Opening an old document never requires
  rebuilding its app or installing its original SDK.
  - **The `runtimeABI` route:** the change that raises `RuntimeABI` carries the stored ABI
    through `OpenedFile` and the page `config` into `boot`, next to the existing assertion
    (`RuntimeABI satisfies 1`), which already fails the build until it does. The host and
    the shell ship together, so this is not a compatibility boundary, and no field is added
    before it's used.
- **Docs:** `docs/engineering-contract.md` ("checks that run on open are versioned by
  `packageFormat`" becomes literally true, and gains the save invariant and persistence
  limits), and the roadmap entry closes.

### 3. One `hitslop` package

- **Layout.** `packages/hitslop/` holds:
  - `src/cli/`, from `packages/cli/src`;
  - `src/sdk/`, from `packages/document/src`, including `app/*.svelte`;
  - `src/schema/` and `generated/`, from `packages/schema`, including step 2's per-format
    schemas and limits. TypeBox still owns the wire, and `bun run schema:generate` writes
    here;
  - `src/shell/`, from `packages/shell/src`. It's built to `shell/` for the CLI and the app,
    and its source is left out of `files`;
  - `templates/`, `skills/`, and the tests under `tests/{cli,sdk,schema,shell}`.

  The root workspaces become `packages/hitslop` and `examples/slops`.
- **Surface.** In `package.json`: name `hitslop`, bin `slop` pointing to `src/cli/cli.ts`,
  and three exports: `.`, `./svelte` and `./embed`. `./abi`, `./internal` and every schema
  subpath go away. Code inside the package uses relative imports, because each author's Vite
  bundles the SDK source from `node_modules`.
- **Internal boundaries stay enforced.** `scripts/hygiene.ts` checks the import direction:
  - `schema` imports nothing internal;
  - `sdk` imports `schema`;
  - `shell` imports `sdk` and `schema`;
  - `cli` may import any of them.
- **No skew between a project's CLI and SDK.** Package scripts (`bun run build`) already
  resolve the project's own bin. For the global `slop`:
  - A pre-parser that never changes runs before any argument parsing. It reads only the
    command name (`check`, `dev`, `build`, `register`) and its directory argument, because
    another version's argument parser may differ.
  - It resolves the target project's copy with `Bun.resolveSync("hitslop/package.json",
    projectDir)`, which also finds hoisted workspace installs.
  - When that copy's real path differs from this one's, it re-runs that copy's
    `src/cli/cli.ts` with the same arguments. An environment marker set on the re-run, plus
    the real-path comparison, prevents recursion.
  - A project with no install is told to run `bun install`.
  - `runtimeABI()` in `build.ts` reads the package's own constant, and the "SDK newer than
    CLI" refusal goes away.
- **Refusals name the remedy for the copy that ran:** `bun add -d hitslop@latest` for a
  project's copy, `bun install -g hitslop@latest` for the global install.
- **Starter.** `templates/checklist/package.json` and `init.ts` write
  `devDependencies: { hitslop: <own version> }`. Svelte stays pinned to the package's
  version, as it is today.
- **Paths that move.** This is mechanical; about 150 files name the old packages:
  - `scripts/build/{generate,rust-contracts,swift-contracts,shell,packages}.ts`;
  - `crates/hitslop-core/build.rs` (the build ID covers the generated schemas), and the
    includes in `manifest.rs` and `envelope.rs`;
  - `scripts/build/embed-hitslop-native.sh` and `.github/actions/native-cache`;
  - the tier inputs in `scripts/verify.ts`, and `scripts/hygiene.ts`;
  - example and fixture imports: `@hitslop/document` becomes `hitslop`.
- **Crust.** `updates.ts` uses `packageName: "hitslop"`. The generated skill stays
  `hitslop-cli`.
- **Packing.** `scripts/build/packages.ts`, `scripts/release/bundle.ts`,
  `scripts/compat/{capture,integrity}.ts` and `tests/packed` work on one tarball.
- **Docs:** the package list in `AGENTS.md` Non-negotiable becomes `packages/hitslop`, with its
  `sdk` (the author SDK) and `shell` (the page shell, which holds no CRDT).

### 4. The engine lives only in the package

- **The app** ships its linked core and the rendering helper. It keeps their identity checks
  and drops the engine:
  - its build, `lipo`, signing and identity check in `scripts/build/embed-hitslop-native.sh`;
  - its architecture and identity checks in the release workflow;
  - the embedding line in `docs/guides/releasing.md`.

  `bun run build` no longer places an engine beside the Debug helper.
- **The CLI** (`packages/hitslop/src/cli/engine.ts`) has one lookup:
  - the development override, `HITSLOP_ENGINE`;
  - the engine bundled for this platform in the tarball (`engine/<platform>-<arch>/`);
  - the checkout's `target/release/slop-engine`.

  `findDocumentEngine` and the rule that an explicit helper selects its sibling engine go
  away.
- **The engine** finds the app's helper and bundled starters by one rule: `/Applications`,
  then `~/Applications`, or its helper override (today `HITSLOP_NATIVE_CLI`).
  - Inside the app, `crates/hitslop-core/src/file/places.rs` keeps finding starters beside
    the executable.
  - A refusal for a document newer than the engine says to update the CLI, and the CLI names
    the remedy for the copy that ran (step 3).
- **Tests:**
  - Tests that point `HITSLOP_NATIVE_CLI` at a missing path only to avoid an installed app no
    longer need to.
  - The CLI's `engine.test.ts` and `helper-discovery.native.test.ts` test the new rules.
  - Swift `HostSuite.cli(tool: "slop-engine")` and `scripts/compat/corpus.ts` use the
    checkout engine.
- **Bun is required.** The "edit without Node or Bun" route leaves:
  - the root `README.md` and the package README;
  - `docs/guides/cli.md` (the engine-request reference stays, as internal reference);
  - `apps/landing/src/content/docs/docs/guides/cli-workflows.mdx`;
  - `.agents/skills/hitslop-native/SKILL.md`;
  - the release check that edits with a system-only `PATH`.
- `docs/architecture.md`: the diagram, and "On a Mac, document commands prefer the app's
  bundled `slop-engine`".

### 5. One version, one tag

- **Prepare.**
  - `scripts/release/version.ts X.Y.Z` writes the version (see [One version](#one-version))
    and increments the build number. Hygiene refuses any disagreement.
  - Corpus capture stays a local step on the candidate before the tag.
- **One tag, `vX.Y.Z`,** replaces `macos-v*` and `cli-v*`. There are no CLI-only releases.
  `.github/workflows/macos-release.yml` becomes `release.yml`.
- **Mac users never get an update before the matching CLI is on npm.** The Sparkle feed is
  `releases/latest/download/appcast.xml` (`SUFeedURL` in `Info.plist`), so marking a GitHub
  Release as latest is what makes an update live. The jobs run in this order:
  1. **`engines`:** the Engines workflow, called as `workflow_call` on the tag commit, builds
     darwin-arm64, linux-x64 and linux-arm64. This replaces the manual dispatch and the
     `gh run download` calls.
  2. **`mac`:**
     - the existing gate;
     - packing, with the engines from job 1;
     - the corpus replayed with the exact darwin-arm64 engine from the final tarball, whose
       hash goes into the release record next to the candidate writer's;
     - signing, notarizing and verification;
     - the GitHub Release created with `--latest=false`, holding the DMG, ZIP, appcast,
       tarball, `SHA256SUMS` and the release record. Its versioned download URLs work, but
       Sparkle doesn't see it yet.
  3. **`npm`:** `npm publish hitslop-X.tgz --provenance --access public`, using npm trusted
     publishing (`id-token: write`, no token), in an `npm` environment. Giving that
     environment a required reviewer adds a one-click approval, if wanted. The npm and Node
     versions are pinned exactly. The review cites npm 11.5.1 and Node 22.14.0 as the
     minimums for trusted publishing; check the npm docs and the registry when implementing.
  4. **`promote`:** `gh release edit vX.Y.Z --latest`, only when X.Y.Z is above the current
     latest release. Sparkle goes live here.
  5. **`site`:** `wrangler deploy` of `apps/landing`, so hitslop.com describes what people
     can now install.
- **Release runs are serialized and resumable.**
  - One `concurrency` group covers every release run, without cancelling one in progress, so
    a delayed older run can't overtake a newer one.
  - The version check in `promote`, and the same check before `site`, keep an older run from
    replacing the latest release or the site.
  - Re-running reuses what's already published. An existing GitHub Release's assets must
    match the record's hashes. When npm already holds `hitslop@X`, the job skips if its
    integrity matches this tarball and fails if it doesn't. A run resumes at the first
    missing stage.
- **Engines stay inside the tarball:** `darwin-arm64`, `linux-x64` and `linux-arm64`, about
  11 MB compressed. Drop `darwin-x64`: the app requires Apple silicon, so an Intel Mac could
  build and edit but never open, export or register. This settles the roadmap item "Confirm
  the platforms the CLI's engine ships for".
- **Engine identity.**
  - Each engine records the tag commit it was built from.
  - Packing checks that commit, and checks the core build ID against the one the corpus
    recorded.
  - The build ID covers `hitslop-core`, the generated schemas and `Cargo.lock`, but not
    `crates/slop-engine`. The commit check is what covers it. That stays sound because the
    tag commit may differ from the corpus's candidate (`release.commit`) only by the corpus
    entry, which the tag workflow already enforces.
- **One platform table.** `enginePlatforms` in `scripts/build/core.ts` lists each platform's
  name, Rust target, runner and Cargo features. The Engines matrix comes from it, through a
  setup job that emits JSON for `fromJSON`, or a hygiene check that
  `.github/workflows/engines.yml` matches. So do the completeness checks in
  `scripts/release/bundle.ts` and `scripts/compat/integrity.ts`.
- **Linux smoke tests.** On both Linux architectures, the Engines workflow:
  - installs a tarball carrying the static musl `dist` binary it just built (SQLite bundled);
  - runs `slop init`, `build`, `create` and `get`.

  Packing for this must stage that target binary, not a fresh host build: on Linux
  `packPackages` stages `cargoOutput`, which is a glibc build.
- **Per-platform packages** (`@hitslop/engine-*` as `optionalDependencies`) are deferred. If
  the download size ever matters, they can be added without touching compatibility.
- **Docs:**
  - `docs/guides/releasing.md` is rewritten as four parts: prepare (version and capture),
    validate (`release:check`), tag, and watch (including how to resume a run). The CLI-only
    release section, the manual npm procedure and the manual engine download go.
  - `AGENTS.md` Compatibility gains: "One version for the app and `hitslop`; it never stands
    for compatibility."

### 6. Docs and skills

- **Three audiences, three homes**, as now:
  - hitslop.com is for people. It deploys with each release, so it always matches the latest
    version.
  - The skills inside the package are for agents, and are versioned with the installed tool.
  - `docs/` is for contributors.
- **Project skills use Crust's project scope.**
  - `init` runs `bun install` in the new project, then the project's own
    `slop skills install --all --scope project`.
  - Crust links `.agents/skills/<name>` relatively to `../../node_modules/hitslop/…`. It also
    links detected agents' own folders, such as `.claude/skills`, which today's copies never
    reach.
  - The links are committed, so a clone gets working guides after `bun install`.
  - If the install fails (offline, for example), `init` prints the two commands to run.
  - The copy, and the "portable copies… CLI and SDK versions may differ" text, come out of
    `init.ts`, the handoff in `agents.ts` and the `init` help body in `app.ts`.
- **Which copy manages which links** (`src/cli/cli.ts`, `paths.ts`):
  - the global install manages global links;
  - a project's own `node_modules/hitslop` manages that project's links;
  - a bunx cache is still refused.
- **One README** for the package: `packages/hitslop/README.md`, short, linking to the site.
  The three package READMEs are deleted.
- **Public docs:**
  - `cli-workflows.mdx`: `bunx hitslop` and `bun install -g hitslop` (step 4 already removes
    the app's `slop-engine` row);
  - the getting-started imports become `hitslop`;
  - the root README matches.
- **Contracts:** `docs/engineering-contract.md`, `docs/architecture.md`, `docs/testing.md`
  (the packed tier and the corpus), and the roadmap's opening line.

## What stays coupled

Each of these is inherent, and each has an update path. With one release train, a mismatch
only lasts until the other side updates.

| Mismatch | What the user sees |
|---|---|
| A slop or document newer than the app (rows 1–4) | **Update hitSlop…** |
| A document newer than the CLI's engine (rows 1–2) | Update the CLI: `bun install -g hitslop@latest`, or `bun add -d hitslop@latest` from a project |
| CLI and app protocols differ (row 5) | Update whichever is older. This affects documents open in the app, opening windows, exports and native artwork |
| A project pinned to a `hitslop` whose protocol the app no longer serves | `bun add -d hitslop@latest` in the project, which can mean updating its SDK code. This only affects `register` and native artwork from that project |

A project's SDK can no longer be newer than its CLI: they're the same package, and a global
`slop` runs the project's own copy.

All four file markers stay. They cover different boundaries, and merging them would tie
unrelated changes together. The layout also lives inside the Loro document, where future
syncing will need it.

## The promise

Every valid document written by a public release opens, renders, edits, saves and reopens on
the latest supported Mac app. Documents from before launch can be replaced. Downgrades, and
the continued availability of external services an app embeds, are outside the promise.
Moving an existing document onto a newer version of its template's app is a separate idea
([additive app upgrades](../docs/ideas.md#additive-app-upgrades)), not part of this plan.

## User steps

These are outward-facing, so they stay with the user:

- Configure npm trusted publishing for `hitslop`: this repository, `release.yml`, and the
  `npm` environment. If npm only lets a trusted publisher be set on an existing package,
  reserve the name with one manual publish first.
- Add the `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repository secrets for the site
  job.
- After `hitslop` is published, `npm deprecate` the three `@hitslop/*` packages with a
  message that names `bun install -g hitslop`.

## Order

1. Step 1, after the user signs off on the `AGENTS.md` change.
2. Step 2, before the first public release, and before the package change.
3. Step 3. It only relocates step 2's generated schemas and limits.
4. Step 4.
5. Steps 5 and 6. Step 6 can land with step 5.

Run `bun run verify` after each step, and `bun run verify --native` after steps 1, 2, 4
and 5.

## Verification

- **Old documents, offline:** native rendering, real page edits, edits through the current
  CLI, PNG/PDF export, and reopen, for every corpus entry. The replay passes unchanged across
  the package move.
- **Protocol mismatches** in both directions: the message names the side to update, the exit
  status is 2 or the reply is `requires_update`, and the document is byte-for-byte unchanged.
- **The permanent boundary:** the cross-version refusal tests in step 1 (protocol N+1 with an
  unknown method, a discovery record with extra fields, the helper's exit 2).
- **Freezing:**
  - Changing the current TypeBox manifest or an authoring limit leaves a released format's
    acceptance unchanged.
  - The check refuses changing or deleting a frozen format's schema or limits.
  - A test-only format that adds a required manifest field still opens a format-1 file, and
    Swift decodes the host-facing manifest it gets.
  - An old document stored at a released persistence limit opens, takes an edit, saves and
    reopens.
- **The one tarball**, installed outside the checkout in a path with spaces:
  - `init` installs, and links skills into `.agents/skills` and any detected agent folder;
    the links resolve;
  - `check`, `dev`, `build` and `register`;
  - `create`, `get`, `set` and export against the app;
  - the getting-started tutorial compiles against `import … from "hitslop"`;
  - `bunx hitslop skills install` is refused, with the instruction to install globally.
- **Delegation:**
  - With a global copy at version N and a project pinned to an N−1 tarball, a global
    `slop build ./project` run from outside the project uses the project's copy. Its stamp
    and `--version` output show it.
  - The same from a workspace whose `hitslop` install is hoisted.
  - A pinned project whose protocol differs from the app's gets
    `bun add -d hitslop@latest`, not the global command.
  - A copy never re-runs itself.
- **All three platforms:** building and closed-document edits with the installed package. On
  a Mac, live edits and exports against an app bundle with no `slop-engine`.
- `compat_writers.rs` passes with the corpus storing only the candidate writer.
- **Release dry runs**, each a manual run of `release.yml` without signing:
  - engines, the gate, packing and the shipped-engine corpus replay, then
    `npm publish --dry-run` and `wrangler deploy --dry-run`;
  - a run whose npm stage fails leaves the release unpromoted and the site unchanged, and a
    re-run resumes at npm;
  - promoting a version below the current latest is refused.
- **After the first real release:**
  - a fresh `bun install -g hitslop`;
  - a Sparkle update from 1.3.0, offered only after promotion;
  - `npm view hitslop` shows provenance;
  - hitslop.com shows the new install line.
- `bun run verify`, `bun run verify --native` and `bun run release:check` before tagging.
