/** Authoring validation uses the same Rust rules as native editing. No CRDT is made. */
async function loadBinding() {
  const core = await import(new URL("../shell/core/hitslop_core_wasm.js", import.meta.url).href);
  core.initSync({ module: await Bun.file(new URL("../shell/core/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes() });
  return core;
}
let binding: ReturnType<typeof loadBinding> | undefined;

/** Runs one core check, reporting a core refusal as `code: message`. */
async function check(label: string, run: (core: any) => void): Promise<void> {
  const core = await (binding ??= loadBinding());
  try { run(core); }
  catch (error) {
    const failure = error as { code?: unknown; message?: unknown };
    if (typeof failure?.code !== "string") throw error;
    throw new Error(`${label}: ${failure.code}: ${String(failure.message)}`, { cause: error });
  }
}

/** The exact build identity of this CLI's document core, which the helper's must match. */
export async function coreBuildId(): Promise<string> {
  return (await (binding ??= loadBinding())).coreBuildId();
}

export function validateDocument(descriptor: unknown, initial: unknown): Promise<void> {
  return check("slop.ts initial", (core) => core.validate(JSON.stringify(descriptor), JSON.stringify(initial)));
}

export function validateTheme(defaults: unknown): Promise<void> {
  return check("slop.ts theme", (core) => core.validateThemeDefaults(JSON.stringify(defaults)));
}

/** Window shapes use the same parser as native window geometry. */
export function validateWindowShape(presentation: { width: number; height: number; shape?: unknown }): Promise<void> {
  const shape = presentation.shape === undefined ? undefined : JSON.stringify(presentation.shape);
  return check("slop.ts presentation.shape", (core) => core.validateWindowShape(shape, presentation.width, presentation.height));
}
