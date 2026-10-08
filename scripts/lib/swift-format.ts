/** The Swift sources `swift format` owns, styled by `apps/apple/.swift-format`: every Swift
 * file under apps/apple except generated bindings and contracts.
 * Run explicitly with `bun run swift:format`. */
import { existsSync } from "node:fs";
import { join } from "node:path";
import { exec } from "../../packages/hitslop/src/cli/process";
import { repository } from "./artifacts";

async function swiftSources(): Promise<string[]> {
  const listed = await exec(["git", "ls-files", "--cached", "--others", "--exclude-standard", "apps/apple/*.swift"], {
    cwd: repository,
  });
  if (listed.code) throw new Error(`git ls-files failed: ${listed.stderr}`);
  // Tracked files deleted in the working tree are listed too; they have nothing to format.
  return listed.stdout
    .split("\n")
    .filter((file) => file && !file.includes("/Generated/") && existsSync(join(repository, file)));
}

/** Formats the Swift sources in place. */
async function swiftFormat() {
  const files = await swiftSources();
  const { code, stdout, stderr } = await exec(["swift", "format", "--in-place", "--parallel", ...files], {
    cwd: repository,
  });
  const output = (stdout + stderr).trim();
  if (code) throw new Error(`swift format failed\n${output}`);
}

if (import.meta.main) await swiftFormat();
