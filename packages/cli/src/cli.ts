#!/usr/bin/env bun
import { defineExtension, defineExtensionId } from "@crustjs/core";
import { app } from "./app";
import { isGlobalInstall } from "./paths";
import { interactiveUpdates } from "./updates";

/** Agent skills link to the global install, which `bun install -g` upgrades in place.
 * Links into a bunx cache or a project would go stale, so other copies only uninstall. */
const globalSkills = defineExtension(defineExtensionId("hitslop:global-skills"), () => ({
  hooks: {
    async preRun(context) {
      if (isGlobalInstall || context.commandPath[1] !== "skills") return;
      if (context.commandPath[2] === "uninstall") return;
      throw new Error(
        "Agent skills link to the globally installed CLI so they update with it. Run `bun install -g @hitslop/cli`, then `slop skills install`.",
      );
    },
  },
}));

const argv = process.argv.slice(2);

await app
  .extend(globalSkills())
  .extend(interactiveUpdates())
  .execute({ argv: argv.length ? argv : ["--help"] });
