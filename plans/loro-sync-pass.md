# Loro sync pass: Loro main, undo via `revert_to`, one text-merge path, coarser rewrites

## Context

Two questions started this: does text merge character by character, and should that be
coarser? A deep dive into Loro main (`_docs/loro-main`) then asked what the newer Loro
lets us simplify. Breaking changes are fine before launch.

**Status (2026-10-05):** planned and not started. Decisions:
- fold the page `text` request into `apply`;
- start only after Phase C (single writer) of the pre-launch follow-up lands. The
  `pre-launch-simplification-deviations` session is editing `execute.rs`, `lib.rs`,
  `text.rs` and `check.rs` right now.

Before step 1:
- confirm that session is idle and that `issues` and `Document::import` are gone;
- then re-read `lib.rs`, `execute.rs` and `text.rs`.

Step 3 overlaps Phase E (one dispatcher). If E hasn't run yet, do step 3 in Rust and
schema terms only, and leave the Swift page plumbing to E.

## Findings

### Text merges per character, but in three layers

Loro 1.16.2 and main have identical code for this: `diff/diff_impl.rs`,
`handler/text_update.rs` and `container/richtext/tracker/crdt_rope.rs`.

1. **CRDT: one ID per Unicode scalar. Keep it.**
   - In Fugue integration (`crdt_rope.rs` `insert`), `origin_left` is the visible
     character before the position and `origin_right` is the next element, tombstones
     included. Concurrent inserts with the same origins are ordered by peer ID.
   - So concurrent typing never interleaves. An insert at the position of a deleted span
     lands *before* that span's tombstones (`get_origin_left_and_right_among_tombstones`).
   - Runs are RLE-encoded, so the cost is small (`docs/evidence/long-text-2026-09-30.json`).
   - Coarser units are worse:
     - word-level duplicates a word when two people fix different letters in it;
     - line-level conflicts across whole paragraphs;
     - whole-field is last-writer-wins, which `s.string()` already provides.
2. **Typing: exact. Keep it.** `script()` (`crates/hitslop-core/src/text.rs:56`) retains
   the common prefix up to the caret plus the common suffix, so a keystroke is one splice
   and no diff runs.
3. **Replacement window: too fine.**
   - Where a span both deletes and inserts (paste over a selection, autocorrect,
     `text.set`, CLI `set`), `window()` (`text.rs:89`) runs Loro's character-level Myers
     diff (`LoroText::update`, which mutates while it diffs, hence the scratch doc).
   - On a rewrite, Myers keeps letters and spaces that happen to match ("chaff"). The
     resulting text is exact, but a concurrent insert anchored inside that span lands
     mid-word.
   - Example: "dry cleaning"→"laundry" can retain `l`, `a` and `n` of "cleaning", so a
     concurrent keystroke after "cle" ends up inside "laundry".
   - **Loro's built-in fix is off, correctly.** `use_refined_diff` (`dj_diff`, used for
     windows under 128×128) prices gaps affinely: +8 to open an insert or delete run, +1
     per character, +1 to re-enter a match. For "one two t"→"One two T", a full
     replacement costs 34 and the precise script 37. So it merges the two small edits
     into one replacement, which breaks
     `disjoint_edits_coalesced_in_flight_merge_around_a_concurrent_change`. An absolute
     gap penalty is wrong for short fields.
   - **The right rule is relative:** diff-match-patch's `cleanupSemantic`. Remove a
     retained run between edits when it is no longer than the larger edit on each side.
     - "ne two " (7) between 1-character edits stays.
     - Single-letter chaff between edits of 3–5 characters goes.

### The agent gap is bigger than granularity

- CLI `set` on text has no base. It diffs from the field as it is when the owner runs it
  (`execute.rs`, text branch of `Intent::Set`).
- So typing done between the agent's `get` and its `set` is deleted unless the agent's
  value contains it. Only keystrokes still in flight (milliseconds) merge, through the
  page's `edit_text` slow path.
