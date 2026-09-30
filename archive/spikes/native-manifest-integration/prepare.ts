import { constants } from "node:fs";
import { copyFile, mkdir, symlink, access, writeFile, lstat, readlink } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const out = join(root, "generated/native-manifest-integration");
const workspace = join(out, "workspace");
try {
  await access(join(out, "prepared.json"));
  throw new Error("Workspace exists; reuse it instead of overwriting an experiment.");
} catch (e: any) {
  if (e.code !== "ENOENT") throw e;
}
await mkdir(workspace, { recursive: true });
const git = Bun.spawn(
  ["/usr/bin/git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
  { cwd: root, stdout: "pipe" },
);
const files = [...new Set((await new Response(git.stdout).text()).split("\0").filter(Boolean))];
if (await git.exited) throw new Error("Cannot inventory working tree");
for (const file of files) {
  await mkdir(dirname(join(workspace, file)), { recursive: true });
  if ((await lstat(join(root, file))).isSymbolicLink()) {
    await symlink(await readlink(join(root, file)), join(workspace, file));
  } else {
    await copyFile(join(root, file), join(workspace, file), constants.COPYFILE_FICLONE);
  }
}
await writeFile(join(out, "source-files.json"), JSON.stringify(files, null, 2));
// Clone disposable caches on APFS: independently writable, without a second physical
// copy of the large Swift/Rust build trees. Production sources are never edited.
for (const directory of [
  "node_modules",
  "packages/cli/node_modules",
  "packages/document/node_modules",
  "packages/schema/node_modules",
  "examples/slops/node_modules",
  "target",
  "apps/apple/Packages/HitSlopApple/.build",
  "apps/apple/Packages/HitSlopApple/Generated",
  "apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/Resources",
  "packages/cli/shell",
  "generated/core",
  "generated/core-tools",
]) {
  try {
    await access(join(root, directory));
  } catch {
    continue;
  }
  await mkdir(dirname(join(workspace, directory)), { recursive: true });
  const copy = Bun.spawn(["/bin/cp", "-cR", join(root, directory), join(workspace, directory)], {
    stdout: "inherit",
    stderr: "inherit",
  });
  if (await copy.exited) throw new Error(`Cannot clone ${directory}`);
}
console.log(`Prepared isolated workspace: ${workspace}`);
await writeFile(
  join(out, "prepared.json"),
  JSON.stringify({ date: new Date().toISOString(), root, workspace }),
);
