# CLI reference

The [public CLI workflows](../../apps/landing/src/content/docs/docs/guides/cli-workflows.mdx)
show how to create, preview, register, edit, theme, attach and export. This page is the
reference behind them: operation shapes, ownership, tool identity and the skills build.

## Entry points

| Entry point | Use |
| --- | --- |
| `bunx @hitslop/cli@4.0.0 COMMAND` | Run the CLI matching this checkout's SDK without installing globally. |
| `slop COMMAND` | Run after `bun install -g @hitslop/cli@4.0.0`, with Bun's bin directory on PATH. |
| `bun slop COMMAND` | Run from this repository after [development setup](development.md). |
| `"/Applications/hitSlop.app/Contents/Helpers/slop-engine" request` | Send one document request directly, without Node or Bun ([helper requests](#helper-requests)). |

`slop-engine` ships with the CLI for macOS and Linux, and beside `hitslop-native` in
`hitSlop.app/Contents/Helpers`. It creates, reads and edits documents without Bun,
WebKit or a running app. The TypeScript CLI sends document requests to the engine;
opening a window, PNG/PDF export and native template artwork use the Swift helper.
`slop check` and `slop build` use the CLI's engine for validation and packing. Add
`--help` to a CLI command for its arguments. Package versions here reflect repository
metadata, not npm availability; see [releasing](releasing.md).

## Operations

`apply` takes one operation and `batch` an array committed all-or-nothing. A path walks the schema from the root: field names and record keys are strings, rows are `{"id": "$id from get"}`, and elements of a scalar list are `{"index": n}`. Never use array positions as row identity. [Document types](../reference/document-types.md) lists every kind's operations. Both print `{ids, sequence}`: `ids` lists inserted row IDs, and `slop get` prints the value.

| Operation | Shape | Targets |
| --- | --- | --- |
| `set` | `{"type":"set","path":[...],"value":v}` | Scalars (within their bounds), optional values and objects, whole text fields (the text as it is when the owner applies it), record entries (`[...,"key"]` creates or replaces), scalar list elements (`[...,{"index":n}]`) and whole scalar lists |
| `clear` | `{"type":"clear","path":[...]}` | Optional fields and record entries; clearing an unset one does nothing |
| `insert` | `{"type":"insert","path":[...],"value":v,"id":"optional","at":{"after":"$id"}}` | Object-row lists; supply `id` for an insert you may retry |
| `insert` | `{"type":"insert","path":[...],"value":v,"index":0}` | Scalar lists; omit `index` to append |
| `remove` | `{"type":"remove","path":[...],"id":"$id"}` | Rows |
| `remove` | `{"type":"remove","path":[...],"index":0,"count":1}` | Scalar lists; `count` defaults to 1 |
| `move` | `{"type":"move","path":[...],"id":"$id","at":{"before":"$id"}}` | Rows; omit `at` to move to the end |
| `increment` | `{"type":"increment","path":[...],"by":1}` | Counters (negative values decrement) |
| `replace` | `{"type":"replace","path":[...],"value":v}` | Any value; an empty path is the whole document. Only the differences are written: rows match by `$id` (a row without one is new), kept rows and text keep their identity, a counter adds the difference, and an optional or record entry the value leaves out is removed. Refused where it would overwrite a stored anomaly |

`slop import PATH FILE [--path JSON]` sends one `replace` with the file's JSON (up to just
under 1 MiB), so `slop get` output, edited or from another document of the same
template, can be written back. To move data between documents of different templates,
map it to one `batch`, supplying row `id`s so a retried batch is refused as a duplicate.

## Ownership and retries

An OS lock on the document's file in the account's registry (`~/.hitslop/live`) decides ownership. Closed editing runs the Rust owner in the engine process, without WebKit or authored app code. Busy documents route through their owner's Unix socket, which lives as long as the owner. Missing or failed discovery never permits a second writer. Each live connection starts with a small `hello` handshake that returns the owner's core build and epoch, without a document snapshot. A closed `export` renders the saved state without taking the lock, so it works while another process holds the document.

Successful mutations acknowledge persistence. No automatic replay or public retry flags exist. After an unknown outcome, run `slop get` before issuing another edit. A live `get` saves and returns owner-accepted state; text still being typed in an open window is not included. Edit ▸ Undo in the window reverts CLI edits made while the document is open, the consecutive ones as one step. Save failures return an error. The owner's epoch rotates when unsaved edits are discarded, so a request aimed at replaced state is refused. Theme, attachment and export commands follow the same rules; the socket deadlines are in the [runtime reference](../reference/runtime.md#security-boundaries).

## Helper requests

`slop-engine request` reads one `HelperRequest` (`@hitslop/schema/socket`) from standard
input, at most 1 MiB (16 MiB for an attachment upload), and prints one `SocketReply` line.
A request names no epoch: the Rust router supplies the live owner's. A success carries its
method and required result fields (`SocketReply` in `@hitslop/schema/socket`); the CLI treats a success
without it as an unknown outcome. A refusal is a reply with
`ok: false`, an outcome `code` and, for a refused edit, the core's `reason` and `opIndex`.
`rejected`, `owner_replaced`, `closing` and `owner_invalidated` were not applied;
`save_failed` was applied but not saved; after `unknown_outcome`, run `get` before another
edit. The executable exits non-zero only when it printed no reply. Files are the caller's: the
CLI reads an imported attachment or theme file itself and writes exported bytes. An export
never replaces a file, including the document under another spelling of its path; to
export again, remove the old file first.

## Helper discovery and identity

`HITSLOP_ENGINE` selects an explicit engine and takes precedence for document commands
and authoring. Without it, macOS document commands first use the app's engine in
`/Applications/hitSlop.app`, then `~/Applications/hitSlop.app`, then the CLI's platform
engine or the checkout's `target/release/slop-engine`. Linux uses the CLI or checkout
engine. Authoring validation and packing always select the CLI engine, independently of
an installed app, unless `HITSLOP_ENGINE` is set.

`HITSLOP_NATIVE_CLI` selects an explicit rendering helper. On macOS, when that override
is set without `HITSLOP_ENGINE`, document commands require `slop-engine` beside the
selected helper; they do not fall back to another deployment. `bun run build` places the
engine beside the Debug helper. Missing or non-executable overrides fail, and an
executed tool is never retried through another binary. Native helper discovery otherwise
checks the same two app locations. `open`, `export`, `build --artwork native` and
`register` require macOS and the native helper; document creation and editing do not.

The CLI and Mac app update separately. Both executables report their command protocol
range with `--protocol` (`{"version":N,"minimum":M}`). The CLI checks that range before
running document commands or native rendering and supplies `--client-protocol VERSION`.
Unversioned calls mean protocol 1; unsupported selections fail before document access.
The selected engine or helper then checks a live owner's exact core build through
`hello`. The app bundles matching engine, helper and owner builds; quit an older running
app and reopen with the installed one if they differ. `slop-engine --build-id` and
`hitslop-native --core-build` print that identity. A CLI-only release may reuse an
installed app while the protocol and [installed-consumer checks](releasing.md) pass.

## File engine

`slop-engine` (`crates/slop-engine`) is the native build of the shared Rust core.
`validate-app` checks bounded evaluated app JSON on standard input. `pack` turns a build
stage into an immutable template. `create --from TEMPLATE --output DOCUMENT` creates a
writable copy, makes missing parent folders and prints its resolved path.

`inspect` prints a file's kind, requirements, manifest, asset/artwork/attachment and
saved-state sizes, and whether an owner published its socket. `schema` prints the
stored descriptor. These two commands read the saved file; `request` routes through the
live or in-process owner and waits for persistence where its method requires it.
Checkpoint and retention maintenance are automatic; there is no public compaction
command.

## Agent skills

The TypeScript CLI uses Crust for command parsing, help and packaged agent skills.
`bun run skills:build` (also part of `bun run build`) packages the authored `hitslop`,
`hitslop-authoring`, `hitslop-design` and `hitslop-document` guides from
`packages/cli/skills`, plus the generated `hitslop-cli` command reference, into the
untracked `packages/cli/.crust/root/skills`. Rebuild after changing command metadata or
authored guidance.

Install, repair and uninstall are described in the public workflows. Details they leave
out:
- `skill` is an alias of `skills`.
- Scope defaults to global (Crust's `defaultScope`), so the installer does not ask for one;
  pass `--scope project` for project links. `--all` skips conflicting real directories;
  interactive replacement asks first.
- Installed links target the global install's packaged skills, under
  `$BUN_INSTALL/install/global/node_modules/@hitslop/cli` (or `BUN_INSTALL_GLOBAL_DIR`).
  `bun install -g` replaces that directory in place, so links survive upgrades and serve
  the new content.
- Only the global install installs or repairs links, and only when asked: `skills repair`
  fixes a link that is dangling or points elsewhere. Crust's `autoUpdate` never runs here,
  because Crust skips it for CLIs run from source rather than a `crust build` bundle.
  bunx and project copies refuse `skills install` and `skills repair` and never rewrite
  links; any copy can uninstall.
- To dogfood unreleased skills, run `bun run packages:pack` and install
  `generated/npm/hitslop-cli-VERSION.tgz` with `bun install -g`. While the matching
  `@hitslop/document` and `@hitslop/schema` are unpublished, first list their tarballs as
  `overrides` in `$BUN_INSTALL/install/global/package.json`, as `scripts/packed-test.ts`
  does.
