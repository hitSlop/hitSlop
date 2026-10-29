# Plain CSS and the theme palette

Keep app styles in `./styles.css`, imported from `App.svelte` (the definition build refuses CSS in tiny.ts). Keep markup class names literal and prefix
classes with the app name. Each WebView owns one app, so document-level CSS is
appropriate. Use native nesting for related states and descendants.

```css
:root {
  --font: "Avenir Next", Avenir, sans-serif;
  --rule: color-mix(in srgb, var(--tiny-ink) 14%, transparent);
}
.checklist-footer {
  border-top: 1px dashed var(--rule);
  background: var(--tiny-paper);
  & > span { color: var(--tiny-muted); }
  & button:focus-visible { outline: 2px solid var(--tiny-accent); }
}
```

Declare the colors a person may change in tiny.ts's `theme`, as lowercase `#rrggbb` or
`#rrggbbaa`; the build refuses anything else. Fonts, sizes and derived colors are plain
custom properties in `styles.css`, as above; a derived color follows the palette color it
mixes. The build stores the defaults in the template; the runtime applies defaults and
document changes before mounting the app. Compiled app styling lives in `ui.css`. Owners use the
window's theme panel or `tiny theme get/set/reset/export/import`; the host saves their
changes in the document's database. Never edit these built files directly. Layout
changes require authoring source and a rebuild. Do not add mutable CSS files or
duplicate theme defaults.

Bits UI portals live outside their trigger's ancestors. Give portal content an
explicit class and anchor descendant rules there, not under the editor wrapper.
Use data attributes for primitive states. Group narrow-window and capture rules
with the relevant surface. Respect reduced motion and visible keyboard focus.

The host disables ordinary text selection in every editor, both natively and in
`tiny dev`. Inputs, textareas (including readonly fields), and `contenteditable` regions
retain normal selection and editing; captures and PDF text are unaffected. Enable
selection for useful output such as notes, addresses, or code:

```css
.my-tiny-content {
  -webkit-user-select: text;
  user-select: text;
}
```

Use `body` instead of the class to make the whole tiny selectable, including portals.
For a region override, give any copyable portal content its own class too. These
ordinary rules override the host's zero-specificity default without `!important`.
Selection is an authoring CSS choice, not a theme setting.

Preserve editor, export, and icon appearance during styling changes. Check both
examples at normal and narrow widths, plus open menus and PNG/PDF captures.
