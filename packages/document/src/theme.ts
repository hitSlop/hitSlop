/** Public theme tokens. Layout remains in authored CSS. */
export function defineTheme<T extends Record<string, string>>(defaults: T) {
  return {
    defaults: Object.freeze({ ...defaults }),
    vars: Object.fromEntries(Object.keys(defaults).map((key) => [key, `var(--slop-${key})`])) as {
      [K in keyof T]: `var(--slop-${string})`;
    },
    css: `:root{${Object.entries(defaults)
      .map(([key, value]) => `--slop-${key}:${value}`)
      .join(";")}}`,
  };
}