- README:28 ("your agent's edit doesn't wipe out what you're typing elsewhere in the same
  field") promises more than this.

### What Loro main changes for us

- **Versions:**
  - crates.io: the newest release is 1.16.2, our pin.
  - Main `c00c9fa501f8d32f68d6255eacb7035a67fb6ab6` (2026-09-30) is the source of npm
    loro-crdt 1.16.3 and 1.16.4.
  - The Rust workspace on main still says 1.16.2, and its public Rust API is unchanged
    (only doc comments differ); all fixes are behavioral.
  - `_docs/loro-main` matches GitHub main.
- **Fixes we use:**
  - **`revert_to` works.**
    - It's all-or-nothing now (1b38000), and movable-list apply-diff and undo
      `OutOfBound` panics are fixed (cdbc239).
    - Reverting past a deleted key no longer adds a mergeable child twice (1eb14b3).
    - It also validates frontiers against shallow roots before checking out (an error,
      not a panic).
    - Our `restore()` reconciles through `replace` only to avoid that panic (`lib.rs:710`,
      `tests/undo.rs:402`), and `docs/architecture.md:150` already says undo uses
      `revert_to`.
  - **Bad input no longer kills the process.**
    - On 1.16.2, a malformed movable-list `move` or `set` panicked inside Loro's locks,
      poisoned them, and aborted during unwind, which `catch_unwind` (owner, FFI) can't
      contain (5c7552d).
    - Out-of-range text ops (892ed30), reused op IDs, counter gaps and unparsable change
      blocks now return errors.
    - Phase C's `check.rs` validates stored state *after* import, so import has to
      return first.
  - **Snapshots are correct and smaller.**
    - State-only and shallow exports no longer keep map and list containers deleted
      before the root (8ff700f, 8488567).
    - They now record the right map writer in the root state (824bb69): checkout used to
      keep a later op's lamport and peer when the value was equal at both versions.
    - That makes `branch_at` exports and Phase F's capture fork smaller and correct.
    - It likely explains the `fork_at` divergence noted at `lib.rs:806` and in
      `tests/replica.rs`.
  - **Failed imports roll back cleanly:** later containers no longer inherit state.
- **Checked, and they stay:**
  - `replica_at`: `fork_at` exports `SnapshotAt` with `reject_partial_history`, so it
    refuses trimmed documents. And `abort` must discard uncommitted operations, which
    Loro has no API for.
  - `release()`/`empty()` for mergeable children: `retain_created_after_root` keeps every
    non-`Normal` container ID, and mergeable children are `Root` IDs in the `🤝:`
    namespace, so trims never prune them. Phase C deletes this code anyway.
  - The `decode_version` guard: it's cheap, keeps our `invalid_version`/`stale_base` codes,
    and keeps unknown IDs away from Loro.
  - `open_with`'s per-update import, which measured faster than `import_batch`.
  - Publication's filter for containers removed in the same change.
  - `branch_at`'s state-only export: `fork_at` would copy all history and refuses trimmed
    docs.
- **Watch:**
  - A root container touched by history now appears in the replayed value even when
    empty (1b38000). We read `data`, `theme` and `meta` by name, so this shouldn't
    matter; the compat corpus will tell.
  - Checkpoint byte sizes in `tests/compat/dev` may move.

## Plan (after Phase C lands)

### 1. Pin Loro main
- In `Cargo.toml`, set `loro = { git = "https://github.com/loro-dev/loro", rev = "c00c9fa501f8d32f68d6255eacb7035a67fb6ab6" }`.
  Every crate has `publish = false`, and the rev is an exact pin, as
  `engineering-contract.md` requires. Run `cargo update -p loro`.
- Run every suite plus `bun run test:compat`. Recapture `dev` (`bun run compat:capture dev`)
  if storage sizes move; it's replaceable before launch.
- Update the 1.16.2 notes:
  - `crates/README.md:14` and `BenchmarkTests.swift:106` (pin metadata);
  - the trim claim at `store.rs:309` and `docs/architecture.md:207`: probe what a cut
    before the latest version keeps now;
  - the `replica_at` comment: its reason is now that `fork_at` refuses trimmed history.
- Add a `docs/roadmap.md` item: return to `=1.16.x` once crates.io publishes it.

### 2. Undo is `revert_to`
- `history()` calls `self.doc.revert_to(&target)` and maps
  `SwitchToVersionBeforeShallowRoot` to `stale_base`.
- Delete `restore()`, `theme::restore` and the state-only export to JSON. The theme root
  reverts with the document.
- Oracle: `redoing_inserted_and_moved_rows_after_a_removal_restores_them` (which panicked
  on 1.16.2), the rest of `tests/undo.rs`, and `tests/model.rs`. If any panics on main,
  keep `restore()` and report.

### 3. One text-merge path for page and agents (breaking)
- **Wire** (`packages/schema/src/core.ts`, page and socket schemas; then
  `bun run schema:generate`):
  - the batch gains an optional `base` (version token);
  - a text `set` gains optional `from` and `selection` (`{start, end}`, UTF-16);
  - the reply gains `authored` and, for a batch carrying a selection, the mapped selection;
  - at most one text set per batch may carry `selection`;
  - the socket `batch` request gains `base`, and `command.rs` forwards it.
- **Delete:** `EditText`, the page `text` method and reply, `Request::Text`, FFI
  `edit_text`/`TextResult`, and their Swift and shell plumbing.
- **Core** (`execute.rs`, `text.rs`, `lib.rs` `apply_batch`):
  - decode `base` once (`decode_version`, `floor` → `stale_base`);
  - for a based text set, `from` is the given `from`, or else the field's text at `base`
    (`branch_at` + `text_at`);
  - if the current text equals `from`, apply `script(from, value, caret)` directly;
  - otherwise check container identity (`base_vv.includes_id`), author the script on
    `branch_at(base)`, import, and map the selection through cursors. This is today's
    `edit_text` slow path, moved rather than copied;
  - fields created after `base`, or already set earlier in the batch, are replaced from
    their current text, as an unbased set is;
  - `abort` covers refusals, including after an import.
- **Undo grouping:**
  - a page batch holding one text set with `from` continues the typing run
    (`record_typing`);
  - a merged one is its own step;
  - agent batches still form runs.
- **Shell:** `bindText` (`packages/shell/src/owner/text.ts`) sends
  `apply {base: confirmed.version, intents: [{type: "set", path, value: sent, from: confirmed.text, selection}]}`
  and reads `authored` and the selection from the reply. The rest of the binding is
  unchanged.
- **CLI:** `apply`/`batch --base VERSION` (`packages/cli/src/documents.ts`), where
  VERSION is `version` from `get --snapshot`.

### 4. Semantic cleanup in `window()`
- After Loro's Myers delta, fold each retained run between edits that is no longer than
  the larger edit on each side into a delete plus an insert. Repeat until stable, then
  merge adjacent deletes and inserts.
- Leave out word-boundary shifting and overlap extraction.
- Keep the replay proof, the 50 ms limit and no new dependency: `dissimilar` has no
  deadline.
- Replace the `use_refined_diff: false` comment with the reason (affine gaps merge small
  disjoint edits).

### 5. Docs
- README:28/:278: per-character merging; an agent's rewrite keeps your typing.
- `docs/architecture.md`:
  - Text: granularity (three layers, why refined diff is off) and one merge path for page
    and agents;
  - Undo: `revert_to`.
- `docs/reference/document-types.md` Text; `docs/guides/cli.md` `--base`; landing
  `guides/data-and-schemas.mdx:80`; `docs/reference/runtime.md` page protocol.
- `.agents/skills/hitslop-document/SKILL.md`: read with `get --snapshot`, then pass
  `--base` when rewriting text.
- `docs/roadmap.md`: the crates.io pin item.

## Verification
- `crates/hitslop-core/tests/text.rs`:
  - a rewrite `set` plus a concurrent page keystroke inside the rewritten word: the new
    word stays whole and the keystroke lands at its edge. Pick the example by confirming
    it fails on today's script, before step 4;
  - a small correction plus a concurrent capitalization of the same word gives "Receive",
    without duplication;
  - the existing merge tests (`queued_edits…`, `two_bindings…`, `disjoint_edits…`,
    `repeated_characters…`, emoji/surrogates, `bad_bases…`), now sent as batches.
- Based-set tests:
  - read V, the page types " and eggs" (acknowledged), then an agent set "Buy oat milk"
    from V gives "Buy oat milk and eggs";
  - an unchanged field takes the fast path;
  - a trimmed document with a base before `floor` gets `stale_base` and stays unchanged;
  - a refused later intent leaves the document unchanged;
  - undoing the agent's step keeps the person's typing.
- Commands:
  - `cargo test --locked --workspace`
  - `bun run schema:check && bun run check && bun run test`
  - `bun run build && bun run swift:test && bun run test:native`
  - `bun run test:compat`
- By hand: type in a field while `slop apply --base` rewrites it, then ⌘Z both.
