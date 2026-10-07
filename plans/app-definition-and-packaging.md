# App definitions, resource packaging and document storage

Status: implementation authorized, 2026-10-06. The user approved updating this plan from the native-dev spike and beginning the refactor. This remains a plan, not a claim that the migration is complete. Accepted checkpoints are consolidated into `ready-ship`; further implementation uses the main checkout and its own build outputs. Experimental branches are preserved before their worktrees are retired. Linux verification is deferred until the end, at the user's request.

### Native development decision (2026-10-06)

Adopt a native Rust owner for `slop dev`, retaining Vite for the UI and HMR. The [spike report](../docs/evidence/native-dev-owner.md) records commands at 45–47 ms p95 on an M1, about 11 MiB RSS per owner, working HMR, real attachment persistence, undo/redo, isolated tabs and fenced shutdown. There is enough evidence to select this architecture. The spike is not merged wholesale: its callback argument, TypeBox preview frame and engine-local command coordinator are temporary scaffolding.

- One disposable SQLite document and native owner per preview page. Refresh resets it; component/CSS HMR retains accepted state. Definition or command changes fence the previous generation before resetting; a broken rebuild cannot leave stale executable commands available.
- Bun owns transport and child lifetime, not document semantics. Use the real page protocol and owner publication path. The command runner is the same restricted runner used by the app and CLI, with a fresh runtime for every evaluation.
- The page does not load a WASM document engine. WASM remains for focused core/SDK tests; the durable OPFS browser host remains a separate, deferred product plan.
- Development attachments use the native resource reader through session-scoped HTTP URLs, including ranges and integrity checks. Delete the production in-memory attachment implementation rather than adding a MIME-sniffing copy in JavaScript or WASM.
- Native definition resets cost 0.8–1.7 seconds median in the spike because it rebuilt and packed the whole old template. Keep UI HMR independent; reuse unchanged Vite artifacts when preparing a replacement preview. Do not add a third resolver to improve reset speed.
- Fresh-process commands meet the local 50 ms p95 gate. A warm process remains conditional on later measurements; a shared JavaScript realm is never allowed.
- Ship command stubs in the UI from the first release. Remove executable `run` bodies during the UI build and prove this at the build gate. The interim callback form cannot become ABI 1.

Implementation progress is recorded at the end of §10. Approval includes the planned AGENTS.md ownership changes; no second approval is required when a checkpoint switches a boundary from TypeBox to Rust.

## 1. Outcome and principles

An author declares an app in TypeScript. The build compiles its UI and commands, collects its resources, and asks Rust to validate and pack it. A `.slop` contains the complete built app and its Loro state. Opening a document requires neither the author’s source tree nor a configuration evaluator.

There are three distinct representations, each with a purpose:

| Representation | Purpose | Owner |
| --- | --- | --- |
| Author declaration | Components, command functions, typed document definition, initial values and metadata | TypeScript SDK and build tools |
| Validated app definition | Plain data describing the app, its resource references and document rules | Rust core |
| Document instance | Embedded app, Loro state, user attachments and current preview/icon artwork | Rust owner and storage worker |

Do not make the source directory layout into a package format. A Svelte component’s source filename is not a runtime contract. Likewise, an imported PNG becomes a resource, not a requirement to recreate a directory on disk.

The target removes:

- TypeBox, entirely: as the source of internal wires and app metadata, as the command-argument authoring API (`Type` re-export), and as a runtime validator. The `typebox` dependency leaves `packages/hitslop/package.json`.
- The `jsonschema` crate and every compiled JSON Schema validator in the core (`envelope.rs`, `manifest.rs`, `file/commands.rs`).
- The generic `manifest` JSON object passed between Rust, Swift and the catalog.
- Required `schema.ts`, `commands.ts`, `App.svelte`, `Export.svelte`, `Icon.svelte` and `styles.css` discovery rules.
- The required `app.json` staging artifact and reserved command metadata JSON asset (`__commands/metadata.json`, `__commands/run.js`), including the `__commands/` key-prefix filter in `AssetReader::row`.
- Duplicated resource-reading logic and the extension-based `content_type()` table in `file/assets.rs`; retain separate storage tables for their different lifecycles.
- Base64 attachment reads over the WebKit bridge (`shell/attachments.ts` `HostAttachments.read`).
- Page-side command evaluation (`OwnerDocument.runCommand` evaluating `run` in the page) and the engine-only evaluation path in `call.rs`. Every command runs through the owner in the restricted evaluator.
- Header-only PNG checks (`file/artwork.rs` `png()` reads 33 bytes); PNGs the core accepts are fully decoded under allocation caps.
- Handwritten Rust/Swift contract generators (`scripts/build/rust-contracts.ts`, `scripts/build/swift-contracts.ts`), quicktype, and Swift JSON decoding of internal messages and app metadata.

Keep the self-contained SQLite document, immutable embedded app, Rust writer ownership, Loro, native window behavior, catalog/Recents, captures, native helper, Analytics/Crashlytics and Sparkle. Keep templates immutable and create editable copies. Collaboration, hosted publishing, app upgrades within existing documents and document-schema evolution remain deferred.

The goal is fewer representations and ownership boundaries. JSON remains a useful encoding for the recursive definition and internal messages. Rust owns their meaning. Ordered definition lists remain arrays inside `definition_json`; they do not become independently versioned SQL tables. JSON Schema remains only an output projection for tool clients, with a test-only validator checking that projection.

## 2. What exists today

The current build evaluates `slop.ts`, extracts metadata into `app.json.manifest`, and writes the descriptor, initial values and theme beside it. Packing (`crates/hitslop-core/src/file/pack.rs`) stores the manifest, descriptor and theme as compacted text columns in the `app` row. Initial values are checked against the descriptor (`Node::validate`, `descriptor.rs:176`) and become the template’s Loro checkpoint (`Document::initial_checkpoint`, `lib.rs:380`).

The core validates the manifest against TypeBox-generated JSON Schema (`manifest.rs`, `manifest.schema.json` and the frozen-format `manifest-format-1.schema.json`), then normalizes it. Swift decodes the normalized JSON again through a quicktype model (`SlopManifest.generated.swift`), and the Rust catalog (`file/catalog.rs`) has another small manifest decoder. The redesign replaces those repeated conversions with typed access to one validated definition.

The current file (`file/storage-1.sql`) has seven STRICT tables: `app` (singleton with `package_format`, `runtime_abi`, `manifest`, `descriptor`, `theme` text), `assets` (`path`, `encoding`, `size`, `bytes`), `artwork` (`preview`/`icon` PNG), `document` (singleton discriminator), `checkpoint` (singleton), `updates` (`seq` rowid) and `attachments` (`id`, `bytes`).

Storage policies already in place and retained by this plan (`file/mod.rs`, `file/pack.rs`, `file/copy.rs`):

- `PRAGMA application_id = 0x48534C50` ("HSLP") and `PRAGMA user_version` (storage version 1), checked first on every open (`markers()`).
- Writers: `journal_mode=DELETE`, `synchronous=EXTRA`, `fullfsync=ON`, `secure_delete=FAST`. Every open: `trusted_schema=OFF`, `cell_size_check=ON`, `mmap_size=0`; readers add `query_only`.
- `auto_vacuum=FULL`, set by pack on the empty file.
- Exact layout check: `layout()` compares `sqlite_master` (type, name, tbl_name, sql, including automatic indexes) against an in-memory build of the expected schema, then checks singleton row counts. `PRAGMA quick_check` runs where integrity is required.
- Staged writes beside the destination, published with `renamex_np(RENAME_EXCL)` / `renameat2(RENAME_NOREPLACE)`; copies use SQLite’s online backup API (`file/copy.rs`).
- Identity-encoded asset ranges are read with `blob_open` + `read_at_exact` without loading the rest (`file/assets.rs` `AssetReader::read_range`). Brotli is applied only to text, JSON, SVG and WebAssembly when smaller, so media is never compressed; a compressed asset range decodes the whole asset and slices.
- Attachment reads re-verify the SHA-256 identity (`store.rs` `attachment`). Reclamation deletes unreferenced attachments at close; clean copies keep only referenced ones.

The current Vite build already emits a single application JavaScript module and a single stylesheet. WebKit already serves resources from SQLite through `slop://app/assets/...` (`SchemeHandler.swift`), including `206` byte-range and `416` responses for media. The visible page is a fixed HTML string that hardcodes `/assets/app.css` and `/__shell__/boot.js`. The native skin mask (`SlopWindowMask.swift`) already caches an 8-bit alpha buffer and maps points to pixels proportionally; the skin layer uses `contentsGravity = .resize`. Pages read user attachments by `attachments.read`, which returns base64 over the bridge and is decoded into a `Blob`.

The schema spike, preserved on branch `spike/schema` at `cce96d55`, is evidence for serde, ts-rs, UniFFI and `s.*` command arguments. It is not a complete implementation of this plan. Its report is `archive/plans/schema-authoring-spike.md` in that commit; the worktree is retired (see [consolidation evidence](../docs/evidence/worktree-consolidation-2026-10-06.md)). Integrate selected changes against the current working tree; do not merge the entire spike or copy its build outputs. Spike findings relevant here: hand-written serde types reproduced the engine wire exactly; ts-rs 12.0.1 output typechecked the repository; UniFFI remote types gave byte-identical helper replies and screenshots; specta 2.0.0-rc.25 and typeshare were rejected; `s.*` argument metadata matched TypeBox’s except for `minLength`.

## 3. Authoring API

### One explicit app entry

Keep `defineSlop` as the author entry point. `slop.ts` remains the default entry for convenience; an explicit `--entry` names another TypeScript module. The project directory remains the build root and dependency boundary. Other filenames have no special meaning.

The declaration references components, the document definition and commands directly. Introduce an explicit `slug` so moving or renaming a source directory does not change the app’s identity. Retain the existing slug grammar (`^[a-z0-9]+(?:-[a-z0-9]+)*$`, 2–64 characters). The starter supplies it once; the build does not infer or rewrite it from a directory name.

Proposed complete declaration:

```ts
import { defineSlop } from "hitslop";
import Checklist from "./views/Checklist.svelte";
import Paper from "./views/Paper.svelte";
import Icon from "./views/Icon.svelte";
import doc from "./model";
import { addTask, archiveFinished, restoreTask } from "./actions";
import "./theme.css";

export default defineSlop({
  slug: "quick-checklist",
  title: "Quick Checklist",
  description: "Capture, finish and file short task lists.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["productivity", "personal"],
  window: {
    kind: "standard",
    width: 480,
    height: 620,
    shape: "22px",
  },
  theme: { paper: "#fff9f3", ink: "#432830" },
  document: doc,
  initial: { title: "Little things, today", tasks: [] },
  view: Checklist,
  export: Paper,
  icon: Icon,
  commands: { addTask, archiveFinished, restoreTask },
});
```

`view` is required. `export`, `icon`, `commands` and `artwork` are optional. The export component keeps its `mode: "preview" | "export"` input. Command names come from keys in `commands`, rather than every export in a specially named module. Supporting modules may contain ordinary helpers without accidentally publishing commands. A command callable registered under two keys, or a command made from a different document definition than `document`, is refused at definition evaluation.

`window` replaces `presentation` in the source API. It is a discriminated union with `kind: "standard"` and `kind: "skin"`. Standard windows retain radius/path shapes, resizing, aspect locking and transparent/glass backgrounds. Defaults remain 22px corners, resizable, unlocked aspect ratio and the current standard background. Existing shape grammar and geometry limits remain Rust-owned. TypeScript models the union so that `resizable`, `lockAspect`, `shape` and `background` are type errors on a skin.

`theme` is an object literal in the source API for convenience; its property order is the declared token order and is preserved through storage (§9). Use plain imports for CSS. A separate CSS file is optional; component styles work normally. No automatic import of a file named `styles.css` remains.

### Document definition and commands

Keep `defineDocument` and the live document handles. Components import their document module; they do not import the app entry, which would create a component/declaration cycle. The module may have any name.

```ts
import { defineDocument, s } from "hitslop";

export default defineDocument({
  title: s.text(),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean(),
    archived: s.boolean(),
  })),
});
```

Commands retain synchronous `run`, typed `ctx.current`, collected `ctx.tx` writes, and host-supplied time/randomness. Page buttons import and await the same command callable an agent invokes by name. Where `run` executes changes: every call, from the page or from the CLI, is executed by the owner in the restricted evaluator (§8 “Command execution”). The page never runs a command body. The engine and the evaluator are the existing QuickJS runner; no new engine is introduced.

```ts
import { s } from "hitslop";
import doc from "./model";

export const addTask = doc.command({
  description: "Add an unfinished task.",
  args: { text: s.string({ minLength: 1 }) },
  run({ tx }, { text }) {
    if (!text.trim()) throw new Error("Enter a task.");
    return tx.fields.tasks.insert({
      text: text.trim(), done: false, archived: false,
    });
  },
});
```

`args` is a plain object of `s` nodes, `{}` for none. There is no `Type`, no `additionalProperties`, and no JSON Schema in author code.

### Command arguments are descriptors

The SDK turns `args` into an object descriptor, `{ "kind": "object", "properties": { ... } }`, in exactly the encoding document descriptors use. That descriptor is the stored, authoritative argument contract. It replaces the compiled JSON Schema of today’s `commandArgs()` (`packages/hitslop/src/schema/commands.ts`) and `argsSchema()` from the spike.

The argument subset, checked recursively in the SDK (at `doc.command`) and authoritatively in Rust (at pack and open):

| Node | In arguments | Value |
| --- | --- | --- |
| `s.string({ minLength?, maxLength?, description? })` | allowed | string |
| `s.number({ min?, max? })`, `s.integer({ min?, max? })` | allowed | finite number; safe integer |
| `s.boolean()` | allowed | boolean |
| `s.enum([...])` | allowed | one of the strings |
| `s.object({...})` | allowed, nested | closed object; unknown keys refused |
| `s.optional(inner)` | allowed when `inner` is a scalar or object | key may be absent |
| `s.list(scalar)` | allowed | array of the scalar |
| `s.text()` | refused: “use s.string() for command arguments” | |
| `s.counter()`, `s.record(...)`, `s.list(s.object(...))` | refused, including nested | |

Unions are deferred. Argument inputs are typed with `Input<ObjectNode<P>>`, without snapshot provenance or generated row IDs.

