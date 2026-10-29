# Exam Deck: stored data

Exam Deck keeps a deck of flashcards scheduled with FSRS-6 (ts-fsrs 5.4.2) so the cards are
learned by an exam date. This page explains every stored field and why it is stored. The
schema is a long-term compatibility surface: after launch, a newer build must open every
document a released build wrote, so fields are added only through the schema-evolution
rules that exist by then, never renamed or reinterpreted.

## Principles

- **Plain data only.** Numbers, strings, enums and ISO 8601 times. ts-fsrs `Card` objects,
  `Date`s and the `FSRS` instance are built inside `fsrs.ts` from stored data and discarded.
- **The card fields decide scheduling.** The review log is history: it can rebuild a card
  (`replay`), but nothing needs it to schedule the next review.
- **The parameters are stored with the deck.** A card's stability and difficulty only mean
  something under the weights that produced them. Storing them means a later library
  default, or a later app version, cannot reinterpret old reviews.
- **Times are UTC instants; the exam is a local calendar date.** Instants are written by
  `Date.prototype.toISOString()` (`2026-10-31T15:30:00.000Z`, millisecond precision), the
  same value ts-fsrs computes with, so nothing is rounded on the way in or out.
- **Derived values are never stored.** Today's count, the 14-day forecast, retrievability
  and the "ready by exam" figure are computed from the snapshot and a supplied `now`.

## Deck settings

| Field | Type | Why |
| --- | --- | --- |
| `title` | string, 1–80 | The deck's name in the window and export. |
| `examDate` | optional `YYYY-MM-DD` | The exam's local calendar date. A date, not an instant: an exam is "on the 14th" wherever the person is. Optional because a template's initial values cannot know a future date; while it is absent the app assumes an exam `ASSUMED_EXAM_DAYS` (28) days after today and says so. A written exam date never moves. |
| `dailyMinutes` | integer, 1–600 | The study budget. Minutes are what a student plans in. |
| `secondsPerCard` | integer, 3–600 | Turns the budget into a card cap: `floor(dailyMinutes × 60 / secondsPerCard)`. Stored (rather than measured) so the cap is explicit, editable and identical in every view. |
| `newPerDay` | optional integer, 1–1000 | A fixed new-card rate. Absent means automatic pacing: the remaining new cards are spread evenly until two days before the exam, recomputed each day. Absent rather than a magic 0. |

## `scheduler`: the algorithm parameters

All eight values go to ts-fsrs exactly as stored (`fsrs.ts` `scheduler()`).

| Field | Type | Why |
| --- | --- | --- |
| `algorithm` | enum `"fsrs-6"` | Names the model the weights and card states belong to. A future model is a new enum value, and the app that reads it must keep handling `"fsrs-6"` decks (or replay their logs). |
| `w` | list of 21 numbers | The FSRS-6 weights. The template writes ts-fsrs 5.4.2's `default_w` as a literal copy (`fsrs6DefaultWeights` in `schema.ts`), never an import, so a library upgrade cannot change a saved deck. A later optimizer may write personal weights here. |
| `requestRetention` | number, 0.7–0.99 | The target probability of recall when a card comes due; the person's "target retention". |
| `maximumInterval` | integer days, 1–36500 | ts-fsrs `maximum_interval`. |
| `enableFuzz` | boolean | Written `false`. Fuzz randomizes intervals; off keeps schedules and forecasts reproducible from the stored data. (ts-fsrs seeds fuzz from the review time and card state, so even `true` would replay identically, but the app never sets it.) |
| `enableShortTerm` | boolean | Whether same-day learning steps apply (ts-fsrs `enable_short_term`). Written `true`. |
| `learningSteps` | list of `"<n>m|h|d"` | Same-day steps for new cards; written `["1m", "10m"]` (the 5.4.2 default, copied). |
| `relearningSteps` | list of `"<n>m|h|d"` | Same-day steps after a lapse; written `["10m"]`. Its length also bounds two weights (see below). |

