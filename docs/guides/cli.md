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
| `"/Applications/hitSlop.app/Contents/Helpers/hitslop-native" request` | Send one document request directly, without Node or Bun ([helper requests](#helper-requests)). |

`hitslop-native` ships in `hitSlop.app/Contents/Helpers` and links the same Rust core as
the app. Document commands need neither Bun nor a running app. The TypeScript CLI sends
each macOS document command to it as one request, and runs its `create` and `open`. `slop build`, `slop schema`
and `slop inspect` run the CLI's file engine instead, on any platform. Add `--help`
to any command for its arguments. Package versions here reflect repository metadata, not
npm availability; see [releasing](releasing.md).

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

An OS lock on the document's file in the account's registry (`~/.hitslop/live`) decides ownership. Closed editing runs the native owner in the helper process, without WebKit or authored app code. Busy documents route through their owner's Unix socket, which lives as long as the owner. Missing or failed discovery never permits a second writer. Each request starts with a small `hello` handshake that returns the owner's core build and epoch, without a document snapshot. A closed `export` renders the saved state without taking the lock, so it works while another process holds the document.

Successful mutations acknowledge persistence. No automatic replay or public retry flags exist. After an unknown outcome, run `slop get` before issuing another edit. A live `get` saves and returns owner-accepted state; text still being typed in an open window is not included. Edit ▸ Undo in the window reverts CLI edits made while the document is open, the consecutive ones as one step. Save failures return an error. The owner's epoch rotates when unsaved edits are discarded, so a request aimed at replaced state is refused. Theme, attachment and export commands follow the same rules; the socket deadlines are in the [runtime reference](../reference/runtime.md#security-boundaries).

## Helper requests

`hitslop-native request` reads one `SocketRequest` (`@hitslop/schema/socket`) from standard
input, at most 1 MiB (16 MiB for an attachment upload), and prints one `SocketReply` line.
A request names no epoch: the helper supplies the live owner's. A success carries its
method's result (`SocketResults` in `@hitslop/schema/socket`); the CLI treats a success
without it as an unknown outcome. A refusal is a reply with
`ok: false`, an outcome `code` and, for a refused edit, the core's `reason` and `opIndex`.
`rejected`, `owner_replaced`, `closing` and `owner_invalidated` were not applied;
`save_failed` was applied but not saved; after `unknown_outcome`, run `get` before another
edit. The helper exits non-zero only when it printed no reply. Files are the caller's: the
CLI reads an imported attachment or theme file itself and writes exported bytes. An export
never replaces a file, including the document under another spelling of its path; to
export again, remove the old file first.

## Helper discovery and identity

`HITSLOP_NATIVE_CLI` selects an explicit executable for both document commands and
template capture. Missing or non-executable overrides fail; an executed helper is never
retried through another binary. Discovery otherwise checks `/Applications/hitSlop.app`,
then `~/Applications/hitSlop.app`. Authoring never compiles Swift; `build --artwork native`
and `register` require an installed app. Bun is the only JavaScript runtime authoring needs; native
document editing is macOS-only, with no Bun fallback.

The CLI and the Mac app update separately. Before each native document command, the
CLI asks the helper which command protocols it serves (`hitslop-native --protocol`
prints `{"version":N,"minimum":M}`), including `HITSLOP_NATIVE_CLI` overrides, and runs
the command with `--client-protocol VERSION` when its own protocol is in that range, whatever core either embeds. Unversioned native calls mean protocol 1; unsupported selections fail before document access.
Otherwise it names the side to update. App updates keep serving older protocols:
protocol 1 is today's request and reply schemas and the `create`, `open` and `screenshot`
subcommands. The helper still
checks the live owner's exact core build (`hello`) before reading, editing or exporting
through its socket, because both ship in one app bundle; quit an older running app and
reopen with the installed one. `hitslop-native --core-build` prints that identity. A
CLI-only release may reuse an installed Mac app while it serves the CLI's protocol and
the installed-consumer checks in [releasing](releasing.md) pass.

## File engine

`slop-engine` (`crates/slop-engine`) is the native build of the same Rust core, for files:
`pack` turns a build's stage into a template, `inspect` prints what a `.slop` file holds
(its kind, requirements, manifest, assets, artwork, attachments, saved-state sizes, and
whether a live owner published its socket), and `schema` prints its descriptor. Reads use
the saved file, never an open window's unsaved edits. `HITSLOP_ENGINE` selects an explicit
executable; otherwise the CLI uses its platform build, then a checkout's
`target/release/slop-engine`, which `bun run build` and `bun run test` prepare.

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
