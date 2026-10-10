# Document types

Every document field has a kind, declared with the `s` builder in `schema.ts`. The kind
decides:
- what a snapshot holds;
- how edits from the page and agents combine;
- which writes are accepted;
- which handle methods, bindings and CLI operations exist.

The Rust core (`crates/hitslop-core`) enforces all of it. The SDK
(`packages/hitslop/src/sdk`) gives authors types and adapters; the private shell implements
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
import { defineDocument, s } from "hitslop";
export default defineDocument({
  title: s.text(),
  mood: s.integer({ min: 1, max: 5 }),
  lane: s.enum(["todo", "doing", "done"]),
  note: s.optional(s.string({ maxLength: 280 })),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) })),
  tags: s.list(s.string()),
  checkins: s.record(s.integer({ min: 1 })),
  visits: s.counter(),
});
```

## At a glance

| Kind | Snapshot value | Edits from several places | Main writes |
|---|---|---|---|
| [`s.text()`](#text) | `string` | character edits merge | `set(value)`, `bindText` |
| [`s.boolean()`](#scalars) | `boolean` | last writer wins | `set`, `preview`, `value` |
| [`s.string({minLength?, maxLength?})`](#scalars) | `string` | last writer wins | `set`, `preview`, `value` |
| [`s.number({min?, max?})`](#scalars) | `number` | last writer wins | `set`, `preview`, `value` |
| [`s.integer({min?, max?})`](#scalars) | `number` | last writer wins | `set`, `preview`, `value` |
| [`s.enum([...])`](#scalars) | one of the values | last writer wins | `set`, `preview`, `value` |
| [`s.counter()`](#counter) | `number` | increments add up | `increment` |
| [`s.optional(inner)`](#optional) | the inner value, or absent | per inner kind | `set`, `clear` |
| [`s.object({...})`](#object) | object | per field | its fields' handles |
| [`s.list(s.object({...}))`](#rows) | array of rows with `$id` | inserts, removes and moves keep identity | `insert`, `remove`, `move`, `item` |
| [`s.list(scalar)`](#scalar-lists) | array of values | by position; one element: last writer wins | `insert`, `set`, `preview`, `remove`, `replace` |
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

**Defaults.** A field that an insert, creation of an absent entry or optional object, or
`initial` leaves out takes its default: text `""`, an empty list or record, a counter at
0, an object whose own fields all have defaults, or the `default` a scalar declares
(`s.boolean({ default: false })`, `s.enum([...], { default })`, and on `string`, `number`
and `integer`). A scalar without a default is required, and optional fields stay absent.
A default must satisfy its field's rules, and only object fields have one: optional
values, list elements and record values do not. Defaults are part of the descriptor, so
changing one makes a new document type; command arguments take defaults the same way.
Replacing a present object with `set`/`put` requires every required field, including
fields with creation defaults. `replace` requires a complete supplied subtree, including
new rows and entries within it; it never fills defaults. Omitting optional fields or
record keys removes them. Input types cannot know whether a keyed value already exists,
so callers of `put`/`set` must follow this state-dependent completeness rule.

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
text fields keep their place. The
[CLI guide](../guides/cli.md#operations) has the rules.

**Always valid.** One owner applies every edit, and every accepted edit keeps the
document matching its descriptor, so a snapshot never holds a wrong type, an
out-of-range value or a duplicate `$id`. Opening refuses a file whose saved state does
not match (`invalid_bytes`) and changes nothing in it.

**Error codes.**

| Code | Meaning |
|---|---|
| `type_mismatch` | wrong value type, unknown enum value, or an operation the kind doesn't support |
| `out_of_range` | outside numeric or string-length bounds, or an index past the end |
| `path_not_found` | no such field, row, entry or element (including fields of an unset optional or entry) |
| `invalid_key` | a record key that is empty, longer than 256 UTF-16 units, or reserved |
| `exists` | a destination already exists and cannot be replaced |
| `duplicate_id` | inserting a row whose `id` already exists |
| `invalid_request` | a malformed request, rows addressed by index, or scalar elements by id |
| `invalid_id` | a row `id` outside 1–64 characters of `A–Z a–z 0–9 _ -` |
| `invalid_path` | a command path longer than 64 segments |
| `stale_base` | a command's `ifVersion` that no longer matches the document (the owner retries once) |
| `too_large` | a batch over 1,000 intents, a request over 4 MiB, or a list or descriptor over its limit |
| `refused` | a command called `refuse(message)`, or a row argument names a row that no longer exists; the message is for the person |

Opening, authoring and storage use further codes: `invalid_schema` (a descriptor the
core refuses), `invalid_bytes` and `missing_dependencies` (saved updates that cannot be
imported), `invalid_shape` (a window shape), `requires_update` (an app format,
storage or document layout newer than this build), `is_template` (a template opened as a
document: create a document from it) and `engine_error` (an unexpected Loro failure). Codes may grow; `isDocumentError` recognizes a code an app has never seen.

**Paths** walk the schema from the root, in these segments:

| Segment | Addresses |
|---|---|
| `"field"` | an object field, or a record entry by key |
| `{"id": "…"}` | a row in a list of rows |
| `{"index": n}` | an element of a scalar list |

## Text

`s.text()` is for anything a person types. Edits merge character by character, and
neither side's typing is lost.

- **`bindText(input, handle)`** keeps the user's text in the field and sends each change
  as "the field was X, now it is Y". The owner merges it with edits made elsewhere, and
  the caret stays put, including through IME composition. Retargeting or unmounting a
  binding sends its unsent text first.
- **`EditableText`** (from `hitslop/svelte`, `field={handle}`) is that binding for any
  text, including a field in every row: it shows the text and mounts a textarea only
  while edited, since WebKit form controls are too expensive to mount by the thousand.
  It places the caret where the person clicked, grows with its text, and leaves IME
  Enter to the composition.
- **`handle.value`** on a live text handle reads the shown text (`""` while an optional
  text is unset); it is read-only.
- **`text.set(value)`** replaces the whole field as the owner holds it when it applies
  the set. It uses a minimal edit script, so typing still on its way from a binding
  merges with it; typing the owner already accepted is replaced unless `value` keeps it.
- **CLI:** `{"type":"set","path":["title"],"value":"Weekend"}`. With `"from"`, the field's
  text as you read it with `get --snapshot`, the set changes the field from that text, so
  typing done since is kept.

## Scalars

`boolean`, `string`, `number`, `integer` and `enum` are single values. Concurrent writes
resolve to one of them (last writer wins).

| Kind | Rules |
|---|---|
| `s.boolean()` | `true` or `false` |
| `s.string({ minLength?, maxLength? })` | string with inclusive length bounds in Unicode code points; `"😀"` counts as 1, `"e\u0301"` as 2 |
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

String length bounds are nonnegative safe integers, with `minLength ≤ maxLength`.
HTML `maxlength` and JavaScript's `.length` count UTF-16 code units, so they are not
substitutes for a field's bound. Text selection offsets and record-key limits still
use UTF-16 units.

## Counter

`s.counter()` holds a tally. An increment adds to the stored total, so the page's and an
agent's increments all count, where a `set` of a number read earlier would lose one.

- **Handle:** `increment(by = 1)`; a negative `by` subtracts. There is no reset.
- **Snapshot:** a safe integer. An increment that would leave the safe range is refused
  (`out_of_range`). Concurrent increments from several devices add up. The total is kept
  as a Loro counter, exact for any realistic count (until increments summed as absolute
  values reach about 9×10^15, as with a JavaScript number).
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
  - `set` creates an absent object with creation defaults, or reconciles a complete
    replacement with the existing object. Surviving text, lists and rows keep identity.
  - Fields of an unset object are `path_not_found`.
- **Optional text:** an unset text reads as `""` in `bindText`, and the first keystroke
  creates it. `set(string)` creates or edits it.
- `set` on an object that is already set writes only the fields that differ.
- Clearing and then setting writes the new value over the field's earlier content. A
  text edit from the old text merges into the field that is there now; while the field is
  cleared it is `path_not_found`.
- **CLI:** `{"type":"clear","path":["note"]}`.

## Object

`s.object({...})` groups fields, each edited through its handle. An object always exists
(unless it is optional). The CLI's `replace` can reconcile its complete value in place.

## Rows

`s.list(s.object({...}))` holds rows with a stable `$id`. Key Svelte loops and selection
by `$id`, never by position.

- **Handle:**
  - `insert(value, { before | after })` mints the id and resolves `{id}`;
  - `remove(id)`, `move(id, { before | after })`, `item(id)`.
  - Omitting the destination appends.
- **Identity:** inserts, removes and moves from anywhere keep each row's `$id`; a removed
  row is no longer a valid target.
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
- **Positions:** an index means the position when the owner applies the edit, so an
  index read before another insertion can name a different element; a later `set` of
  one element wins.
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
- **Entries behave like optional fields.** A new object entry receives creation
  defaults; replacing an existing entry requires a complete value and reconciles its
  surviving children. Delete followed by put writes the new value over the entry's
  earlier content.
- **Fields:** edits to different fields of an existing object entry both survive.
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
| the authored document data | the root map `data`, one entry per field |
| document theme overrides | the root map `theme`, declared color token → canonical color string |
| `s.text()` | a mergeable `LoroText` |
| `s.boolean()`, `s.string()`, `s.enum()` | a boolean or string value |
| `s.number()` | an f64 value; integral values project as integers |
| `s.integer()` | an i64 value |
| `s.counter()` | a mergeable `LoroCounter`; the total projects as an integer |
| `s.optional(inner)` | the inner kind's representation, or no entry when unset |
| `s.object({...})` | a mergeable `LoroMap` |
| `s.list(s.object({...}))` | a mergeable `LoroMap` of `rows` (a mergeable `LoroMap` of `$id` → mergeable `LoroMap` row) and `order` (a mergeable `LoroMovableList` of `$id` strings) |
| `s.list(scalar)` | a mergeable `LoroMovableList` of values |
| `s.record(value)` | a mergeable `LoroMap` of key to the value's representation |

The theme map is host-owned and outside the authored descriptor. Defaults remain in the
immutable app row; the effective palette combines them with these overrides. Per-color
writes use the same undo history and saved updates as data, and
advance the same publication sequence. JSON replacement targets `data`, so it never
replaces the theme. State carries the effective `theme`; publications include it when
it changes, including on a theme-only edit.

Every container a field holds is a mergeable child of its parent map
(`ensure_mergeable_*`): its identity follows from its place, so two devices that create
the same field, entry or row converge on one container instead of one hiding the
other. Clearing a field or removing a row hides its container; writing it again, or an
undo, revives the same container and writes the new value over its earlier content.

A row list's application order is `order` read once per row at its first place, then
rows `order` misses, by `$id`. Merging valid edits from several devices can leave `order`
naming a row twice (two restores of one row), naming a removed row, or missing a row;
every reading resolves those the same way, and `replace` writes a clean order.

Stored state always matches the descriptor, and opening checks it: container kinds,
scalar rules, valid row IDs and record keys, finite counters, and maps holding only
declared fields. Any merge of valid edits passes this check (`tests/merge.rs`).

Complete replacements retain surviving containers. A text edit addresses the field at
its path and merges from the text it started from, so a binding edits a restored row
normally. Moving a row keeps its containers and `$id`.

Live commits record timestamps and an origin message: `page`, `agent` (CLI and socket),
`command:<name>`, `window`, `undo`, `redo` or `create`. These messages are attribution
for the retained changes, not a durable audit log. Undo is session-only.
Template creation uses the deterministic message `create` and timestamp zero. Rows in
an app's initial value without a `$id` get one derived from their position by a frozen
function (`identity.rs`), so packing the same app writes the same template.

## Not supported

Trees, rich text, `optional(list)`, `optional(record)` and `optional(counter)`, and
`move` on scalar lists are not implemented; no slop needs them. Schema evolution is
deferred: changing a descriptor makes a new document type. Each new kind lands in the
core, the SDK and a fixture together, and raises the app format and runtime requirements.
