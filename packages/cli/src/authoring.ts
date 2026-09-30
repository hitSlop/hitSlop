import { join } from "node:path";
import { mkdir, readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { shellDirectory } from "./paths";
import { buildTemplate, prepareRenderer, installTemplate } from "./template";

export async function build(source: string) {
  console.log(await buildTemplate(source, await prepareRenderer()));
}

export async function register(source: string) {
  const output = await buildTemplate(source, await prepareRenderer());
  const manifest = JSON.parse(await readFile(join(output, "manifest.json"), "utf8"));
  const templates = join(homedir(), ".hitslop/templates");
  await mkdir(templates, { recursive: true });
  const destination = join(templates, manifest.slug + ".slop");
  await installTemplate(output, destination);
  console.log(destination);
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
