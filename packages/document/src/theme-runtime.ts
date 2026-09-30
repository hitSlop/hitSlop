import type { BridgeMethod as Method, BridgeRequest as Message, BridgeReply as Result } from "@hitslop/schema/bridge";
export type ThemeValues = Record<string, string>;
export class ThemeController {
  private overrides: ThemeValues = {};
  constructor(
    private defaults: ThemeValues,
    private save: (values: ThemeValues) => Promise<void>,
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
  async set(values: ThemeValues) {
    const next = { ...this.overrides, ...values };
    await this.save(next);
    this.load(next);
    return this.get();
  }
  async reset(token?: string) {
    const next = token === undefined ? {} : { ...this.overrides };
    if (token !== undefined) delete next[token];
    await this.save(next);
    this.load(next);
    return this.get();
  }
}
export async function openTheme(native: boolean) {
  const response = await fetch("/assets/theme.json");
  if (!response.ok) throw new Error("Missing theme defaults");
  const defaults = await response.json();
  const call = <M extends Method>(args: Message<M>): Promise<Result<M>> =>
    (globalThis as any).webkit.messageHandlers.hitslop.postMessage(args);
  const theme = new ThemeController(
    defaults,
    async (values) => {
      if (native) await call({ method: "theme.save", values });
    },
    (values) => {
      for (const [key, value] of Object.entries(values))
        document.documentElement.style.setProperty(`--slop-${key}`, value);
    },
  );
  theme.load(native ? (await call({ method: "theme.load" })).values : {});
  return theme;
}
