# CLI reference

The [public CLI workflows](../../apps/landing/src/content/docs/docs/guides/cli-workflows.mdx)
show how to create, preview, register, edit, theme, attach and export. This page is the
reference behind them: operation shapes, ownership, tool identity and the skills build.

## Entry points

| Entry point | Use |
| --- | --- |
| `bunx hitslop@1.0.0 COMMAND` | Run the CLI matching this checkout's SDK without installing globally. |
| `slop COMMAND` | Run after `bun install -g hitslop@1.0.0`, with Bun's bin directory on PATH. |
| `bun slop COMMAND` | Run from this repository after [development setup](development.md). |

`slop-engine` ships with the CLI for macOS and Linux. The executable creates, reads and edits documents without Bun,
WebKit or a running app. The TypeScript CLI runs only the engine; opening a window,
PNG/PDF export and native template artwork run in the Swift helper, which the engine
passes them to.
`slop check` and `slop build` use the CLI's engine for validation and packing. Add
`--help` to a CLI command for its arguments. Package versions here reflect repository
metadata, not npm availability; see [releasing](releasing.md).

## Operations

`apply` takes one operation and `batch` an array committed all-or-nothing. A path walks the schema from the root: field names and record keys are strings, rows are `{"id": "$id from get"}`, and elements of a scalar list are `{"index": n}`. Never use array positions as row identity. [Document types](../reference/document-types.md) lists every kind's operations. Both print `{ids}`, the inserted row IDs; `slop get` prints the value. A refused batch exits 1 and prints `Refused ops[N] (reason): message` on stderr, naming the zero-based index of the operation to fix, then whether anything was applied.

Both take `--base VERSION`, the `version` of the `slop get --snapshot` you read the text with. Text `set`s in the batch then change each field from its text at that version and merge with edits made since, such as typing in an open window, instead of replacing them. Pass it whenever you rewrite text you read, and read again before each rewrite. Without it, a text `set` replaces the field as it is when the owner applies it; `replace` and `slop import` take no base. Other operations are unaffected. A version older than the document's kept history is refused as `stale_base`: read again and redo the rewrite.

| Operation | Shape | Targets |
| --- | --- | --- |
| `set` | `{"type":"set","path":[...],"value":v}` | Scalars (within their bounds), optional values and objects, whole text fields (from `--base`, else the text as it is when the owner applies it), record entries (`[...,"key"]` creates or replaces), scalar list elements (`[...,{"index":n}]`) and whole scalar lists |
| `clear` | `{"type":"clear","path":[...]}` | Optional fields and record entries; clearing an unset one does nothing |
| `insert` | `{"type":"insert","path":[...],"value":v,"id":"optional","at":{"after":"$id"}}` | Object-row lists; supply `id` for an insert you may retry |
| `insert` | `{"type":"insert","path":[...],"value":v,"index":0}` | Scalar lists; omit `index` to append |
| `remove` | `{"type":"remove","path":[...],"id":"$id"}` | Rows |
| `remove` | `{"type":"remove","path":[...],"index":0,"count":1}` | Scalar lists; `count` defaults to 1 |
| `move` | `{"type":"move","path":[...],"id":"$id","at":{"before":"$id"}}` | Rows; omit `at` to move to the end |
| `increment` | `{"type":"increment","path":[...],"by":1}` | Counters (negative values decrement) |
| `replace` | `{"type":"replace","path":[...],"value":v}` | Any value; an empty path is the whole document. Only the differences are written: rows match by `$id` (a row without one is new), kept rows and text keep their identity, a counter takes the value, and an optional or record entry the value leaves out is removed |

`slop import PATH FILE [--path JSON]` sends one `replace` with the file's JSON (up to just
under 1 MiB), so `slop get` output, edited or from another document of the same
template, can be written back. To move data between documents of different templates,
map it to one `batch`, supplying row `id`s so a retried batch is refused as a duplicate.

## Ownership and retries

An OS lock on the document's file in the account's registry (`~/.hitslop/live`) decides ownership. Closed editing runs the Rust owner in the engine process, without WebKit or authored app code. Busy documents route through their owner's Unix socket, which lives as long as the owner. Missing or failed discovery never permits a second writer. Each request names the command protocol it is written in; the owner checks it before anything else. A closed `export` renders the saved state without taking the lock, so it works while another process holds the document.

