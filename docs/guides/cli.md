# CLI workflows and reference

Start with the [public CLI workflows](../../apps/landing/src/content/docs/docs/guides/cli-workflows.mdx) for common tasks. This guide adds the operation reference and contributor details.

## Choose an entry point

| Entry point | Use |
| --- | --- |
| `bunx @hitslop/cli@1.2.0 COMMAND` | Run the CLI matching this checkout's SDK without installing globally. |
| `slop COMMAND` | Run after `bun install -g @hitslop/cli@1.2.0`, with Bun's bin directory on PATH. |
| `bun slop COMMAND` | Run from this repository after [development setup](development.md). |
| `"/Applications/hitSlop.app/Contents/Helpers/hitslop-native" COMMAND` | Run native document commands directly, without Node or Bun. |

Swift `hitslop-native` ships in `hitSlop.app/Contents/Helpers` with the same pinned JS/WASM runtime as the app. Installed document commands need neither Bun nor a running app. The helper is not automatically added to PATH; invoke the bundled executable directly. For an app under `~/Applications`, use `"$HOME/Applications/hitSlop.app/Contents/Helpers/hitslop-native"`. The TypeScript CLI forwards macOS document commands to the helper.

Use `bun slop --help`, `bun slop COMMAND --help`, or the native helper's `--help` for additional commands and arguments. Package versions here reflect repository metadata, not npm availability; see [releasing](releasing.md) for the publication workflow.

## Create, preview, and register an app

With Bun 1.4.2 or newer:

```sh
bunx @hitslop/cli@1.2.0 init weekend-kit
cd weekend-kit
bun install
bun run check
bun run dev
```

In a terminal, `init` asks what your slop should do and who the author is. Title, categories, and description start as placeholders; the launched agent sets them in `manifest.json` to match what it builds, and you can edit them there anytime. It saves the build brief in `BRIEF.md` and offers to launch an agent to implement it. Choose a detected Codex, Claude Code, Gemini CLI, or OpenCode installation, **Other CLI…** to enter an executable and its prompt option, or **Finish without launching**. Detection only searches PATH. The agent runs in the project with its normal permissions; no agent is installed automatically. A failed or cancelled launch keeps the project.

For automation, supply metadata explicitly:

```sh
bun slop init budget-book --yes \
  --brief 'Track spending by category with a monthly summary.' \
  --title 'Budget Book' --slug budget-book \
  --category finance --category personal --author Jordan \
  --description 'A simple monthly spending tracker.'
```

Metadata flags set values explicitly; `--brief` and `--author` also skip their prompts. `--yes`, CI, and non-TTY runs never prompt or launch an agent. Missing metadata defaults to the directory name (truncated to the title limit), `productivity`, `Anonymous`, and `A hitSlop mini app.`; an omitted brief uses the description. Slugs are normalized from the directory name unless supplied explicitly. The complete manifest is validated before writing, and existing destinations are refused. Categories and their limits come from the platform manifest schema.

Read the generated `AGENTS.md`, `BRIEF.md`, and `manifest.json` before editing. Preview data is disposable; refreshing resets it. Restart preview after source changes. Stop it before building:

```sh
bun run build
bun run register
```

Build creates `dist/weekend-kit.slop`; register builds and installs an immutable master in `~/.hitslop/templates`. Existing documents keep their app version. Both commands require a compatible installed Mac app. Prefer the generated scripts, which pin the CLI and its required SDK separately. From the checkout, use `bun slop check SOURCE`, `bun slop dev SOURCE [--port 5174]`, `bun slop build SOURCE`, and `bun slop register SOURCE`. The preview port defaults to 5173. See [authoring](authoring.md) for source structure.

## Create and open a writable document

Choose a template and **Create** in the Mac app, or run the native helper from your authoring project:

```sh
"/Applications/hitSlop.app/Contents/Helpers/hitslop-native" create \
  --from dist/weekend-kit.slop --output "$HOME/Documents/Weekend.slop"
"/Applications/hitSlop.app/Contents/Helpers/hitslop-native" open \
  "$HOME/Documents/Weekend.slop"
```

`create` and `open` are native-only commands, not `slop` subcommands. Creation uses template starting data, refuses an existing destination, and adds `.slop` when omitted. Open launches the installed Mac app. Both print the document path. Keep writable documents outside the template cache and synced folders; never edit built or registered masters.

## Read and edit data

Read `manifest.json` first; only `hitslop-v1` is accepted. Inspect the schema and current data before choosing operations. These edits assume a text field named `title`; substitute your own document path and schema fields.

```sh
bun slop schema /path/to/List.slop
bun slop get /path/to/List.slop
bun slop apply /path/to/List.slop --op '{"type":"text.replace","path":["title"],"value":"Today"}'
bun slop batch /path/to/List.slop --ops '[{"type":"text.replace","path":["title"],"value":"Tomorrow"}]'
```

`schema` prints the descriptor; `get`, `apply`, and `batch` print JSON data. Errors go to stderr with a nonzero exit status. `get` flushes pending edits before returning. After an uncertain mutation result, read again before another edit; never blindly replay mutations.