Every call is validated before the command body runs, by one validator in one place: the owner, using the core’s descriptor checker (`Node::validate`, `descriptor.rs:176`) against the stored argument descriptor. TypeScript types do not make page calls safe. They cannot express `minLength`, integer bounds or the range of a value typed into an input, so `s.integer({ min: 1, max: 10 })` happily receives `100` from well-typed code. Checking document state after the command does not enforce the command’s own preconditions either.

- **Agent and CLI calls.** `slop call` sends the owner a `call` request: the live owner over its socket, or an in-process owner under the writer lock. The owner parses and checks the arguments before starting the evaluator.
- **Page calls.** The SDK callable sends the page request `commands.run { name, args }` to the owner after draining the page’s pending edits, and resolves with the command’s result once the publication that contains its changes has reached the page store. Today’s `OwnerDocument.runCommand` (`packages/hitslop/src/shell/owner/document.ts:311`) evaluates the body in the page and then calls `apply`; that page-side evaluation is removed. `makeCommand` keeps the JSON round trip (`JSON.parse(JSON.stringify(args))`) so the owner receives the same plain data a CLI caller would send.
- **Refusals are identical on both paths:** `rejected/invalid_request` with the message shape callers see today, `Invalid arguments for addTask at /text: must be a string`. The evaluator never receives unchecked arguments, and the runner prelude (`packages/hitslop/src/shell/runner.ts`) no longer validates. There is no TypeScript copy of the descriptor rules.

Agents see arguments in the document’s vocabulary. `describe` lists each command’s arguments as fields (path, kind, description, bounds, optionality), produced by the visitor in `describe.rs`. For tool-calling clients it also includes a JSON Schema projection generated by Rust from the descriptor:

| Descriptor | JSON Schema projection |
| --- | --- |
| object | `{"type":"object","required":[...],"properties":{...},"additionalProperties":false}`; `required` omitted when empty |
| string | `{"type":"string","minLength"?,"maxLength"?}` |
| number / integer | `{"type":"number"/"integer","minimum"?,"maximum"?}`; integer bounds include the core's implicit safe-integer range |
| boolean | `{"type":"boolean"}` |
| enum | `{"type":"string","enum":[...]}` |
| list(scalar) | `{"type":"array","items":...,"maxItems":100000}`; includes the core's implicit list limit |
| optional(x) | projection of x, key absent from `required` |
| `description` on any node | `"description"` on its projection |

The projection is output only; nothing in the product validates against it. Byte equality with the TypeBox-era schemas is **not** a contract; the pre-launch TypeBox output was never released. The regression gate is semantic, a differential test (§11). For every argument descriptor in the bundled templates and fixtures, and for a corpus of values, the descriptor checker’s verdict must equal a JSON Schema validator’s verdict on the projection. The validator is a test-only dependency. The corpus covers:

- missing required keys and unknown keys, at the top level and nested;
- bounds at the limit and one past it, for strings, numbers and integers;
- non-integers for integers, and `1e300`, `-0` and safe-integer edges;
- every enum value plus one outside the set;
- scalar lists with a wrong-typed element;
- `null` where a value is required;
- supplementary-plane strings (`"😀"`) exactly at `minLength` and `maxLength`.

The last case pins code-point counting on both sides.

### String bounds

Add `minLength` to `s.string`, for documents and arguments alike. Both document and command string bounds count Unicode code points (`str::chars().count()` in Rust; `[...s].length` in the SDK where it displays a bound). They do not count UTF-16 code units, and not grapheme clusters:

- **Not UTF-16.** JSON Schema’s `minLength`/`maxLength` count characters (code points), and its conformance suite covers supplementary characters. A UTF-16 bound would make the descriptor and its projection disagree (`"😀"` is 2 code units but 1 code point).
- **Not graphemes.** Grapheme segmentation changes with Unicode versions and would make acceptance depend on the build’s tables.

Bounds are nonnegative safe integers; minimum cannot exceed maximum. This is an intentional pre-launch change from document strings’ UTF-16 counting (`descriptor.rs` `utf16_len`, `check.rs:15`). DOM selections, caret and text edit offsets, record-key limits (`encode_utf16().count() <= 256`) and diagnostic truncation keep their own existing UTF-16 units; those are positions, not field constraints. Authors should know that the HTML `maxlength` attribute counts UTF-16 code units, so it is not a substitute for the field’s bound; the authoring guide says so.

## 4. Build architecture and headless evaluation

### Two Vite builds, one declaration

An explicit entry importing Svelte cannot simply replace today’s `await import("slop.ts")`: Bun cannot evaluate uncompiled Svelte, and metadata extraction must not mount a UI. Instead of a custom resolver, the build runs **two Vite builds** over the same entry. Vite already resolves imports, rewrites CSS `url()`s (including those reached through `@import` and aliases) and emits assets, so that pipeline is the only resolver. The build adds a key function, boundary and AST transforms, and an evaluation step.

**Shared configuration** (one function produces both configs, so they cannot drift):

- `build.assetsInlineLimit: 0`, so every imported asset is emitted, never base64-inlined.
- `build.rolldownOptions.output.assetFileNames` is a function that returns `media/<sha256>.<ext>`. The hash is the SHA-256 of `assetInfo.source`, and the extension is the canonical one for the asset’s media type (§5). Keys are content-addressed, so the same bytes get the same key in both builds and across rebuilds.
- No code splitting (`codeSplitting: false` in the pinned Vite 8), no CSS splitting (`cssCodeSplit: false`), no module preload polyfill, and external boot dependencies refused.
- The same `resolve.alias`, tsconfig paths and Svelte preprocessing in both.
- The completed `build()` output records every emitted file: its file name, bytes and type (chunk, asset or CSS). A pre-plugin `generateBundle` hook is too early: Vite can emit CSS afterward. The completed output and its Vite dependency metadata are the authoritative inventory. Vite’s `build.manifest` may be enabled for debugging but is not the source of truth.

**Build 1, UI.** The entry is a virtual module that imports the author’s declaration and mounts its selected document, view, export, icon and command callables through the existing SDK adapter. The Svelte plugin compiles real components. It emits `ui.js` (one ES module), an optional `ui.css`, and every asset reached from components, scripts and stylesheets of all selected views (view, export and icon).

The UI transform projects `defineSlop` to `document`, `view`, `export`, `icon` and `commands`. Metadata-only expressions and artwork references do not belong in the UI declaration. The supported entry is a default call to the named `defineSlop` import with an object literal and explicit field names; field values may use imported values and ordinary computed expressions. Root spreads, computed field names and indirect entry calls fail with an author-facing error. This syntactic boundary makes projection reviewable without evaluating author code in the build process.

**Build 2, definition and commands.** The entry is a virtual module that imports the author’s declaration and exposes two functions on a global for the restricted evaluator:

- `describe()` returns the declaration as JSON;
- `run(name, input)` executes a registered command.

The temporary output is one IIFE file, `commands.js`; it is evaluated for every build, but stored only when the declaration registers at least one command. A pre-plugin supplies two substitutions:

- **Component tokens.** Every `.svelte` import, wherever it appears, loads as `export default Object.freeze({ "~hitslop": "component", id: "<module id>" })`. `defineSlop` recognizes tokens by shape, never by `typeof === "function"`. The Svelte plugin is not part of this build.
- **No styles.** Stylesheet imports resolve to virtual empty JavaScript modules, so Vite does not parse a JavaScript stub as CSS. Asset imports (`?url`, static `new URL(..., import.meta.url).href`, eager `import.meta.glob`) are still emitted. After Vite resolves a static asset URL, an AST transform keeps its emitted `/assets/` path in the headless chunk; there is no browser origin or URL polyfill in QuickJS. The skin and artwork therefore appear in the definition output without a second resolver.

This build fails in `resolveId`, naming the importer chain, on any import of the Svelte runtime (`svelte`, `svelte/*`), of the SDK’s page-only entry points (`hitslop/svelte` and the shell), or of the author’s entry from any module other than the generated virtual entry. That last rule makes the entry/command cycle an error.

**Definition evaluation and the headless proof.** After build 2, the build loads `commands.js` once in the restricted evaluator. That is the same helper, sandbox, prelude and limits that execute commands for the owner (§8). It calls `describe()`, and no command body runs. The result is:

- metadata;
- the window, with the skin as its `/assets/<key>` URL;
- the ordered theme;
- the descriptor and initial values;
- the commands’ names, descriptions and argument descriptors;
- which roles are present (view, export, icon);
- the supplied artwork URLs.

This single run is also the proof that the declaration and every command dependency initialize headless. A module that touches `window`, `document` or another host global at initialization fails the build here, with the evaluator’s error. The prelude refuses ambient time and randomness, so declarations, including initial values, are deterministic. There is no separate Bun child, and no list of forbidden packages beyond the `resolveId` rules.

The UI build transforms each `doc.command({ description, args, run })` declaration into a callable stub and removes its executable `run` body before bundling. Registration still comes from the declaration's `commands` keys. Use an AST transform with a source map and fail clearly on unsupported command declaration syntax; never a regex that can misidentify JavaScript. The command build retains the original body. Tree shaking then removes dependencies used only by that body. Gate this with a recognizable body-only sentinel absent from `ui.js` and present in `commands.js`, and an actual stub call through a host context. No callback executor ships in ABI 1.

### Resource assembly and BuildInput

`BuildInput` is assembled after both builds and the evaluation. Its TypeScript type is generated from the Rust type with ts-rs:

```text
BuildInput
  packageFormat / runtimeABI   checked before interpreting declaration or resources
  declaration   from describe(): metadata, window, ordered theme, descriptor, initial,
                commands [{ name, description, args }]
  roles         { ui: key, style?: key, commands?: key, skin?: key }
  resources     [{ kind: "app" | "command", key, mediaType, path }]
  artwork       { preview?: path, icon?: path }
```

Assembly rules:

1. Every asset emitted by build 1 is an `app` resource, except an artwork import left over after tree shaking whose key is absent from Vite’s rendered `importedAssets` metadata. The fixture also checks the converse: an artwork image used by CSS remains an app resource. This ownership exception uses dependency metadata, never the supplementary URL scan. `ui.js` and `ui.css` are `app` resources under their fixed keys, and `commands.js` is a private asset only when commands exist. Its absence otherwise is intentional; the temporary definition bundle is not packaged.
2. Assets emitted by build 2 come from imports reached by the declaration and command code. Each is an `app` resource, **except** an image referenced only as `artwork.preview`/`artwork.icon`. That becomes an initial artwork row and is not an app resource.
3. Entries with equal keys are one resource. Equal keys mean equal bytes by construction.
4. Every URL in the declaration (skin, artwork) must name an asset build 2 emitted; otherwise the build fails, naming the declaration field.
5. **Supplementary check:** scan `ui.js`, `ui.css` and `commands.js` for `/assets/<key>` strings and fail on any that names no emitted asset. This is not dependency discovery. Strings can be escaped, computed or unrelated to resource loads, so an emitted asset whose URL the scan does not find is not an error. Emitted bundle metadata and explicit declaration references are the sources of truth.

`slop dev` uses the same two configurations with Vite’s dev server, so source URLs replace package URLs and HMR applies (§10).

### Feasibility gate (implementation step 1)

This is the riskiest unknown in the plan, so it comes first. Prove it on a fixture containing:

- component CSS;
- a font reached only through a CSS `@import`;
- an aliased (`$lib`) CSS `url()`;
- an imported PNG skin that no component references;
- an export view with its own image;
- a supplied preview image used only as artwork;
- a command;
- a shared document module.

The gate passes when all of these hold:

- build 2 replaces every component with a token, `describe()` returns the complete declaration in the restricted evaluator, and the skin URL survives;
- no component code runs during definition evaluation;
- build 1 renders the view, and its emitted assets include the `@import`-only font and the aliased image;
- a shared image gets the same key from both builds and from a rebuild;
- assembly routes the artwork-only image to an artwork row and nothing else;
- the UI bundle size is recorded, command bodies are absent, and the generated command stub sends the registered name and arguments through the host context.

Then break each part. Each must fail with the documented error:

- delete the skin file (Vite’s unresolved import, with source location);
- import `svelte` from a command module (`resolveId` chain);
- import the entry from a command module (cycle rule);
- touch `document` at a command module’s top level (evaluator initialization error);
- reference an `/assets/` key that does not exist (supplementary check).

Treat failure of this gate as a reason to revise the build design, not to silently restore filename discovery or add a second app declaration.

Use ordinary bundler resolution/load hooks rather than parsing TypeScript source to rediscover its types or introducing a configuration language. Metadata checks happen in Rust after evaluation. App declarations and command dependencies must be safe to initialize in a headless environment: UI effects belong in components and lifecycle callbacks; pure TypeScript helpers, imports and composition remain available. This preserves an acyclic dependency graph.

### Packing without an app.json artifact

The build produces the `BuildInput` above: the evaluated app declaration, initial values and a bounded list of compiled resource entries. Each resource entry names a logical key, resource kind, media type and a path within a private staging directory. Large binary data stays out of JSON stdin.

The engine’s `pack` request carries the build input plus staging root and destination. Rust bounds the request, checks compatibility requirements first, validates the typed app and initial state, then opens only listed regular resource files beneath the staging root (`symlink_metadata`, regular files only, sizes checked against budgets before reading, as `stage_assets` does today). The Rust packer owns the resource bytes it validates and writes; it never re-reads a different source after validation.

Temporary JavaScript, CSS and binary output files are acceptable build artifacts. There is no required `app.json`, author-supplied HTML entry, directory copy into the document, or runtime extraction directory. Rust creates the SQLite file and publishes it only after the complete app and checkpoint pass validation and a full `check(…, integrity = true)` of the staged file.

`slop check` uses the same native acceptance functions without publishing a template. `slop dev` serves validated preview configuration from memory at a host endpoint rather than reading staged `app.json`. Pack must still validate its actual input; an earlier successful check is not a trusted capability to bypass validation.

Retain raw nested build-input JSON until the engine has checked its compatibility requirements. The existing future-marker/`1e999` case must continue to produce `requires_update` before parsing a future payload. Normal app values have semantic equality; preserving incidental source JSON whitespace is not a storage requirement. Rust writes `definition_json` by serializing the validated typed definition, so the `compact()` text helper in `pack.rs` goes away. Command arguments use `serde_json::Value`, since their existing execution path already parses and reserializes them.

