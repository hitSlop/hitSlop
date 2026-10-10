---
name: hitslop-document
description: Inspect and edit a hitSlop document with the local CLI.
---
Run slop inspect PATH first: it shows whether the file is a template or a document, its app, artwork and attachments, and whether it is open.
Use slop schema PATH and slop get PATH, then slop apply PATH --op JSON or slop batch PATH --ops JSON. To write a whole value at once, save it as JSON and run slop import PATH FILE [--path JSON]: it replaces the value at the path (the whole document by default) and writes only the differences.

The native Rust owner applies every command. Create documents from immutable templates and their initial values, then edit with typed commands or import. Mutations are never replayed automatically: after an unknown outcome, run slop get before editing again.

Paths contain field strings, record keys (strings), `{"id":"row ID from get"}` row segments and `{"index":n}` scalar-list element segments. Commands are `set {path,value}` for scalars (string, number, integer, enum, boolean; within the schema's bounds), optional values and optional objects, whole text fields, record entries (a key path creates or replaces the entry), scalar-list elements and whole scalar lists; `clear {path}` for optional fields and record entries; `insert {path,value,id?,at?}` for rows or `insert {path,value,index?}` for scalar lists; `remove {path,id}` for rows or `remove {path,index,count?}` for scalar lists; `move {path,id,at?}`, `increment {path,by}`, and `replace {path,value}` (an empty path is the whole document; rows match by `$id`, a row without one is new, omitted optionals and record entries are removed, counters take the value). `at` is `{before:id}` or `{after:id}`; omission appends. Use batch for related edits. Negative increment implements decrement. apply and batch print `{ids}`: the inserted row IDs, including minted ones; run `get` for the value. Supply `id` for an insert you may need to retry; a rerun with the same `id` is refused as a duplicate, while a minted ID is new every run.

When you rewrite text you read, read it with `slop get PATH --snapshot` and put the text you read in the set as `from`: `{"type":"set","path":[...],"value":"new text","from":"text you read"}`. The set then changes the field from that text and keeps what was typed since, for example in an open window. Without `from`, a text set replaces whatever the field holds when it runs; `replace` and `slop import` do the same.

Read schema first. Supported types are text, boolean, string, number, integer, enum, optional, object, object-row lists, scalar lists, records and counters. A document's app (its manifest, assets, descriptor and initial values) is immutable. `get --snapshot` returns `{schema, defaults, version, value, theme}`: the descriptor, the template's colors, an opaque version that changes with every edit, the value and the effective colors. Every accepted edit keeps the document valid, so a value always matches its schema.

Never open a .slop file with SQLite or another tool, and never keep a JSON copy of a document. The CLI routes to the live host or acquires exclusive ownership when closed.
A refused batch prints `Refused ops[N] (reason): message` and then `Not applied.`: `N` is the zero-based index of the operation to fix, and nothing in the batch was applied. A failed transport can have an unknown outcome. Run slop get before issuing another edit; never automatically replay a mutation.

get saves and returns the state the document owner has accepted; text still being typed in an open window is not included. A save failure returns an error. Native export flushes pending edits and renders saved state in a fresh hidden page, using its default transient view state. Every export (native, theme and attachment) refuses an existing destination; to export again, remove the old file first.

A theme is a palette of declared colors. Use `slop theme get PATH` to inspect the
template's colors, the document's changes and the effective palette. Change declared
colors with `slop theme set PATH --values '{"accent":"#123456"}'` (lowercase `#rrggbb`,
or `#rrggbbaa` when translucent); reset one with `slop theme reset PATH --token accent`,
or omit the token to reset all. `slop theme export PATH [--output FILE]` writes the full
palette as a theme file, and `slop theme import PATH FILE` replaces the palette with a
file made for the same template. These commands preserve the writer lock and update the
open view and its theme panel. In a batch, `{"type":"setTheme","values":{"accent":"#123456","ink":null}}`
sets colors (`null` resets one; add `"replace":true` to reset every unlisted color) and
`{"type":"importTheme","file":"<theme file text>"}` imports, atomically with data edits. The template declares the colors and their defaults; the
host saves a document's changes in Loro with ordinary edit publications and Undo. JSON data replacement preserves the palette. Never edit the file directly or patch
its compiled CSS. Fonts and layout require editing the authoring
source and rebuilding. After an uncertain result inspect `theme get` before another
change. PNG/PDF exports include the effective theme.

Use `slop attachments ref FILE` to print a file's reference (id/name/mimeType/byteLength)
without touching the document, then store it in the app schema with
`slop apply PATH --attach FILE --op ...` (or `batch --attach`, repeatable): the file and
its reference are saved together. `slop attachments list PATH` and
`slop attachments export PATH ID --output FILE` read them back. Store the `id` exactly as
printed; a file whose ID appears nowhere in the document is removed when it closes.
Never write attachments into the file yourself. Limits are 10 MiB per file, 100 MiB and
256 unique files per document. After an uncertain edit, run `get` before another.

The author SDK is `hitslop`; the private page shell runtime is host-owned.
Catch semantic refusals with `isRejected(error)` and other document outcomes with
`isDocumentError(error)`, not `instanceof`. Transaction handles collect writes only;
use live handles for previews and bound `.value` assignments. Explicit `flush()`
remains available.

## Named commands

Run `slop describe PATH --json` to inspect the schema, operations, command arguments,
current values, row IDs and version. Prefer a matching domain action with
`slop call PATH NAME --args JSON`. It runs the command stored in that document and saves
one atomic batch. A refusal applies no edits; reason `refused` carries the command's
message for the person, such as an empty entry or a row that no longer exists. An
argument `describe` lists as "The $id of a row in tasks" takes that row's `$id`. After an
unknown outcome, read the state before deciding on another action.

Authors register `doc.command({ description, args, run })` in `defineSlop({ commands })`. Arguments use `s.*` descriptors, for example `{ text: s.string({ minLength: 1 }) }`. Rust checks arguments, evaluates the stored program in a restricted child and applies one batch. Page buttons await the same owner-routed command. `describe` projects arguments to JSON Schema for tools; JSON Schema is not stored or used to validate the document. Opening a document never upgrades its embedded app.