For an explicit storage checkpoint, use `bun slop compact /path/to/List.slop`. It retains history and returns JSON state; it is not required after each edit.

## Operations

`apply` takes one operation and `batch` an array committed all-or-nothing. A path walks the schema from the root: field names are strings and rows are `{"id": "$id from get"}`. Never use array positions as row identity. Both print `{ids, sequence, value}`; `ids` lists inserted row IDs.

| Operation | Shape | Targets |
| --- | --- | --- |
| `set` | `{"type":"set","path":[...],"value":v}` | Booleans, and whole text fields (the text as it is when the owner applies it) |
| `insert` | `{"type":"insert","path":[...],"value":v,"id":"optional","at":{"after":"$id"}}` | Object-row lists; supply `id` for an insert you may retry |
| `remove` | `{"type":"remove","path":[...],"id":"$id"}` | Rows |
| `move` | `{"type":"move","path":[...],"id":"$id","at":{"before":"$id"}}` | Rows; omit `at` to move to the end |
| `increment` | `{"type":"increment","path":[...],"by":1}` | Counters (negative values decrement) |

## Full JSON import

Read the source data and destination schema, then prepare a complete JSON object
matching the destination's fields. An agent can help map the data for you.

```sh
bun slop get Source.slop > source.json
bun slop schema Template.slop
# Prepare mapped.json in the destination schema, then create a new document:
bun slop import New.slop --from Template.slop --file mapped.json

# For an existing destination, capture its data, schema and version together:
bun slop get Destination.slop --snapshot > destination.json
# Prepare mapped.json using that snapshot. Replace VERSION_TOKEN with its
# exact version string, keeping it quoted:
bun slop import Destination.slop --replace --if-version 'VERSION_TOKEN' --file mapped.json
```

`--from` and `--replace` are mutually exclusive. Creation never overwrites an
existing path and publishes the new package only after a successful save and
close. Both import modes return `{data, schema, version}`. Plain `get` continues
to return only data. Version tokens survive an unchanged close/reopen or
checkpoint, but belong to one destination path, schema and Loro version.

The input file is a complete root object, not a patch or a snapshot envelope.
Required fields must be supplied. Omitted optional fields, record entries and
rows are removed; imported order is authoritative. Unknown fields, invalid
values and unresolved references reject the whole import with a JSON Pointer
in the error. There are no implicit defaults or type coercions. Creation uses
fresh row/tree IDs. Replacement retains supplied `$id` values that identify
existing rows in the same collection; unmatched IDs create new rows. Duplicate
IDs and moving an existing ID between collections are rejected. Omit source
IDs when you intend to create fresh rows in an existing destination.

Rich text accepts a string (unformatted) or `{text, delta}` as returned by `get`.
Delta inserts must concatenate to `text` and use only destination-declared
marks. String fields can reference imported rows explicitly:

```json
{
  "groups": [{"name": "Work"}],
  "tasks": [{"title": "Send invoice", "groupId": {"$ref": "/groups/0"}}]
}
```

`$ref` uses an RFC 6901 JSON Pointer into the input root and must target a row or
tree node. This includes forward and cyclic links, optional strings, string
record entries and string-list elements. Ordinary strings are never rewritten.
References resolve to actual destination IDs inside the same transaction.

Open documents receive imports through their owning WebView. Pending drafts
and writes flush first; the runtime checks the expected version immediately
before applying the staged operations synchronously. A newer edit rejects the
replacement: read another snapshot and reconsider the mapping. A save failure
after acceptance retains the live changes for normal native retry. Never replay
an import automatically after an uncertain result.

Accepted imports persist a full Loro checkpoint, retaining history. This also
preserves absolute counter values whose rounding could otherwise change when
separate historical increments are replayed together. An unchanged import does
not write another checkpoint.

Files and encoded requests are bounded at 16 MiB, nesting at 128 levels, and
candidate Loro checkpoints at the 32 MiB limit on stored checkpoint plus update bytes. Counter
targets that cannot be represented exactly by a finite increment reject rather
than silently round.

JSON contains attachment references, not blob bytes. Copy blobs with
`attachments export` and `attachments import`, then retain their content IDs
in the mapped data. For a new document with attachments, first create a writable
copy of the template, transfer its blobs, then use version-checked replacement.
Existing blobs and theme overrides are retained; import only replaces document
data. Templates, authored assets, initial data, source history and source peer
IDs are never imported or rewritten.

## Ownership and retries

The permanent `state/writer.lock` decides ownership. Closed editing runs an engine-only invisible WebKit session; it never loads authored app code. Busy documents route through their owner's Unix socket. Missing or failed discovery never permits a second writer. A small `hello` handshake returns session identity without a document snapshot; ordinary reads need no handshake.

Successful mutations acknowledge persistence. No automatic replay or public retry flags exist. After an unknown outcome, run `slop get` before issuing another edit. `get` flushes pending drafts and writes before returning; save failures return an error. One internal epoch identifies each WebView lifetime.

## Export