## 5. Images, fonts, CSS and other resources

### Import resources through the build graph

```svelte
<script lang="ts">
  import photograph from './images/cover.webp';
</script>

<img src={photograph} alt="A notebook on a desk" />

<style>
  .paper { background-image: url('./images/paper.png'); }
  @font-face {
    font-family: 'Notebook';
    src: url('./fonts/notebook.woff2') format('woff2');
  }
</style>
```

The same applies to images imported by export/icon components, audio, video and data loaded with an explicit asset URL. The build must collect resources reachable from all selected views, not just the initial editor view. The UI build compiles all of them, and the declaration’s own references come from the definition build (§4).

The shared Vite key function (`assetFileNames`, §4) gives imported binary/data assets stable logical keys of the form `media/<sha256>.<canonical-extension>`. Hash the unencoded bytes. The canonical extension comes from the media type, not the source filename: `png`, `jpg`, `webp`, `gif`, `svg`, `woff2`, `woff`, `ttf`, `otf`, `mp3`, `wav`, `ogg`, `mp4`, `webm`, `json`, `wasm`. Media elements and font loaders still sniff URLs, so the extension stays in the URL. The media type is stored in the row; the core never derives it from the key. Reusing the same bytes and media type uses the same key. Identical bytes with different media types are distinct resources with distinct keys. Do not deduplicate across app, command, attachment or artwork ownership kinds.

Entry code uses fixed non-hashed keys: `ui.js`, optional `ui.css` and optional private `commands.js`. These are format-1 conventions, not app-row pointer columns. The page asset route never serves `commands.js`.

Rust owns the extension/media-type/canonical-extension registry and exports its build-time projection to TypeScript. Pack checks recognized binary app formats against the shared signature recognizers. This does not apply the passive attachment allowlist to app assets: JavaScript, CSS, JSON, SVG and fonts retain their app-specific rules. Attachment imports ignore author-supplied MIME claims and use the passive allowlist (§7); unknown or active attachment formats remain `application/octet-stream`.

The production URL is `/assets/<key>`. It is document-local: WebKit resolves it beneath `slop://app`, and the owning resource reader reads that document’s SQLite database. Never serialize an author’s absolute path, a temporary-directory path, or `file://` URL into the package.

Set Vite asset inlining to zero (`build.assetsInlineLimit: 0`) for the supported asset-import pipeline. Use the normal application build rather than library mode, whose asset-inlining rules differ. Keep code splitting disabled, CSS splitting disabled and external boot dependencies refused. The output consists of one UI module, an optional stylesheet, an optional command program and collected resources. A missing stylesheet needs no artificial empty file.

Do not automatically copy an entire `assets/` or `public/` directory. Refuse a nonempty project `public/` directory with instructions to import those assets explicitly; silently ignoring it would produce misleading builds. Static imports, CSS URLs and statically analyzable `new URL(..., import.meta.url)` identify resources. For dynamic collections, support Vite’s eager asset globs, for example:

```ts
const pictures = import.meta.glob('./pictures/*.png', {
  eager: true,
  query: '?url',
  import: 'default',
});
```

Code selects among the resulting URLs. A runtime expression such as `'/assets/' + filename` cannot discover source files at build time and is not a packaging mechanism. An unreferenced local image is omitted. An unresolved imported resource fails the build with its source location; a missing runtime URL fails as a resource request rather than falling back to the author’s disk.

Literal resource paths in markup must be converted to imports or explicit URL imports when they need bundling. Preserve external HTTPS data/media requests as external requests under the current policy; do not silently download them or promise offline availability. Boot scripts, required fonts and styles remain packaged locally. Generated inline SVG markup remains ordinary component output.

### Compression and serving

Keep text compression when it makes the stored representation smaller. Compress JavaScript, CSS, JSON, SVG, WebAssembly and other text resources with the existing Brotli policy: quality 10, or 9 above 4 MiB; `lgwin` 22; stored only if strictly smaller. PNG, JPEG, WebP, GIF, WOFF/WOFF2, audio and video are always identity-encoded. Rust enforces that media-type rule at packing and acceptance; SQL enforces the stored/decoded size relationship (§9). Only identity rows are range-served directly from storage, and a Brotli row is decoded whole and sliced. Store both decoded length and storage encoding; enforce bounds before allocation/decompression (the decoder reads at most `size + 1` bytes).

The page shell creates the HTML document. Today `SchemeHandler.visiblePage` hardcodes `<link href="/assets/app.css">`. Instead, the shell’s boot reads the fixed UI key and optional stylesheet presence from the page configuration and inserts the stylesheet link and module import itself, so a missing stylesheet means no link rather than a 404. Authors do not ship an `index.html` just to load their app. Keep `/__shell__/` host-owned and separate from document resources.

The resource reader supports full reads and ranges. Preserve media types, `206` responses, `Content-Range`, unsatisfiable `416` responses, task cancellation and media playback. Identity-encoded media ranges read just the requested bytes from SQLite through `blob_open`. Do not base64 resources into JavaScript or manufacture long-lived object URLs for immutable app assets.

Browser development uses Vite’s normal source URLs and HMR, with the same logical asset references available to the preview configuration. Source URLs may differ from final package URLs. Vite owns this translation; authors do not branch on native versus browser mode.

### Script sources (content security policy)

Today’s native policy (`packages/hitslop/src/schema/policy.ts`) allows scripts from the whole `slop:` scheme:

```text
default-src 'none'; script-src slop: 'wasm-unsafe-eval'; connect-src slop: https: blob:;
media-src slop: https: blob:; frame-src https:; style-src slop: 'unsafe-inline';
img-src slop: data: https: blob:; font-src slop: data:
```

Once user attachments are served from `slop://app/attachments/`, that policy would let attachment bytes load as scripts or modules. Scripts must come only from locations the app or host owns. The policy moves to a Rust constant, exported to the page shell and to Swift instead of generated from TypeBox, and becomes, for native pages:

```text
default-src 'none';
script-src slop://app/__shell__/ slop://app/assets/ 'wasm-unsafe-eval';
worker-src slop://app/__shell__/ slop://app/assets/;
connect-src slop: https: blob:;
media-src slop: https: blob:;
frame-src https:;
style-src slop://app/assets/ slop://app/__shell__/ 'unsafe-inline';
img-src slop: data: https: blob:;
font-src slop://app/assets/ data:
```

- **Path-scoped sources.** `slop://app/assets/` admits only app-kind resources: the author’s UI module and anything else the author packaged, which is the app itself. Attachment URLs never match a script, worker, style or font source.
- **Passive kinds stay open.** Images, media and `fetch` may use attachments by URL; that is the point of §7.
- **Browser development is unchanged** (`'self'` plus the HMR socket), because the dev server serves no attachments from the app origin.
- **WebKit support must be proven.** CSP3 host-source paths with a custom scheme (`slop://app/assets/`) need a native test before step 4 relies on them. If WebKit does not match paths for custom schemes, serve attachments from a separate host (`slop://attachments/<id>`), so that `script-src slop://app` excludes them by host. The fallback must also pass attachment image/canvas export tests with host-managed CORS; authors must not configure CORS themselves.

## 6. PNG skins and native window geometry

### Declare a skin with an imported image

```ts
import { defineSlop } from 'hitslop';
import skin from './art/recorder.png?url';
import Recorder from './Recorder.svelte';
import doc from './model';

export default defineSlop({
  slug: 'pocket-recorder',
  title: 'Pocket Recorder',
  description: 'A small recorder-shaped document.',
  author: { name: 'Example Author' },
  categories: ['utilities'],
  window: { kind: 'skin', width: 480, height: 640, image: skin },
  theme: { ink: '#24201b', accent: '#ce5037' },
  document: doc,
  initial: { title: 'Untitled' },
  view: Recorder,
});
```

The image need not live under a specially named `assets/` directory. The metadata evaluator records its resource reference even if no component renders that image. Packing includes it because the native window needs it. The normalized Rust definition stores a resource key, never the source path or an arbitrary remote URL.

### Resolution: 1× or 2×

A skin PNG must be exactly 1× or exactly 2× its declared point size: `(pw, ph) == (width, height)` or `(pw, ph) == (2·width, 2·height)`. The scale is inferred from the pixel dimensions; there is no `scale` field and no `@2x` filename convention. Any other size is refused at definition evaluation (as a source diagnostic) and by Rust at pack and open, with a message that names both accepted sizes. A 2× skin is the recommended form: at 1×, macOS draws every skin pixel as a 2×2 block on Retina displays, which looks soft.

This is decided now because the acceptance rule freezes with `packageFormat` 1 at launch.

Budgets: the existing image limits apply to the pixel size (at most 16,384 pixels per side and 24,000,000 pixels; `AssetLimits.imageSide`/`imagePixels`), as does the 25 MiB per-asset limit. A 2× skin is therefore limited to about 6,000,000 square points (for example 2,449 × 2,449 pt). The window point bounds (240 × 180 to 4,096 per side) are unchanged.

### Validation

At packing and full acceptance (§9), Rust checks that the referenced resource is an app-kind row with media type `image/png`. It then proves the PNG decodes, using the `png` crate (0.18.1, promoted from a dev-dependency to a pinned normal dependency of the `storage` feature), **row by row and never into a full frame**:

1. Use `Decoder::new_with_options` over a `std::io::Cursor`, explicitly set `ignore_adler32 = false` and `skip_ancillary_crc_failures = false`, then `set_limits(Limits { bytes: 8 MiB })`. Source inspection of pinned png 0.18.1 found both defaults otherwise tolerate checksum failures. Here `png::Limits` bounds only the decoder’s own allocations: inflate state, the previous row and ancillary chunks. It does not bound a caller-owned output buffer, so this plan never allocates one. Before decoding, inspect bounded chunk framing to refuse any `acTL`, `fcTL` or `fdAT`: png intentionally ignores some malformed/late animation-control chunks, so `info().animation_control` alone does not enforce the still-image rule. The library still owns checksums, compression, filtering and pixels.
2. `read_info()`. Then, from the header and before decoding any pixel data:
   - refuse APNG: `info().animation_control` is present. Skins and artwork are still images, and the extra frames would otherwise go unchecked;
   - check colour type and bit depth (RGBA, 8 or 16 bits per channel, for skins);
   - check `width ≤ 16,384`, `height ≤ 16,384` and `width × height ≤ 24,000,000` (`AssetLimits.imageSide`/`imagePixels`) with `checked_mul`;
   - check the 1×/2× rule;
   - compute the output row length `width × channels × bytes_per_sample` with checked arithmetic. A 16-bit RGBA pixel is 8 bytes, so the longest permitted row is 16,384 × 8 = 128 KiB.
3. Decode with `Reader::next_row()` until it returns `None`, holding at most the one row the decoder lends. Do not count rows as a height check. For an interlaced (Adam7) image, `next_row` yields the rows of each of the seven passes, so the count differs from `height`; the decoder itself validates the pass structure. Interlaced PNGs are accepted.
4. Call `Reader::finish()` (png 0.18.1, `decoder/mod.rs:590`). It reads the remaining chunks through `IEND`, verifying their structure and CRCs. Stopping at `next_row() == None` would leave the trailing chunks unchecked.
5. Require the input to be fully consumed: after `finish()`, the cursor position must equal the stored length. `finish()` stops at `IEND` and does not reject trailing bytes, so this check is the plan’s own.

A truncated stream, bad CRC, bad filter, missing `IEND`, an animation chunk or any byte after `IEND` is refused.

Peak memory is the 8 MiB decoder bound plus one row, whatever the image size or bit depth. The validator never needs the pixels; the host decodes the image for display (ImageIO), and the alpha map is built there (§ Native behavior).

Supplied and captured artwork (§7) use the same streaming check: RGBA or RGB at 8 or 16 bits, the existing per-image budgets, and exact dimensions where they are fixed (the icon’s 512 × 512 surface). The check runs at pack and in `Store::set_artwork` before a capture is stored. The header-only `png()` helper in `file/artwork.rs` is deleted.

Open-time cost stays bounded. Full acceptance streams the skin once (the host decodes it anyway to draw it). Artwork already in a file is not re-decoded on every read, because it was checked when written. The catalog summary (§9) never decodes.

### Native behavior

A skin remains both native backing artwork and alpha-mask input. Swift receives the typed window definition (`WindowDefinition::Skin { width, height, skin }`) and the validated PNG bytes through the core, decodes the image, and uses the existing native mask implementation. It does not decode a manifest JSON object or parse a resource URL to locate a file.

At 2× little changes natively. The skin layer already draws with `contentsGravity = .resize` into the window’s point bounds, and the alpha map (`SlopWindowMask.AlphaMap`) maps a point to `x = point.x / bounds.width · image.width`, so a 2× image hit-tests at pixel precision with no code change. Set `layer.contentsScale` to `image.width / width` so Core Animation treats the image as native-resolution content on Retina and at 1× on non-Retina displays. Keep linear filtering. Nearest-neighbor filtering is not used.

Preserve these behaviors:

- Alpha 0–25 is click-through; alpha 26–255 receives pointer input (unchanged threshold, evaluated on the image’s own pixels).
- The PNG is drawn behind the WebView. Ordinary page backgrounds stay transparent in skin mode.
- The native host owns window dragging through the toolbar handle.
- Controls and focus rings must fit inside the usable silhouette. A CSS `pointer-events` change is not proof of native click-through.
- The page receives the existing presentation data attributes and initial dimension CSS variables; the internal `presentation` vocabulary there need not match the renamed author `window` property.
- Standard radius/path silhouettes retain the existing Rust shape parser (now on `svgtypes`) and native clipping/hit-testing pipeline, including SVG arcs. The archived rejection of `svgtypes` is superseded; do not resurrect it.

The UI should not render the skin image a second time merely to reproduce its native background. It may import the same image when a dedicated export needs it; the common resource key avoids a second app-resource copy. Skin resources are immutable parts of the embedded app. Updating a user attachment or refreshing artwork cannot replace a skin.

`slop dev` renders the selected skin behind the preview with a transparent page at the declared point size (an `<img>` at `width × height` CSS pixels, so a 2× image is downsampled by the browser). Browser preview demonstrates alignment and appearance; only a native test proves that a transparent hole passes input to an application behind the window.

## 7. Export views, artwork and user attachments

The declaration’s `export` and `icon` roles replace discovery of specially named components. All their dependencies are bundled even though the components mount only during capture. Their components read the same document definition as the editor.