Successful mutations acknowledge persistence. Generic edits are never automatically replayed and have no public retry flags. Named commands may reevaluate once after a definite pre-admission conflict, keeping their original time and seed. After an unknown outcome, run `slop get` before issuing another edit. A live `get` saves and returns owner-accepted state; text still being typed in an open window is not included. Edit ▸ Undo in the window reverts CLI edits made while the document is open, consecutive generic edits as one step and each named command as its own step. Save failures return an error. Theme, attachment and export commands follow the same rules; the socket deadlines are in the [runtime reference](../reference/runtime.md#security-boundaries).

## Engine requests

`slop-engine --client-protocol N` reads one `EngineRequest` (`packages/hitslop/src/schema/engine.ts`) from standard
input, bounded at 16 MiB, and prints one `EngineReply` line. The core keeps the 1 MiB
limit for ordinary owner requests and the app metadata limit for `validateApp`.
A request names no protocol: the engine adds the one it was called with. A success carries its
method and required result fields (`EngineReply` in `packages/hitslop/src/schema/engine.ts`); the CLI treats a success
without it as an unknown outcome. A refusal is a reply with
`ok: false`, an outcome `code` and, for a refused edit, the core's `reason` and `opIndex`.
`rejected`, `owner_replaced`, `closing` and `owner_invalidated` were not applied;
`save_failed` was applied but not saved; after `unknown_outcome`, run `get` before another
edit. The executable exits non-zero only when it printed no reply. Files are the caller's: the
CLI reads an attached file or theme file itself and writes exported bytes. An export
never replaces a file, including the document under another spelling of its path; to
export again, remove the old file first.

## Helper discovery and identity

The CLI uses its own packaged engine for both authoring and document commands, on macOS
and Linux. A checkout uses `target/release/slop-engine` (or `HITSLOP_CARGO_PROFILE`).
`HITSLOP_ENGINE` selects an explicit engine. File markers protect saved documents;
the exact command protocol protects communication with a live owner or rendering helper.

The CLI runs only the document engine. What needs AppKit or WebKit (`open`, `export`,
`build --artwork native` and `register`) the engine passes, unchanged, to the app's
rendering helper, so those require macOS and hitSlop.app; document creation and editing
do not. The engine finds the helper in `/Applications/hitSlop.app`, then
`~/Applications/hitSlop.app`. `HITSLOP_NATIVE_CLI` independently selects an explicit helper.
The app contains its linked core and renderer; the engine ships only in npm.
Missing or non-executable overrides fail, and an executed tool is never retried
through another binary.

The CLI and Mac app update separately. The CLI names its command protocol on every
engine call (`--client-protocol VERSION`), and the engine names it in every request to a
live owner. An engine, helper or owner that does not serve it refuses before document
access, with exit status 2 or `requires_update`, and says which side to update; a
document command that names no protocol is a usage error. Each executable reports the range it serves with
`--protocol` (`{"version":N,"minimum":M}`). The app bundles its engine, helper and owner
built from one core; `slop-engine --build-id` and `hitslop-native --core-build` print it.
A CLI-only release may reuse an installed app while the protocol and
[installed-consumer checks](releasing.md) pass.

## File engine

`slop-engine` (`crates/slop-engine`) is the native build of the shared Rust core.
`validate-app` checks bounded evaluated app JSON on standard input. `pack` turns a build
stage into an immutable template. `templates` prints the templates the app's catalog lists
as JSON: its `folders` (the starters inside the selected or installed Mac app, then the
installed folder, `HITSLOP_TEMPLATES_ROOT` or `~/.hitslop/templates`, which `slop register`
builds into), each template found there, and an issue for each `.slop` file left out.
`create --from TEMPLATE --output DOCUMENT` creates a writable copy, makes missing parent
folders and prints its resolved path; TEMPLATE is a template's path, or a listed slug when
it is a bare name, and an installed template shadows a bundled one.

`inspect` prints a file's kind, requirements, metadata, window and asset/artwork/attachment and
saved-state sizes, and whether an owner published its socket. `schema` prints the
stored descriptor. These two commands read the saved file; `request` routes through the
live or in-process owner and waits for persistence where its method requires it.
Checkpoint and retention maintenance are automatic; there is no public compaction
command.

## Agent skills

The TypeScript CLI uses Crust for command parsing, help and packaged agent skills.
`bun run skills:build` (also part of `bun run build`) packages the authored `hitslop`,
`hitslop-authoring`, `hitslop-design` and `hitslop-document` guides from
`packages/hitslop/skills`, plus the generated `hitslop-cli` command reference, into the
untracked `packages/hitslop/.crust/root/skills`. Rebuild after changing command metadata or
authored guidance.

Global skills come from the global install; `bun install -g hitslop` upgrades their
content through stable links. `skills repair` changes links only when explicitly invoked.
Bunx copies may uninstall global links but do not install them.

`slop skills --scope project` installs the pinned project's guides through unresolved
`node_modules/hitslop` paths, so Bun can replace its package-cache target during upgrades.
Run `bun install` first. `init` creates local links that become usable after installation.
`--scope project` also supports repair and uninstall; ordinary files are never replaced.

`bun run packages:pack` creates `generated/npm/hitslop-VERSION.tgz` for dogfooding. Install
that one tarball globally or in a temporary project; no unpublished dependency overrides
are needed.

## Project version selection

Use `bun run check`, `bun run dev` and `bun run build` in a project. These scripts run
its pinned package. A global authoring command refuses a different installed project
version and directs you to those scripts. `slop --project=DIR build .` explicitly delegates
to that install. Merely changing cwd never redirects document commands.

## Domain commands

`slop describe PATH` prints fields, allowed operations, named commands, values, row IDs
and the snapshot version. Add `--json` for its complete structured representation.
`slop call PATH NAME --args JSON` invokes the command stored inside that document, using
Rust descriptor argument validation and one atomic owner batch. A success is durable. A refusal
applies no collected edits; an unknown outcome is never automatically replayed.

The runner retries one definite stale snapshot conflict using the original clock and
random seed. A second conflict returns `stale_base`. Commands have their own undo steps;
`apply`, `batch`, handles and `change()` remain available for other edits.
