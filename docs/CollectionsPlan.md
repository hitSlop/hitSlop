# Milestone: records, scalar lists and optional text (+ the document types reference)

## Context

Scalars have landed (`docs/ScalarsPlan.md`). A survey of the 47 archived slops shows:

**What the archive needs:**
- **Already covered:** 30 slops use only supported kinds. 5 more need nested object lists
  (flashcards, grade-calculator, side-quest, trip-itinerary, weekly-planner). The core
  already supports those (`crates/hitslop-core/fixtures/nested.json`, `tests/nested.rs`).
  These 35 are blocked only by porting to the async API.
- **Records** (`s.record(value)`): 5 slops.
  - habit-heatmap: `integer` by date, inside a list row.
  - harada-method: `boolean` by `"theme:action"`.
  - morning-pages: object `{date, text, completedAt}` by date.
  - pocket-sheet: object cells `"A1"…"F12"` with optional fields; integer widths by
    column.
  - wordle: `integer` by `"1".."6"`, inside an object.
  - Writes are `put`, `delete`, `entry(key).field.set/clear`, `doc.at(entry)` and
    `bindText` on an entry's text.
- **Scalar lists** (`s.list(scalar)`): 8 slops.
  - pixel-art: 256 fixed colours, `set` and `preview`.
  - harada-method: 8 cells per theme in a row, `set` and `preview`.
  - slide-deck: `points`, `insert`, `set` and `preview`.
  - wordle: `guesses`, `insert` and `remove(0, n)`.
  - alien-radio, meeting-notes: `insert`, `remove`.
  - metronome-tapper: `insert(v, i)`, `set` and `remove(i, n)`.
  - workout-planner: `replace(values)`.
  - No `move`, and no bindings on elements.
- **`optional(text)`:** reading-tracker (`set`), and slide-deck (10 fields bound with
  `bindText`).
- **Nothing else is needed.** No slop uses trees, rich text, `optional(list/record)` or
  any other kind. Moving a row between lists is always remove + insert inside one
  `change`, which already works.

**Decided with the user:**
- Add records, scalar lists and `optional(text)` in one milestone. With these, every
  archived schema is supported.
- Restore and bundle the 12 slops they unblock.
- Port the other 35 in a separate "restore sweep" milestone next.
- Write a single reference for every document type:
  `docs/reference/document-types.md`.

**Where the DSL/SDK lives:** yes, in `packages/document`.
- `src/schema.ts`: `s`, `defineDocument`, the `Value`/`Input` types, `validate`,
  `checkNode`.
- `src/handle-types.ts` and `src/async-types.ts`: the handle API types.
- `src/owner/document.ts`: handles, previews, `bindValue`, `bindText` host.
- `src/owner/text.ts`: the text binding.
- `src/owner/store.ts`: snapshot store, `readPath`, `applyOps`.

Every kind also lands in the Rust core, the TypeBox wire (`packages/schema/src/owner.ts`)
and a fixture together (AGENTS rule).

## Semantics (decided)

### Records — `s.record(value)`
- **Values:** a scalar, or an object (which may hold text, optionals and lists). Stored as
  a Loro map. A snapshot reads as a plain object `{[key]: value}`.
- **Keys:** 1–256 UTF-16 units. `$id`, `__proto__`, `constructor` and `prototype` are
  refused.
- **Paths address an entry by its key as a plain string segment**, as for object
  fields: resolution is schema-driven, so no new segment type is needed.
- **Entries behave like optional fields:**
  - `set` on `[...record, key]` creates or replaces the entry. As with optional objects,
    replacing an object entry that holds text, a list or a counter is refused with
    `exists`; edit its fields instead.
  - `clear` on `[...record, key]` deletes it; deleting an absent key is a no-op.
  - Fields of an absent entry return `path_not_found`.
  - So there are no new wire intents. The CLI uses `set`/`clear` with key paths.
- **Merge:** concurrent `put`s of one key are last-writer-wins (whole entry). Edits to
  fields of an existing object entry merge per field.
