/** The sharing Worker: Rust built to WASM. Its static pages ship from `apps/sharing/public`. */
import { existsSync } from "node:fs";
import { cp, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { exec } from "../../packages/pop/src/cli/process";
import { repository, publishFolder } from "../lib/artifacts";
import { generateContracts } from "./generate";

export async function buildSharing() {
  await generateContracts();
  const temporary = await mkdtemp(join(tmpdir(), "pop-relay-build-"));
  try {
    const tool = process.env.POP_WORKER_BUILD ?? join(repository, "generated/core-tools/bin/worker-build");
    if (!process.env.POP_WORKER_BUILD && !existsSync(tool)) {
      const installed = await exec(["cargo", "install", "worker-build", "--version", "0.8.7", "--locked", "--force", "--root", join(repository, "generated/core-tools")], {
        cwd: repository, inherit: ["stdout", "stderr"],
      });
      if (installed.code) throw new Error("Cannot install worker-build 0.8.7");
    }
    const version = await exec([tool, "--version"]);
    if (version.code || version.stdout.trim() !== "0.8.7") throw new Error("Sharing requires worker-build 0.8.7; set POP_WORKER_BUILD to that version.");
    const result = await exec([tool, "--release", "--no-opt", "--out-dir", temporary, "--", "--locked"], {
      cwd: join(repository, "apps/sharing"), inherit: ["stdout", "stderr"],
    });
    if (result.code) throw new Error("Relay build failed. Install worker-build 0.8.7 in generated/core-tools, or set POP_WORKER_BUILD.");
    await publishFolder(join(repository, "generated/sharing/relay"), ready => cp(temporary, ready, { recursive: true }));
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
if (import.meta.main) await buildSharing();
