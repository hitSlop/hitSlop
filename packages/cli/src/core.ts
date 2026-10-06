/** The browser preview core's exact identity, checked when assembling a release. */
export async function coreBuildId(): Promise<string> {
  const core = await import(new URL("../shell/core/hitslop_core_wasm.js", import.meta.url).href);
  core.initSync({ module: await Bun.file(new URL("../shell/core/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes() });
  return core.coreBuildId();
}
