/** Live sharing artifacts: the trusted host, isolated authored frame and Rust relay. */
import { existsSync } from "node:fs";
import { cp, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { frameDocument } from "../../packages/tinys/src/cli/frame-document";
import { exec } from "../../packages/tinys/src/cli/process";
import { repository, publishFolder } from "../lib/artifacts";
import { buildBrowserWasm } from "./core";
import { generateContracts } from "./generate";

export async function buildSharing() {
  await generateContracts();
  await buildBrowserWasm();
  const source = join(repository, "packages/tinys/src/sharing");
  await publishFolder(join(repository, "generated/sharing/assets"), async stage => {
    const browser = join(stage, "browser");
    await mkdir(browser, { recursive: true });
    const result = await Bun.build({
      entrypoints: [join(repository, "packages/tinys/src/browser/host.ts"), join(source, "worker.ts"), join(repository, "packages/tinys/src/browser/frame.ts"), join(repository, "packages/tinys/src/browser/service-worker.ts")],
      outdir: browser, naming: "[name].[ext]", target: "browser", minify: true,
    });
    if (!result.success) throw new Error(result.logs.join("\n"));
    await cp(join(repository, "packages/tinys/src/browser/index.html"), join(stage, "index.html"));
    await cp(join(repository, "generated/browser/core"), join(browser, "core"), { recursive: true });
    await cp(join(repository, "apps/sharing/openapi.json"), join(stage, "openapi.json"));
    await Bun.write(join(stage, "frame.html"), frameDocument);
  });
  const temporary = await mkdtemp(join(tmpdir(), "tinys-relay-build-"));
  try {
    const tool = process.env.TINYS_WORKER_BUILD ?? join(repository, "generated/core-tools/bin/worker-build");
    if (!process.env.TINYS_WORKER_BUILD && !existsSync(tool)) {
      const installed = await exec(["cargo", "install", "worker-build", "--version", "0.8.7", "--locked", "--force", "--root", join(repository, "generated/core-tools")], {
        cwd: repository, inherit: ["stdout", "stderr"],
      });
      if (installed.code) throw new Error("Cannot install worker-build 0.8.7");
    }
    const version = await exec([tool, "--version"]);
    if (version.code || version.stdout.trim() !== "0.8.7") throw new Error("Sharing requires worker-build 0.8.7; set TINYS_WORKER_BUILD to that version.");
    const result = await exec([tool, "--release", "--no-opt", "--out-dir", temporary, "--", "--locked"], {
      cwd: join(repository, "apps/sharing"), inherit: ["stdout", "stderr"],
    });
    if (result.code) throw new Error("Relay build failed. Install worker-build 0.8.7 in generated/core-tools, or set TINYS_WORKER_BUILD.");
    await publishFolder(join(repository, "generated/sharing/relay"), ready => cp(temporary, ready, { recursive: true }));
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
if (import.meta.main) await buildSharing();
