# Loro sync pass: Loro main, undo via `revert_to`, one text-merge path, coarser rewrites

**Status (2026-10-05):** implemented, uncommitted; `bun run verify --native` passes.
Remaining: the by-hand checks below.

## Why

- Loro 1.16.2 (crates.io's newest) panicked in `revert_to` on movable lists, so undo
  reconciled through `replace`. A malformed movable-list update also panicked inside
  Loro's locks and aborted the process.
- Loro main `c00c9fa` (2026-09-30, still main's head) fixes both, with an unchanged
  public Rust API.
- An agent's text `set` replaced the field as the owner held it, deleting whatever the
  person typed after the agent's `get`. Character-level diffs of rewrites kept stray
  matching letters ("chaff"), so a concurrent keystroke could land inside the new word.

## What changed

1. **Pin.** `loro = { git, rev = "c00c9fa…" }`, with only the seven Loro crates changing
   source. `docs/roadmap.md` tracks the return to crates.io.
   - Probed: with 400 edits after a cut, 1.16.2 keeps 31 KB of deleted rows in a cut
     before the latest version; main keeps 2.1 KB. The close-time trim keeps its policy,
     and its comment now gives the real reason (undo is session-only).
   - `fork_at` is still unimplemented for trimmed documents on main, so `replica_at`
     stays; its comment now says so.
2. **Undo is `revert_to`.** `restore()` and `theme::restore` are deleted. Refusals map
   `SwitchToVersionBeforeShallowRoot`/`FrontiersNotFound` to `stale_base`; Loro rolls
   back a failed revert itself.
3. **One text-merge path.**
   - A batch takes `base`, and a text `set` takes `from` and `selection`.
   - A based set merges from `base`. `from` is the given text, else the field at `base`.
   - Paths: fast when the field is unchanged, otherwise the merge path, which is
     `edit_text`'s body moved.
   - A set carrying `selection` is the page's edit, alone in its batch. Its reply adds
     `authored` and the merged selection.
   - The socket `batch` takes `base` and replies with `version`; the CLI has
     `apply`/`batch --base` and prints `version`.
   - Deleted: `EditText`, the page `text` method, `Request::Text`/`Reply::Text`, the FFI
     and WASM text entry points, and the shell transport's `text`.
   - Same-batch edits merge like any concurrent edit; the old plan's "replace from
     current" special case was dropped.
   - Agent merges carry the agent commit message.
4. **Semantic cleanup.** `window()` folds a kept run between edits, when it is no longer
   than the larger edit on each side, into the replacement (diff-match-patch's
   `cleanupSemantic` core). Refined diff stays off, and the comment says why.
5. **Docs.**
   - README, `docs/architecture.md` (Text, Undo, trim);
   - `docs/reference/document-types.md`, `docs/guides/cli.md`, `docs/reference/runtime.md`;
   - the landing data guide and the `hitslop-document` skill.

## Evidence

- New tests:
  - `text.rs`:
    - `an_agents_set_from_its_read_keeps_what_the_person_typed_since` (without `base`: "Buy oat milk");
    - `a_rewrite_keeps_its_new_word_whole_beside_a_concurrent_keystroke` (before step 4: "lauXndry");
    - `a_correction_and_a_capitalization_of_one_word_both_apply`;
    - `a_based_batch_refused_after_its_merge_changes_nothing`;
    - `text_set_fields_are_refused_where_they_do_not_apply`.
  - `command.rs`: `a_based_batch_keeps_text_written_since_the_agents_read`.
- The model's agent batches now carry `base`. A temporary probe counted hundreds of
  merge-path hits from agents and page typists per run.
- `bun run verify --all`, `bun run verify --native` and `bun run core:test:extended` pass.

## Not in this pass

- A based `replace`/`slop import`: it still rewrites text from the current value.

## By hand

- Type in a field while `slop apply --base V` rewrites it; ⌘Z both steps.
- Undo a row removal, a move and a palette change in the app.
