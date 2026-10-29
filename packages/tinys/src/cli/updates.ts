import { defineExtension, defineExtensionId } from "@crustjs/core";
import { updateNotifier } from "@crustjs/extensions";
import { cliPackage } from "./paths";

/** Keep registry/cache activity out of machine-readable and unattended runs. */
export function interactiveUpdates() {
  if (
    !process.stdout.isTTY ||
    !process.stderr.isTTY ||
    process.env.CI ||
    process.env.TINYS_NO_UPDATE_CHECK === "1"
  )
    return defineExtension(defineExtensionId("tinys:updates"));
  return updateNotifier({
    packageName: cliPackage,
    timeoutMs: 1_000,
    // Upgrading the global install is also what refreshes agent skills.
    updateCommand: { scope: "global" },
    updateDocsUrl: "https://tinys.click/docs/guides/cli-workflows/#upgrade-the-cli",
  });
}