What the Rust owner cannot enforce, `fsrs.ts` `paramsProblem()` does, and the app refuses to
schedule with (never rewrites) parameters that fail it:

- `w` has exactly 21 finite entries.
- `w` is unchanged by ts-fsrs `clipParameters`. ts-fsrs silently clamps out-of-range weights
  (`w[17]`/`w[18]` depend on `relearningSteps.length`); a stored weight it would change is
  refused, so the scheduler never runs on values other than the saved ones.
- Every step matches `^\d+(\.\d+)?[mhd]$`.

## `cards`: one row per flashcard

A list of rows with stable `$id`s; list order is the order new cards are introduced. The FSRS
fields mirror ts-fsrs 5.4.2's `Card` interface, checked against `dist/index.d.ts`:

| ts-fsrs `Card` | Stored as | Notes |
| --- | --- | --- |
| — | `front`, `back`: text | The prompt and answer, as collaborative text. |
| `state: State` (0–3) | `state`: enum `new`, `learning`, `review`, `relearning` (default `new`) | Names rather than numbers: readable to agents and the CLI, and immune to an enum renumbering. ts-fsrs's own `TypeConvert` accepts the same names. |
| `due: Date` | `due`: optional ISO instant | Absent exactly while `state` is `new`: a new card has no due date, and ts-fsrs does not read `due` for a new card (the wrapper supplies `now`). This keeps the template free of fixed timestamps that would go stale. |
| `stability: number` | `stability`: number ≥ 0 (default 0) | Days until recall falls to 90%. Stored at full f64 precision. |
| `difficulty: number` | `difficulty`: number 0–10 (default 0) | 1–10 once reviewed; 0 while new. |
| `scheduled_days: number` | `scheduledDays`: integer ≥ 0 (default 0) | The interval the last review chose (0 during same-day steps). ts-fsrs only writes it, but it is part of `Card` and shows "next in N days" without recomputing. |
| `learning_steps: number` | `learningStep`: integer ≥ 0 (default 0) | The index into the current learning or relearning steps. Singular, so it is not confused with `scheduler.learningSteps`. |
| `reps: number` | `reps`: integer ≥ 0 (default 0) | Reviews, including same-day steps. Also tells whether the log is complete for this card. |
| `lapses: number` | `lapses`: integer ≥ 0 (default 0) | Times forgotten after graduating. |
| `last_review?: Date` | `lastReview`: optional ISO instant | Absent while new. ts-fsrs derives elapsed days from it. |
| `elapsed_days: number` | **not stored** | Deprecated in ts-fsrs 5 (removed in 6.0) and recomputed from `last_review` at every review; the wrapper passes 0. The proof shows the result is identical either way. |

Invariant the schema cannot express, checked by `fsrs.ts` `toCard()` (it throws
`ScheduleError`, and the app shows the card as unreadable rather than guessing): a card whose
`state` is not `new` has both `due` and `lastReview`.

A new card is inserted with just `{ front, back }`; the defaults above are written at insert
time. Reviewing writes all nine FSRS fields in one change, together with the log entry.

## `reviews`: the review log

`{ card, rating, at }`, oldest first, one row per answer (same-day steps included).

| Field | Type | Why |
| --- | --- | --- |
| `card` | string | The reviewed card's `$id`. |
| `rating` | integer 1–4 | 1 Again, 2 Hard, 3 Good, 4 Easy (ts-fsrs `Grade`; `Manual` is never logged). |
| `at` | ISO instant | When the answer was given; the `now` passed to ts-fsrs. |

Why keep it: it counts today's work against the budget; it rebuilds any card with
`replay(reviews)` (which reproduces the stored state exactly under the stored parameters,
and reschedules from history under new ones); it is the input a future weight optimizer
needs; and if two copies ever merge two reviews of the same card, the log is how the card
can be repaired. ts-fsrs's own `ReviewLog` fields (stability, due, elapsed days…) are not
stored: they are all reproducible by replay.

**Growth and compaction policy.**

