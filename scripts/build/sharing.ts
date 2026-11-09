/** Live sharing artifacts: the trusted host, isolated authored frame and Rust relay. */
import { existsSync } from "node:fs";
import { cp, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { frameDocument } from "../../packages/pop/src/cli/frame-document";
import { exec } from "../../packages/pop/src/cli/process";
import { repository, publishFolder } from "../lib/artifacts";
import { buildBrowserWasm } from "./core";
import { generateContracts } from "./generate";
import { SharingHost } from "../../packages/pop/src/wire/constants.generated";

export async function buildSharing() {
  await generateContracts();
  await buildBrowserWasm();
  const source = join(repository, "packages/pop/src/sharing");
  await publishFolder(join(repository, "generated/sharing/assets"), async stage => {
    const browser = join(stage, "browser");
    await mkdir(browser, { recursive: true });
    const result = await Bun.build({
      entrypoints: [join(repository, "packages/pop/src/browser/host.ts"), join(source, "worker.ts"), join(repository, "packages/pop/src/browser/frame.ts"), join(repository, "packages/pop/src/browser/service-worker.ts")],
      outdir: browser, naming: "[name].[ext]", target: "browser", minify: true,
    });
    if (!result.success) throw new Error(result.logs.join("\n"));
    // Messages reads the initial HTML without running the browser editor. Every
    // invitation gets the same public metadata; its capability stays in the fragment.
    const index = await readFile(join(repository, "packages/pop/src/browser/index.html"), "utf8");
    const image = `${SharingHost.origin}/share-icon.png`;
    const description = "Open this invitation in dotpop to edit together.";
    await Bun.write(join(stage, "index.html"), index
      .replace(/<title>[^<]*<\/title>/, "<title>Open a shared pop</title>")
      .replace("</head>", `    <meta name="description" content="${description}" />
    <meta property="og:title" content="Open a shared pop" />
    <meta property="og:site_name" content="dotpop" />
    <meta property="og:type" content="website" />
    <meta property="og:description" content="${description}" />
    <meta property="og:image" content="${image}" />
    <meta property="og:image:alt" content="dotpop app icon" />
    <link rel="icon" type="image/png" href="${image}" />
    <link rel="apple-touch-icon" href="${image}" />
  </head>`));
    await cp(join(repository, "apps/apple/App/Shared/Assets.xcassets/AppIcon.appiconset/appicon-1024.png"), join(stage, "share-icon.png"));
    await cp(join(repository, "generated/browser/core"), join(browser, "core"), { recursive: true });
    await Bun.write(join(stage, "frame.html"), frameDocument);
  });
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
