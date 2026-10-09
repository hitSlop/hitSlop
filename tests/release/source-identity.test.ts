import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { run } from "../../packages/hitslop/src/cli/process";
import { fileDigest, sourceBlobHash } from "../../scripts/lib/artifacts";
import { sourceFingerprint, verifyCandidate } from "../../scripts/compat/integrity";

// Staging identical bytes used to change verify's input identity and rerun every
// affected tier. Git's actual index is the oracle, including Unicode/binary lengths.
test.each(["sha1", "sha256"] as const)("source identity survives staging in a %s repository", async format => {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-source-identity-"));
  const git = (...args: string[]) => run(["git", ...args], { cwd: folder });
  try {
    await git("init", "--quiet", `--object-format=${format}`);
    const bytes = Buffer.from("A source file 🦀\n\0with binary bytes\xff", "utf8");
    await writeFile(join(folder, "source.ts"), bytes);
    const unstaged = sourceBlobHash(bytes, format);
    await git("add", "source.ts");
    expect(unstaged).toBe((await git("rev-parse", ":source.ts")).trim());
    const changed = Buffer.concat([bytes, Buffer.from("changed")]);
    await writeFile(join(folder, "source.ts"), changed);
    expect(sourceBlobHash(changed, format)).not.toBe(unstaged);
    await git("add", "source.ts");
    expect(sourceBlobHash(changed, format)).toBe((await git("rev-parse", ":source.ts")).trim());
  } finally { await rm(folder, { recursive: true, force: true }); }
});

test("release acceptance compares presentation fixtures at their built location", async () => {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-candidate-"));
  try {
    await run(["git", "init", "--quiet"], { cwd: folder });
    await mkdir(join(folder, "generated/presentation"), { recursive: true });
    await mkdir(join(folder, "generated/templates"), { recursive: true });
    const fixture = join(folder, "generated/presentation/ellipse.slop");
    const template = join(folder, "generated/templates/minimal.slop");
    await writeFile(fixture, "captured presentation bytes");
    await writeFile(template, "captured shipped template bytes");
    const release = {
      inputs: await sourceFingerprint(folder),
      templates: {
        "presentation-ellipse": await fileDigest(fixture),
        minimal: await fileDigest(template),
      },
    };
    await verifyCandidate(folder, release, folder);
    await writeFile(fixture, "changed presentation bytes");
    await expect(verifyCandidate(folder, release, folder)).rejects.toThrow("Captured template differs from candidate: presentation-ellipse");
    await mkdir(join(folder, "tests/presentation"), { recursive: true });
    await writeFile(join(folder, "tests/presentation/slop.ts"), "changed producing input");
    await expect(verifyCandidate(folder, release, folder)).rejects.toThrow("Release inputs changed after compatibility capture");
  } finally { await rm(folder, { recursive: true, force: true }); }
});
