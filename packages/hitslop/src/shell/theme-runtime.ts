/** Apply effective values already validated and merged by the owner. */
export function applyTheme(values: Record<string, string>) {
  for (const [key, value] of Object.entries(values))
    document.documentElement.style.setProperty(`--slop-${key}`, value);
}
