---
name: hitslop-document
description: Inspect and edit a hitSlop document with the local CLI.
---
Run slop inspect PATH first: it shows whether the file is a template or a document, its app, artwork and attachments, and whether it is open.
Use slop schema PATH and slop get PATH, then slop apply PATH --op JSON or slop batch PATH --ops JSON. To write a whole value at once, save it as JSON and run slop import PATH FILE [--path JSON]: it replaces the value at the path (the whole document by default) and writes only the differences.

The native Rust owner applies every command. Create documents from immutable templates and their initial values, then edit with typed commands or import. Mutations are never replayed automatically: after an unknown outcome, run slop get before editing again.

Paths contain field strings, record keys (strings), `{"id":"row ID from get"}` row segments and `{"index":n}` scalar-list element segments. Commands are `set {path,value}` for scalars (string, number, integer, enum, boolean; within the schema's bounds), optional values and optional objects, whole text fields, record entries (a key path creates or replaces the entry), scalar-list elements and whole scalar lists; `clear {path}` for optional fields and record entries; `insert {path,value,id?,at?}` for rows or `insert {path,value,index?}` for scalar lists; `remove {path,id}` for rows or `remove {path,index,count?}` for scalar lists; `move {path,id,at?}`, `increment {path,by}`, and `replace {path,value}` (an empty path is the whole document; rows match by `$id`, a row without one is new, omitted optionals and record entries are removed, counters take the difference; refused where it would overwrite a stored anomaly). `at` is `{before:id}` or `{after:id}`; omission appends. Use batch for related edits. Negative increment implements decrement. apply and batch print `{ids, sequence}`: `ids` lists inserted row IDs, including minted ones; run `get` for the value. Supply `id` for an insert you may need to retry; a rerun with the same `id` is refused as a duplicate, while a minted ID is new every run.

Read schema first. Supported types are text, boolean, string, number, integer, enum, optional, object, object-row lists, scalar lists, records and exact integer counters. A document's app (its manifest, assets, descriptor and initial values) is immutable. `get --snapshot` returns the `schema` and the `state`: sequence, version, value and issues. Issue paths address rows by `{"id"}`, as edits do. Derived row IDs remain addressable; stored anomalies are preserved, never repaired on read. Report issues instead of guessing repairs.

Never open a .slop file with SQLite or another tool, and never keep a JSON copy of a document. The CLI routes to the live host or acquires exclusive ownership when closed.
A failed transport can have an unknown outcome. Run slop get before issuing another edit; never automatically replay a mutation.

get saves and returns the state the document owner has accepted; text still being typed in an open window is not included. A save failure returns an error. Native export captures the live selected view when open and the initial view when closed. Every export (native, theme and attachment) refuses an existing destination; to export again, remove the old file first.

A theme is a palette of declared colors. Use `slop theme get PATH` to inspect the
template's colors, the document's changes and the effective palette. Change declared
colors with `slop theme set PATH --values '{"accent":"#123456"}'` (lowercase `#rrggbb`,
or `#rrggbbaa` when translucent); reset one with `slop theme reset PATH --token accent`,
or omit the token to reset all. `slop theme export PATH [--output FILE]` writes the full
palette as a theme file, and `slop theme import PATH FILE` replaces the palette with a
file made for the same template. These commands preserve the writer lock and update the
open view and its theme panel. The template declares the colors and their defaults; the
host saves a document's changes in the document. Never edit the file directly or patch
its compiled CSS. Fonts and layout require editing the authoring
source and rebuilding. After an uncertain result inspect `theme get` before another
change. PNG/PDF exports include the effective theme.

Use `slop attachments list PATH`, `slop attachments import PATH FILE`, and
`slop attachments export PATH ID --output FILE`. Import returns a reference with
id/name/mimeType/byteLength; store it in the app schema through apply/batch.
Never write attachments into the file yourself. Limits are 10 MiB per file, 100 MiB and
256 unique files per document. Removing a reference retains its blob. Inspect
attachments after an uncertain import.

The author SDK is `@hitslop/document`; the private `@hitslop/shell` runtime is host-owned.
Catch semantic refusals with `isRejected(error)` and other document outcomes with
`isDocumentError(error)`, not `instanceof`. Transaction handles collect writes only;
use live handles for previews and bound `.value` assignments. Explicit `flush()`
remains available.
