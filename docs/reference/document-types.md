# Document types

Every document field has a kind, declared with the `s` builder in `schema.ts`. The kind
decides:
- what a snapshot holds;
- how edits from several places merge;
- which writes are accepted;
- which handle methods, bindings and CLI operations exist.

The Rust core (`crates/hitslop-core`) enforces all of it. The SDK
(`packages/document`) gives authors types and adapters; the private shell implements
the handles and immutable snapshots.

`defineDocument` produces a plain object-root descriptor. Build validation calls the
same Rust rules as native editing; the small TypeScript schema module is metadata and
types, not a second validator. Catch document errors with `isDocumentError(error)`
and semantic refusals with `isRejected(error)`; these guards work across the separately
bundled app and shell. Transaction handles expose writes only: `preview()` and
assignable `.value` belong to live handles. A rejected write exposes `DocumentError.reason`
and, for a batch, `opIndex`. Uncertain outcomes have distinct error codes: inspect recovered
state before deciding on a new edit.

```ts
import { defineDocument, s } from "@hitslop/document";
export default defineDocument({
  title: s.text(),
  mood: s.integer({ min: 1, max: 5 }),
  lane: s.enum(["todo", "doing", "done"]),
  note: s.optional(s.string({ maxLength: 280 })),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
  tags: s.list(s.string()),
  checkins: s.record(s.integer({ min: 1 })),
  visits: s.counter(),
});
```

## At a glance

