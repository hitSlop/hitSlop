# CLI reference

The [public CLI workflows](../../apps/landing/src/content/docs/docs/guides/cli-workflows.mdx)
show how to create, preview, register, edit, theme, attach and export. This page is the
reference behind them: operation shapes, ownership, tool identity and the skills build.

## Entry points

| Entry point | Use |
| --- | --- |
| `bunx @hitslop/pop COMMAND` | Run the published CLI without installing globally. |
| `pop COMMAND` | Run after `bun install -g @hitslop/pop`, with Bun's bin directory on PATH. |
| `bun pop COMMAND` | Run from this repository after [development setup](development.md). |

`pop-engine` ships with the CLI for macOS and Linux. The executable creates, reads and edits documents without Bun,
WebKit or a running app. On both platforms it also exports a closed document as PNG/PDF and
renders template artwork, in its own restricted evaluator. The TypeScript CLI runs only
the engine; opening a window is the one operation that starts dotpop.app.
`pop check` and `pop build` use the CLI's engine for validation and packing, and on both
platforms `pop build` and `pop register` render any Preview or Icon artwork the project
does not supply inside the same `pack` request (see
[Finder artwork](../../apps/landing/src/content/docs/docs/guides/build-and-share.mdx#finder-artwork)).
Add `--help` to a CLI command for its arguments. Package versions here reflect repository
metadata, not npm availability; see [releasing](releasing.md).

## Operations

`apply` takes one operation and `batch` an array committed all-or-nothing. A path walks the schema from the root: field names and record keys are strings, rows are `{"id": "$id from get"}`, and elements of a scalar list are `{"index": n}`. Never use array positions as row identity. [Document types](../reference/document-types.md) lists every kind's operations. Both print `{ids}`, the inserted row IDs; `pop get` prints the value. A refused batch exits 1 and prints `Refused ops[N] (reason): message` on stderr, naming the zero-based index of the operation to fix, then whether anything was applied.

A text `set` may carry `from`, the field's text as you read it (`pop get --snapshot`). The set then changes the field from that text and merges with edits made since, such as typing in an open window, instead of replacing them. Pass it whenever you rewrite text you read. Without it, a text `set` replaces the field as it is when the owner applies it, as `replace` and `pop import` do. Other operations are unaffected.

| Operation | Shape | Targets |
| --- | --- | --- |
| `set` | `{"type":"set","path":[...],"value":v}` | Scalars (within their bounds), optional values and objects, whole text fields (merged from `from` when given, else replacing the text as it is when the owner applies it), record entries (`[...,"key"]` creates or replaces), scalar list elements (`[...,{"index":n}]`) and whole scalar lists |
| `clear` | `{"type":"clear","path":[...]}` | Optional fields and record entries; clearing an unset one does nothing |
| `insert` | `{"type":"insert","path":[...],"value":v,"id":"optional","at":{"after":"$id"}}` | Object-row lists; supply `id` for an insert you may retry |
| `insert` | `{"type":"insert","path":[...],"value":v,"index":0}` | Scalar lists; omit `index` to append |
| `remove` | `{"type":"remove","path":[...],"id":"$id"}` | Rows |
| `remove` | `{"type":"remove","path":[...],"index":0,"count":1}` | Scalar lists; `count` defaults to 1 |
| `move` | `{"type":"move","path":[...],"id":"$id","at":{"before":"$id"}}` | Rows; omit `at` to move to the end |
| `increment` | `{"type":"increment","path":[...],"by":1}` | Counters (negative values decrement) |
| `replace` | `{"type":"replace","path":[...],"value":v}` | Any value; an empty path is the whole document. Only the differences are written: rows match by `$id` (a row without one is new), kept rows and text keep their identity, a counter takes the value within its [exactness envelope](../reference/document-types.md#counter) (about 9×10^15 in summed absolute increments; beyond it off-by-one is possible and exactness is not guaranteed), and an optional or record entry the value leaves out is removed |

`pop import PATH FILE [--path JSON]` sends one `replace` with the file's JSON (up to just
under 1 MiB), so `pop get` output, edited or from another document of the same
template, can be written back. To move data between documents of different templates,
map it to one `batch`, supplying row `id`s so a retried batch is refused as a duplicate.

## Ownership and retries

An OS lock on the document's file in the account's registry (`~/.pop/live`) decides ownership. Closed editing runs the Rust owner in the engine process, without WebKit or authored app code. Busy documents route through their owner's Unix socket, which lives as long as the owner. Missing or failed discovery never permits a second writer. Each request names the command protocol it is written in; the owner checks it before anything else. An `export` probes the writer lock before trusting discovery, clearing a crashed owner’s discovery while holding the lock. Rendering reads an independent saved source without holding the writer lock, so a closed export also works while another process holds the document without publishing a session.

Successful mutations acknowledge persistence. Generic edits are never automatically replayed and have no public retry flags. Named commands may reevaluate once after a definite pre-admission conflict, keeping their original time and seed. After an unknown outcome, run `pop get` before issuing another edit. A live `get` saves and returns owner-accepted state; text still being typed in an open window is not included. Edit ▸ Undo in the window reverts CLI edits made while the document is open, consecutive generic edits as one step and each named command as its own step. Save failures return an error. Theme, attachment and export commands follow the same rules; the socket deadlines are in the [runtime reference](../reference/runtime.md#security-boundaries).

## Engine requests

`pop-engine --client-protocol N` reads one `EngineRequest` (`crates/pop-contracts/src/wire/engine.rs`, exported to TypeScript by ts-rs) from standard
input, bounded at 16 MiB, and prints one `EngineReply` line. The core keeps the 1 MiB
limit for ordinary owner requests and the app metadata limit for `validateApp`.
A request names no protocol: the engine adds the one it was called with. A success carries its
method and required result fields (`EngineReply` in `crates/pop-contracts/src/wire/engine.rs`); the CLI treats a success
without it as an unknown outcome. A refusal is a reply with
`ok: false`, an outcome `code` and, for a refused edit, the core's `reason` and `opIndex`.
`rejected`, `owner_replaced`, `closing` and `owner_invalidated` were not applied;
`save_failed` was applied but not saved; after `unknown_outcome`, run `get` before another
edit. The executable exits non-zero only when it printed no reply. Files are the caller's: the
CLI reads an attached file or theme file itself and writes exported bytes. An export
never replaces a file, including the document under another spelling of its path; to
export again, remove the old file first.

## Engine discovery and identity

The CLI uses its own packaged engine for both authoring and document commands, on macOS
and Linux. A checkout uses `target/release/pop-engine` (or the profile
`POP_CARGO_PROFILE` names). `POP_ENGINE` selects an explicit engine; a missing or
non-executable override fails, and an engine that ran is never retried through another
binary. File markers protect saved documents; the exact command protocol protects
communication with a live owner.

`open` (macOS only) is the one operation that starts dotpop.app.
`open` refuses a template, a link and any file that is not a regular file; a linked parent folder is fine.
It opens the document with `/usr/bin/open -a` in the app that contains the running
engine, else the app `POP_APP` names, else `/Applications/dotpop.app`, then
`~/Applications/dotpop.app`; a `POP_APP` that names no app is an error rather than a
cue to try another copy. A document that is already open in the app is edited and
exported by that app's Rust owner, which the engine reaches through its socket. The owner saves accepted edits and captures an independent source, then renders
through the same capture path as a closed export. CLI export does not drain unsent page text. Everything
else runs in the engine, including a closed document's export and the artwork a build
renders. The one other thing read from an app is the bundled starter templates:
`templates` and `create --from` list them from an app found the same way (a
`POP_APP` without starters is skipped).
The Mac app contains its linked core and bundles the same `pop-engine` executable
that ships in npm. Restricted children start through `--evaluate-command`.

The CLI and Mac app update separately. The CLI names its command protocol on every
engine call (`--client-protocol VERSION`), and the engine names it in every request to a
live owner. An engine or owner that does not serve it refuses before document
access, with exit status 2 or `requires_update`, and says which side to update; a
document command that names no protocol is a usage error. The engine reports the protocol
it serves with `--protocol` (`{"version":N}`). A release builds the app and the engine
from one core: `pop-engine --build-id` and the app executable's `--build-id` print it,
and release acceptance requires them to match.
A CLI-only release may reuse an installed app while the protocol and
[installed-consumer checks](releasing.md) pass.

## File engine

`pop-engine` (`crates/pop-engine`) accepts the JSON `EngineRequest` methods above,
not CLI subcommands. `validateApp` checks evaluated app input, `validateMetadata` checks
new-project metadata, and `pack` writes an immutable template from a build stage; with
`renderArtwork` it also renders the Preview and Icon artwork the build does not supply,
in the same request (the CLI asks for it on macOS and Linux).
`templates` returns the catalog's folders, accepted templates and per-file issues;
`create` takes `from` and `output`, creates parent folders and returns `documentPath`.
A bare template slug resolves installed templates before bundled ones.

`inspect` returns file kind, requirements, metadata, window, resource/state sizes and
the app digest (equal digests mean the same app);
`schema` reads its stored descriptor. `get`, `describe`, `batch` and `call` route through
the live or in-process owner and wait for persistence as required. `artwork.export` exports stored artwork. `attachments.ref` reads a local regular
file and returns its hash, byte length, name and detected MIME type without opening a
document. `export` renders a closed document in the engine, and routes an open one to its
owner on macOS. Closed export also works on Linux; `open` is macOS only and asks
LaunchServices to open the document in dotpop.app. See the Rust `EngineRequest` enum for the complete method and field inventory.

The executable flags are `--client-protocol N`, `--protocol`, `--build-id`, the restricted
`--evaluate-command` entry point and `--preview-owner PATH` (with a client protocol).
There are no `validate-app`, `request` or `create --from` engine subcommands; those are
JSON methods or public `pop` workflows. Checkpoint/retention maintenance is automatic,
with no public compaction command.

## Agent skills

The TypeScript CLI uses Crust for command parsing, help and packaged agent skills.
`bun run skills:build` (also part of `bun run build`) packages the authored `pop`,
`pop-authoring`, `pop-design` and `pop-document` guides from
`packages/pop/skills`, plus the generated `pop-cli` command reference, into the
untracked `packages/pop/.crust/root/skills`. Rebuild after changing command metadata or
authored guidance.

Global skills come from the global install; `bun install -g @hitslop/pop` upgrades their
content through stable links. `skills repair` changes links only when explicitly invoked.
Bunx copies may uninstall global links but do not install them.

`pop skills --scope project` installs the pinned project's guides through unresolved
`node_modules/@hitslop/pop` paths, so Bun can replace its package-cache target during upgrades.
Run `bun install` first. `init` creates local links that become usable after installation.
`--scope project` also supports repair and uninstall; ordinary files are never replaced.

`bun run packages:pack` creates `generated/npm/pop-VERSION.tgz` for dogfooding. Install
that one tarball globally or in a temporary project; no unpublished dependency overrides
are needed.

## Project version selection

Use `bun run check`, `bun run dev` and `bun run build` in a project. These scripts run
its pinned package. `pop check` runs the project's own svelte-check and TypeScript, which
the starter pins as devDependencies; a project without them is told what to install. A
global authoring command refuses a different installed project version and directs you to
those scripts. `pop --project=DIR build .` explicitly delegates to that install. Merely
changing cwd never redirects document commands.

## Domain commands

`pop describe PATH` prints fields, allowed operations, named commands, values, row IDs
and the snapshot version. Add `--json` for its complete structured representation.
`pop call PATH NAME --args JSON` invokes the command stored inside that document, using
Rust descriptor argument validation and one atomic owner batch. A success is durable. A refusal
applies no collected edits; an unknown outcome is never automatically replayed.

The runner retries one definite stale snapshot conflict using the original clock and
random seed. A second conflict returns `stale_base`. Commands have their own undo steps;
`apply`, `batch`, handles and `change()` remain available for other edits.

For an OS-assigned preview port, use `pop dev <source> --port 0` and read the URL
printed at startup. Omitting `--port` still uses 5173.
