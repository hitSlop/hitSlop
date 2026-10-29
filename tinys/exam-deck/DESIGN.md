# Exam Deck

## Job
Get a deck of facts memorized by an exam date. Cards are scheduled with FSRS-6 (ts-fsrs 5.4.2)
using the parameters stored in the document; `SCHEMA.md` explains every stored field.

## Readout
"N cards today" in large pencil numerals: the due and new cards that still fit today's budget,
with due / new / done beside it. Under it, a forecast strip of daily load (synchronous SVG bars) for
min(14 days, days until the exam), and a "Ready by exam" figure: expected recall on exam
morning and how many cards reach the target retention. It is labelled an estimate and says
it assumes the planned cards get studied; it is never worded as a promise. Without an exam
date the app says it assumes an exam 28 days out (`ASSUMED_EXAM_DAYS`).

The budget (minutes ÷ seconds per card) caps today's count and the forecast. When the cap
cuts the day short, the Today box says so in a red-ruled note ("Budget full: … don't fit
today") and over-budget forecast days get a `!`. The bars count every answer, so a day can
rise above the cap line with same-day repeats of new or missed cards, which are never cut;
the legend says this when it happens. Scheduling itself is not capped at the exam.

## Object
A machine-read answer sheet: cream form paper, printed parts in a red drop-out ink, black
timing marks down the left edge, write-in boxes for Subject and Exam date, and boxed
sections with ink tabs. Everything the student "writes" (numbers, card text) is graphite
pencil. The four answer buttons are answer bubbles (1 Again, 2 Hard, 3 Good, 4 Easy) that fill
with graphite on hover or focus, each showing the next interval from `fsrs.ts` `preview`.

## Palette (tiny.ts theme, every key read by the CSS)
`paper` sheet, `fiber` paper flecks and icon edge, `form` printed ink (labels, rules, cap
line, warnings), `tint` printed fills and new-card bars, `graphite` pencil marks, primary
buttons and review bars, `pencil` secondary text, `timing` timing marks and shadows. The chart
reads them as `var(--tiny-*)`, so the theme panel recolors the chart. No dark mode: the
object is paper.

## Type
Barlow Condensed SemiBold for everything printed on the form; Kalam for everything
handwritten (the big count, card text, inputs). Both are bundled through Vite under the
SIL OFL (`assets/fonts`). Kalam is a Latin subset; rarer glyphs (₂, ⁺) fall back per glyph.

## Interaction
- Study session: the card's front, then Show answer (Space or Enter), then the bubbles
  (keys 1–4; after revealing, focus sits on Good, so Space again means Good). Esc ends the
  session. Same-day learning steps that are due come first and are never capped; then due
  reviews and new cards while the budget lasts; then a step due within 20 minutes. When the
  budget is spent the session says so and offers "Study N more anyway".
- One answer is one `doc.change` through `reviewCard` in `deck.ts`: all nine FSRS fields,
  one `reviews` row, and removal of the oldest rows past `REVIEW_LOG_LIMIT`. A card ts-fsrs
  cannot read is listed as Unreadable (with the reason and a "Reset this card to new"
  repair) and left out of every count; bad parameters show a "Can't schedule this deck" box.
- Settings dialog: exam date, target retention (70–99% slider), minutes a day, seconds a
  card, automatic or fixed new cards a day, reset progress (confirmed). Cards dialog: add,
  edit in place (EditableText), remove (removes the card's log rows in the same change).
- Commands (`commands.ts`): `status`, `addCard`, `reviewCard`, `setExamDate`,
  `setRetention`, `setBudget`, `removeCard`, `resetProgress`. Commands only have `ctx.now`,
  so `status` and `setExamDate` take an optional `utcOffsetMinutes` (default 0) to decide
  which day is today.

## Motion
The answer writes in (220 ms fade and lift); buttons, bubbles and the switch use 120–140 ms
color or position changes. All are removed under `prefers-reduced-motion`.

## View state (local, never saved)
The clock (`now`, ticking every 30 s while visible and on every answer) and the UTC offset;
whether a session is open, the pinned card, whether its back is showing, the "study past the
budget" choice and the session's answer count; which dialog is open; the composer's text.
Export cannot see any of it: it shows the header, the Today readout, the forecast strip and
the readiness estimate as of the moment it renders, with no session UI.

## Capture
The forecast strip is synchronous SVG in both the editor and capture views. It
uses the same FSRS values, daily cap and saved settings, without a browser chart
library. Date labels are deterministic English in the editor and exports.