- **Issues:** an entry value that doesn't match the value kind is flagged and preserved.
- **SDK handle:** `entry(key)` (the value's handle), `put(key, value)` (= set) and
  `delete(key)` (= clear). `doc.at(entrySnapshot)` works, because `register` walks record
  entries.

### Scalar lists — `s.list(string|number|integer|boolean|enum)`
- **Storage:** a Loro movable list of plain values; a snapshot reads as an array. Optional
  elements are not allowed.
- **Wire (additive):**
  - A new path segment `{index: n}` addresses one element.
  - `insert` gains an optional `index` (default: append). `id`/`at` stay row-only.
  - `remove` makes `id` optional and gains `index` and `count` (default 1).
  - `set` on `[...list, {index}]` sets one element. `set` on the list path replaces the
    whole list, keeping unchanged positions through a common prefix/suffix plus
    per-position sets, inserts and removes (like text `set`).
  - No `move` (unused).
- **Merge:** index operations resolve through Loro's list positions. Concurrent inserts
  keep both; concurrent sets of one element are last-writer-wins.
- **Publications:** a scalar-list change publishes the whole list exactly. This reuses
  the existing list fallback in `publication.rs` `list_ops`, without flagging a rescan.
  The lists in the archive are small (≤256).
- **Issues:** an element of the wrong kind or outside its bounds is flagged; a plain
  (non-movable) list is flagged.
- **SDK handle:**
  - `insert(value, index?)`, `set(index, value)`, `preview(index, value)`,
    `remove(index, count?)`, `replace(values)`.
  - The store's `readPath`/`applyOps` accept `{index}` segments (for previews).

### `optional(text)`
- **Absent until set.**
  - `set(string)` creates the text, or edits it with the precomputed script when present.
  - `clear()` deletes it.
- **`bindText` on an absent optional text** shows `""`. The first edit sends `from: ""`,
  and `edit_text` creates the text with `to`, reporting `authored` as the owner's version.
- **After a concurrent `clear`**, an edit with a non-empty `from` returns
  `path_not_found`, and the field shows the store's value.
- **Merge:** two replicas creating the same optional text concurrently keep one whole text
  (same as optional objects; documented).
- `optional(list)`, `optional(record)` and `optional(counter)` stay refused (unused).

## Steps

### 0. Plan of record
Save this plan as `docs/CollectionsPlan.md` and link it from `docs/README.md`.

### 1. Rust core (`crates/hitslop-core`)
- **`src/lib.rs`, new `Node` variants:** `Record { value }` and `List` with a scalar
  `item`. Allow `Optional { inner: Text }`.
- **Per-kind changes:**
  - `check`: record value kinds; list item must be an object or a scalar.
  - `validate`: record key rules; scalar list elements.
  - `put`/`fill`: record → `LoroMap`; scalar list → `LoroMovableList` of values.
  - `issues`/`container_issues`/`project`/`project_container`: records and scalar lists.
- **`resolve`:**
  - A string segment under a `Record` addresses an entry. A missing entry at the final
    segment sets `absent` (reuse the optional path).
  - `{index}` under a scalar list addresses an element; out of range → `path_not_found`.
- **`execute`:**
  - `Set` covers record entries (create/replace), list elements and whole scalar lists
    (minimal rewrite).
  - `Clear` covers record entries.
  - `Insert`/`Remove` branch on the list's item kind (rows by id/anchor; scalars by
    index/count).
  - Absent optional text is created by `Set`.
- **`src/edit.rs`:** `edit_text` on an absent optional text with `from == ""` creates it
  and publishes.
- **`src/publication.rs`:**
  - `node_at` walks into `Record` values (`Index::Key`).
  - `map_ops`: removing a record entry, or an optional field, needs no rescan.
  - `list_ops`: a movable list whose schema item is a scalar publishes the whole list
    (exact `set`) without a rescan.
- **Wire (`packages/schema/src/owner.ts`, then `bun run schema:generate`):**
  - `Segment` gains `{index}`;
  - `insert` gains `index?`;
  - `remove` gets `id?`, `index?` and `count?`.
- **Tests:**
  - `fixtures/collections.json` literal scenarios:
    - record put, replace, delete and entry-field set, plus key rules;
    - scalar list insert at an index, set, remove with a count, whole-list set, bounds
      and wrong kind;
    - `optional(text)` create, edit and clear;
    - nested cases: a record in a list row, a scalar list in an object;
    - an atomic refusal.
  - `tests/conformance.rs` and the Swift and Bun fixture loops already iterate every
    `fixtures/*.json`.
  - Merge cases in `tests/collections.rs`:
    - concurrent put of one record key;
    - concurrent edits of different fields of one entry;
    - concurrent scalar-list inserts and a set;
    - concurrent optional-text creation;
    - a merged invalid entry is preserved and flagged.
  - The publication-equals-fresh-snapshot property over a random scalar-list and record
    workload, mirroring `tests/nested.rs`.

### 2. SDK (`packages/document`)
- **`src/schema.ts`:**
  - `s.record(value)`, `s.list(scalar)` and `s.optional(s.text())` (restore the types from
    `ec77377`).
  - `Value`: a record is `{readonly [key: string]: Value<V>}`; a scalar list is
    `ReadonlyArray<Value<I>>`.
  - `Input`, `validate` and `checkNode` follow.
- **`src/handle-types.ts`:** the record and scalar-list handles above (the shapes from
  `ec77377`, minus scalar-list `move`), and `optional(text)` as `TextHandle & {clear}`.
- **`src/owner/document.ts`:**
  - `makeHandle` for records (put = set, delete = clear, entry) and scalar lists
    (insert, set, preview, remove, replace = whole-list set).
  - `register` walks records.
  - The preview overlay accepts `{index}` paths.
- **`src/owner/store.ts`:** `readPath` and `applyOps` handle `{index}` segments. Records
  already work: they are string segments on plain objects.
- **Text binding** (`src/owner/text.ts` host `read` in `document.ts`): an absent optional
  text reads as `""` and stays editable. An absent required field keeps disabling.
