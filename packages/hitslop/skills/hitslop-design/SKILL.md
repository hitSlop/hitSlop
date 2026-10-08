---
name: hitslop-design
description: Design or refine hitSlop mini apps and documents with a purpose-led tactile language, responsive native-window behavior, transparent or PNG-skinned surfaces, and export-aware capture.
---

# hitSlop design

**Do not create an automated test or test suite for each slop.** Creating, styling,
animating, or updating a slop calls for existing checks/builds and hands-on
preview/export review, not new example-specific tests. Shared SDK, storage, or host
regressions belong in existing tests at their owning boundary. Do not turn visual
review into permanent tests of artwork coordinates, CSS strings, or animation timing.
Ordinary tests use minimal infrastructure fixtures, not live examples; generic
shipped-artifact smoke checks and frozen compatibility replay remain separate.

Do not display “Saved,” “Saving…,” or routine persistence indicators inside authored slops. The native host owns save-failure and retry UI. Use task-specific feedback for explicit operations, such as “Importing skin…” or “Skin applied.”

Read `slop.ts` first and design at its `window`'s exact dimensions. A slop
is one complete digital object, not a small website.

Each slop should be unique, expressive, and instantly graspable for its single
purpose. Share standards of clarity and familiar interaction behavior; let each
object choose its own palette, typography, composition, and material. Paper,
Instrument, and Skin are starting points, not a required retro or physical look.

The guidance below is self-contained; reference images are not required. If the
project provides `_vibe/`, use it as optional inspiration. Never copy those
images into generated projects or runtime packages.

## Give the object its own character

Choose an expression that makes the job feel natural: an orderly paper record,
a precise desktop instrument, a cheerful pocket companion, or a quiet modern
tool. Let that choice shape the whole composition, not just its accent color.
The checklist supplied by `slop init` is a working starting point; adapt its
layout, typography, palette, and controls to the requested object.

- Give typography a job: editorial headings for reading, aligned tabular figures
  for accounting, a prominent numeric display for timing. Keep essential labels
  readable even when the display type is playful.
- Choose a deliberate surface and palette: warm paper with fine ink rules,
  saturated molded plastic with a recessed display, or dark glass with crisp
  readouts. Keep highlights, borders, shadows, and corner shapes consistent with
  the chosen material. Quiet, flat treatments can have just as much identity.
- Build recognition through proportions and composition: the long strip of a
  ticket, the rhythm of a ledger, or the display-and-controls grouping of an
  instrument. Use an unusual silhouette only when it helps the object.
- Make controls feel responsive through visible pressed, selected, and focused
  states. Depth and brief motion should explain operation; decorative knobs,
  fake window chrome, and unreadable display effects add no useful character.
- Implement the expression with plain CSS, declared theme colors, and styled
  Bits UI primitives. Use spacing, rules, gradients, borders, and restrained
  shadows before reaching for image skins; reserve PNG skins for meaningful
  silhouettes as described in the windows reference.

Related slops share interaction quality, not a universal shell. A recipe, timer,
and budget tool should remain distinguishable even with their titles removed.
Follow the user's visual direction and preserve an existing object's identity
when refining it.

## Decide before styling

1. State the single job in one sentence.
2. Choose a visual direction suited to the job; Paper, Instrument, and Skin are
   useful starting points, not a required taxonomy.
3. Identify the primary action/readout and persistent state.
4. Decide whether the window is standard/resizable, transparent, or PNG-skinned.
5. Define what live editing, static capture, and icon must show.

Read [references/object-families.md](references/object-families.md) for visual
patterns and anti-patterns. Read
[references/presentation-and-export.md](references/presentation-and-export.md)
for transparency, resizing, skins, responsive layout, capture, icon, PNG,
and PDF behavior.

## Core design rules

- Let the host window be the outer object boundary; do not wrap everything in a
  generic app card or fake desktop window.
- Use hierarchy, spacing, typography, rules, and tonal contrast before adding
  containers.
- Choose a coherent palette for the object's purpose: a neutral or saturated
  surface can lead. Reserve additional state colors for meaning, and keep
  state understandable through labels and structure as well as color.
- Size the initial viewport around realistic default content. Compactness comes
  from rhythm and hierarchy, never tiny type.
- Preserve critical actions at narrow sizes. Prefer container queries and
  reflow to simply hiding features.
- Keep focus visible, labels concise, numbers tabular, contrast accessible, and
  motion respectful of `prefers-reduced-motion`.
- The host disables ordinary text selection in every editor, including `slop dev`.
  Inputs, textareas (including readonly fields), and `contenteditable` regions retain
  normal selection and editing; captures and PDF text are unaffected. Enable copying
  for useful content with `-webkit-user-select: text; user-select: text` on its class,
  or on `body` for the whole slop. Ordinary CSS overrides the host default without
  `!important`; this is an authoring choice, not a theme setting.