Keep saved-state capture: flush the live document, acquire an independent saved copy (`Store::capture_source`), then render in a fresh hidden page. No source project, authoring evaluator or live DOM is needed. A failing capture must not replace the editor or corrupt existing artwork.

Preserve the distinctions below:

| Image use | Storage/lifetime | Rendering behavior |
| --- | --- | --- |
| Template photograph, texture or font | Immutable app resource | Loaded by URL from the embedded app (`/assets/<key>`) |
| PNG window skin | Immutable app resource referenced by the window definition | Native backing and hit mask |
| User-imported image or media | Immutable attachment bytes managed by the owner | Loaded by URL (`/attachments/<id>`); reference stored in document state |
| Preview and icon | Derived artwork for the template/document | Refreshed by native capture; readable without running the app |

Dedicated export views do not inherit native masks or skin backing. An export that should contain skin artwork must explicitly render it. Window-sized fallback PNG captures retain the native mask. Longer full-content exports and PDFs retain their current unmasked behavior. Glass blur is a native effect and is not reproduced in an export; dedicated views should paint an appropriate surface.

Preserve 2× PNG export, the current image dimension/pixel budgets, selectable PDF text, and the icon’s transparent 512×512 capture surface. Await fonts, visible image decoding and stable geometry. Authors use `capture.onPrepare` for asynchronous charts, CSS-background resources or other content not covered by visible-image readiness. Do not promise that loading the UI module alone means every image has decoded.

Replace filename discovery for supplied artwork with optional explicit `artwork: { preview, icon }` imported PNG references in the app declaration. These are build inputs for initial artwork rows, not additional immutable artwork copies in the app. If an artwork image is also referenced by UI/skin, retain that separate app-resource use. Preserve native generation when supplied artwork is absent (`--artwork native`). Later document captures may replace artwork, while the embedded app remains unchanged.

### User attachments by URL

User attachments keep content-derived IDs (lowercase hex SHA-256), `attachments.import`, owner-controlled import plus reference insertion as one operation, reclamation at close, and per-document limits (10 MiB each, 256 files, 100 MiB). What changes is how a page reads them.

- **Route.** `slop://app/attachments/<id>` is served by the same scheme handler and resource reader as app assets, scoped to attachment rows. The route validates the ID grammar (64 lowercase hex characters) before any query, and refuses every other kind. App rows are reachable only under `/assets/`, attachments only under `/attachments/`; artwork and the command program are never page-fetchable.
- **Ranges.** Attachments are always identity-encoded, so every range is a `blob_open` read of the requested span: `206`, `Content-Range`, `416` and cancellation, exactly as for app media. A 10 MiB recording no longer crosses the bridge as a 13.3 MiB base64 string.
- **Integrity.** Today every whole read re-verifies the SHA-256 (`store.rs` `attachment`). A range read cannot. The reader verifies the full hash of an attachment the first time any request touches it during the reader’s lifetime (one pass over at most 10 MiB), caches the verified ID, and serves ranges afterwards. A mismatch fails the request with the existing damage message and is never served.
- **Media type, sniffed by Rust.** The served type comes from the bytes, never from the importer: `File.type` can be empty, wrong or hostile. At import, `Store::put_attachment` matches the leading bytes against a fixed table of **passive** formats and stores the matching type in the row. Anything that matches no entry is stored as `application/octet-stream`. Because the type is a function of the bytes, identical bytes always get the same type, and deduplication by hash stays correct. The table is a few dozen lines in the core, with no crate:

  | Signature | Stored media type |
  | --- | --- |
  | `89 50 4E 47 0D 0A 1A 0A` | `image/png` |
  | `FF D8 FF` | `image/jpeg` |
  | `GIF87a` / `GIF89a` | `image/gif` |
  | `RIFF....WEBP` | `image/webp` |
  | `....ftypavif` / `ftypavis` | `image/avif` |
  | `....ftypheic` / `ftypheix` / `ftypmif1` | `image/heic` |
  | `ID3` or an MPEG audio frame sync (`FF Ex`/`FF Fx`) | `audio/mpeg` |
  | `....ftypM4A ` | `audio/mp4` |
  | `....ftypqt  ` | `video/quicktime` |
  | other `....ftyp` brands (`isom`, `mp41`, `mp42`, `avc1`, …) | `video/mp4` |
  | `1A 45 DF A3` (EBML; WebM doctype) | `video/webm` |
  | `RIFF....WAVE` | `audio/wav` |
  | `OggS` | `audio/ogg` |
  | `fLaC` | `audio/flac` |
  | `%PDF-` | `application/pdf` |

  SVG, HTML, XML, JavaScript, CSS and text are deliberately absent: they are active or sniffable content, and are served as `application/octet-stream`. Pages that need such a file’s contents read it with `attachments.read` (a `Blob`) and handle it themselves. References keep their own `name` and `mimeType` for display and downloads; the URL serves only the sniffed type. `attachments.put` takes no media-type field.
- **Inert responses.** Every `/attachments/` response carries `X-Content-Type-Options: nosniff` and `Content-Security-Policy: sandbox`, so if one is ever loaded as a document it cannot run script or reach the bridge. The script sources in §5 never match `/attachments/`, so an attachment cannot load as a script, module, worker, stylesheet or font. `frame-src https:` already prevents framing `slop:` URLs. The host’s navigation delegate refuses any top-level navigation of the app’s web view away from `slop://app/`, which covers `/attachments/` URLs. A native test must confirm that WebKit honors `nosniff` and the `sandbox` CSP header on `WKURLSchemeHandler` responses. If it does not, the navigation and framing refusals plus the passive-type table still hold, and the plan records that.
- **SDK.** `attachments.url(id)` returns `/attachments/<id>` natively. `attachments.read(id, { type })` keeps its `Blob` result but is implemented as `fetch(url)` rather than a base64 bridge call; `type` overrides the `Blob` type as today. In browser development, the native owner serves the bytes through a session-scoped HTTP attachment route backed by the same Rust resource reader, integrity check and media-type sniffing. There is no production `MemoryAttachments` store. Authors never branch on host.
- **Bridge and socket.** Remove `attachments.read` from the page request vocabulary; the native host no longer answers it. Keep `attachments.put` over the bridge (base64, at most 10 MiB, bytes only) for this migration; a streaming upload is deferred. The CLI/agent socket keeps `attachments.read` and `attachments.list` with base64 JSON results, because agents need bytes in JSON; `attachments.list` also reports each stored media type.
- **Capture.** Hidden capture pages read attachments from the capture source copy through the same route and policy, so exports that show user images keep working without the live document.

The resource reader serves these explicit roles; authored code receives no general writable filesystem.

## 8. Rust types and acceptance

Introduce a typed `AppDefinition` containing:

- metadata;
- `WindowDefinition`: a Rust enum with associated data, `Standard { width, height, resizable, lock_aspect, background, shape }` and `Skin { width, height, skin: ResourceKey }`;
- the document descriptor;
- ordered theme defaults;
- command definitions (name, description, argument descriptor);
- entry-resource references.

Separate untrusted `BuildInput` from the validated definition used by the core and host. Constructors/checks establish invariants before the definition reaches the owner or renderer. Swift receives `WindowDefinition` as a UniFFI enum (remote type), so a skin that is also resizable is unrepresentable on every side.

**Stored form.** `AppDefinition` is decoded from two places in the `app` row (§9):

- the scalar columns: permanent markers and catalog metadata;
- one bounded `definition_json` value that holds everything else.

`definition_json` is written by Rust from the validated typed value, and decoded with `deny_unknown_fields` by the acceptance module of the file’s package format:

```text
definition_json (package format 1)
  window     { "kind": "standard", "width", "height", "resizable", "lockAspect",
               "background", "shape" }
           | { "kind": "skin", "width", "height", "skin" } -- stored resource key, not an author URL
  theme      [ { "token", "color" }, ... ]              -- array: declared order is kept
  commands   [ { "name", "description", "args" }, ... ] -- args is an args-subset descriptor
  views      { "export": bool, "icon": bool }           -- which optional views the UI mounts
  document   the document descriptor
```

Ordered data is always a JSON array. Objects are fixed typed structs whose field order carries no meaning. The workspace `serde_json` does not enable `preserve_order`, and this design does not need it. No scalar stored in a column is repeated inside `definition_json`. Rust resolves the skin key from the window variant, requires `ui.js`, and checks that `commands.js` exists exactly when the typed command list is nonempty. No role pointer is duplicated in a column. The host reads `views` to choose dedicated export/icon capture versus the window fallback (§7) without loading the UI module.

Because every field of `definition_json` is interpretation rather than layout, a new interpreted field (a window option, a theme feature, a capability vocabulary once designed) is a package-format change with no storage migration. Columns change only with the storage version (§9 “Ownership of the layout”).

Rust owns fixed app metadata and all internal wire types. Use serde decoding (`deny_unknown_fields` throughout; internally tagged enums for `{ "method": ... }` requests) plus explicit checks for lengths, ranges, patterns, enum relationships and resource references. Generate TypeScript using exactly pinned ts-rs 12.0.1 (features `serde-json-impl`, `no-serde-warnings`; behind a `ts` feature that the WASM build never enables). Use the existing exactly pinned UniFFI 0.32.2 and its remote-type pattern for records/enums Swift consumes. Specta is excluded based on the spike’s tested versions, and typeshare because it cannot express internally tagged data enums. Do not emit JSON Schema from Rust types.

`RawValue` cannot sit inside an internally tagged enum. Where opaque bytes must be retained (the build input before marker checks), parse a `{ method }` or marker header first and then the per-variant body, as `wire.generated.rs` does today, with a small `macro_rules!` instead of generated code.

`defineSlop` is a typed TypeScript helper. Its scalar metadata/window types come from Rust; its generic document/initial relationship and component/command function types stay in the SDK. Those author-only function/component values never cross the engine wire. The exported TypeScript interface and the persisted SQL row are different representations of the same domain model, not competing authorities.

Use the validated definition directly for typed Swift file/window metadata of an opened file. Catalog and Quick Look entries use the separate `Summary` type (§9), which is deliberately not an `AppDefinition`. The core can serialize explicit public projections for CLI output (`inspect`, `describe`) and the page bootstrap. Avoid keeping a stored manifest string plus a normalized manifest string alongside a typed model.

**Page routing.** Rust decodes every page request once and returns either an opaque JSON reply or a typed host action through UniFFI. JavaScript serializes the request before posting it and parses the reply string; Swift does not inspect JSON fields or decode the descriptor, theme or definition. Swift retains native origin, frame, view-token and lifecycle checks, performs the host action, and returns typed results to Rust for reply encoding. Rust builds config from the accepted definition while passing the original format descriptor to the page. Only the small host-action enum needs UniFFI exposure, not a second model of the whole page wire.

Replace TypeBox metadata checks in interactive `init` (`packages/hitslop/src/cli/init.ts`) and `build` (slug check in `build.ts`) with a native engine validation operation that accepts the candidate metadata and returns field-path diagnostics (`[{ path: ["window","width"], message }]`). Keep field-level feedback and no filesystem changes on invalid input. `check`, `dev`, `build` and pack use the same Rust acceptance rules. Do not use the browser WASM engine as the authoring acceptance authority.

Delete:

- manifest JSON Schema generation (`manifest.schema.json`, `manifest-format-1.schema.json`), quicktype’s manifest model, and the TypeBox command-metadata envelope (`commands.schema.json`, `command-call.schema.json`);
- the internal-envelope validators (`envelope.rs`, and the `Envelope` enum and `envelope_is_valid` FFI that Swift calls today);
- the `jsonschema` dependency.

Command names, descriptions and argument descriptors are typed entries of `definition_json`; argument contracts are descriptors, not JSON Schema (§3).

### Command execution

The owner executes every command, from the page and from the CLI, in the restricted evaluator. This replaces today’s two execution paths. Page commands currently run in page JavaScript (`shell/owner/document.ts:311` `runCommand`), and CLI commands run in the engine’s QuickJS child (`crates/slop-engine/src/call.rs`, `runner/mod.rs`). Having one path removes page/evaluator disagreement by construction, puts argument validation in one place, and keeps authored command code out of the page’s privileges.

```text
page (commands.run) or CLI (call)
  → owner: drain the caller’s pending edits; check the arguments (descriptor checker)
  → evaluator: fresh runtime, the stored command program, the snapshot value, now, seed
  → owner: validate the returned intents; apply them as one batch with ifVersion and the
           command name (undo label)
  → publication → result to the caller
```

- **Fresh runtime per evaluation.** Each evaluation runs in a new QuickJS runtime and context. A persistent realm per document is not used: module-level variables would survive between calls, and a stale-base retry could then compute a different result. A reusable *process* that creates a fresh runtime per evaluation is a later, separate optimization (below).
- **Off the serial queue.** The owner snapshots `{ value, version }`, evaluates outside its serial edit queue (evaluation may take up to the evaluator’s 3 s limit), then submits the intents through the queue with `ifVersion`. On a definite `stale_base` the owner re-snapshots and evaluates once more, in a fresh runtime, with the **same** `now` and seed. These belong to the invocation, as `call.rs` does today. Any other failure is returned and never replayed.
- **Kept semantics.**
  - Before evaluating, the owner drains edits the calling page already sent.
  - A page call resolves only after the publication containing its changes reaches the page store, as `store.reached(sequence)` does today.
  - The batch carries the command name for the undo label.
  - A successful command is one undo step.
  - An unknown outcome is reported, never retried.
  - A refused or failing command leaves no partial edit.
- **Page protocol.** The page request `commands.run { name, args }` replaces page-side evaluation. Its reply is the command’s JSON result plus the inserted row `ids`, after publication. Its refusals use the shared refusal shape. The round-two `commands.check` request is not needed.
- **The evaluator crate and binaries.** The shared runner lives in `crates/hitslop-runner/src/`: the rquickjs evaluation, `sandbox.rs` with macOS `sandbox_init` and Linux seccomp, the generated prelude, and the input/output/time limits. This crate has no storage dependency.
  - `slop-engine` keeps evaluating by re-executing itself (`--evaluate-command`), so the npm package still ships one binary.
  - The Mac app ships a separate helper built from the same crate, `Contents/Helpers/hitslop-evaluator`, signed and embedded by `scripts/build/embed-hitslop-native.sh` beside `hitslop-native`. Today that script deliberately ships no `slop-engine`, and the app does not start depending on the npm package.
