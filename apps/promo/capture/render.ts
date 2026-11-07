/** Bundle once; serve local media in place so frame sequences are not copied per render. */
import { bundle } from "@remotion/bundler";
import { selectComposition, renderMedia, renderStill } from "@remotion/renderer";
import { symlink, mkdir, rmdir } from "node:fs/promises";
import { join, resolve } from "node:path";
const root = resolve(import.meta.dir, "..");
const browserExecutable =
  process.env.REMOTION_BROWSER ??
  `${process.env.HOME}/Library/Caches/ms-playwright/chromium_headless_shell-1243/chrome-headless-shell-mac-arm64/chrome-headless-shell`;
const emptyPublic = join(root, "out/empty-public");
await mkdir(emptyPublic, { recursive: true });
const serveUrl = await bundle({ entryPoint: join(root, "src/index.ts"), publicDir: emptyPublic });
await rmdir(join(serveUrl, "public")).catch((error) => {
  if (error.code !== "ENOENT") throw error;
});
await symlink(join(root, "public"), join(serveUrl, "public"), "dir");
await mkdir(join(root, "out"), { recursive: true });
const review = process.argv.includes("--review");
for (const id of ["LaunchWide", "LaunchVertical"]) {
  const composition = await selectComposition({ serveUrl, id, browserExecutable });
  const slug = id === "LaunchWide" ? "wide" : "vertical";
  for (const [name, frame] of review
    ? ([
        ["together", 400],
        ["montage", 780],
        ["end", 860],
        ["intro", 120],
        ["pocket", 185],
      ] as const)
    : ([["poster", 780]] as const)) {
    await renderStill({
      serveUrl,
      composition,
      browserExecutable,
      frame,
      output: join(root, `out/launch-${slug}-${name}.png`),
    });
  }
  if (!review) {
    let last = -1;
    await renderMedia({
      serveUrl,
      composition,
      browserExecutable,
      codec: "h264",
      crf: 16,
      concurrency: 2,
      outputLocation: join(root, `out/launch-${slug}.mp4`),
      onProgress: ({ progress }) => {
        const p = Math.floor(progress * 10);
        if (p !== last) {
          last = p;
          console.log(`${slug}: ${p * 10}%`);
        }
      },
    });
  }
}
console.log("Launch exports complete.");