- Append-only in normal use. Measured: about 155 bytes per review in the saved file
  (50,000 reviews ≈ 7.8 MB, under the 32 MiB document limit).
- At most `REVIEW_LOG_LIMIT` (20,000) rows, about 3 MB: a 1,000-card deck reviewed through a
  whole semester. When a review would exceed the limit, the same change removes the oldest
  row, so the log stays at the limit. Scheduling is unaffected; replay is then offered only
  for cards whose log is still complete (`logComplete`: the card has exactly `reps` log rows).
- Removing a card removes its log rows in the same change. Editing a card's text keeps them.
- No other compaction. Old rows are never summarized or rewritten.

## Verified (phase A proof, run outside the repo)

Against a document built and created with the real Rust owner (`tiny build`, `tiny create`,
`tiny import`, `tiny get`):

- The stored weights equal ts-fsrs 5.4.2 `default_w`, and `clipParameters` leaves them
  unchanged; an out-of-range weight and a 19-weight list are refused.
- 206 seeded reviews through the wrapper, which only ever sees JSON-cloned stored data, agree
  with a native ts-fsrs `Card` chain at every step (so omitting `elapsed_days` and the absent
  `due` of new cards change nothing).
- The resulting deck (new, learning, review and relearning cards) round-trips every FSRS
  field exactly through Rust, Loro and SQLite; the read-back cards give the same next state as
  the native cards for all four ratings and the same retrievability; `replay` of each card's
  log reproduces it.
- `forecast()` returns identical output for the same input, for the pre-store and read-back
  documents, for reordered object keys, and in a separate process; it does not mutate input.
  It takes 2 ms for 24 cards and about 35 ms for 1,000 cards with the exam 19 days out
  (about 135 ms at 120 days), 8 simulated futures.
- Storage: 50,000 reviews appended in batches of 1,000 gave a 7.8 MB file (checkpoint
  7.7 MB); `tiny get` read it in 1.7 s with the debug engine.

## Open risks

1. **Concurrent reviews of one card.** Each field merges independently, so two copies that
   review the same card offline could merge into a mix of two states. Collaboration is
   deferred; the log keeps both reviews, so a merge can be repaired by replay.
2. **Raw edits.** The CLI's `apply`/`batch` can write a reviewed card without `due`, a bad
   ISO string, or 20 weights. Rust accepts them (the schema cannot express these rules);
   `fsrs.ts` refuses to schedule with them and the UI must show that instead of crashing.
   Phase B commands should be the documented path for agents.
3. **Day boundaries.** ts-fsrs counts elapsed days in UTC calendar days; the app counts
   "today" in local days from a fixed UTC offset passed in, which can be an hour off across a
   daylight-saving change inside the forecast. Neither affects stored data. Commands have no
   timezone (`ctx.now` only), so a command that reports "today" needs an offset argument.
4. **The forecast is a model, not a promise.** New and learning cards are assumed answered
   Good; review cards are recalled with ts-fsrs's probability. "Ready by exam" is the mean
   predicted recall across all cards on exam morning (unseen cards count 0) and is
   optimistic for hard material. The UI should word it as an estimate.
5. **Intervals past the exam.** FSRS will happily schedule a card after the exam. Capping
   due dates at the exam is a phase B product decision; it would be derived behavior, not a
   schema change.
6. **ts-fsrs in commands.** ts-fsrs patches `Date.prototype` when imported. A throwaway
   probe command (removed again) imported `fsrs.ts`, reviewed a card with `ctx.now` and wrote
   it through `tiny call`; it worked in the command sandbox. Recheck if the sandbox ever
   freezes intrinsics.
7. **Library upgrades.** A new ts-fsrs could change the FSRS-6 formulas or rounding even
   with the same weights. Before upgrading, replay a stored deck's logs and compare with the
   stored cards (the phase A proof does exactly this).
8. **Bounds are permanent.** The ranges above (retention 0.7–0.99, budget 1–600 minutes,
   3–600 seconds per card) cannot be widened for released documents without schema evolution.
