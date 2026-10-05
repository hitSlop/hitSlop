// The published npm packages (generated/npm), installed outside the checkout with Node
// absent: the SDK alone, then the CLI through init, check, build, preview and the
// getting-started tutorial. With HITSLOP_PACKED_NATIVE=1 (a release), also the native
// workflow: register, global skills, create, theme and export with the debug helper. Each
// test continues the one before it.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { strict as assert } from "node:assert";
import { mkdtemp, mkdir, writeFile, readFile, rm, symlink, cp, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { appAsset, repository } from "../../scripts/lib/artifacts";
import { exec } from "../../packages/cli/src/process";
import { debugHelper } from "../../scripts/lib/helper";

const native = process.env.HITSLOP_PACKED_NATIVE === "1";
const minutes = (n: number) => n * 60_000;
let root: string, coreRoot: string, noNode: Record<string, string>, env: Record<string, string | undefined>;
const tarballs: Record<string, string> = {};
const versions: Record<string, string> = {};

beforeAll(async () => {
  root = await mkdtemp(join(tmpdir(), "hitslop packed "));
  coreRoot = await mkdtemp(join(tmpdir(), "hitslop framework neutral "));
  // The installed CLI must find the file engine it ships, never this checkout's.
  const { HITSLOP_ENGINE: _engine, ...inherited } = process.env;
  env = { ...inherited, HITSLOP_NATIVE_CLI: native ? debugHelper : join(root, "native-helper-must-not-be-used") };
  const bin = join(root, "bin");
  await mkdir(bin);
  await symlink(process.execPath, join(bin, "bun"));
  noNode = { PATH: bin + ":/usr/bin:/bin:/usr/sbin:/sbin" };
  for (const name of ["cli", "document", "schema"]) {
    const metadata = JSON.parse(await readFile(join(repository, "packages", name, "package.json"), "utf8"));
    versions[name] = metadata.version;
    tarballs[metadata.name] = resolve(repository, `generated/npm/hitslop-${name}-${metadata.version}.tgz`);
  }
});
afterAll(async () => {
  await rm(root, { recursive: true, force: true });
  await rm(coreRoot, { recursive: true, force: true });
});

/** `args` in `cwd`, which must succeed: its stdout. */
async function run(args: string[], cwd: string, overrides: Record<string, string> = {}) {
  const { stdout, stderr, code } = await exec(args, { cwd, env: { ...env, ...overrides } });
  assert.equal(code, 0, `${args[1] === "-e" ? "SDK import probe" : args.slice(1).join(" ")}: ${stdout}${stderr}`);
  return stdout;
}
const project = () => join(root, "my-slop");
const cli = () => join(root, "node_modules/@hitslop/cli/src/cli.ts");
const built = () => join(project(), "dist/my-slop.slop");

test("the document SDK installs and type-checks alone, without Svelte or the workspace", async () => {
  await writeFile(
    join(coreRoot, "package.json"),
    JSON.stringify({ private: true, dependencies: { "@hitslop/document": tarballs["@hitslop/document"] }, overrides: { "@hitslop/schema": tarballs["@hitslop/schema"] } }),
  );
  await run([process.execPath, "install"], coreRoot, noNode);
  expect(JSON.parse(await readFile(join(repository, "packages/shell/package.json"), "utf8")).private).toBe(true);
  await run(
    [
      process.execPath,
      "-e",
      `
    import {strict as assert} from "node:assert";
    import {defineDocument, defineSlop, s} from "@hitslop/document";
    const schema = defineDocument({title: s.text()});
    assert.ok(schema.descriptor);
    assert.equal(defineSlop({title: "Probe", description: "Probe", author: {name: "Probe"}, categories: ["utilities"], presentation: {width: 320, height: 240}, theme: {accent: "#123456"}, schema, initial: {title: ""}}).schema, schema);
    assert.throws(() => Bun.resolveSync("svelte", process.cwd()));
    assert.throws(() => Bun.resolveSync("loro-crdt", process.cwd()));
    assert.throws(() => Bun.resolveSync("@hitslop/shell", process.cwd()));
  `,
    ],
    coreRoot,
    noNode,
  );
  // Published source must resolve its transitive platform types outside the workspace.
  await writeFile(
    join(coreRoot, "consumer.ts"),
    `
    import type {SlopApp} from "@hitslop/document/abi";
    import type {HelperRequest} from "@hitslop/schema/socket";
    const app: SlopApp = {descriptor: {}, mount: (ctx, target) => (target.textContent = String(ctx.document.current), {})};
    const read: HelperRequest = {documentPath: "/doc", method: "get"};
    void app; void read;
  `,
  );
  await run(
    [process.execPath, join(repository, "node_modules/typescript/bin/tsc"), "--noEmit", "--strict", "--skipLibCheck", "--target", "ES2022", "--module", "ESNext", "--moduleResolution", "bundler", "--allowImportingTsExtensions", "consumer.ts"],
    coreRoot,
    noNode,
  );
}, minutes(5));

test("the installed CLI creates, checks and builds a project with the engine it ships", async () => {
  await writeFile(join(root, "package.json"), JSON.stringify({ private: true, dependencies: { "@hitslop/cli": tarballs["@hitslop/cli"] }, overrides: tarballs }));
  await run([process.execPath, "install"], root, noNode);
  // The folder's name is the slug; the path above it holds spaces.
  await run([process.execPath, "x", "--no-install", "@hitslop/cli", "init", project()], root, noNode);
  const metadata = JSON.parse(await readFile(join(project(), "package.json"), "utf8"));
  expect(metadata.dependencies["@hitslop/document"]).toBe(versions.document);
  expect(metadata.devDependencies["@hitslop/cli"]).toBe(versions.cli);
  metadata.overrides = tarballs;
  await writeFile(join(project(), "package.json"), JSON.stringify(metadata));
  await run([process.execPath, "install"], project(), noNode);
  await run([process.execPath, "run", "check"], project(), noNode);
  // Building, inspecting and reading a schema use the engine the package ships, on any
  // platform, without the Mac app.
  await run([process.execPath, "run", "build"], project(), noNode);
  const inspected = JSON.parse(await run([process.execPath, cli(), "inspect", built()], root, noNode));
  expect(inspected.kind).toBe("template");
  expect(inspected.manifest.slug).toBe("my-slop");
  expect(JSON.parse(await run([process.execPath, cli(), "schema", built()], root, noNode)).kind).toBe("object");
  expect(appAsset(built(), "app.css")).toContain(".slop-paper");
  expect(appAsset(built(), "app.js")).not.toContain(repository);
}, minutes(5));

test.if(native)("the global CLI registers, links agent skills, follows upgrades and edits natively", async () => {
  const home = join(root, "home");
  await mkdir(home);
  await run([process.execPath, cli(), "register", project()], root, { ...noNode, HOME: home });
  // Agent skills come from the global install, which `bun install -g` upgrades in place.
  const bunHome = join(root, "bun home");
  const globalDir = join(bunHome, "install/global");
  await mkdir(globalDir, { recursive: true });
  // Unreleased SDK packages resolve to their tarballs; the CLI itself is installed below.
  const { "@hitslop/cli": _, ...sdkTarballs } = tarballs;
  await writeFile(join(globalDir, "package.json"), JSON.stringify({ overrides: sdkTarballs }));
  const globalEnv = { ...noNode, HOME: home, BUN_INSTALL: bunHome };
  const slop = join(bunHome, "bin/slop");
  await run([process.execPath, "install", "-g", tarballs["@hitslop/cli"]!], root, globalEnv);
  await run([slop, "skills", "--all", "--scope", "global"], root, globalEnv);
  const packagedSkills = await realpath(join(globalDir, "node_modules/@hitslop/cli/.crust/root/skills"));
  const agentSkill = (name: string) => join(home, ".agents/skills", name);
  for (const name of ["hitslop", "hitslop-authoring", "hitslop-design", "hitslop-document", "hitslop-cli"])
    expect(await realpath(agentSkill(name))).toBe(join(packagedSkills, name));
  // A later global install serves its skills through the same links.
  const next = join(root, "next");
  await mkdir(next);
  await run(["/usr/bin/tar", "-xzf", tarballs["@hitslop/cli"]!, "-C", next], root, noNode);
  const nextVersion = versions.cli + "-next";
  const nextPackage = join(next, "package");
  const manifestPath = join(nextPackage, "package.json");
  await writeFile(manifestPath, JSON.stringify({ ...JSON.parse(await readFile(manifestPath, "utf8")), version: nextVersion }));
  const cliSkill = join(nextPackage, ".crust/root/skills/hitslop-cli/SKILL.md");
  await writeFile(cliSkill, (await readFile(cliSkill, "utf8")).replace(/version: ".*"/, `version: "${nextVersion}"`));
  await run([process.execPath, "pm", "pack", "--destination", next], nextPackage, noNode);
  await run([process.execPath, "install", "-g", join(next, `hitslop-cli-${nextVersion}.tgz`)], root, globalEnv);
  expect(await readFile(join(agentSkill("hitslop-cli"), "SKILL.md"), "utf8")).toContain(`version: "${nextVersion}"`);
  const document = join(root, "Document.slop");
  await run([process.execPath, cli(), "create", "--from", built(), "--output", document], root, noNode);
  // The global CLI repairs a hitSlop skill link that names another copy.
  const stray = join(root, "stray/skills/hitslop-cli");
  await cp(join(packagedSkills, "hitslop-cli"), stray, { recursive: true });
  await rm(agentSkill("hitslop-cli"));
  await symlink(stray, agentSkill("hitslop-cli"));
  await run([slop, "skills", "repair", "--scope", "global"], root, globalEnv);
  expect(await realpath(agentSkill("hitslop-cli"))).toBe(join(packagedSkills, "hitslop-cli"));
  await run([process.execPath, cli(), "theme", "set", document, "--values", '{"accent":"#123456"}'], root, noNode);
  expect(JSON.parse(await run([process.execPath, cli(), "theme", "get", document], root, noNode)).effective.accent).toBe("#123456");
  for (const format of ["png", "pdf"])
    await run([process.execPath, cli(), "export", document, "--format", format, "--output", join(root, "output." + format)], root, noNode);
}, minutes(5));

test("the installed CLI previews the project with its own page shell and core", async () => {
  const preview = Bun.spawn([process.execPath, cli(), "dev", project(), "--port", "5197"], {
    cwd: root,
    env: { ...env, ...noNode },
    stdout: "pipe",
    stderr: "pipe",
  });
  const previewOutput = new Response(preview.stdout).text();
  const previewError = new Response(preview.stderr).text();
  let previewExited = false;
  void preview.exited.then(() => (previewExited = true));
  async function stopPreview() {
    const timeout = setTimeout(() => preview.kill("SIGKILL"), 5_000);
    try {
      preview.kill("SIGINT");
      await preview.exited;
    } finally {
      clearTimeout(timeout);
    }
  }
  try {
    let frame: Response | undefined;
    const deadline = Date.now() + 180_000;
    while (Date.now() < deadline && !previewExited) {
      const response = await fetch("http://127.0.0.1:5197/", { signal: AbortSignal.timeout(2_000) }).catch(() => undefined);
      if (response?.ok) {
        frame = response;
        break;
      }
      await Bun.sleep(50);
    }
    if (!frame) throw new Error("Packed preview failed: " + (await stopPreview().then(() => previewError)) + (await previewOutput));
    expect(await frame.text()).toContain('src="/app.html"');
    const app = await fetch("http://127.0.0.1:5197/app.html");
    expect(app.status).toBe(200);
    expect(await app.text()).toContain("/__shell__/boot.js");
    expect((await fetch("http://127.0.0.1:5197/__shell__/index.js")).status).toBe(200);
    const wasm = await fetch("http://127.0.0.1:5197/__shell__/core/hitslop_core_wasm_bg.wasm");
    expect(wasm.status).toBe(200);
    expect(Buffer.from(await wasm.arrayBuffer()).subarray(0, 4).toString("hex")).toBe("0061736d");
    if (native) {
      const { webkit } = await import("playwright");
      const browser = await webkit.launch();
      try {
        const page = await browser.newPage();
        page.on("pageerror", (error) => console.error("Packed preview:", error.message));
        page.on("console", (message) => {
          if (message.type() === "error") console.error("Packed preview:", message.text());
        });
        page.on("response", async (response) => {
          if (response.status() >= 400)
            console.error("Packed preview response:", response.url(), await fetch(response.url()).then((r) => r.text()).catch(String));
        });
        await page.goto("http://127.0.0.1:5197/");
        const frame = page.frameLocator("iframe");
        await frame.getByRole("textbox", { name: "List title", exact: true }).fill("Installed SDK works");
        expect(await frame.locator("[data-hitslop-root]").count()).toBe(1);
        const styles = join(project(), "styles.css");
        await writeFile(styles, (await readFile(styles, "utf8")) + "\n.slop-eyebrow { color: rgb(11, 22, 33); }\n");
        await page.frames().find((frame) => frame.url().includes("/app.html"))!.waitForFunction(
          () => getComputedStyle(document.querySelector(".slop-eyebrow")!).color === "rgb(11, 22, 33)",
        );
        expect(await frame.getByRole("textbox", { name: "List title", exact: true }).inputValue()).toBe("Installed SDK works");
      } finally {
        await browser.close();
      }
    }
  } finally {
    await stopPreview();
  }
}, minutes(5));

// The getting-started code compiles, so the documentation is an executable contract.
test("the getting-started tutorial's code checks and builds", async () => {
  const tutorial = await readFile(join(repository, "apps/landing/src/content/docs/docs/getting-started.mdx"), "utf8");
  for (const match of tutorial.matchAll(/```(?:ts|svelte|css) title="([^"]+)"\n([\s\S]*?)\n```/g))
    await writeFile(join(project(), match[1]!), match[2]!);
  await run([process.execPath, "run", "check"], project(), noNode);
  if (native) await run([process.execPath, "run", "build"], project(), noNode);
}, minutes(5));
