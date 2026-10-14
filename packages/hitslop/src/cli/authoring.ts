import { join } from "node:path";
import { mkdir } from "node:fs/promises";
import { shellDirectory } from "./paths";
import { execute } from "./engine";
import { buildTemplate } from "./template";

export async function build(source: string) {
  console.log(await buildTemplate(source));
}

/** The installed templates folder, as the app's engine lists it: `HITSLOP_TEMPLATES_ROOT`,
 * or `~/.hitslop/templates` in the account's home folder. */
async function installedTemplates() {
  const { folders } = (await execute({ method: "templates" })).catalog;
  const installed = folders?.find((folder) => folder.source === "installed")?.path;
  if (typeof installed !== "string") throw new Error("Cannot find this account's home folder");
  return installed;
}

/** Builds into the installed templates folder the app lists, replacing only an earlier build. */
export async function register(source: string) {
  const templates = await installedTemplates();
  await mkdir(templates, { recursive: true });
  console.log(await buildTemplate(source, (slug) => join(templates, slug + ".slop")));
}

export async function dev(source: string, port = 5173) {
  if (!(await Bun.file(join(shellDirectory, "boot.js")).exists()))
    throw new Error("CLI page shell is missing. Reinstall hitslop.");
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
