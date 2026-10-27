import { expect, test } from "bun:test";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { run } from "../../packages/hitslop/src/cli/process";
import { collectReleaseContext, loadReleaseNotes, parseReleaseNotes, releaseProject, writeReleaseNotes } from "../../scripts/release/notes";

const project = `options:
  deploymentTarget:
    macOS: "15.2"
settings:
  base:
    MARKETING_VERSION: "7.2.0"
packages:
  Other:
    exactVersion: 1.2.3
  Sparkle:
    exactVersion: 9.8.7
`;
const source = "# hitSlop 7.2.0\n\n## Mac app\n\n### Fixed\n\n- Preserve a document title containing & or <.\n\n## iPhone app\n\n- Remove an iCloud download without deleting the document.\n\n## Authoring\n\n- Add a command.\n";

test("release notes separate Mac changes from authoring and retain Markdown", () => {
  const parsed = parseReleaseNotes(source.replaceAll("\n", "\r\n"), "7.2.0");
  expect(parsed.mac).toBe("### Fixed\n\n- Preserve a document title containing & or <.");
  expect(parsed.iphone).toBe("- Remove an iCloud download without deleting the document.");
  expect(parsed.authoring).toBe("- Add a command.");
  expect(parseReleaseNotes("# hitSlop 7.2.0\n\n## Mac app\n\nNo Mac app changes in this release.\n", "7.2.0").authoring).toBeUndefined();
  const withExample = `${source}\n\`\`\`md\n## Mac app\nExample heading\n\`\`\`\n`;
  expect(parseReleaseNotes(withExample, "7.2.0").authoring).toContain("## Mac app\nExample heading");
});

test("release notes keep fence-like content inside examples until a valid closing fence", () => {
  for (const fence of ["```", "~~~"]) {
    const example = [fence + "md", fence + "md", "## Mac app", "Example heading", "  " + fence + fence[0] + " \t"].join("\n");
    const notes = `# hitSlop 7.2.0\n\n## Mac app\n\n${example}\n\n## Authoring\n\n- Add a command.\n`;
    expect(parseReleaseNotes(notes, "7.2.0")).toEqual({ mac: example.trimEnd(), authoring: "- Add a command." });
  }
});

test("release notes reject a wrong version, missing or empty Mac section, and ambiguous sections", () => {
  for (const invalid of [
    source.replace("7.2.0", "7.1.0"),
    "# hitSlop 7.2.0\n",
    "# hitSlop 7.2.0\n\n## Mac app\n\n### Fixed\n<!-- draft -->\n",
    "# hitSlop 7.2.0\n\n## Authoring\n\n- Something\n",
    `${source}\n## Mac app\nDuplicate\n`,
    `${source}\n## iPhone app\nDuplicate\n`,
    "# hitSlop 7.2.0\n\n## Mac app\n\nNo changes.\n\n## iPhone app\n\n<!-- draft -->\n",
    `${source}\n## Other\nUnmapped notes\n`,
    `${source}\n\`\`\`md\nUnclosed example`,
  ]) expect(() => parseReleaseNotes(invalid, "7.2.0")).toThrow();
});

