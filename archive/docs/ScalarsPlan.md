# Milestone: scalar kinds (string, number, integer, enum, optional)

Status (2026-09-29): implemented. The five kinds, `clear`, scalar `bindValue` and
`preview` are live in the core, SDK and fixtures; small-expenses, kanban-board, recipe
and doodle-board are restored and bundled (`bun run test:restored` drives them in
`slop dev`).

## Context

The host-owned reset is complete; `docs/architecture.md` describes the live system. The
core implements text, boolean, counter, object and list(object). An archive survey of
all 51 slops found string in 39, enum in 30, integer in 19, number in 17 and optional in
15. Most are row fields set by buttons, sliders and form inputs. This milestone adds those
five kinds end to end, plus scalar `bindValue`, scalar `preview` and `clear`.

It also restores four archived slops as acceptance targets, **bundled in the app**:
- **small-expenses:** enum, integer cents as `number`, `optional(string)` in a row,
  `clear`, a key omitted in an insert.
- **kanban-board:** optional string, `optional(integer {0,999})` in a row, integer `order`,
  clear from `""`.
- **recipe:** optional numbers at the top level and in rows, a reused descriptor,
  `optional(object)` set/clear holding an attachment reference, and an enum.
- **doodle-board:** a top-level enum, and `preview` on a row string while drawing.

The user will bundle every slop eventually, so each restored slop ships.

Rule (AGENTS.md): a kind exists in the types only once Rust, the SDK and a fixture
implement it, so each kind lands in all three in this milestone.

## Semantics (decided)

| Kind | Stored as | Write rules | Read / issues |
|---|---|---|---|
| `string {maxLength?}` | plain map value (last writer wins) | string; `maxLength` in UTF-16 units | wrong type or too long → issue, value preserved |
| `number {min?,max?}` | f64 | finite; within bounds | integral values project as JSON integers (JS has one number type), so snapshots compare equal in Rust, Swift and JS |
| `integer {min?,max?}` | i64 | safe integer (±2^53−1), within bounds | out of range or non-integer → issue |
| `enum [values]` | string | one of the values (non-empty, unique strings; any characters) | unknown value → issue, preserved |
| `optional(inner)` | key absent when unset | `set` writes a value; new `clear` intent deletes the key; `clear` on an absent value is a no-op | an absent optional is valid; no issue or rescan |

- **Allowed optional inners:** string, number, integer, enum, boolean and object.
  - `optional(object)`: `set` creates it when absent, and replaces it when present, but
    only if its descriptor contains no list, counter or text (never replace
    identity-bearing collections). Otherwise the core returns `exists`, and you edit its
    fields instead.
  - Fields of an absent optional object return `path_not_found`.
  - `optional(text)`, `optional(list)` and `optional(counter)` are rejected by the
    descriptor check for now. Only 2 slops use optional text: reading-tracker, and
    slide-deck, which also needs scalar lists.
- **Inserts and `initial.json`:** may omit optional keys. `null` is never a value.
- **Merge:** scalars are last-writer-wins per key. Two concurrent `set`s of an absent
  optional object resolve to one whole object (documented).
- **Descriptor JSON:**
  - `{kind:"string",maxLength?}`, `{kind:"number"|"integer",min?,max?}`,
    `{kind:"enum",values}`, `{kind:"optional",inner}`, camelCase.
  - Validated identically in Rust (`Node::check`) and the SDK (`checkNode`).
  - Bounds must be finite with `min ≤ max`; integer bounds must be integers.
  - The schema key stays canonical JSON; no evolution.
- **Still out of scope:** records, scalar lists, trees and rich text (records in 5 slops,
  scalar lists in 8).

## Steps

### 0. Plan of record
Copy this plan to `docs/ScalarsPlan.md` and link it from `docs/README.md` (the reset
plan was kept the same way).

