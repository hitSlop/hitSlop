import { join } from "node:path";
import { mkdir } from "node:fs/promises";
import { homedir } from "node:os";
import { shellDirectory } from "./paths";
import { buildTemplate, prepareRenderer } from "./template";
import { projectSlug } from "./build";

/** Builds anywhere; only `native` artwork needs the Mac app. */
export async function build(source: string, artwork?: string) {
  if (artwork !== undefined && artwork !== "native") throw new Error("--artwork accepts only native");
  console.log(await buildTemplate(source, artwork ? await prepareRenderer() : undefined));
}

/** Builds into the template folder the app lists, replacing only an earlier build. */
export async function register(source: string) {
  const slug = projectSlug(source);
  const renderer = await prepareRenderer();
  const templates = join(homedir(), ".hitslop/templates");
  await mkdir(templates, { recursive: true });
  console.log(await buildTemplate(source, renderer, join(templates, slug + ".slop")));
}

export async function dev(source: string, port = 5173) {
  if (!(await Bun.file(join(shellDirectory, "boot.js")).exists()))
    throw new Error("CLI page shell is missing. Reinstall @hitslop/cli.");
  const { startDev } = await import("./dev");
  const controller = new AbortController();
  const stop = () => controller.abort();
  for (const signal of ["SIGINT", "SIGTERM"] as const) process.once(signal, stop);
  try {
    const server = await startDev(source, port, controller.signal);
    console.log(`Disposable preview: ${server.url}`);
  } catch (error) {
    process.removeListener("SIGINT", stop);
    process.removeListener("SIGTERM", stop);
    if (!controller.signal.aborted) throw error;
  }
}