- **Who spawns it.** The core’s owner spawns the evaluator through an `Evaluator` handle the host configures once: a path plus arguments.
  - The app passes the bundled helper’s path over FFI (`set_evaluator(path)`).
  - The engine passes `current_exe() --evaluate-command`.
  - An owner without a configured evaluator refuses commands with a typed error. It never falls back to running authored code in-process.
- **Browser development.** `slop dev` starts a native owner over a disposable SQLite document per page. Vite's authenticated loopback transport forwards the same page requests and publications as the native host. The owner, not a browser WASM engine, validates and executes commands. EOF and generation changes fence evaluations before close; a lost owner produces a visible unknown-outcome error and requires an explicit reset, never replay. Reuse the spike's proven lifetime behavior, adapted to Rust-authored wire types and the shared owner coordinator.
- **Latency gate.** Process start, runtime creation, program load and evaluation are now on the page’s click path. Measure on the slowest supported Mac with a bundled template: p50 and p95 of a page command from request to resolved promise.
  - Target: p95 ≤ 50 ms with a fresh process per evaluation.
  - If it misses, the first remedy is a reusable evaluator process that still creates a fresh runtime per evaluation, restarted on any failure and never sharing a realm.
  - Record the numbers in `docs/evidence/`.
- **Deferred.** A warm evaluator process is conditional on measured latency. Removing `run` bodies from the UI bundle is part of this implementation and the first frozen ABI.

Keep `bun run schema:generate` as the umbrella command for generated TypeScript, exported constants and bindings orchestration. Its `--check` must detect drift without rewriting files. Move internal limits/codes/requirements to Rust; TypeScript gets them from a small Rust exporter of constants (ts-rs exports types only). Preserve the distinction between current authoring rules and acceptance of released package formats. Future readers must retain released acceptance behavior in code (one acceptance module per released `packageFormat`, selected by the file’s marker) and in corpus tests, replacing today’s frozen `packages/hitslop/acceptance/packageFormat-1.json` record.

### Small CLI reply guard

Keep the previously selected transport guard:

- the reply is an object, not an array;
- `ok` is boolean;
- the success method matches the request;
- a failure has a string code and message, with correctly typed optional fields.

Unknown code strings remain presentable outcomes. Successful `batch`/`call` replies require a string-array `ids`; `call` requires a present `result`, including `null`. Extra reply fields are allowed. Other same-build payloads use the generated types without a second full schema validator.

Malformed/incomplete replies are an unknown outcome, not success. Never retry an ambiguous mutation. Preserve the permanent command-protocol preflight, discovery tolerance and refusal shape. Exact request protocol is checked before document access or payload-specific validation.

## 9. SQLite layout

Seven STRICT tables: `app`, `assets`, `attachments`, `artwork`, `document`, `checkpoint` and `updates`. Their lifecycles differ: the embedded app is sealed, attachments are immutable while retained, artwork is replaceable, and Loro state is mutable. One Rust resource reader serves these tables without merging their write rules. Retain the durability and defensive-open policies in §2.

### Ownership of the layout

The storage version (`user_version`) owns every physical table, column, constraint, index, trigger and storage limit. `packageFormat` owns interpretation and app acceptance, including `definition_json`; `runtimeABI` owns app-facing behavior; Loro layout owns checkpoint/container interpretation. No release version substitutes for these markers.

Keep the marker-first refusal: read `application_id`, `user_version`, then the permanent `app.package_format` and `app.runtime_abi` columns by name before comparing the exact layout. A newer requirement returns `requires_update` without writes. The current layout check includes all seven tables, automatic indexes and six triggers.

A new definition field needs a package-format change only. A new physical column needs a storage change, plus a package-format change when it adds interpreted app data. Freeze the version-1 DDL at the first release. Implement migration replay and `expected_tables(N)` together with storage version 2, not speculatively at launch. At that point, test that migrated and newly packed layouts match; writes migrate under the writer lock in a transaction, display reads never migrate, and the embedded app retains its original package format.

### Creating a file

1. Open the private staged database. Before creating tables, explicitly set `page_size = 4096`, `auto_vacuum = FULL`, `application_id = 0x48534C50` and `user_version = 1`.
2. In one transaction create the version-1 schema; insert assets, supplied artwork and the initial checkpoint; insert the `app` row **last**, sealing assets. Commit with the existing `journal_mode=DELETE`, `synchronous=EXTRA` and `fullfsync` policy. There are no theme or command tables.
3. Close and reopen read-only, run full acceptance with integrity checking, then publish atomically. Replace only an existing template; new documents use exclusive publication. Publish no journal companion.
4. Creating a document uses SQLite backup to copy the template and inserts its `document` row. Backup preserves page size and triggers without firing them.

Use 4096 now. SQLite page size is not part of the exact `sqlite_master` layout contract; a later build can choose a different size for newly created files without changing format 1. Benchmarking is optional future performance work, not a launch gate.

### Schema and seal

SQL checks storage shape. Product rules, resource-reference checks and limits stay in Rust. Catalog columns support summary reads without parsing `definition_json`; all window settings, the skin key, ordered theme, typed command definitions, views and the descriptor live in that one JSON value. Theme overrides stay in Loro.

```sql
CREATE TABLE app(
  id INTEGER PRIMARY KEY CHECK(id = 1),
  package_format INTEGER NOT NULL CHECK(package_format >= 1),
  runtime_abi INTEGER NOT NULL CHECK(runtime_abi >= 1),
  slug TEXT NOT NULL, title TEXT NOT NULL, description TEXT NOT NULL,
  author_name TEXT NOT NULL, author_url TEXT,
  category_primary TEXT NOT NULL,
  category_secondary TEXT CHECK(category_secondary IS NULL OR category_secondary != category_primary),
  definition_json TEXT NOT NULL
) STRICT;
CREATE TABLE assets(
  key TEXT PRIMARY KEY,
  media_type TEXT NOT NULL,
  encoding TEXT NOT NULL CHECK(encoding IN ('identity', 'br')),
  size INTEGER NOT NULL CHECK(size >= 0),
  bytes BLOB NOT NULL,
  CHECK((encoding = 'identity' AND size = length(bytes))
        OR (encoding = 'br' AND length(bytes) < size))
) STRICT;
CREATE TABLE attachments(
  id TEXT PRIMARY KEY CHECK(length(id) = 64),
  media_type TEXT NOT NULL,
  bytes BLOB NOT NULL
) STRICT;
CREATE TABLE artwork(
  name TEXT PRIMARY KEY CHECK(name IN ('preview', 'icon')),
  png BLOB NOT NULL
) STRICT;
CREATE TABLE document(id INTEGER PRIMARY KEY CHECK(id = 1)) STRICT;
CREATE TABLE checkpoint(id INTEGER PRIMARY KEY CHECK(id = 1), bytes BLOB NOT NULL) STRICT;
CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL) STRICT;
CREATE TRIGGER app_update BEFORE UPDATE ON app
  BEGIN SELECT RAISE(ABORT, 'The embedded app is immutable'); END;
CREATE TRIGGER app_delete BEFORE DELETE ON app
  BEGIN SELECT RAISE(ABORT, 'The embedded app is immutable'); END;
CREATE TRIGGER app_insert BEFORE INSERT ON app WHEN EXISTS(SELECT 1 FROM app)
  BEGIN SELECT RAISE(ABORT, 'The embedded app is written once'); END;
CREATE TRIGGER assets_update BEFORE UPDATE ON assets
  BEGIN SELECT RAISE(ABORT, 'App assets are immutable'); END;
CREATE TRIGGER assets_delete BEFORE DELETE ON assets
  BEGIN SELECT RAISE(ABORT, 'App assets are immutable'); END;
CREATE TRIGGER assets_insert BEFORE INSERT ON assets WHEN EXISTS(SELECT 1 FROM app)
  BEGIN SELECT RAISE(ABORT, 'App assets are sealed by the app row'); END;
```

Ordinary rowids support `blob_open`; keep blob columns last. Asset `size` is decoded length; `length(bytes)` is stored length and reads the BLOB record header without loading its overflow pages. No cross-table deduplication occurs. Ordered theme and command arrays preserve declaration order; object-field order carries no meaning.

The seal protects both templates and documents. BEFORE INSERT refuses writes even with REPLACE, IGNORE, UPSERT or an explicit colliding rowid; all asset inserts are refused after the app exists, so no rowid or kind predicate is needed. Before sealing, pack uses ordinary INSERT and treats duplicate asset keys as errors. UPDATE and DELETE of assets are always refused. Exact layout checking rejects altered triggers; recursive triggers remain off.

The [seal evidence](../docs/evidence/app-seal-2026-10-06.json), reproduced by [the in-memory check](../scripts/dev/check-app-seal.ts), covers all 13 proposed bypasses, unchanged rows after refusal, and the allowed attachment/artwork paths. At the storage implementation step, transfer these cases to Rust storage tests.

Attachment immutability remains a writer invariant: compute the content address, check an existing row before an ordinary insert, never update/replace its bytes, and delete only through reclamation. Retain duplicate-import, first-touch hash verification and reclamation tests. Artwork keeps its existing upsert in one transaction; delete plus insert and clean-copy deletion also remain legal. The three mutable state tables retain their existing write path. No generic SQL writer crosses FFI.

### Keys, routes and budgets

| Storage | Keys | Encoding | Reader |
| --- | --- | --- | --- |
| assets | `ui.js`, optional `ui.css`, `media/<sha256>.<ext>` | Brotli for supported text when smaller; otherwise identity | page `/assets/`; Rust for skins |
| assets (private program) | optional `commands.js` | Brotli when smaller | owner/evaluator only |
| attachments | lowercase hex SHA-256 | identity | page `/attachments/`; socket attachment reader |
| artwork | `preview`, `icon` | identity PNG | native capture, summary, catalog, Quick Look |

Rust accepts asset keys only as the three fixed entry names or `media/` plus 64 lowercase hex digits, a dot and a canonical extension from its registry. The public app route accepts only `ui.js`, `ui.css` and media keys; `commands.js`, encoded traversal and unknown keys return no resource. Attachment routes accept only attachment IDs and never query assets. A skin key must name a PNG asset. `ui.js` is mandatory; nonempty command definitions and the private program must occur together. Optional CSS is detected by its fixed key.

Keep current budgets: app assets (including commands) 25 MiB each, 256 entries, 50 MiB total; attachments 10 MiB each, 256 entries, 100 MiB total; Loro persistence 32 MiB and 4,096 updates; existing artwork byte/pixel budgets. Metadata columns retain the 64 KiB budget. Bound `definition_json` before reading it by the existing descriptor, window/theme/views and command budgets; then check each typed part separately.

Use aggregate queries per table without loading blobs: `count`, `max(size)` and `sum(size)` for assets; BLOB `length()` for attachment/artwork byte lengths. There is no extra kind column or second size copy for attachments. Do not tighten unrelated limits.

### State and identity

Keep the singleton document discriminator, singleton checkpoint and ordered update rows. Preserve the current identity mechanism in the checkpoint/core; do not invent a second document ID in SQL. A template’s checkpoint contains its seed state, packed from `initial` with the fixed template peer and place-derived row IDs (`initial_checkpoint`, `name_rows`). Creating a document copies the app/resources/checkpoint and establishes document identity through the existing path. There is no persisted duplicate `initial` JSON and no separate seed-state table.

Add one acceptance check to pack. After building the checkpoint, load it and compare its exported value with the parsed `initial` value after row naming. They must be semantically equal: no dropped keys, coerced numbers or reordered lists. Otherwise the checkpoint, the only copy of the seed state, could silently differ from what the author declared.

Leave Loro container layout, `$id` identity and save/compaction behavior unchanged. Sharing a resource reader does not merge mutable document state into app metadata. Attachment reachability stays in the CRDT (`Document::attachment_references`); there is no SQL reference table, which would drift the first time undo restores a deleted reference.

### Reading a file: summary and full acceptance

There are two read APIs with different guarantees. Neither writes, and neither evaluates author code. Today there is only one: the catalog lists templates through `open_template` → `opened` → `check_app` (`file/catalog.rs`, `file/mod.rs`), which runs full acceptance and even reads assets. The split makes listing cheap and makes explicit that a listing certifies nothing.

**Shared preamble.** Both APIs start the same way, each step refusing without writing:

1. The file’s markers: `application_id`, then `user_version`. A newer storage version is refused with `requires_update`.
2. The app’s requirements: `SELECT package_format, runtime_abi FROM app WHERE id = 1`, read by name before any layout comparison. A newer requirement is refused with `requires_update`.
3. The exact layout for the storage version: tables, automatic indexes, triggers and singleton counts.

**Summary**, `file::summary(path)` (catalog, Quick Look, Finder-facing hosts):

4. Read the scalar app columns and the artwork rows the host shows:

   ```sql
   SELECT slug, title, description, author_name, author_url, category_primary, category_secondary
   FROM app;
   SELECT rowid, length(png) FROM artwork WHERE name = ?;  -- then blob_open
   ```

5. Apply only the metadata checks needed to display the fields safely, using the package format’s rules for the scalar metadata it shows (lengths, slug grammar, category vocabulary).

What a summary means: the file is a hitSlop file this build can read, and these fields are presentable. **It is not a validity certificate.** It never reads `definition_json` or any asset or attachment blob; it reads only the artwork it shows. It never decodes a PNG, and never builds an `AppDefinition`. The catalog’s `issues` report summary failures only. A template whose app fails full acceptance (only possible through corruption, since pack fully validated it and the file is immutable) can appear in the catalog and fails at `create` with its acceptance error. The summary type is distinct from `AppDefinition` in Rust and in Swift, so code cannot treat a listed entry as an accepted app.

**Full acceptance**, `file::open(path, integrity)` (`create`, `check`, `inspect`, the owner, capture sources, `slop` commands that read app semantics):

4. Resource budgets by table (the aggregates above) and `length(CAST(definition_json AS BLOB))`. No values are read yet.
5. Decode the app row (scalar columns plus `definition_json`) into `AppDefinition` with the acceptance module of the file’s `package_format`. Then resolve and type-check every reference: right kind, right media type, row present.
6. For a skin, the streaming PNG check (§6).
7. With `integrity`, `PRAGMA quick_check`.
8. An owner then loads and validates Loro state against the descriptor.