```sh
bun slop export /path/to/List.slop --format png --output /path/to/List.png
bun slop export /path/to/List.slop --format pdf --output /path/to/List.pdf
```

Export prints the destination path on success.

Open-document exports capture the live view, including current width and selected tab. The existing capture flow commits text drafts, flushes storage, mounts the authored export snippet, and restores the editor. Closed exports render a disposable saved-state snapshot using the app's initial view. For Quick Checklist that means To do; the selected tab is not persisted.

The source writer lock is held while copying a closed writable document. Managed or read-only templates remain state-free. User export destinations must be outside the source package; completed output replaces its destination atomically. Template screenshots are a separate build operation and may write QuickLook assets in their staging package.

The server waits 30 seconds for a command; the client allows 35 seconds for a response. Expired captures cannot publish their result. A lost export acknowledgement has an uncertain outcome: inspect the destination before retrying. There is no automatic mutation or export replay.

## Helper discovery and authoring

`HITSLOP_NATIVE_CLI` selects an explicit executable for both document commands and template capture. Missing/non-executable overrides fail; an executed helper is never retried through another binary. Helper discovery otherwise checks `/Applications/hitSlop.app`, then
`~/Applications/hitSlop.app`. Authoring never compiles Swift. Build and register
require an installed app supporting the required runtime contract and revision.
The authored project SDK must match the SDK identity required by the CLI. CLI 1.2.0 requires document/schema SDK 1.1.0; the CLI package version itself need not equal the SDK version.

Bun is the only JavaScript runtime required for authoring and skill management.
Native document editing is macOS-only; there is no Bun document engine fallback.

## Change theme tokens

```sh
bun slop theme get /path/to/List.slop
bun slop theme set /path/to/List.slop --values '{"accent":"#123456"}'
bun slop theme reset /path/to/List.slop --token accent
bun slop theme reset /path/to/List.slop
```

`theme get` reports defaults, overrides, and effective values. Set only declared
tokens; reset one with `--token` or omit it to reset all. Live commands use
the owner socket; closed commands use engine-only WebKit. Inspect `theme get`
after an uncertain transport result before retrying.

## Attachments

```sh
bun slop attachments list /path/to/Receiver.slop
bun slop attachments import /path/to/Receiver.slop /path/to/Classic.wsz
bun slop attachments export /path/to/Receiver.slop SHA256 --output /path/to/Copy.wsz
```

Import returns `{id, name, mimeType, byteLength}` without choosing an app field.
Use schema/get and apply/batch to store the reference. List returns IDs and sizes;
export refuses existing destinations and paths inside the source document.
Commands use the native owner or engine-only session. After an uncertain import,
inspect attachments before another command. Blobs are immutable and deduplicated;
deletion is deferred.

## Agent skills

The TypeScript CLI uses Crust for command parsing, help, and packaged agent skills. Run `bun slop --help` or `bun slop COMMAND --help` for current arguments.

```sh
bun run skills:build
bun slop skills install
bun slop skills install --all --scope project
bun slop skills install --all --scope global
bun slop skills repair --scope global
bun slop skills uninstall --all --scope project
```

The build packages the authored `hitslop`, `hitslop-authoring`, `hitslop-design`, and `hitslop-document` guides plus the generated `hitslop-cli` command reference. Authored sources live in `packages/cli/skills`; `bun run build` also builds these artifacts. Rebuild after changing command metadata or authored guidance. Generated output lives in `packages/cli/.crust/root/skills` and is not tracked.

Interactive installation prompts for scope, skills, and agent targets, with one selection for all chosen skills. Installation is additive: unselected skills and agents remain untouched. `skills` is shorthand for `skills install`; `skill` remains an alias. `--all` defaults to global scope unless `--scope` is supplied. Conflicting real directories are skipped by `--all`; interactive replacement requires explicit confirmation. Installed links target a durable versioned copy in `~/.hitslop/cli/VERSION/skills`, so clearing the bunx package cache does not break them.

`skills repair` repairs existing links; it does not install absent skills or download newer content. `skills update` remains a compatibility alias. `skills uninstall` removes selected owned links at the chosen scope; `--all` is its non-interactive form. Link repair is explicit (`autoUpdate: false`), so document and authoring commands do not rewrite repository discovery links. Old native-created links can be repaired; the old `~/.hitslop/skills` cache is left untouched.

`init` includes portable copies of authored guides in the project. These real directories are not managed links: repair does not refresh them, and install does not overwrite them without confirmation. Review their contents manually when upgrading the project.

Interactive CLI use may show a cached update notice on stderr. Checks are skipped for CI, piped output, or `HITSLOP_NO_UPDATE_CHECK=1`; registry failures do not affect command success. Follow the [public upgrade guidance](../../apps/landing/src/content/docs/docs/guides/cli-workflows.mdx#upgrade-the-cli) to keep the CLI, SDK, and Mac app compatible.

The Mac app no longer installs or updates agent skills. Skill management requires the Bun-based TypeScript CLI; native document editing and export still require neither Bun nor the TypeScript CLI.