- **Tests:**
  - `tests/collections.test.ts` over WASM: every handle round trip, a preview on a list
    element, `doc.at` on a record entry, and `bindText` creating an absent optional text.
  - `tests/types.ts`: the new handle and value types.

### 3. Fixtures and Swift
- **New `tests/fixtures/collections/`:** document, scenario and expected files, with a
  plain-JS app touching a record entry and a list element. It is replayed by Bun and by
  the Swift host-path test.
- **Native CLI test:** `LoroCLITests` covers a record `set`/`clear` by key path and a
  scalar-list insert at an index.
- No Swift production changes are expected; confirm the generated `PlatformContract`
  accepts the new segment and fields.

### 4. Restore and bundle 12 slops
- **The slops:** habit-heatmap, harada-method, morning-pages, pocket-sheet, wordle,
  alien-radio, meeting-notes, metronome-tapper, pixel-art, workout-planner,
  reading-tracker and slide-deck.
- **Move and record them:** move them to `examples/slops/`, add them to `bundled.json`
  and `examples/slops/tsconfig.json`, and update `archive/slops/provenance.json`.
- **Port them.** Reuse the checklist from the scalars milestone:
  - `text.replace` → `set`;
  - `await` insert ids, or read them inside `change`;
  - handle every write's promise;
  - `attachments.import(file, (tx, ref) => …)`;
  - `resizeWindow`.
- **Specific fixes:**
  - morning-pages: move the `$effect` writes into event handlers, or make them idempotent
    and guarded.
  - ambient-style read-modify-writes (wordle stats `put(key, old+1)`): do them inside
    `change`.
  - wordle's guesses reset: `remove(0, n)`.
- **Visual check and verification:** check each against `PRODUCT.md`, then run
  `build:templates`, `test:render` and `test:restored` (extend
  `scripts/v1/restored-smoke.ts` with one interaction per slop that exercises its new
  kind).

### 5. Documentation
- **New `docs/reference/document-types.md`**, the single reference for every kind:
  text, boolean, string, number, integer, enum, optional, counter, object,
  list(object), list(scalar) and record.
- **For each kind it covers:**
  - the descriptor and options;
  - the snapshot value and TypeScript type;
  - storage and merge behavior;
  - write rules and error codes (`type_mismatch`, `out_of_range`, `path_not_found`,
    `exists`, `duplicate_id`);
  - issues on merged anomalies;
  - the handle API (`set`/`preview`/`clear`/`insert`/`remove`/`move`/`put`/`delete`/
    `entry`/`item`/`increment`);
  - `bindText`/`bindValue` support;
  - CLI operations with JSON examples and paths (field strings, `{id}`, record keys,
    `{index}`).
- **Also covered on that page:**
  - common behavior: writes are async and resolve after the snapshot updates; `change`
    collectors; previews; `flush`; identity rules; what is not supported and why.
  - an "async patterns" section, moved from the authoring skill.
- **Link it from:** `docs/architecture.md` (replace its kinds paragraph with a link),
  `docs/README.md`, `docs/guides/authoring.md`, `docs/guides/cli.md` (operations table
  plus the new fields), `docs/engineering-contract.md` (the kinds list), the landing
  `guides/data-and-schemas.mdx`, and both skills.
- **Status lines:** `docs/ScalarsPlan.md` and `docs/CollectionsPlan.md` get "implemented"
  status lines when done, and `docs/README.md` lists what has shipped.

## Critical files
- **Rust:** `crates/hitslop-core/src/{lib.rs,edit.rs,publication.rs}`,
  `fixtures/collections.json`, `tests/collections.rs`.
- **Wire:** `packages/schema/src/owner.ts`, then `bun run schema:generate`.
- **SDK:** `packages/document/src/{schema.ts,handle-types.ts,async-types.ts,owner/document.ts,owner/store.ts,owner/text.ts}`,
  `tests/{collections.test.ts,types.ts}`.
- **Swift tests:** `Tests/HitSlopHostTests/LoroCLITests.swift`, plus the new host fixture.
- **Slops:** `examples/slops/<12 slugs>`, `examples/slops/bundled.json`,
  `scripts/v1/restored-smoke.ts`.
- **Docs:** `docs/reference/document-types.md` (new), and the pages listed in step 5.

## Verification
- `cargo test --locked --workspace` (new scenarios, merge cases, publication property).
- `bun run schema:generate && bun run check && bun run test`.
- `bun run build && bun run swift:test && bun run test:native && bun run test:native-helper`,
  plus the crash matrix.
- `bun run build:templates && bun run test:render` (17 bundled templates plus fixtures)
  and `bun run test:restored` (16 restored slops in `slop dev`).
- **Performance:** the 1k Quick Checklist benchmark still meets its targets. A
  pixel-art stroke (a 256-element list) and a habit-heatmap year (365 record keys)
  publish quickly; measure with a small `text_cost`-style example.

## Next (separate milestone)
**Restore sweep:** port and bundle the remaining 35 archived slops in batches with the
same checklist and smoke test. After that the archive is empty.
