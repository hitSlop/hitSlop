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
| `"/Applications/hitSlop.app/Contents/Helpers/hitslop-native" COMMAND` | Run native document commands directly, without Node or Bun. |

`hitslop-native` ships in `hitSlop.app/Contents/Helpers` and links the same Rust core as
the app. Document commands need neither Bun nor a running app. The TypeScript CLI forwards
macOS document commands to it, including `create` and `open`. Add `--help`
to any command for its arguments. Package versions here reflect repository metadata, not
npm availability; see [releasing](releasing.md).

## Operations

`apply` takes one operation and `batch` an array committed all-or-nothing. A path walks the schema from the root: field names and record keys are strings, rows are `{"id": "$id from get"}`, and elements of a scalar list are `{"index": n}`. Never use array positions as row identity. [Document types](../reference/document-types.md) lists every kind's operations. Both print `{ids, sequence, value}`; `ids` lists inserted row IDs.

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

The permanent `state/writer.lock` decides ownership. Closed editing runs the native owner in the helper process, without WebKit or authored app code. Busy documents route through their owner's Unix socket, which lives as long as the owner. Missing or failed discovery never permits a second writer. A small `hello` handshake returns the owner's epoch without a document snapshot; ordinary reads need no handshake.

Successful mutations acknowledge persistence. No automatic replay or public retry flags exist. After an unknown outcome, run `slop get` before issuing another edit. A live `get` saves and returns owner-accepted state; text still being typed in an open window is not included. Edit ▸ Undo in the window reverts CLI edits too, the consecutive ones as one step, including edits made while the document was closed. Save failures return an error. The owner's epoch rotates when unsaved edits are discarded, so a request aimed at replaced state is refused. Theme, attachment and export commands follow the same rules; the socket deadlines are in the [runtime reference](../reference/runtime.md#security-boundaries).

## Helper discovery and identity

`HITSLOP_NATIVE_CLI` selects an explicit executable for both document commands and
template capture. Missing or non-executable overrides fail; an executed helper is never
retried through another binary. Discovery otherwise checks `/Applications/hitSlop.app`,
then `~/Applications/hitSlop.app`. Authoring never compiles Swift, and build and register
require an installed app. Bun is the only JavaScript runtime authoring needs; native
document editing is macOS-only, with no Bun fallback.

The CLI and the Mac app update separately. Before each native document command, the
CLI asks the helper which command protocols it serves (`hitslop-native --protocol`
prints `{"version":N,"minimum":M}`), including `HITSLOP_NATIVE_CLI` overrides, and runs
the command with `--client-protocol VERSION` when its own protocol is in that range, whatever core either embeds. Unversioned native calls mean protocol 1; unsupported selections fail before document access.
Otherwise it names the side to update. App updates keep serving older protocols:
protocol 1 is today's commands, arguments, outputs and exit statuses. The helper still
checks the live owner's exact core build (`hello`) before reading, editing or exporting
through its socket, because both ship in one app bundle; quit an older running app and
reopen with the installed one. `hitslop-native --core-build` prints that identity. A
CLI-only release may reuse an installed Mac app while it serves the CLI's protocol and
the installed-consumer checks in [releasing](releasing.md) pass.

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
- Only the global install installs or repairs links. Before each of its commands it
  repairs owned global links that are dangling or point elsewhere (Crust's `autoUpdate`);
  project links are repaired only by `skills repair --scope project`, so the guide copies
  `init` leaves in `.agents/skills` are never reported as conflicts. bunx and project
  copies refuse `skills install` and `skills repair` and never rewrite links; any copy can
  uninstall.
- To dogfood unreleased skills, run `bun run packages:pack` and install
  `generated/npm/hitslop-cli-VERSION.tgz` with `bun install -g`. While the matching
  `@hitslop/document` and `@hitslop/schema` are unpublished, first list their tarballs as
  `overrides` in `$BUN_INSTALL/install/global/package.json`, as `scripts/packed-test.ts`
  does.
