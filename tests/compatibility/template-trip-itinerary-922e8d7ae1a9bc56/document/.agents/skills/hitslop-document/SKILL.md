---
name: hitslop-document
description: Inspect and edit a hitSlop v1 document with the local CLI.
---
Read manifest.json first. Only hitslop-v1 is supported.
Use slop schema PATH and slop get PATH, then slop apply PATH --op JSON or slop batch PATH --ops JSON.

For complete JSON transfer, export with `slop get SOURCE`
and read both schemas. Map source data to the destination schema yourself; the
CLI never guesses mappings. Create with `slop import NEW --from TEMPLATE --file
mapped.json`. Replace with `slop get DEST --snapshot`, then `slop import DEST
--replace --if-version TOKEN --file mapped.json`, using that snapshot's version.
The input is the complete root data object, not `{data,schema,version}`. Missing
required/unknown fields reject; omitted optional values, record entries and rows
are deleted. Keep destination `$id` values to retain rows, omit them for fresh
rows. Creation always uses fresh identities. Rich text accepts plain strings or
the full `{text,delta}` snapshot. Use `{"$ref":"/groups/0"}` in string fields to
link to imported rows/tree nodes; pointers address the input and resolve inside
the transaction. Ordinary strings are literal. Duplicate IDs, cross-collection
identity reuse and unresolved references reject atomically. Import returns
`{data,schema,version}` after persistence. Concurrent destination edits reject;
reread and reconsider the mapping. Do not automatically replay failures.
Attachments transfer separately using the attachment commands below. To transfer
attachments into a new document, create a writable template copy first, copy the
blobs, then import its data using --replace and a fresh snapshot version. Import
does not replace themes, authored assets or schemas. It has a 16 MiB request/file
limit and retains the existing 32 MiB checkpoint limit.
Paths name fields with strings, rows and tree nodes with {"id":"$id from get"}, record entries with {"key":"..."} and scalar list elements with {"index":n}. Never use array indexes as row identity.
Operations: set (scalars), clear (optional fields, record entries), assign (whole optional objects, record entries, scalar lists), text.replace, text.splice {index,delete,insert}, text.mark/text.unmark {start,end,key,value} for declared rich text marks, insert {value, destination {before|after|parent}} for rows and tree nodes or {value,index} for scalar lists, remove {id} or {index,count}, move {id,destination} or {from,to}, increment {value} for counters. Read slop schema to see which kind each field is; prefer batch for several related edits.

assign can initialize absent optional row lists/trees and record entries containing them. It cannot replace an existing identity-bearing collection, even through a containing object. Use insert/remove/move, or explicitly clear/delete before creating new identities. Text offsets are UTF-16 and must fall on whole code-point boundaries. Checkpoints retain history; automatic history pruning is deferred.
Keep manifest.json, assets/, state.schema.json and initial.json immutable. Row and tree `$id`s are stable application IDs. `slop get SOURCE --snapshot` includes `issues`: merged anomalies preserved as stored, never repaired. Unusable values read as fallbacks and refuse edits beneath them; rows with broken IDs carry derived `x-…` IDs that stay addressable. Report issues to the user instead of guessing a repair.
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
