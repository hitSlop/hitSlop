import { hostCall } from "./bridge";
type ThemeValues = Record<string, string>;
/** The page applies themes; the native owner validates and saves every write. */
export class ThemeController {
  private overrides: ThemeValues = {};
  constructor(
    private defaults: ThemeValues,
    private apply: (values: ThemeValues) => void = () => {},
  ) {}
  /** Applies saved overrides as they are: loading never fails on theme, and the browser
   * ignores CSS it cannot parse. The native owner validates every write. */
  load(values: ThemeValues) {
    this.overrides = { ...values };
    this.apply(this.get().effective);
  }
  get() {
    return {
      defaults: { ...this.defaults },
      overrides: { ...this.overrides },
      effective: { ...this.defaults, ...this.overrides },
    };
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
