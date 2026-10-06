import { defineExtension, defineExtensionId } from "@crustjs/core";
import { app } from "./app";
import { isGlobalInstall } from "./paths";
import { interactiveUpdates } from "./updates";

/** Agent skills link to the global install, which `bun install -g` upgrades in place.
 * Links into a bunx cache or a project would go stale, so other copies only uninstall. */
const globalSkills = defineExtension(defineExtensionId("hitslop:global-skills")).preRun((context) => {
  if (isGlobalInstall || context.commandPath[1] !== "skills") return;
  if (context.commandPath[2] === "uninstall") return;
  throw new Error(
    "Agent skills link to the globally installed CLI so they update with it. Run `bun install -g hitslop`, then `slop skills install`.",
  );
});

/** A helper that exited without a reply printed why: the CLI exits with its status, after
 * what that means for the command. */
const helperExit = defineExtension(defineExtensionId("hitslop:helper-exit")).onError(async (error) => {
  const { ExitStatus } = await import("./native");
  if (!(error instanceof ExitStatus)) return false;
  if (error.message) console.error(error.message);
  process.exit(error.code);
});

const argv = process.argv.slice(2);

await app
  .extend(globalSkills)
  .extend(helperExit)
  .extend(interactiveUpdates())
  .execute({ argv: argv.length ? argv : ["--help"] });