Enforce the summary’s read set in tests with a SQLite authorizer (`Connection::authorizer`, rusqlite `hooks` feature, test-only) that fails on any `SQLITE_READ` of `assets.bytes`, `attachments.bytes` or `app.definition_json` during `summary`. Artwork remains readable without launching WebKit. Failed validation writes nothing. Retain current integrity checks at their owning boundaries and typed errors through the native façade.

### Native resource reader

Rename `AssetReader` to `ResourceReader`. Its FFI surface takes a route enum that exposes only page-visible kinds:

```text
enum ResourceRoute { App, Attachment }
ResourceReader.size(route, key)  -> Option<{ size: u64, media_type: String }>
ResourceReader.read_range(route, key, offset, length) -> Option<bytes>
```

The scheme handler maps `/assets/<key>` to `App` and `/attachments/<id>` to `Attachment`. It uses the returned media type (no `contentType(path:)` FFI), and keeps its `ByteRange` parsing and `206`/`416` responses unchanged. Artwork keeps its own reader. The page-visible reader always refuses `commands.js`; only the owner’s internal reader supplies it to the evaluator.

## 10. Development, compatibility and rollout

Keep normal Vite component/CSS HMR over a native owner. Watch the dependency graph produced by the explicit entry rather than a hardcoded filename list (`entryFiles` in `cli/entry.ts` goes away). Definition, skin, artwork and command changes prepare a new validated preview and fence the old generation before reset; ordinary component/CSS edits preserve accepted state. Refresh creates a fresh temporary document. Corrected build errors clear diagnostics without retaining a half-valid configuration. Remove the WASM preview and in-memory attachment store from the shipped CLI once this path passes; retain WASM where tests use it. This does not implement `slop open --browser` or OPFS.

Make the source API and storage changes as a pre-launch reset. Do not add readers or migrations for disposable development documents. Regenerate `tests/compat/dev` after the final model is implemented. Never modify frozen releases; none exists today (`tests/compat` holds only the unfrozen `dev` corpus, and `hitslop` is not published to npm). If a frozen release exists when implementation begins, the storage/app changes must instead follow the released-format compatibility rules before proceeding.

Keep every marker at its baseline for this reset: package format 1, runtime ABI 1, storage version 1, Loro layout 1 and command protocol 1. The new `pack`, metadata-validation and attachment shapes are part of the same unreleased protocol 1. No program speaking the old shapes has shipped, so a bump would only add ceremony. Keep the permanent refusal path unchanged: `--client-protocol N` as the first argument, exit status 2, one stderr line, and the `requires_update` reply shape. From the first public release, any change to request or reply shapes raises the protocol in lockstep across CLI, engine, native helper and owner.

After launch, app format, storage version, runtime ABI and Loro layout remain independent requirements. The storage version owns the physical layout, and the package format owns interpretation (§9). A future change dispatches or migrates according to the marker that owns what changed, and raises both when it changes both. Upgrading hitSlop never rewrites or replaces an existing embedded app simply because the installed runtime is newer.

### Implementation sequence

1. **Vite two-build gate** (the riskiest unknown, so it comes first).
   - Implement the shared Vite configuration and content-hash key function, build 1 (UI) and build 2 (definition and commands, with the component-token and no-style plugins and the `resolveId` refusals), `describe()` evaluation in the existing restricted runner, and `BuildInput` assembly.
   - Prove it on the §4 fixture: every pass condition, and every deliberate breakage failing with its documented error.
   - Strip executable command bodies from the UI with a tested AST transform; record the UI bundle size and prove that page calls are stubs.
   - Establish entry dependency tracking for `slop dev`.
   - `BuildInput` may start as a TypeScript type and move to ts-rs in step 2.
2. **Wire foundation.**
   - Selectively bring over the serde/ts-rs/UniFFI spike work for the engine and native wires, plus `BuildInput`.
   - Delete the matching internal-envelope `jsonschema` validators as each wire moves.
   - Fix raw build-input marker ordering and the CLI acknowledgement guard.
   - Run `bun run verify` before proceeding to other wires.
3. **Typed app acceptance.**
   - Add Rust `AppDefinition` with its stored form (columns plus `definition_json`), `WindowDefinition` (enum), command definitions with argument descriptors and the args-subset acceptance.
   - Add the descriptor-based argument check and the JSON Schema projection for `describe`, with the differential projection test.
   - Add `minLength` and code-point counting to the descriptor checker.
   - Add the `Summary` type and `file::summary`, separate from `AppDefinition`.
   - Add TypeScript exports and native records; replace manifest validation and the repeated Rust/Swift decoders.
   - Keep author/initial inference in the SDK.
4. **Package/storage switch.**
   - Implement the seven-table §9 schema and app-row seal at explicit page size 4096. Keep exact layout validation, add `ResourceReader`, and port the 13 bypass cases plus writer-invariant tests.
   - Keep the existing artwork upsert; immutable attachment insert/reclamation remains separate. Defer replay machinery to storage version 2.
   - Add the attachment URL route, first-touch verification and Rust media-type sniffing at import, response headers (`nosniff`, CSP `sandbox`), the navigation refusal, and the path-scoped CSP (with the native WebKit checks in §5 and §7, and the separate-host fallback if paths do not match).
   - Add the streaming `png` validation (rows, `finish()`, consumed input, interlace, APNG refusal) and 2× skin acceptance with native `contentsScale`.
   - Add pack’s initial round-trip check, replace pack staging input with `BuildInput`, update owner attachment/artwork access, and switch all native consumers together (catalog and Quick Look to `summary`).
   - No alternate writable path remains.
5. **Authoring and development.**
   - Adopt the explicit entry, slug and window API.
   - Switch the SDK to `s.*` arguments; remove `commandArgs()`, the `Type` re-export and runner-prelude validation.
   - Add `attachments.url` and fetch-based `read`; native-dev HTTP routes use the same Rust resource reader and sniffed types as the app.
   - Update check/dev/init (native metadata diagnostics), HMR, dependency discovery, templates and installed-package behavior.
   - Page commands still evaluate in the page until step 6, with no validation gap: the owner validates arguments in step 6, and before then pre-launch builds are not released.
6. **Owner-routed commands** (§8 “Command execution”).
   - Use the extracted `hitslop-runner` crate. Keep `slop-engine --evaluate-command`, and build `hitslop-evaluator` for the app.
   - Embed and sign `hitslop-evaluator` in `Contents/Helpers` (`embed-hitslop-native.sh`) and configure it over FFI.
   - Add the `Evaluator` handle to the owner: snapshot evaluation off the serial queue, apply with `ifVersion`, one stale retry with the same `now` and seed, and a fresh runtime per evaluation.
   - Add the page request `commands.run`, the owner’s `call` handling for the CLI (replacing the engine-side evaluation in `call.rs`) and the native `slop dev` host. Port the spike's HMR/lifetime tests, discard its callback argument, and delete the production WASM preview path.
   - Remove page-side evaluation (`OwnerDocument.runCommand`’s `evaluate`).
   - Measure page-command latency against the gate; add the reusable-process optimization only if it misses.
7. **Remaining wires and cleanup.**
   - Migrate the socket/page/host/core wires; Rust routes page messages to opaque replies or typed host actions. Move JSON serialization to the JS bridge edge; preserve native sender/lifetime fences and remove Swift JSON decoding. Remove `attachments.read` from the page vocabulary.
   - Remove the `typebox` dependency, the `jsonschema` crate, TypeBox wire/manifest schemas, the custom contract generators, quicktype manifest generation and obsolete artifacts/tests.
   - Remove dead schema-package exports, and update npm packaging to include the generated TypeScript the SDK and CLI need.
8. **Documentation and release evidence.**
   - Update AGENTS.md ownership. Replace “TypeBox owns the wire” with “Rust types own the wire and app definition; ts-rs generates TypeScript; UniFFI carries types to Swift; command arguments are descriptors”.
   - Restate “named commands evaluate in a restricted child that returns intents to the owner” as covering page and CLI calls alike.
   - In the compatibility section, state that the storage version owns the physical layout.
   - Update architecture, engineering contracts, testing, authoring docs, landing guides, starter, bundled skills and the development corpus.
   - Retain the spike report as evidence of tool selection rather than an active competing plan.

Each step must leave one authoritative edit/storage path. Continue in `ready-ship` after consolidating verified checkpoints. Preserve every spike’s unique source/evidence in its existing branch before retiring that worktree. Never share `target/` between checkouts. The earlier spike showed that sharing `target/` overwrites the main tree’s uplifted native library (`target/debug/libhitslop_core_ffi.dylib`). A fresh worktree also needs the built page shell (`Sources/HitSlopDocument/Resources/shell`, gitignored) before SwiftPM builds. Do not discard or overwrite concurrent main-tree changes.

### Opening a released file

Before the first release, give each persisted interpretation one dispatch point:

- The storage version selects its exact layout. Add replay-based expected layouts when storage version 2 is implemented. Display reads never migrate; the first necessary write migrates under the writer lock and accepts the result in the same transaction. Freeze `storage-1.sql` when the first release is cut.
- `package_format_1::decode` owns the frozen stored structs, defaults, constraints and `deny_unknown_fields`, then translates to the current host `AppDefinition`. Do not decode an old file with the latest host struct. A compile-time guard prevents raising the marker without a reader.
- Swift receives translated typed metadata and `Summary` through UniFFI. It never reparses a raw manifest or theme. The page receives the format's original descriptor representation so its embedded SDK can compare it to the descriptor compiled into the original UI; host normalization must not change that comparison.
- Move today's page context into `shell/abi/1.ts` before launch and dispatch exhaustively on `runtimeABI`. The evaluator prelude and accepted intent vocabulary are part of that same ABI. Adding a new arm never replaces old behavior or loosens intent validation.
- Keep the pinned Loro checkpoint/update import path. Replay original corpus bytes, original UI and original command programs; never rebuild them on open. Keep the shell host-owned so host bug fixes can ship.
- Same-build page/socket/engine wires use the current serde types and exact command protocol. They do not acquire document-format adapters.

### Implementation progress

- Consolidation: all five side checkouts retired after preservation commits; `ready-ship` is the only working checkout. Branches, provenance and remaining selective ports are listed in the [consolidation record](../docs/evidence/worktree-consolidation-2026-10-06.md).
- Native-dev feasibility: passed on macOS; evidence retained. Linux is deferred to the final platform gate.
- Step 1, Vite two-build gate: implemented and verified in `app-definition-refactor`, then integrated into `ready-ship` as commit `27e5d6cc`. See the [compiler gate evidence](../docs/evidence/app-definition-build-gate.md) for the real-bundle, asset, restricted-runner and WebKit tests and the resulting authoring syntax boundary.
- Step 2, engine checkpoint: implemented and verified (`bun run verify`: 226 Rust tests, 151 Bun tests and installed-package checks). Rust owns engine requests, structured replies and outcome-code enums. ts-rs 12.0.1 exports recursive dependencies into TypeScript; engine JSON Schema validators and artifacts are removed. Raw future app payloads still reach the marker check before interpretation, and the CLI keeps a small acknowledgement guard. See [checkpoint evidence](../docs/evidence/engine-wire-checkpoint.md). The native-helper checkpoint follows below; `BuildInput` remains in step 2. The new compiler remains internal until typed acceptance and packing land; the remaining TypeBox contracts, default builder and storage/authoring format have not switched yet.
- Step 2, native-helper checkpoint: implemented and verified with `bun run verify --native` (229 Rust, 150 Bun, 182 Swift and 56 native integration tests). Rust decodes the helper request and UniFFI carries it to Swift. The native TypeBox schemas, `schema/engine.ts`, native JSON Schema validators and 183 generated Swift lines are deleted. See [checkpoint evidence](../docs/evidence/native-wire-checkpoint.md), including the remaining minimum-macOS build-target warning. No pre-launch legacy reader or migration was added.
- Step 2, build-input checkpoint: complete. `BuildInput`, fixed metadata and window input types are authored in Rust and exported by ts-rs; the explicit declaration keeps SDK inference for initial values. The real-bundle fixture now uses `window.image`. Full verification and the focused WebKit gate pass; see [checkpoint evidence](../docs/evidence/build-input-checkpoint.md). This completes the type foundation, not app acceptance or the default-builder/storage switch. Completed work is staged and remains uncommitted. Next: step 3 typed acceptance and descriptor arguments.
- Step 3, descriptor foundation: implemented and verified. Arguments use the core's `Node::validate`; their JSON Schema projection has differential tests, including implicit integer/list bounds. `s.string` supports `minLength` and code-point bounds. Integral JSON number spellings canonicalize safely into Loro. Full verification passed (238 Rust, 152 Bun and installed-package checks); see [checkpoint evidence](../docs/evidence/descriptor-arguments-checkpoint.md). Typed definition acceptance and the production command/storage switch remain. Changes are staged without commits.
- Step 3, typed acceptance foundation: implemented and verified. `app/package_format_1.rs` owns the new decoder and fixed metadata types; the host window is a separate typed enum. Acceptance shares the descriptor, theme and marker checks, preserves the original descriptor for page boot, and validates command descriptors and initial values. `BuildInput` carries markers checked before its payload. Full verification passed (246 Rust, 152 Bun and installed-package checks); see [checkpoint evidence](../docs/evidence/app-acceptance-checkpoint.md). The SQLite/default-builder switch, `summary`, native consumers and removal of the current manifest validators remain outstanding. Changes are staged without commits.
- Step 4, media/PNG foundation: Rust owns the media registry and its generated TypeScript projection, passive signature recognition and package key grammar. Artwork writes and skin opens use bounded full PNG decoding; 1×/2× skins are accepted. Full verification passed (257 Rust, 152 Bun), followed by six focused native mask tests. See [media evidence](../docs/evidence/media-registry-checkpoint.md) and [PNG evidence](../docs/evidence/png-acceptance-checkpoint.md). This prepares resource acceptance; the SQLite/default-builder switch and attachment URLs remain outstanding. Changes are staged without commits.

- Step 3/4, build acceptance and step 6 runner preparation: verified. The explicit inventory is checked into owned bytes, including confined regular-file reads, media/hash checks and full skin/artwork acceptance. Initial values must survive their Loro checkpoint unchanged. The existing restricted child now lives in `hitslop-runner`, with a standalone helper and a fresh-runtime regression test; the engine still uses the same command path. Full verification passed (266 Rust, 152 Bun); see [checkpoint evidence](../docs/evidence/build-acceptance-and-runner-checkpoint.md). The production pack/storage switch and owner/app-helper integration remain outstanding. Changes are staged without commits.