### 1. Rust core (`crates/hitslop-core`)
- **`lib.rs` `Node`:** add `String{max_length}`, `Number{min,max}`, `Integer{min,max}`,
  `Enum{values}` and `Optional{inner}`, with serde tag `kind`, camelCase and
  `deny_unknown_fields`. Extend these per kind:
  - `check`: bounds, enum uniqueness, allowed optional inners, depth;
  - `validate`: write-time type, bounds and `maxLength` checks;
  - `fill`: skip absent optionals; store f64 for number and i64 for integer;
  - `issues` and `container_issues`: absent optional is fine; flag type, bounds and enum
    violations;
  - `project`: optional absent → key omitted; normalize integral f64.
- **`execute`:**
  - `Set` accepts every scalar kind and optional (creating or replacing an optional
    object per the rules above).
  - New `Clear { path }` deletes an optional key; it is refused on required fields.
  - `insert_row` validates rows with optional keys omitted.
- **`publication.rs` `map_ops`:** removing a declared optional key must not force a
  rescan. `clean_value` already follows `issues`.
- **Wire:** add `clear: { path }` to `variants` in `packages/schema/src/owner.ts`. The
  generator (`scripts/v1/rust-contracts.ts`) already emits variants; run
  `bun run schema:generate`.
- **Tests:**
  - New `fixtures/scalars.json` with schema, initial and literal scenarios in the
    `checklist.json` format (set, clear, bounds, UTF-16 `maxLength`, enum refusal,
    integer range, number normalization, optional object set, replace and refuse,
    insert with an omitted optional, and a batch that fails at op 2 leaving no partial
    state).
  - Make `tests/conformance.rs` loop over every `fixtures/*.json`.
  - Add a merge case (concurrent optional-object create, and a concurrent scalar set)
    to `tests/nested.rs` or a new `tests/scalars.rs`.

### 2. SDK (`packages/document`)
- **`schema.ts`:**
  - Restore the builders `s.string/number/integer/enum/optional`.
  - Restore the optional-key `Input`/`Value` types (`OptionalKeys`, `ObjectInput`)
    from commit `ec77377:packages/document/src/schema.ts`.
  - Extend `validate` (used by `packages/cli/src/build.ts` and
    `scripts/v1/template-cache.ts` for `initial.json`) and `checkNode`.
- **`handle-types.ts` and `async-types.ts`:**
  - `ScalarHandle<V> = {set, preview}`. An optional scalar adds `clear()`.
  - An optional object handle is its object handle plus `{set(value), clear()}`.
  - `preview` stays synchronous in `AsyncHandle`.
- **`owner/document.ts`:**
  - `makeHandle` covers the new kinds. `clear` sends `{type:"clear"}`.
  - **Preview overlay:** a map from path to value applied over `store.state.value`
    with the existing `applyOps`.
    - A `set` for that path, or `flush`, removes it. `flush` and the barrier commit all
      previews in one batch.
    - Previews count as pending in status, and notify only their own path.
    - Adapt the implementation from `ec77377:.../owner/document.ts`, which had
      boolean-only previews.
- **`bindValue`, extended from boolean to all scalars:**
  - Checkbox → boolean. `<select>` → enum or string.
  - `range` and `number` inputs → number or integer via `valueAsNumber`.
    - `range`: preview on `input`, set on `change`.
    - `number`: set on `change`. An empty or invalid value clears an optional field and
      reverts a required one.
  - Text, date and time inputs → string: set on `input`, coalesced to one write in
    flight with the latest value winning. An empty string clears an optional field.
  - Refused values revert to the store value; the rejection is reported centrally.
- **Store:** a `remove` op is already applied (`owner/store.ts`); no change needed.
- **Tests (`tests/owner.test.ts` and `types.ts`):**
  - Each kind's handle round trip over WASM.
  - `clear` and an optional insert.
  - The preview overlay: shown locally, never in the core until `set`/flush, removed
    on accepted set, committed by the barrier.
  - `bindValue` for range, number, select, text and optional-empty.
  - Compile-time checks for optional keys, `clear` availability and enum literal types.
- **`fixtures.test.ts`:** replays the new host fixture.

