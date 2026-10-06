# Plain CSS and the theme palette

Keep app styles in `./styles.css`; the build imports it with `App.svelte`. Keep markup class names literal and prefix
classes with the app name. Each WebView owns one app, so document-level CSS is
appropriate. Use native nesting for related states and descendants.

```css
:root {
  --font: "Avenir Next", Avenir, sans-serif;
  --rule: color-mix(in srgb, var(--slop-ink) 14%, transparent);
}
.checklist-footer {
  border-top: 1px dashed var(--rule);
  background: var(--slop-paper);
  & > span { color: var(--slop-muted); }
  & button:focus-visible { outline: 2px solid var(--slop-accent); }
}
```

Declare the colors a person may change in slop.ts's `theme`, as lowercase `#rrggbb` or
`#rrggbbaa`; the build refuses anything else. Fonts, sizes and derived colors are plain
custom properties in `styles.css`, as above; a derived color follows the palette color it
mixes. The build stores the defaults in the template; the runtime applies defaults and
document changes before mounting the app. Compiled app styling lives in `assets/app.css`. Owners use the
window's theme panel or `slop theme get/set/reset/export/import`; the host saves their
changes in the document's database. Never edit these built files directly. Layout
changes require authoring source and a rebuild. Do not add mutable CSS files or
duplicate theme defaults.

Bits UI portals live outside their trigger's ancestors. Give portal content an
explicit class and anchor descendant rules there, not under the editor wrapper.
Use data attributes for primitive states. Group narrow-window and capture rules
with the relevant surface. Respect reduced motion and visible keyboard focus.

Preserve editor, export, and icon appearance during styling changes. Check both
examples at normal and narrow widths, plus open menus and PNG/PDF captures.