- Wire generator cleanup: verified. Deleted `rust-contracts.ts` and `wire.generated.rs`; Rust modules own the remaining definitions and shared constants. ts-rs now exports core intents/publications as well as engine/build types. Unused TypeBox core input schemas are gone. Full verification passed (266 Rust, 152 Bun and installed-package checks); see [checkpoint evidence](../docs/evidence/rust-wire-generator-removal.md). Page/socket validators, the Swift generator and current manifest/command schemas remain for their boundary checkpoints. Changes are staged without commits.

- Socket wire checkpoint: verified. Rust owns decoding, protocol-first refusal and request constraints; TypeScript uses ts-rs requests and shared engine replies. Socket TypeBox schemas, five JSON Schema artifacts and 380 generated Swift lines are deleted. Full verification passed (271 Rust, 150 Bun), followed by Swift and 56 native integration tests. The Cargo C deployment target now matches the supported Swift package floor. See [checkpoint evidence](../docs/evidence/socket-wire-checkpoint.md). Page/host and manifest/command boundaries remain. Changes are staged without commits.

- Page/host wire checkpoint: verified. Rust decodes and routes every page request, admits native actions through the owner lifecycle/view fence and carries them through UniFFI. Swift no longer parses page/host message JSON. Deleted `swift-contracts.ts`, `Contracts.generated.swift`, the page schema and the last wire validator (`envelope.rs`). Full verification passed (273 Rust, 149 Bun), followed by Swift and 56 native integration tests. See [checkpoint evidence](../docs/evidence/page-wire-checkpoint.md). Both handwritten generators are gone; current manifest/command schemas, quicktype and constants generation remain for the packaging switch. Changes are staged without commits.

- Packaging integration (steps 4–8): implemented, verified and staged without commits. Seven tables with the app-row asset seal replace the manifest layout. Explicit entries, `s.*` commands, typed native definitions, attachment URLs, streamed PNG acceptance and owner-routed commands are now the default path. The page context and evaluator prelude dispatch by ABI. TypeBox, quicktype, the remaining schema outputs and production JSON Schema validators are removed. The browser preview uses a native owner; WASM remains test-only. Full `verify --native` passed: 277 Rust, 143 Bun, 184 Swift and 57 native tests, plus installed-package checks. The fresh unfrozen development corpus replays; all markers remain 1. The built app passed artifact checks and a command through its embedded evaluator; the real-app crash case passed separately with an isolated test registry. M1 command latency meets the gate. See [integration evidence](../docs/evidence/packaging-integration.md). HEAD remains `6557240e`; Linux qualification stays deferred.

## 11. Verification and acceptance

Tests belong to the boundary that owns the behavior. Test outcomes and resource isolation, not generated source strings, private call sequences or exact CSS formatting.

| Boundary | Required scenarios |
| --- | --- |
| SDK types | Explicit app entry, document/initial inference, standard/skin exclusivity (`resizable` on a skin is a type error), typed command args from `s.*` (missing required, wrong type and unknown field are compile errors), nested forbidden argument nodes refused at `doc.command`, `s.text` in args refused with its message, no public TypeBox in the package |
| Rust acceptance | Missing/unknown fields, absent versus null, lengths/ranges, bad URLs/categories, invalid shapes, resource references of the wrong kind or media type, code-point bounds (emoji and combining marks counted as code points), `minLength` > `maxLength` refused, args subset (each forbidden node, nested), marker-first future payloads |
| Commands | **Differential projection test**: for every argument descriptor in the bundled templates and fixtures and the §3 value corpus, the descriptor checker and a test-only JSON Schema validator agree, including `"😀"` exactly at `minLength`/`maxLength`; agent call with a valid argument, a wrong type (`at /text`), an unknown key and an empty string under `minLength`; page call of `s.integer({ min: 1, max: 10 })` with `100` from well-typed code is refused by the owner with the CLI’s message before the evaluator starts (an evaluator-spawn counter stays 0); page and CLI refusals are identical; atomic failure; open and closed documents |
| Build | Arbitrary supporting filenames, explicit slug independent of folder name, every §4 gate condition (component tokens in build 2, `describe()` in the restricted evaluator retaining the skin URL, no component code run, `@import`-only font and aliased `url()` emitted by build 1, identical keys across builds and rebuilds, artwork-only image routed to an artwork row), every §4 deliberate breakage failing with its documented error, the supplementary `/assets/` scan refusing a key no build emitted, no source paths in outputs, no registered commands stores no command program; a nonempty public directory fails with guidance; pack refuses invalid resources without publishing |
| Assets | Imported PNG/WebP, CSS image, WOFF2 reached only through `@import`, an aliased (`$lib`) CSS URL, export-only asset, skin-only asset, reused asset (one row), same bytes with two media types (two rows), eager glob, missing import with source location, unreferenced-file omission, no boot dependency on the network, no stylesheet produces no link |
| Resource storage | Assets/attachment route isolation and command program unreadability; CHECK failures for identity-size mismatch, ineffective compression and bad artwork key; all 13 app/asset seal bypasses in the evidence script; immutable attachment duplicate import and first-touch verification; legal attachment reclamation, artwork upsert/delete-insert and clean copies; compression limits and missing keys |
| Attachments | Import then `url()` renders in `<img>`, `<audio>` and `<video>`; full read, ranges at the start, middle and end; suffix range; `416`; cancellation; first-touch hash verification refuses a damaged row; the same bytes imported twice keep one row; reclamation at close; native-dev session attachment URLs and isolation between preview pages; capture of an export that shows an attachment; **sniffing**: each allowlisted signature stores its type, a PNG uploaded with `File.type = "text/html"` is stored as `image/png`, and JavaScript, HTML, SVG, XML and text bytes are stored as `application/octet-stream`; **execution**: an attachment containing JavaScript cannot load through `<script src>`, `<script type="module">`, dynamic `import()`, a `Worker` or a stylesheet link (CSP refusal), an HTML or SVG attachment cannot be framed or navigated to, every attachment response has `nosniff` and `Content-Security-Policy: sandbox` (native WebKit test confirms both are honored, or the result is recorded) |
| Content security policy | Native pages: scripts, workers, styles and fonts load from `/__shell__/` and `/assets/` only; images, media and `fetch` may use `/attachments/`; WebKit matches CSP host-source paths for the `slop` scheme (native test), else the separate-host fallback (`slop://attachments/<id>`) is in place and tested the same way; browser development keeps `'self'` plus the HMR socket |
| Native skins | 1× and 2× accepted; 1.5× and off-by-one refused with both sizes named; 8- and 16-bit RGBA accepted; interlaced (Adam7) PNGs accepted; a 16-bit 2× skin at the pixel budget validates with peak heap below the 8 MiB decoder bound plus one row (allocation-counting test); RGB, palette, truncated mid-rows, bad CRC in a chunk after the last row (caught by `finish()`), missing `IEND`, bytes after `IEND` (consumed-input check) and APNG (`acTL`) refused; header dimensions overflowing the checked arithmetic refused before decoding; pixel budget for 2×; alpha 25 versus 26 at both scales; transparent center clicking through to another window; fixed size; alignment and native backing at `contentsScale` 2; no doubled skin |
| Shapes | Radius/path clipping and hit testing, SVG arcs and holes, transparent/glass backgrounds, resizing and aspect locking |
| Capture | Saved-state copy, editor fallback, dedicated export without mask, explicit skin in export, icon role, font/image readiness, selectable PDF, failed capture preserving prior artwork/editor |
| Transport | Malformed acknowledgements, wrong method, extra fields, unknown error codes, permanent update refusal, no ambiguous replay; Rust-only page decoding, typed host-action replies and unchanged origin/view fencing |
| Persistence | Save/reopen/undo, failed save retains ownership, crash recovery, copying templates/documents (backup preserves page size and triggers), row IDs, untouched Loro layout, initial round-trip equality at pack, newer requirement refuses without writes |
| Command execution | Page and CLI calls both run in the restricted evaluator through the owner; the page never evaluates a body; drain-before-evaluate (an edit sent just before the call is visible to the command); the page promise resolves only after the publication reaches the store; the undo label is the command name and one undo reverts the whole command; a definite `stale_base` retries once in a fresh runtime with identical `now` and seed (the result equals a single uncontended run); module-level state does not survive between calls (a counter in module scope reads its initial value on every call); an evaluator crash, timeout or oversized output is a refusal with no partial edit; an unknown outcome is reported and never replayed; an owner with no configured evaluator refuses with a typed error and never evaluates in-process; `hitslop-evaluator` is present, signed and sandboxed in the app bundle; the native `slop dev` owner gives the same result and refusals as the app and CLI for the same input; page-command p50/p95 latency recorded against the gate |
| Layout and markers | Exact layout comparison includes triggers; a file with an extra or edited trigger is refused; a newer storage version or `packageFormat` is refused with `requires_update` before the layout is compared; migration replay tests are added with storage version 2; `summary` reads no `bytes` except artwork and no `definition_json` (authorizer test); `summary` of a template with a corrupted descriptor lists it, and `create` from it fails with the acceptance error; a published file has no journal companion |
| Installed package | Init/check/build/dev using the npm artifact outside the checkout; no missing generated types or source-only assumptions; macOS rendering and Linux data commands |

The old spike’s byte-identical `__commands/metadata.json` gate is retired with that file. Its replacement is semantic: the differential projection test (§3) plus preserved command behavior. Pre-launch TypeBox output was never released, so its bytes are not a contract. Storage changes are deliberate; constraint loss is not.

While implementing, run focused Rust, SDK, CLI and native tests as appropriate; run `bun run schema:generate --check` and `bun run verify` at integration checkpoints. Finish with `bun run verify --native`, native skin/capture checks, and all bundled-template renders. Run `bun run release:check` before releasing. A regression test must fail for the intended reason before its fix.

## 12. Documentation changes and source references

Update the current guides together with their implementations:

- **Skin guide:** replace the `assets/...` path requirement with an imported image reference. Teach the 1×/2× rule, recommend 2×, and keep the RGBA, dimension and alpha rules.
- **Window guide:** teach explicit app identity and `window`.
- **Authoring guide:** teach entry composition, `s.*` arguments (the table in §3, `minLength`, no unions), and how agents see arguments in `describe`. It also teaches that commands always run in the restricted evaluator, even when a page button calls them: no DOM, no network, no ambient time or randomness; module-level state does not persist between calls. And it notes that the HTML `maxlength` attribute counts UTF-16 code units, unlike field bounds.
- **Attachments:** document `attachments.url` and that user media plays by URL. Remove stale statements that user media import is deferred where they contradict the shipped attachment API.
- **Native click-through:** keep it distinct from browser appearance.
- **Bundled skills:** update `hitslop-authoring` and `hitslop-document`, which today say args use TypeBox `Type`, and `hitslop-native`, which describes TypeBox-generated contracts.

Current repository sources used to ground this plan:

- [Architecture](../docs/architecture.md), [engineering contract](../docs/engineering-contract.md), [testing](../docs/testing.md).
- [SDK app declaration](../packages/hitslop/src/sdk/slop.ts), [SDK commands](../packages/hitslop/src/sdk/commands.ts), [explicit declaration compiler](../packages/hitslop/src/cli/definition-build.ts), [Vite compilation](../packages/hitslop/src/cli/vite.ts), [build staging](../packages/hitslop/src/cli/build.ts), [command transform](../packages/hitslop/src/cli/command-transform.ts), [runner prelude](../packages/hitslop/src/shell/abi/runner-1.ts), [page attachments](../packages/hitslop/src/shell/attachments.ts).
- [Storage schema](../crates/hitslop-core/src/file/storage-1.sql), [file acceptance](../crates/hitslop-core/src/file/mod.rs), [resource reading](../crates/hitslop-core/src/file/assets.rs), [packing](../crates/hitslop-core/src/file/pack.rs), [copies](../crates/hitslop-core/src/file/copy.rs), [artwork checks](../crates/hitslop-core/src/file/artwork.rs), [store and attachments](../crates/hitslop-core/src/store.rs), [descriptor checker](../crates/hitslop-core/src/descriptor.rs), [describe](../crates/hitslop-core/src/describe.rs), [manifest acceptance](../crates/hitslop-core/src/app/package_format_1.rs), [owner command coordinator](../crates/hitslop-core/src/owner/commands.rs), [command evaluator](../crates/hitslop-runner/src/lib.rs), [page command execution](../packages/hitslop/src/shell/owner/document.ts), [app helper embedding](../scripts/build/embed-hitslop-native.sh), [content security policy](../packages/hitslop/src/schema/policy.ts).
- [Native URL handler](../apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/SchemeHandler.swift), [window mask](../apps/apple/Packages/HitSlopApple/Sources/HitSlopHost/SlopWindowMask.swift), [renderer](../apps/apple/Packages/HitSlopApple/Sources/HitSlopHost/SlopRenderer.swift).
- [PNG skin guide](../apps/landing/src/content/docs/docs/guides/png-window-skins.mdx), [window guide](../apps/landing/src/content/docs/docs/guides/manifest-and-windows.mdx), [presentation and export skill reference](../.agents/skills/hitslop-design/references/presentation-and-export.md).
- Loro’s local source snapshot (`_docs/loro-main/crates/loro-common/src/value.rs` and `crates/loro/src/lib.rs`) was inspected as an implementation reference rather than an active hitSlop contract. That ignored snapshot is not required in a fresh checkout. Loro’s distinct scalar/text/container representations support retaining hitSlop document descriptors.