| Kind | Snapshot value | Merge | Main writes |
|---|---|---|---|
| [`s.text()`](#text) | `string` | character edits merge | `set(value)`, `bindText` |
| [`s.boolean()`](#scalars) | `boolean` | last writer wins | `set`, `preview`, `value` |
| [`s.string({maxLength?})`](#scalars) | `string` | last writer wins | `set`, `preview`, `value` |
| [`s.number({min?, max?})`](#scalars) | `number` | last writer wins | `set`, `preview`, `value` |
| [`s.integer({min?, max?})`](#scalars) | `number` | last writer wins | `set`, `preview`, `value` |
| [`s.enum([...])`](#scalars) | one of the values | last writer wins | `set`, `preview`, `value` |
| [`s.counter()`](#counter) | `number \| null` | increments add up | `increment` |
| [`s.optional(inner)`](#optional) | the inner value, or absent | per inner kind | `set`, `clear` |
| [`s.object({...})`](#object) | object | per field | its fields' handles |
| [`s.list(s.object({...}))`](#rows) | array of rows with `$id` | inserts, removes and moves keep identity | `insert`, `remove`, `move`, `item` |
| [`s.list(scalar)`](#scalar-lists) | array of values | positions merge; one element: last writer wins | `insert`, `set`, `preview`, `remove`, `replace` |
| [`s.record(value)`](#records) | `{[key]: value}` | per key; object entries per field | `put`, `delete`, `entry` |

Kinds nest:
- lists of rows and records can sit inside rows and objects;
- scalar lists and records can sit inside rows.

Trees and rich text are not implemented; no slop uses them.

## Common behavior

**Reads.**
- `doc.current` is an immutable snapshot. Unchanged objects and rows keep their identity
  between edits.
- `doc.at(value)` returns the typed handle for any object taken from `doc.current`: the
  root, a row, a nested object or a record entry.

**Writes are asynchronous.**
- `await handle.set(x)` resolves once the owner has accepted the edit and `doc.current`
  shows it. A refused write rejects with a code, and nothing changes.
- Handle returned promises with `await`/`try` or `.catch()`. Caught failures are not
  also reported as unhandled errors. Bindings report their internal failures once.

**Batches.**
- `await doc.change(tx => { … })` collects writes synchronously and applies them all or
  none.
- Inside the callback, `insert` returns its id immediately, so later writes in the same
  change can address the new row.
- Async, nested and escaped callbacks are refused.
- The collector runs immediately, before earlier queued writes necessarily finish.
  It provides atomic writes, not transactional reads: `doc.current` is still the
  current immutable snapshot. Await prior writes before dependent reads; use a counter
  increment when the operation is an increment.

**Previews.**
- `preview(value)` on a scalar or scalar-list element shows a value locally without
  writing history, for drags, sliders and drawing.
- The next `set` for that field, `flush`, close or export commits it.
- Any accepted write at or above the previewed path settles the preview: `set`,
  `clear`, `put`, `delete`, a replaced object or a `change` batch. Inserting into or
  removing from a scalar list settles every element preview in that list, since indexes
  shift. A preview whose row, entry or optional object is gone is dropped, never
  recreated.
- A semantic refusal reverts that preview to the owner snapshot and reports once;
  another valid preview can still commit. An uncertain outcome retains the draft and
  fails close/export preparation.

**Durability.** `await doc.flush()` sends unsent text, waits for pending writes and
saves. Close, quit and export do the same first. A failed save keeps every edit.

**Undo.** `await doc.undo()` and `doc.redo()` are Edit ▸ Undo and Redo, after sending
what the person sees. They step back through the changes made since the document
opened, the person's and an agent's (CLI). A typing run in one field, and an agent's
consecutive edits, are each one step.

**Import.** The CLI's `replace` operation (`slop import`) makes any value, or the whole
document, equal a JSON value by writing only the differences: rows match by `$id`, and
kept rows and text keep their identity, and unchanged values are not written, so open
fields and concurrent edits elsewhere survive. The
[CLI guide](../guides/cli.md#operations) has the rules.

**Merged anomalies.** Imported CRDT state may contain a value that breaks
the rules: a wrong type, a value out of bounds, an unknown enum value, a bad key.
- It is preserved as stored, never repaired on read, and reported in `doc.issues` with
  its path.
- A value of the right type may be overwritten; a wrong-typed one is refused
  (`type_mismatch`).

**Error codes.**

| Code | Meaning |
|---|---|
| `type_mismatch` | wrong value type, unknown enum value, or an operation the kind doesn't support |
| `out_of_range` | outside bounds or `maxLength`, or an index past the end |
| `path_not_found` | no such field, row, entry or element (including fields of an unset optional or entry) |
| `invalid_key` | a record key that is empty, longer than 256 UTF-16 units, or reserved |
| `exists` | replacing an object whose value holds text, a list or a counter |
| `duplicate_id` | inserting a row whose `id` already exists |
| `invalid_request` | a malformed request, rows addressed by index, or scalar elements by id |
| `invalid_id` | a row `id` outside 1–64 characters of `A–Z a–z 0–9 _ -` |
| `invalid_path` | a command path longer than 64 segments |
| `stale_base` | a text edit whose `from` no longer matches the field at its version, or a version from another history |
| `invalid_version` | a version token that is not one the core issued |
| `too_large` | a batch over 1,000 intents, a request over 4 MiB, or a list or descriptor over its limit |

Opening, authoring and storage use further codes: `invalid_schema` (a descriptor the
core refuses), `invalid_bytes` and `missing_dependencies` (saved updates that cannot be
imported), `invalid_shape` (a manifest window shape), `requires_update` (an app format,
storage or document layout newer than this build), `is_template` (a template opened as a
document: create a document from it) and `engine_error` (an unexpected Loro failure). Codes may grow; `isDocumentError` recognizes a code an app has never seen.

**Paths** walk the schema from the root. Commands and issues use the same segments; an
issue names a row by its effective `$id` (the one `doc.current` shows, derived for a row
without a unique stored ID), and a stored entry that is not a row object by `{"index"}`:

| Segment | Addresses |
|---|---|
| `"field"` | an object field, or a record entry by key |
| `{"id": "…"}` | a row in a list of rows |
| `{"index": n}` | an element of a scalar list |

## Text

`s.text()` is for anything a person types. Concurrent character edits merge, and
neither side's typing is lost.

- **`bindText(input, handle)`** keeps the user's text in the field and sends each change
  as "the field was X, now it is Y". The owner merges it with edits made elsewhere, and
  the caret stays put, including through IME composition. Retargeting or unmounting a
  binding sends its unsent text first.
- **`text.set(value)`** replaces the whole field as it is when the owner applies it. It
  uses a minimal edit script, so concurrent typing outside the changed span survives.
- **CLI:** `{"type":"set","path":["title"],"value":"Weekend"}`.

## Scalars

`boolean`, `string`, `number`, `integer` and `enum` are single values. Concurrent writes
resolve to one of them (last writer wins).

| Kind | Rules |
|---|---|
| `s.boolean()` | `true` or `false` |
| `s.string({ maxLength })` | any string; `maxLength` counts UTF-16 units, like JavaScript's `length` |
| `s.number({ min, max })` | a finite number within inclusive bounds; integral values read as integers |
| `s.integer({ min, max })` | a safe integer (±2⁵³−1) within inclusive bounds |
| `s.enum(["a", "b"])` | one of the listed strings; TypeScript narrows to their literal types |

- **Handle:** `set(value)`, `preview(value)` and `value`.
  - `set` shows the value at once and resolves when accepted; a refused value reverts.
  - `value` is for Svelte `bind:` (`bind:checked`, `bind:value`, `bind:group`) on native
    inputs and component libraries. Assigning previews the value and commits it once
    assignments pause for 150 ms, or at the next flush, close or export.
  - Assigning `null` or `undefined` clears an optional field and is ignored otherwise; a
    refused value reverts and is reported.
- **CLI:** `{"type":"set","path":["mood"],"value":4}`.

Store amounts in minor units with `s.integer` when exactness matters (cents), and use
`s.number` for measurements and ratios.

## Counter

`s.counter()` holds a tally. Each writer's increments are kept separately and summed,
so concurrent increments all count.

- **Handle:** `increment(by = 1)`; a negative `by` subtracts. There is no concurrent reset.
- **Snapshot:** a safe integer, or `null` if the stored contributions are invalid (show
  it as unavailable).
- **CLI:** `{"type":"increment","path":["visits"],"by":1}`.

## Optional

`s.optional(inner)` may be absent. The inner kind is a scalar, `s.text()` or an object.

- **Snapshot:** the key is absent when unset; TypeScript types it as `value | undefined`.
- **Setting and clearing:**
  - `set(value)` gives it a value;
  - `clear()` removes it (clearing an unset field does nothing);
  - inserts and initial values may leave it out.
  - `null` is never a value.
- **Optional objects:**
  - `set` creates the object, or replaces it when it holds only scalars.
  - If its descriptor holds text, a list or a counter, a second `set` is refused
    (`exists`), so identities are never discarded; edit its fields instead.
  - Fields of an unset object are `path_not_found`.
- **Optional text:** an unset text reads as `""` in `bindText`, and the first keystroke
  creates it. `set(string)` creates or edits it.
- **Merge:**
  - Two replicas that create the same unset object or text at once share one value: its
    fields resolve one by one, last writer wins; text, lists and rows keep both sides;
    counter starting values add.
  - `set` on an object that is already set writes only the fields that change, so a
    concurrent edit to another field survives.
  - `clear` hides concurrent edits to the cleared value. It removes what this replica has
    seen; an edit it has not seen reappears if a replica that has not seen it either sets
    the value again.
- **CLI:** `{"type":"clear","path":["note"]}`.

## Object

`s.object({...})` groups fields. Its fields merge independently. An object always exists
(unless it is optional) and is never replaced as a whole.

## Rows

`s.list(s.object({...}))` holds rows with a stable `$id`. Key Svelte loops and selection
by `$id`, never by position.

- **Handle:**
  - `insert(value, { before | after })` mints the id and resolves `{id}`;
  - `remove(id)`, `move(id, { before | after })`, `item(id)`.
  - Omitting the destination appends.
- **Merge:** concurrent inserts, removes and moves keep identity; a removed row is no
  longer a valid target.
- **Moving between lists:** only moves within the same list preserve the Loro row
  container. SDK insertion always mints a new id; removing and reinserting is a new row.
  Use one list plus a status/group field when identity must survive moving sections.
- **CLI:**
  - `{"type":"insert","path":["tasks"],"id":"optional","value":{…},"at":{"after":"<$id>"}}`;
  - `{"type":"remove","path":["tasks"],"id":"<$id>"}`;
  - `{"type":"move",…}`.
  - Explicit ids let later commands address inserted rows. After an unknown outcome,
    inspect current state; never blindly replay the insertion.

## Scalar lists

`s.list(scalar)` holds plain values addressed by position: tags, attendees, presets, or
a 16×16 grid of colours.

- **Handle:**
  - `insert(value, index?)` (default: the end), `set(index, value)`,
    `preview(index, value)`, `remove(index, count = 1)`;
  - `replace(values)` rewrites the list, keeping unchanged positions. It sends a `set`
    of the whole list, not the CLI's `replace` operation.
  - There is no `move`.
- **Merge:** concurrent inserts are both kept; concurrent sets of one element resolve
  last-writer-wins.
- **Publishing:** each change publishes the whole list. They are meant to be small (the
  largest in the archive is 256 elements).
- **CLI:**
  - `{"type":"insert","path":["tags"],"value":"urgent","index":0}`;
  - `{"type":"set","path":["tags",{"index":0}],"value":"later"}`;
  - `{"type":"remove","path":["tags"],"index":0,"count":1}`;
  - `{"type":"set","path":["tags"],"value":["a","b"]}` replaces the list.

## Records

`s.record(value)` holds entries by string key: check-ins by date, cells by `"A1"`,
widths by column. Values are scalars or objects.

- **Keys:** 1–256 UTF-16 units; `$id`, `__proto__`, `constructor` and `prototype` are
  refused.
- **Handle:**
  - `put(key, value)` creates or replaces an entry;
  - `delete(key)` removes it (deleting a missing key does nothing);
  - `entry(key)` is the entry's handle, and its fields are `path_not_found` while the
    entry is unset;
  - `doc.at(doc.current.cells["A1"])` resolves an object entry.
- **Entries behave like optional fields.** Replacing an object entry that holds text or
  a list is refused (`exists`).
- **Merge:** as for optional fields: concurrent puts of one key share one entry, and edits
  to different fields of an existing object entry both survive.
- **Snapshot:** a plain object; iterate it with `Object.entries`.
- **CLI:**
  - `{"type":"set","path":["checkins","2026-09-23"],"value":1}`;
  - `{"type":"clear","path":["checkins","2026-09-23"]}`;
  - `{"type":"set","path":["cells","A1","input"],"value":"42"}`.

## Async patterns

- **Don't read a value and write it back** (`set(qty + 1)`): the snapshot may be a moment
  old. Use `increment` on a counter, or await the earlier writes before reading. A
  `change` makes its writes atomic, but reads the same snapshot.
- **Don't write in `$effect` or on mount.** Put defaults in `slop.ts`'s `initial`. If an effect must
  create something, guard it so it runs once, because it can rerun before the write is
  accepted.
- **Await an insert** before using its id outside `change`. Inside `change`, the id is
  synchronous.
- **Await writes whose failure you handle**, and read `doc.current` only after the write
  resolves.

## Storage layout

How kinds map to Loro containers is a persisted contract: every saved document records
its layout in the root map `meta` (`{"layout": 1}`), written with the initial values.
A build reads every layout it knows, or migrates one losslessly, and refuses a newer one
with `requires_update` ([engineering contract](../engineering-contract.md#compatibility)).

Layout 1:

| Kind | Stored as |
|---|---|
| the document | the root map `data`, one entry per field |
| `s.text()` | a `LoroText` |
| `s.boolean()`, `s.string()`, `s.enum()` | a boolean or string value |
| `s.number()` | an f64 value; integral values project as integers |
| `s.integer()` | an i64 value |
| `s.counter()` | a `LoroMap` of writer (Loro peer ID) to that writer's integer total; the snapshot is their sum |
| `s.optional(inner)` | the inner kind's representation, or no entry when unset |
| `s.object({...})` | a `LoroMap` |
| `s.list(s.object({...}))` | a `LoroMovableList` of `LoroMap` rows, each with a `$id` string entry |
| `s.list(scalar)` | a `LoroMovableList` of values |
| `s.record(value)` | a `LoroMap` of key to the value's representation |

A container whose path from its nearest row (or the document) passes an optional field or
a record entry can be created by more than one replica. It is created with Loro's
`ensure_mergeable_*`, so its identity comes from its parent, key and kind, and concurrent
creations share it. Every other container is created once, with its row or the document,
by `insert_container`. Every replica derives the same choice from the descriptor.

Loro keeps a mergeable container after its key is removed, including in history-trimmed
snapshots. So removing a value, removing a row that holds one, and undoing either first
empty the mergeable containers inside, and creating one empties it again before filling
it. Each mergeable container ever created stays in the document, empty, at about 19
bytes.

Agent (CLI and socket) commits carry the commit message `agent`. Effective row IDs for
rows without a unique stored `$id` are derived from container identity by a frozen
function (`identity.rs`).

## Not supported

Trees, rich text, `optional(list)`, `optional(record)` and `optional(counter)`, and
`move` on scalar lists are not implemented; no slop needs them. Schema evolution is
deferred: changing a descriptor makes a new document type. Each new kind lands in the
core, the SDK and a fixture together, and raises the app format and runtime requirements.
