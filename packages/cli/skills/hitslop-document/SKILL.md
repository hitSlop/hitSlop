---
name: hitslop-document
description: Inspect and edit a hitSlop document with the local CLI.
---
Read manifest.json first.
Use slop schema PATH and slop get PATH, then slop apply PATH --op JSON or slop batch PATH --ops JSON. To write a whole value at once, save it as JSON and run slop import PATH FILE [--path JSON]: it replaces the value at the path (the whole document by default) and writes only the differences.

The native Rust owner applies every command. Create documents from immutable templates and their initial values, then edit with typed commands or import. Mutations are never replayed automatically: after an unknown outcome, run slop get before editing again.

Paths contain field strings, record keys (strings), `{"id":"row ID from get"}` row segments and `{"index":n}` scalar-list element segments. Commands are `set {path,value}` for scalars (string, number, integer, enum, boolean; within the schema's bounds), optional values and optional objects, whole text fields, record entries (a key path creates or replaces the entry), scalar-list elements and whole scalar lists; `clear {path}` for optional fields and record entries; `insert {path,value,id?,at?}` for rows or `insert {path,value,index?}` for scalar lists; `remove {path,id}` for rows or `remove {path,index,count?}` for scalar lists; `move {path,id,at?}`, `increment {path,by}`, and `replace {path,value}` (an empty path is the whole document; rows match by `$id`, a row without one is new, omitted optionals and record entries are removed, counters take the difference; refused where it would overwrite a stored anomaly). `at` is `{before:id}` or `{after:id}`; omission appends. Use batch for related edits. Negative increment implements decrement. apply and batch print `{ids, sequence, value}`: `ids` lists inserted row IDs, including minted ones. Supply `id` for an insert you may need to retry; a rerun with the same `id` is refused as a duplicate, while a minted ID is new every run.

Read schema first. Supported types are text, boolean, string, number, integer, enum, optional, object, object-row lists, scalar lists, records and exact integer counters. Keep manifest, assets, descriptor and initial values immutable. `get --snapshot` returns the `schema` and the `state`: sequence, version, value and issues. Issue paths address rows by `{"id"}`, as edits do. Derived row IDs remain addressable; stored anomalies are preserved, never repaired on read. Report issues instead of guessing repairs.

Never edit state/document.sqlite or invent stores/data.json. The CLI routes to the live host or acquires exclusive ownership when closed.
A failed transport can have an unknown outcome. Run slop get before issuing another edit; never automatically replay a mutation.

get saves and returns the state the document owner has accepted; text still being typed in an open window is not included. A save failure returns an error. Native export captures the live selected view when open and the initial view when closed; export output must be outside the source package.

Use `slop theme get PATH` to inspect public token defaults and overrides.
Change declared tokens with `slop theme set PATH --values '{"accent":"#123456"}'`;
reset one with `slop theme reset PATH --token accent`, or omit the token to reset
all. These commands preserve the writer lock and update the open view.
`assets/theme.json` declares tokens and defaults; the host saves document overrides
in `state/document.sqlite`. Never edit either file directly or patch compiled CSS in
`assets/`. Layout changes require editing the authoring source and rebuilding.
After an uncertain result inspect
`theme get` before another change. PNG/PDF exports include the effective theme.

Use `slop attachments list PATH`, `slop attachments import PATH FILE`, and
`slop attachments export PATH ID --output FILE`. Import returns a reference with
id/name/mimeType/byteLength; store it in the app schema through apply/batch.
Never write `state/attachments` yourself. Limits are 10 MiB per file, 100 MiB and
256 unique files per document. Removing a reference retains its blob. Export
refuses existing destinations. Inspect attachments after an uncertain import.
