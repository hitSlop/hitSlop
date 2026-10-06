/** The colors a person may override, as lowercase `#rrggbb` or `#rrggbbaa` (opaque colors
 * omit `ff`). Each becomes `--slop-<name>`; fonts, sizes and colors derived from these
 * belong in the app's CSS. `slop build` checks every value. */
export function defineTheme<T extends Record<string, `#${string}`>>(defaults: T) {
  return { defaults: Object.freeze({ ...defaults }) };
}
