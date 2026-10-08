/** The Swift sources `swift format` owns, styled by `apps/apple/.swift-format`: every Swift
 * file under apps/apple except generated bindings and contracts. The swift tier lints them;
 * `bun run swift:format` rewrites them. */
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

/** Lints the Swift sources (warnings fail), or rewrites them in place. */
export async function swiftFormat(mode: "lint" | "format") {
  const files = await swiftSources();
  const command = mode === "lint" ? ["lint", "--strict"] : ["--in-place"];
  const { code, stdout, stderr } = await exec(["swift", "format", ...command, "--parallel", ...files], {
    cwd: repository,
  });
  const output = (stdout + stderr).trim();
  if (code) throw new Error(`swift format ${mode} failed; run \`bun run swift:format\`\n${output}`);
}

if (import.meta.main) await swiftFormat("format");
