import { strict as assert } from "node:assert";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  readdir,
  rm,
  symlink,
  cp,
  readlink,
  rename,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const root = await mkdtemp(join(tmpdir(), "hitslop packed "));
const coreRoot = await mkdtemp(join(tmpdir(), "hitslop framework neutral "));
const repository = process.cwd();
const native = process.argv.includes("--native");
const env = {
  ...process.env,
  HITSLOP_NATIVE_CLI: native
    ? resolve("apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native")
    : join(root, "native-helper-must-not-be-used"),
};
async function run(args: string[], cwd: string, overrides: Record<string, string> = {}) {
  const started = performance.now();
  const label = args[1] === "-e" ? "SDK import probe" : args.slice(1).join(" ");
  console.log("START", label);
  const p = Bun.spawn(args, { cwd, env: { ...env, ...overrides }, stdout: "pipe", stderr: "pipe" });
  const [out, error, code] = await Promise.all([
    new Response(p.stdout).text(),
    new Response(p.stderr).text(),
    p.exited,
  ]);
  assert.equal(
    code,
    0,
    `${label} (${((performance.now() - started) / 1000).toFixed(1)}s): ${out}${error}`,
  );
  console.log("PASS", label, `(${((performance.now() - started) / 1000).toFixed(1)}s)`);
  return out;
}
const bin = join(root, "bin");
await mkdir(bin);
await symlink(process.execPath, join(bin, "bun"));
const noNode = { PATH: bin + ":/usr/bin:/bin:/usr/sbin:/sbin" };
try {
  const tarballs: Record<string, string> = {};
  const versions: Record<string, string> = {};
  for (const name of ["cli", "document", "schema"]) {
    const metadata = JSON.parse(
      await readFile(join(repository, "packages", name, "package.json"), "utf8"),
    );
    versions[name] = metadata.version;
    tarballs[metadata.name] = resolve(`generated/v1/npm/hitslop-${name}-${metadata.version}.tgz`);
  }
  await writeFile(
    join(coreRoot, "package.json"),
    JSON.stringify({
      private: true,
      dependencies: { "@hitslop/document": tarballs["@hitslop/document"] },
      overrides: { "@hitslop/schema": tarballs["@hitslop/schema"] },
    }),
  );
  await run([process.execPath, "install"], coreRoot, noNode);
  const documentPackage = join(coreRoot, "node_modules/@hitslop/document");
  assert.ok(!(await readdir(documentPackage)).includes("test-support"));
  const documentSource = await readdir(join(documentPackage, "src"));
  for (const internal of [
    "sqlite.ts",
    "writer-lock.ts",
    "document.ts",
    "runtime-entry.ts",
    "boot.ts",
  ])
    assert.ok(
      !documentSource.includes(internal),
      `Engine or test implementation shipped in npm SDK: ${internal}`,
    );
  await run(
    [
      process.execPath,
      "-e",
      `
    import {strict as assert} from "node:assert";
    import {defineDocument, s} from "@hitslop/document";
    import {attachments} from "@hitslop/document/attachments";
    assert.equal(typeof attachments.import, "function");
    assert.ok(defineDocument({title: s.text()}).descriptor);
    assert.throws(() => Bun.resolveSync("svelte", process.cwd()));
    assert.throws(() => Bun.resolveSync("loro-crdt", process.cwd()));
    assert.throws(() => Bun.resolveSync("@hitslop/document/runtime", process.cwd()));
  `,
    ],
    coreRoot,
    noNode,
  );
  console.log("PASS packed document SDK loads without Svelte installed");
  // Published source must resolve its transitive platform types outside the workspace.
  await writeFile(
    join(coreRoot, "consumer.ts"),
    `
    import type {SlopApp} from "@hitslop/document/abi";
    import type {SocketRequest} from "@hitslop/schema/socket";
    import type {BridgeReply} from "@hitslop/schema/bridge";
    const app: SlopApp = {mount: (ctx, target) => (target.textContent = String(ctx.document.current), {})};
    const read: SocketRequest = {id: "read", documentPath: "/doc", method: "get"};
    const checkpoint: BridgeReply<"load">["checkpoint"] = null;
    void app; void read; void checkpoint;
  `,
  );
  await run(
    [
      process.execPath,
      join(repository, "node_modules/typescript/bin/tsc"),
      "--noEmit",
      "--strict",
      "--skipLibCheck",
      "--target",
      "ES2022",
      "--module",
      "ESNext",
      "--moduleResolution",
      "bundler",
      "--allowImportingTsExtensions",
      "consumer.ts",
    ],
    coreRoot,
    noNode,
  );
  await writeFile(
    join(root, "package.json"),
    JSON.stringify({
      private: true,
      dependencies: { "@hitslop/cli": tarballs["@hitslop/cli"] },
      overrides: tarballs,
    }),
  );
  await run([process.execPath, "install"], root, noNode);
  const cli = join(root, "node_modules/@hitslop/cli/src/cli.ts");
  const project = join(root, "my slop");
  await run([process.execPath, "x", "--no-install", "@hitslop/cli", "init", project], root, noNode);
  const metadata = JSON.parse(await readFile(join(project, "package.json"), "utf8"));
  assert.equal(metadata.dependencies["@hitslop/document"], versions.document);
  assert.equal(metadata.devDependencies["@hitslop/cli"], versions.cli);
  metadata.overrides = tarballs;
  await writeFile(join(project, "package.json"), JSON.stringify(metadata));
  await run([process.execPath, "install"], project, noNode);
  await run([process.execPath, "run", "check"], project, noNode);
  if (native) {
    await run([process.execPath, "run", "build"], project, noNode);
    const built = join(project, "dist/my-slop.slop");
    assert.ok((await readFile(join(built, "assets/app.css"), "utf8")).includes(".slop-paper"));
    assert.ok(!(await readFile(join(built, "assets/app.js"), "utf8")).includes(repository));
    const home = join(root, "home");
    await mkdir(home);
    await run([process.execPath, cli, "register", project], root, { ...noNode, HOME: home });
    await run([process.execPath, cli, "skills", "--all", "--scope", "project"], project, {
      ...noNode,
      HOME: home,
    });
    const links = [];
    for (const relative of new Bun.Glob("**/skills/*").scanSync({
      cwd: project,
      onlyFiles: false,
      dot: true,
    })) {
      const path = join(project, relative);
      const target = await readlink(path).catch(() => undefined);
      if (target && target.includes(".hitslop")) links.push(path);
    }
    assert.ok(links.length, "Skills must link to the durable cache");
    const packagedSkills = join(root, "node_modules/@hitslop/cli/.crust/root/skills");
    await rename(packagedSkills, packagedSkills + ".evicted");
    for (const link of links)
      assert.ok((await readFile(join(link, "SKILL.md"), "utf8")).length > 0);
    const document = join(root, "Document.slop");
    await cp(built, document, { recursive: true });
    await run(
      [process.execPath, cli, "theme", "set", document, "--values", '{"accent":"#123456"}'],
      root,
      noNode,
    );
    const theme = JSON.parse(
      await run([process.execPath, cli, "theme", "get", document], root, noNode),
    );
    assert.equal(theme.effective.accent, "#123456");
    for (const format of ["png", "pdf"])
      await run(
        [
          process.execPath,
          cli,
          "export",
          document,
          "--format",
          format,
          "--output",
          join(root, "output." + format),
        ],
        root,
        noNode,
      );
  }
  const preview = Bun.spawn([process.execPath, cli, "dev", project, "--port", "5197"], {
    cwd: root,
    env: { ...env, ...noNode },
    stdout: "pipe",
    stderr: "pipe",
  });
  const previewOutput = new Response(preview.stdout).text();
  const previewError = new Response(preview.stderr).text();
  let previewExited = false;
  void preview.exited.then(() => {
    previewExited = true;
  });
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
      const response = await fetch("http://127.0.0.1:5197/", {
        signal: AbortSignal.timeout(2_000),
      }).catch(() => undefined);
      if (response?.ok) {
        frame = response;
        break;
      }
      await Bun.sleep(50);
    }
    if (!frame) {
      await stopPreview();
      throw new Error("Packed preview failed: " + (await previewError) + (await previewOutput));
    }
    assert.ok((await frame.text()).includes('src="/app.html"'));
    const app = await fetch("http://127.0.0.1:5197/app.html");
    assert.equal(app.status, 200);
    assert.ok((await app.text()).includes("/__shell__/boot.js"));
    assert.equal((await fetch("http://127.0.0.1:5197/__shell__/index.js")).status, 200);
    const wasm = await fetch("http://127.0.0.1:5197/__shell__/core/hitslop_core_wasm_bg.wasm");
    assert.equal(wasm.status, 200);
    assert.equal(
      Buffer.from(await wasm.arrayBuffer())
        .subarray(0, 4)
        .toString("hex"),
      "0061736d",
    );
  } finally {
    await stopPreview();
  }
  // Compile the actual getting-started code, so documentation is an executable contract.
  const tutorial = await readFile(
    join(repository, "apps/landing/src/content/docs/docs/getting-started.mdx"),
    "utf8",
  );
  for (const match of tutorial.matchAll(/```(?:ts|svelte|css) title="([^"]+)"\n([\s\S]*?)\n```/g)) {
    await writeFile(join(project, match[1]!), match[2]!);
  }
  await run([process.execPath, "run", "check"], project, noNode);
  if (native) await run([process.execPath, "run", "build"], project, noNode);
  console.log(
    native
      ? "PASS packed CLI native workflow: init/install/check/dev/build/register/theme/export with Node absent"
      : "PASS packed SDK and CLI: install/types/init/check/dev with Node and native rendering absent",
  );
} finally {
  await rm(root, { recursive: true, force: true });
  await rm(coreRoot, { recursive: true, force: true });
}
