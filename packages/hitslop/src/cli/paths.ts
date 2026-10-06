import { realpathSync } from "node:fs";
import { homedir } from "node:os";
import { join, sep } from "node:path";
import { fileURLToPath } from "node:url";

/** The npm package this CLI ships in; update commands follow this name. */
export const cliPackage = "hitslop";
export const cliRoot = fileURLToPath(new URL("../../", import.meta.url));
/** The page shell (and dev-only WASM core) the preview serves under /__shell__/. */
export const shellDirectory = join(cliRoot, "shell");

/** Whether this copy is Bun's global install. `bun install -g` replaces that package
 * directory in place, so agent skill links into it survive upgrades; bunx caches and
 * project installs live elsewhere. */
export const isGlobalInstall = (() => {
  const globalDir =
    process.env.BUN_INSTALL_GLOBAL_DIR ||
    join(process.env.BUN_INSTALL || join(homedir(), ".bun"), "install/global");
  try {
    return realpathSync(cliRoot).startsWith(realpathSync(join(globalDir, "node_modules")) + sep);
  } catch {
    return false;
  }
})();