test("release publication uses the reviewed source and the project's Sparkle pin", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-release-notes-"));
  try {
    await mkdir(join(root, "apps/apple"), { recursive: true });
    await mkdir(join(root, "docs/releases"), { recursive: true });
    await writeFile(join(root, "apps/apple/project.yml"), project);
    expect(releaseProject(project).sparkle).toBe("9.8.7");
    expect(() => releaseProject(project.replace("    exactVersion: 9.8.7", "    from: 9.8.7"))).toThrow("exactVersion");
    await expect(loadReleaseNotes(root)).rejects.toThrow("Missing release notes:");
    await writeFile(join(root, "docs/releases/7.2.0.md"), source);
    const output = join(root, "dist");
    await writeReleaseNotes(output, root);
    const mac = await readFile(join(output, "hitSlop-7.2.0.md"), "utf8");
    const github = await readFile(join(output, "release-notes.md"), "utf8");
    expect(mac).toBe("# hitSlop 7.2.0\n\n### Fixed\n\n- Preserve a document title containing & or <.\n");
    expect(github).toContain("## Authoring\n\n- Add a command.");
    expect(github).toContain("## iPhone app\n\n- Remove an iCloud download without deleting the document.");
    expect(mac).not.toContain("iCloud download");
    expect(github).toContain("bun install -g hitslop@7.2.0");
    expect(github).toContain("Requires macOS 15.2 or newer.");
    expect(github).not.toContain("prelaunch experiments");
    await writeReleaseNotes(output, root);
    expect(await readFile(join(output, "release-notes.md"), "utf8")).toBe(github);
    await writeFile(join(root, "apps/apple/project.yml"), project.replace("7.2.0", "1.0.0"));
    await writeFile(join(root, "docs/releases/1.0.0.md"), source.replace("7.2.0", "1.0.0"));
    expect((await loadReleaseNotes(root)).github).toContain("Earlier Mac builds and documents were prelaunch experiments and are unsupported");
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("drafting context uses explicit committed endpoints, including initial-release mode", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-notes-context-"));
  const git = (...args: string[]) => run(["git", "-c", "user.name=Release Test", "-c", "user.email=release@example.invalid", "-c", "commit.gpgsign=false", ...args], { cwd: root });
  try {
    await git("init", "--quiet");
    await mkdir(join(root, "apps/apple"), { recursive: true });
    await mkdir(join(root, "docs/guides"), { recursive: true });
    await mkdir(join(root, "slops"));
    await writeFile(join(root, "apps/apple/project.yml"), project);
    await writeFile(join(root, "README.md"), "Supported product capabilities\n");
    await writeFile(join(root, "docs/guides/authoring.md"), "Supported authoring capabilities\n");
    await writeFile(join(root, "slops/bundled.json"), "[]\n");
    await git("add", ".");
    await git("commit", "--quiet", "-m", "Baseline history");
    await git("tag", "baseline");
    await writeFile(join(root, "change.txt"), "Released improvement\n");
    await git("add", ".");
    await git("commit", "--quiet", "-m", "Improve document export (#42)", "-m", "Preserve the title.");
    await git("tag", "candidate");
    await writeFile(join(root, "change.txt"), "Future work\n");
    await git("add", ".");
    await git("commit", "--quiet", "-m", "Future work");
    const context = await collectReleaseContext({ from: "baseline", to: "candidate" }, root);
    expect(context).toContain("Improve document export (#42)");
    expect(context).toContain("Preserve the title.");
    expect(context).toContain("change.txt");
    expect(context).not.toContain("Baseline history");
    expect(context).not.toContain("Future work");
    await expect(collectReleaseContext({}, root)).rejects.toThrow("explicit --from");
    await expect(collectReleaseContext({ initial: true, from: "baseline" }, root)).rejects.toThrow("Choose --initial");
    await expect(collectReleaseContext({ from: "missing", to: "candidate" }, root)).rejects.toThrow();
    await git("checkout", "--quiet", "--detach", "baseline");
    await writeFile(join(root, "release-branch.txt"), "A change on the previous release branch\n");
    await git("add", ".");
    await git("commit", "--quiet", "-m", "Previous release branch only");
    const crossBranch = await collectReleaseContext({ from: "HEAD", to: "candidate" }, root);
    expect(crossBranch).toContain("D\trelease-branch.txt");
    expect(crossBranch).toContain("A\tchange.txt");
    expect(crossBranch).not.toContain("Previous release branch only");
    const initial = await collectReleaseContext({ initial: true, to: "candidate" }, root);
    expect(initial).toContain("Supported product capabilities");
    expect(initial).toContain("Supported authoring capabilities");
    expect(initial).not.toContain("Baseline history");
  } finally { await rm(root, { recursive: true, force: true }); }
}, 30_000); // Several real Git commits and context queries can exceed Bun's five-second default.
