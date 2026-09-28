---
name: hitslop-document
description: Inspect and edit a hitSlop v1 document with the local CLI.
---
Read manifest.json first.
Use slop schema PATH and slop get PATH, then slop apply PATH --op JSON or slop batch PATH --ops JSON.

The native Rust owner applies every command. There is no JSON import or replacement; create documents from immutable templates and their initial values, then edit with typed commands. Mutations are never replayed automatically: after an unknown outcome, run slop get before editing again.

Paths contain field strings and `{"id":"row ID from get"}` segments. Commands are `set {path,value}` for booleans, `insert {path,value,id?,at?}`, `remove {path,id}`, `move {path,id,at?}`, `increment {path,by}`, and `splice {path,index,delete,insert}`. `at` is `{before:id}` or `{after:id}`; omission appends. Text offsets are UTF-16 code-point boundaries, interpreted at host execution time; CLI callers do not supply a base. Use batch for related edits. Negative increment implements decrement.

Read schema first. Supported types are text, boolean, object, object-row lists and exact integer counters. Keep manifest, assets, descriptor and initial values immutable. `get --snapshot` includes data, schema, version and issues. Derived row IDs remain addressable; stored anomalies are preserved, never repaired on read. Report issues instead of guessing repairs.

Never edit state/document.sqlite or invent stores/data.json. The CLI routes to the live host or acquires exclusive ownership when closed.
A failed transport can have an unknown outcome. Run slop get before issuing another edit; never automatically replay a mutation.

get flushes pending edits and returns persisted state; a save failure returns an error. Native export captures the live selected view when open and the initial view when closed; export output must be outside the source package.

Use `slop theme get PATH` to inspect public token defaults and overrides.
Change declared tokens with `slop theme set PATH --values '{"accent":"#123456"}'`;
reset one with `slop theme reset PATH --token accent`, or omit the token to reset
all. These commands preserve the writer lock and update the open view.
`assets/theme.json` declares tokens and defaults; the host writes document overrides
to `state/theme.json`. Never edit either file directly or patch compiled CSS in
`assets/`. Layout changes require editing the authoring source and rebuilding.
After an uncertain result inspect
`theme get` before another change. PNG/PDF exports include the effective theme.

Use `slop attachments list PATH`, `slop attachments import PATH FILE`, and
`slop attachments export PATH ID --output FILE`. Import returns a reference with
id/name/mimeType/byteLength; store it in the app schema through apply/batch.
Never write `state/attachments` yourself. Limits are 10 MiB per file, 100 MiB and
256 unique files per document. Removing a reference retains its blob. Export
refuses existing destinations. Inspect attachments after an uncertain import.