- Prefer direct editing and progressive disclosure. When interactive controls are
  needed (dialogs, dropdowns, selects, sliders, segmented tabs, checkboxes, calendars,
  popovers, tooltips), always use Bits UI headless primitives (`bits-ui`). Never home-make
  custom modal overlays, focus traps, or range hacks. Read [references/bits-ui-styling.md](references/bits-ui-styling.md).
- Style Bits UI primitives via data attributes (`[data-dialog-overlay]`, `[data-select-trigger]`,
  `[data-slider-track]`, `[data-state="checked"]`) and CSS custom properties
  (`--slop-surface`, `--slop-accent`, `--slop-ink`). This ensures each slop retains its
  authentic physical personality (Paper, Instrument, Skin) while remaining effortless
  to restyle or re-theme at runtime.
- Keep structural styles in plain `styles.css`, imported explicitly from `slop.ts`. Declare the
  colors a person may change in slop.ts's `theme`: lowercase `#rrggbb`, or `#rrggbbaa`
  when translucent. Only colors belong there; fonts, sizes and colors derived with
  `color-mix(… var(--slop-ink) …)` go in `styles.css`. The build stores them as the
  template's theme defaults; the runtime applies them before mounting the app. Use `var(--slop-TOKEN)` in CSS. The window's theme panel lists every
  declared color in the order written, so declare the ones worth changing.
- Group base rules, states, descendants, and responsive rules together. Use
  app-prefixed classes, including explicit classes on Bits UI portal content.
  Read [references/css.md](references/css.md) for the authoring pattern.
- Owners change declared colors in the window's theme panel or through
  `slop theme get/set/reset/export/import`.
  The host saves overrides in the document's database; never edit it or compiled assets
  directly. Layout changes require authoring source and a rebuild.
- Keep exportable content in normal flow. Without an `export` view, mark
  editing-only UI with `data-slop-export="hide"`; with one, the editor is never
  captured.
- Make each slop purpose-specific. Shared SDK patterns must not make unrelated
  objects look like one reskinned dashboard.

## Review efficiently

Use `bun run dev` for disposable browser preview. Review at the `window` size
and a narrow width, with menus open and keyboard focus visible. Build with the
matching Mac app to verify export and icon artwork. Read
[references/review-workbench.md](references/review-workbench.md) for a short pass.

## Check the real task

Preserve the established identity during refinement; a redesign requires an explicit
change of direction. Use realistic content to judge hierarchy before adding decoration.
Give typography distinct roles and load only the fonts and weights those roles need.
Use familiar controls, clear labels, visible focus, and meaningful loading/error feedback.
Do not invent facts or marketing claims while polishing copy.

Exercise empty, typical, long-content, failed, and busy states. Check text wrapping,
keyboard operation, narrow windows, zoom, emoji/IME input, and reduced motion. Keep
critical actions reachable. Overlays must escape scrolling/clipping containers.

Review editor, narrow, export, and icon views together, fix material findings in one
batch, then verify the affected behavior. Source changes invalidate old captures;
nonvisual changes do not justify another full visual review. Keep PRODUCT.md and each
app's DESIGN.md concise and grounded in what is actually shipped.

## Motion

Use motion to explain an interaction or reinforce the object's character: a
pressed control, settling needle, or changing liquid level. Keep idle objects
quiet unless ongoing motion communicates a real function.

- Keep simple hover/press feedback in CSS transitions. Use Svelte
  transitions for entering/leaving DOM and `animate:flip` for keyed-list
  reordering. Use `Spring` for physical responses or `Tween` for predictable
  interpolation from `svelte/motion`; prefer these over legacy `spring`/`tweened`.
  No additional animation dependency is needed. See the
  [Svelte motion reference](https://svelte.dev/docs/svelte/svelte-motion).
- Persist the target state immediately; animated values are presentation only.
  Keep precise readouts and accessible values current. Retarget from the current
  visual value on rapid input, and prefer transforms/opacity over layout changes.
- Honor `prefersReducedMotion.current` for JavaScript motion and
  `prefers-reduced-motion` in CSS. Snap active motion when the preference changes
  (on the instance, call `spring.set(target, { instant: true })` or
  `tween.set(target, { duration: 0, delay: 0 })`).
- Keep skinned controls inside the painted, hit-test-safe silhouette. Stop
  transient animation work and clean up timers, audio, and registrations on
  unmount. Pause clocks and loops while `document.visibilityState` is `hidden`
  (minimized or hidden window) and derive elapsed time from `Date.now()`.
  Celebrations should finish and should not fire just because a saved document
  opens in a completed state.
- Export/icon views render final data, not interpolated values. Capture's CSS
  overrides do not stop JavaScript springs, tweens, or animation loops. When
  capturing an animated editor, settle it through `capture.onPrepare`, await
  the DOM update, and clean up the returned registration. Do not change saved
  data to prepare a capture or rely only on `capture.isRenderer()`.
- Check normal/reduced motion, rapid retargeting, and capture during animation.

Runtime packages remain source-free: never ship authoring skills, editable CSS,
dependencies, build caches, or seed stores inside a `.slop`. The builder alone
adds the canonical document skill.