Relevant upstream behavior: [Vite asset imports and URLs](https://vite.dev/guide/assets.html), [Vite build options and library-mode inlining](https://vite.dev/config/build-options.html), [Svelte compiled CSS options](https://svelte.dev/docs/svelte/svelte-compiler), [SQLite incremental BLOB I/O](https://www.sqlite.org/c3ref/blob_open.html), [SQLite internal versus external BLOBs](https://www.sqlite.org/intern-v-extern-blob.html), and [JSON Schema string constraints](https://json-schema.org/understanding-json-schema/reference/string). These inform the build defaults; the repository’s pinned versions and implementation gates determine acceptance.

## Appendix A. Feedback considered and rejected

This appendix records historical decisions. The fourth-review rows below supersede earlier resource-table and artwork decisions; §§1–12 specify the current target.

Recorded so the same proposals are not re-litigated without new evidence.

| Proposal | Decision | Reason |
| --- | --- | --- |
| `journal_mode=WAL` | Rejected | Creates `-wal`/`-shm` companions; a document is one file that Finder can copy at any time. DELETE + EXTRA + fullfsync stays. |
| `synchronous=NORMAL` | Rejected | Weaker durability than the current EXTRA + fullfsync for user documents. |
| `auto_vacuum=INCREMENTAL` | Rejected | `FULL` already reclaims pages on commit; incremental needs an explicit step nobody would run. |
| `WITHOUT ROWID` resources | Rejected | Incremental BLOB I/O needs a rowid. |
| Sidecar files for large media | Rejected | Breaks the self-contained `.slop`; budgets (≤ 180 MiB) are in the range where in-file blobs are reasonable. |
| A `sha256` column on resources | Rejected | Duplicates content-addressed keys; SQLite `cell_size_check` and `quick_check`, plus first-touch attachment verification, cover integrity. |
| `zeroblob` + `blob_write` streaming inserts | Deferred | Pack holds at most 50 MiB of app resources and attachments at most 10 MiB; revisit if budgets grow. |
| SQLite JSONB, `sqlite-zstd`, a compression VFS | Rejected | Nothing queries JSON in SQL; another codec fights range reads. |
| rkyv/bitcode/FlatBuffers for the descriptor | Rejected | A binary blob adds a codec without removing the recursive concept. |
| Grapheme-cluster string lengths | Rejected | Segmentation changes with Unicode versions; acceptance would depend on the build’s tables. Code points are stable. |
| Nearest-neighbor skin filtering | Rejected | Keeps 1× skins looking blocky; 2× skins solve the problem. |
| A per-pixel mask bitset | Already done | `SlopWindowMask.AlphaMap` caches an 8-bit alpha buffer with O(1) lookup. |
| `mime_guess`, the `image` crate | Rejected | The build knows each media type; `image` is heavy for a PNG check the `png` crate does. |
| Zod, Valibot, ArkType for arguments | Rejected | Reintroduces a TypeScript schema authority; `s.*` descriptors are the model. |
| A Rust JSON Schema checker (`jsonschema`, `boon`) for arguments | Superseded | Arguments are descriptors checked by the core’s own checker; JSON Schema is output only. |
| Emitting JSON Schema from Rust types (schemars) | Rejected | Would reintroduce the validators this plan removes. |
| specta 2.0.0-rc.25, typeshare | Rejected | Spike: no literal types, BigInt/Value refusals, a Swift exporter crash on tagged enums; typeshare cannot express internally tagged data enums. |
| Command protocol 2 | Rejected for this reset | Nothing has shipped; all markers stay at their baselines. |
| Two markers owning different tables (`packageFormat` for app tables, storage version for the rest) | Superseded (second review) | One marker, the storage version, owns the whole physical layout; the package format owns interpretation. A column change raises both. One expected layout per storage version instead of a matrix. |
| An up-front staging manifest consumed by every build mode | Superseded (second review) | Its contents exist only after the modes run; a shared resolver and registry fill during the build, and `BuildInput` is finalized afterwards. |
| Unvalidated page command calls (“typed code is not a trust boundary”) | Rejected (second review) | TypeScript cannot express `minLength`, integer bounds or input-derived values; page calls are checked by the owner before the body runs (in the third review, by owner-routed execution rather than `commands.check`). |
| A TypeScript copy of the descriptor checker for page calls | Rejected | It would duplicate the core’s rule; one message to the Rust owner is enough, including in development. |
| Artwork replacement by upsert (`ON CONFLICT … DO UPDATE`) | Replaced (second review) | Resource rows are never updated; artwork is replaced by DELETE + INSERT in one transaction. |
| UPDATE guards keyed on `OLD.kind`; DELETE triggers as the only replacement guard | Replaced (second review) | Reproduced bypasses: an artwork row updated into a command row; `INSERT OR REPLACE` replacing attachment and template app rows, because REPLACE does not fire DELETE triggers while `recursive_triggers` is off. Guards are BEFORE INSERT existence checks plus unconditional no-UPDATE. |
| `PRAGMA recursive_triggers = ON` to make REPLACE fire DELETE triggers | Rejected | A per-connection setting the guarantee would silently depend on; existence checks in BEFORE INSERT triggers hold on every connection. |
| Importer-supplied attachment media types (`File.type`) | Replaced (second review) | Can be empty, wrong or hostile, and identical bytes would not imply one type. Rust sniffs the type from the bytes against a passive allowlist; anything else is `application/octet-stream`. |
| `script-src slop:` for native pages | Replaced (second review) | Would let attachment bytes load as scripts. Script, worker, style and font sources are path-scoped to `/__shell__/` and `/assets/` (or attachments move to a separate host if WebKit does not match paths). |
| `png::Limits { bytes: 4 · pw · ph }` with a full-frame decode | Replaced (second review) | A 16-bit RGBA frame is 8 bytes per pixel, and `Limits` does not bound the caller’s output buffer. Validation streams rows with checked header arithmetic and an 8 MiB decoder bound. |
| The catalog running full acceptance (or a summary that implies validity) | Replaced (second review) | `summary` is cheap and certifies only readability and presentable metadata; `open`/`create`/`check` run full acceptance; the types are distinct. |
| `theme` and `commands` tables; window columns with variant CHECKs | Superseded (third review) | Immutable parts of one definition, loaded together and never queried individually. Under the single-marker rule, columns would make every interpreted change a storage migration; they live in `definition_json`, so such changes are package-format changes only. Five tables remain. |
| `commands.check` before page-side evaluation | Superseded (third review) | The owner executes every command in the restricted evaluator and validates arguments there; there is no page-side evaluation left to pre-check. |
| A custom resolver and resource registry across three build modes | Superseded (third review) | Vite already resolves imports, CSS `url()`s and assets. Two Vite builds share a content-hash key function, and the declaration is evaluated by running build 2 in the restricted evaluator. |
| “Scanning outputs for `/assets/` URLs is exact” | Retracted (third review) | Strings can be escaped, computed or unrelated to loads. Emitted bundle metadata and explicit declaration references are the sources of truth; the scan only catches references to keys no build emitted. |
| UTF-16 code units for `minLength`/`maxLength` | Rejected (third review) | JSON Schema counts code points; UTF-16 bounds would make the descriptor and its projection disagree (`"😀"`). Caret and edit offsets keep UTF-16 because they are positions, not field constraints. |
| Byte equality with TypeBox-era argument schemas | Replaced (third review) | Pre-launch output is not a contract; a differential test against a JSON Schema validator checks meaning. |
| A `capabilities: []` placeholder field | Deferred (third review) | Reserving storage space is unnecessary with `definition_json`: a field is added with its package format when its vocabulary and behavior are designed, and its acceptance freezes with that format. |
| One persistent JavaScript realm per document | Rejected (third review) | Module state would survive between calls and could change a stale-retry result. Each evaluation gets a fresh runtime. |
| A warm evaluator process | Deferred (third review) | A reusable process that still creates a fresh runtime per evaluation is the first remedy if measured latency misses the gate; it is not built speculatively. |
| Keeping executable `run` bodies or the callback command form in the first UI ABI | Set aside after compatibility review and native-dev spike | Ship stubs only. Deferring this would preserve a second executable command form in ABI 1 and weaken the single-runner boundary. |
| WASM dev owner plus an evaluator HTTP endpoint | Superseded by the native-dev spike | The CLI already ships the native engine. A real temporary document exercises one owner, storage, attachments and command coordinator with responsive HMR; no cross-engine dev command coordinator is needed. |
| Rowid-blind insert guards | Replaced (third review) | `INSERT OR REPLACE` with an existing row’s explicit rowid deleted an app resource. Guards test `rowid = NEW.rowid`, and an AFTER INSERT guard refuses rowids below 1. |
| Ending PNG validation at the last row | Replaced (third review) | `finish()` checks the trailing chunks and `IEND`; a consumed-input check refuses bytes after `IEND`; interlaced images are accepted without counting rows; APNG is refused. |
| One `resources` table with kind-based guards | Replaced (fourth review) | Distinct immutable assets, immutable retained attachments and replaceable artwork use separate tables and one reader. The app row seals assets in templates and documents. |
| Resource-pointer columns (`ui_key`, `style_key`, `command_key`, `skin_key`) | Removed (fourth review) | Entry keys are fixed; the skin key belongs to the window variant. Rust checks resource existence without storing redundant pointers. |
| Experimental CLI-only commands and unvalidated page execution | Rejected by the user (fourth review) | Keep one owner-routed runner and released-command compatibility. Ordinary change collectors do not replace command draining, version guards or stale retries. |
| Artwork upsert ban and kind/rowid trigger predicates | Superseded (fourth review) | Artwork is separate and keeps its upsert; the asset seal refuses every insert after packing, including replacement by rowid. |
| Page-size benchmark and speculative storage-2 replay as launch gates | Deferred (fourth review) | Use 4096 now; add replay and migration tests with the first actual storage revision. Exact current-layout checking remains. |
| Dropping the separate-origin attachment fallback without testing WebKit | Rejected (fourth review) | Prefer same-origin paths; retain the fallback if required, with host-managed CORS and canvas/export tests. |

## Appendix B. Review history (superseded where noted above)

These records describe the design at each review. Later decisions in Appendix A and
the current sections above supersede intermediate table, trigger and launch-gate choices.

Revision summary (decisions taken in review, 2026-10-06):

- Command arguments are written with `s.*` and stored as args-subset **descriptors**, validated by the core's existing descriptor checker. JSON Schema is only a projection for agents. TypeBox and the `jsonschema` crate leave the product entirely (§3, §8).
- User attachments are served to pages by URL with byte ranges (`slop://app/attachments/<id>`) instead of base64 over the WebKit bridge (§7).
- PNG skins may be exactly 1× or 2× their declared point size; the scale is inferred (§6).
- Catalog metadata and resource pointers are columns; the rest of the app definition (window, theme, commands, descriptor) is one bounded `definition_json` value (§9). (Revised in the third review: this replaces `theme` and `commands` tables.)
- SQL carries structural CHECKs and immutability triggers; policy limits stay in Rust (§9).
- Vite is the only resource resolver, across two builds; the declaration is evaluated in the restricted evaluator (§4). (Revised in the third review: this replaces a custom resolver and registry, which had replaced an up-front staging manifest.)
- Commands from the page and the CLI execute through the owner in the restricted evaluator (§8). (Added in the third review.)
- Page size is chosen by a recorded benchmark (§9).
- The command protocol stays at 1: nothing has shipped (§10).
- Feedback considered and rejected is recorded with reasons in Appendix A.

Second review (2026-10-06). Each finding was checked against the code; the SQL findings were reproduced in an in-memory SQLite 3.51.0:

1. Page command calls validate their arguments before execution, through the Rust owner, so there is no second validator (§3). (Superseded in the third review: the owner now executes every command, so validation happens there by construction.)
2. The staging manifest is replaced by a resource registry that the build modes fill, with `BuildInput` finalized after compilation. The command boundary is restated, and this gate becomes implementation step 1 (§4, §10). (The registry was in turn superseded in the third review by Vite as the only resolver.)
3. The first trigger set was bypassable. `UPDATE` changed an artwork row into a command row, and `INSERT OR REPLACE` replaced attachment and template app rows: REPLACE’s implicit delete does not fire DELETE triggers while `recursive_triggers` is off. Resources are now never updated, inserts refuse existing keys, and artwork is replaced by delete plus insert (§9).
4. Attachments by URL needed an execution policy. The CSP allowed scripts from all of `slop:`, and an HTML/SVG attachment shown on the app origin would share the app’s bridge. Script sources are now path-scoped, attachment media types are sniffed by Rust against a passive allowlist, and attachment responses are `nosniff` and sandboxed (§5, §7).
5. PNG validation decodes row by row with checked arithmetic. A 16-bit RGBA frame needs 8 bytes per pixel, and `png::Limits` does not bound the caller’s output buffer (§6).
6. The catalog summary is separated from full acceptance (§9).
7. One marker, the storage version, owns the whole physical layout; the package format owns interpretation (§9, §10).

Third review (2026-10-06). The reviewer also weighed an outside proposal. Findings were checked against the code, the pinned `png` 0.18.1 source and an in-memory SQLite 3.51.0:

1. **One `definition_json` value** replaces the `theme` and `commands` tables, and the window columns and their CHECKs. Under the single-marker rule every column is layout (storage version), while fields inside the definition are interpretation (package format only), so a new interpreted field never needs a storage migration. Five tables remain (§9).
2. **Owner-routed commands.** Page and CLI calls go to the owner, which validates the arguments, runs the command in the restricted evaluator (a fresh runtime per evaluation), validates the intents and applies them. The app ships the evaluator as a signed helper, and `slop dev` runs the same evaluator. The round-two `commands.check` request is superseded (§3, §8, §10).
3. **Vite is the only resolver.** Two Vite builds share one content-hash key function. The declaration is evaluated by running the command bundle in the restricted evaluator, which also proves headless initialization. Resources come from Vite’s emitted bundle metadata plus the declaration’s explicit references. The earlier claim that scanning outputs for `/assets/` is exact is retracted; the scan is supplementary validation (§4).
4. **String bounds stay in code points.** JSON Schema `minLength`/`maxLength` count code points, so UTF-16 bounds would make the projection disagree with the descriptor (§3).
5. **Projection tests are semantic.** A differential test against a JSON Schema validator replaces byte equality with the TypeBox-era output (§3, §11).
6. **The resource triggers had a rowid bypass.** `INSERT OR REPLACE` with an existing app row’s explicit `rowid` and a new attachment key deleted the app row. Guards now also refuse rowid collisions and nonpositive rowids (§9).
7. **PNG validation now completes the stream:** `finish()` after the last row, fully consumed input, interlaced images accepted without counting rows, APNG refused (§6).


Fourth review decisions (2026-10-06): split tables by lifecycle and seal assets with the app row; fixed entry keys replace pointer columns; store commands only when present; Rust routes all page messages; centralize media metadata while keeping separate app/attachment policies; defer the page-size benchmark and migration replay. The user explicitly retained the shared command runner and its compatibility contract. The experimental CLI-only proposal is not adopted.
