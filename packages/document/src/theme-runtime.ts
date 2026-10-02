import { hostCall } from "./bridge";
type ThemeValues = Record<string, string>;
/** The page applies themes; the native owner validates and saves every write. Apps read
 * tokens as CSS variables. */
export class ThemeController {
  constructor(
    private defaults: ThemeValues,
    private apply: (values: ThemeValues) => void = () => {},
  ) {}
  /** Applies the defaults with the saved overrides as they are: loading never fails on
   * theme, and the browser ignores CSS it cannot parse. Every default is applied again, so
   * a reset override returns to its default. */
  load(overrides: ThemeValues) {
    this.apply({ ...this.defaults, ...overrides });
  }
}
export async function openTheme(native: boolean) {
  const [defaults, overrides] = await Promise.all([
    fetch("/assets/theme.json").then((response) => {
      if (!response.ok) throw new Error("Missing theme defaults");
      return response.json();
    }),
    native ? hostCall({ method: "theme.load" }).then((reply) => reply.values) : {},
  ]);
  const theme = new ThemeController(
    defaults,
    (values) => {
      for (const [key, value] of Object.entries(values))
        document.documentElement.style.setProperty(`--slop-${key}`, value);
    },
  );
  theme.load(overrides);
  return theme;
}