### 3. Fixtures and Swift
- **New `tests/fixtures/scalars/`** (`document/` with a plain-JS `assets/app.js` in the
  style of `4-1`, `expected.json`, `scenario.json`). It is replayed by Bun
  (`fixtures.test.ts`) and by Swift (`OwnerHostPathTests`).
- **Swift needs no semantic change.** `DocumentOwnerTests.nativeBindingExecutesLiteralFixtures…`
  should iterate every `crates/hitslop-core/fixtures/*.json`. Check that the generated
  `PlatformContract` validates the new `clear` intent in the socket `batch` path
  (`OwnerCommands.request`).
- **CLI:** `set`/`clear` work through the existing socket `apply`/`batch`. Add one native
  CLI test (`LoroCLITests`) for `clear` and an out-of-range refusal ("Not applied.").

### 4. Restore the four slops
- **Move and bundle.** Move `archive/slops/{small-expenses,kanban-board,recipe,doodle-board}`
  to `examples/slops/`, add them to `bundled.json`, and update
  `archive/slops/provenance.json`/README.
- **Port to the async API and current SDK** (the skill's "async patterns" list):
  - small-expenses: `await items.insert(...)` before using the id.
  - kanban-board: compute `order` inside a `change(tx)` collector.
  - recipe: attachments via `attachments.import(file, (tx, ref) => tx.fields.photo.set({...}))`.
  - doodle-board:
    - replace `globalThis.slop.window.resize` with `resizeWindow`;
    - keep the stroke geometry local while drawing, or `await` the insert and then
      `preview` it;
    - commit on pointerup.
  - Remove any other use of APIs that no longer exist (`text.replace`, the
    `{commit}` attachment option, synchronous ids).
- **Visual check** against `examples/slops/PRODUCT.md` and `docs/guides/authoring.md`
  in `bun run slops:dev`. Run the template build and cache (`bun run build`,
  `test:render`) so each builds, opens, exports PNG/PDF and reopens.

### 5. Docs and skills
- **Re-add the kinds** in `docs/guides/authoring.md`, the landing
  `guides/data-and-schemas.mdx` table (with `bindValue` and `preview` examples),
  `docs/architecture.md`, `AGENTS.md`/`docs/engineering-contract.md` ("today: …"), and
  `packages/cli/skills/hitslop-authoring` and `hitslop-document` (`clear`, bounds,
  optional rules).
- **CLI operations table** in `docs/guides/cli.md`: add `clear`, and `set` for scalars.

## Critical files
- **Rust:** `crates/hitslop-core/src/{lib.rs,publication.rs}`, `fixtures/scalars.json`,
  `tests/conformance.rs`.
- **Contracts:** `packages/schema/src/owner.ts`, then `bun run schema:generate`.
- **SDK:** `packages/document/src/{schema.ts,handle-types.ts,async-types.ts,owner/document.ts}`,
  `tests/{owner.test.ts,types.ts,fixtures.test.ts}`.
- **Swift tests:** `Tests/HitSlopDocumentTests/DocumentOwnerTests.swift`,
  `Tests/HitSlopHostTests/LoroCLITests.swift`.
- **Slops:** `examples/slops/{small-expenses,kanban-board,recipe,doodle-board}`,
  `examples/slops/bundled.json`.

## Verification
- `cargo test --locked --workspace` (new scenarios and merge cases).
- `bun run schema:generate && bun run check && bun run test` (WASM kinds, bindValue,
  preview, type checks, fixture replay).
- `bun run build && bun run swift:test && bun run test:native && bun run test:native-helper`,
  plus the crash matrix.
- `bun run test:render --fixtures` and the template render for the four restored slops.
- **Manual (`bun run slops:dev`):** add and clear a note in Small Expenses; switch the
  currency; set and clear a kanban lane limit; set and replace a recipe photo; draw a
  doodle (preview then commit).
- **Performance:** the 1k Quick Checklist benchmark still meets its targets (open ≤1 s,
  checkbox p95 ≤50 ms).
