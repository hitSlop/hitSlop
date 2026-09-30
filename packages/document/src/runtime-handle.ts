/** The lifecycle invoked by the native host through `globalThis.__slop`. */
export interface SlopRuntimeHandle {
  /** Applies theme overrides the native owner already validated and saved. */
  applyTheme(overrides: Record<string, string>): void;
  flush(): Promise<void>;
  prepareClose(): Promise<void>;
  cancelClose(): void;
  close(): Promise<void>;
  reloadInterface(): Promise<void>;
}

declare global {
  var __slop: SlopRuntimeHandle | undefined;
}
