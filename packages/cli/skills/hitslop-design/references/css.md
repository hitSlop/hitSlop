# Plain CSS and public theme tokens

Keep app styles in `./styles.css`; the build imports it with `App.svelte`. Keep markup class names literal and prefix
classes with the app name. Each WebView owns one app, so document-level CSS is
appropriate. Use native nesting for related states and descendants.

```css
.checklist-footer {
  border-top: 1px dashed var(--slop-rule);
  background: var(--slop-paper);
  & > span { color: var(--slop-muted); }
  & button:focus-visible { outline: 2px solid var(--slop-accent); }
}
```

Keep public defaults in `theme.ts` using `defineTheme` from
`@hitslop/document/theme`. The builder writes defaults to `assets/theme.json`;
the runtime applies defaults and document overrides before mounting the app.
Compiled app styling lives in `assets/app.css`. Owners use `slop theme get/set/reset`;
the host writes overrides separately in `state/theme.json`. Never edit these built
files directly. Layout changes require authoring source and a rebuild.
Do not add mutable CSS files or duplicate token defaults.

Bits UI portals live outside their trigger's ancestors. Give portal content an
explicit class and anchor descendant rules there, not under the editor wrapper.
Use data attributes for primitive states. Group narrow-window and capture rules
with the relevant surface. Respect reduced motion and visible keyboard focus.

Preserve editor, export, and icon appearance during styling changes. Check both
examples at normal and narrow widths, plus open menus and PNG/PDF captures.
