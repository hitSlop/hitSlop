import "./generate";
import { build } from "esbuild";
import { compile } from "svelte/compiler";
import { readFile, mkdir, cp } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { Document } from "../../packages/document/src/document";
import { MemoryStore } from "../../packages/document/src/memory";
import { checklist, richtext, mixed, initial } from "./fixtures";
const here = import.meta.dir,
  dist = join(here, "dist");
await mkdir(dist, { recursive: true });
for (const [kind, definition] of Object.entries({ checklist, richtext, mixed })) {
  await Bun.write(join(dist, `${kind}.schema.json`), JSON.stringify(definition.descriptor));
  for (const count of [1, 1000, 5000, 40000]) {
    const doc = await Document.open(definition, new MemoryStore(), initial(count, kind));
    await Bun.write(join(dist, `${kind}-${count}.snapshot`), doc.exportSnapshot());
    await Bun.write(join(dist, `${kind}-${count}.json`), JSON.stringify(doc.current));
    await doc.close();
  }
}
for (const variant of ["baseline", "matched"]) {
  const loro = dirname(
    Bun.resolveSync(
      "loro-crdt/package.json",
      variant === "baseline" ? resolve(here, "../..") : here,
    ),
  );
  await cp(join(loro, "web"), join(dist, `loro-${variant}`), { recursive: true });
  await build({
    entryPoints: [join(here, "web/wasm.ts")],
    outfile: join(dist, `wasm-${variant}.js`),
    bundle: true,
    format: "esm",
    platform: "browser",
    target: "safari18",
    plugins: [
      {
        name: "loro",
        setup(b) {
          b.onResolve({ filter: /^loro-crdt\/web\/loro_wasm(?:\.js)?$/ }, () => ({
            path: `./loro-${variant}/loro_wasm.js`,
            external: true,
          }));
          b.onResolve({ filter: /^loro-crdt(?:\/web)?$/ }, () => ({
            path: `./loro-${variant}/index.js`,
            external: true,
          }));
        },
      },
    ],
  });
}
await build({
  entryPoints: [join(here, "web/app.ts")],
  outfile: join(dist, "app.js"),
  bundle: true,
  format: "esm",
  platform: "browser",
  target: "safari18",
  conditions: ["browser"],
  plugins: [
    {
      name: "svelte",
      setup(b) {
        b.onLoad({ filter: /\.svelte$/ }, async (a) => ({
          contents: compile(await readFile(a.path, "utf8"), {
            filename: a.path,
            generate: "client",
            css: "injected",
          }).js.code,
          loader: "js",
          resolveDir: dirname(a.path),
        }));
      },
    },
  ],
});
await Bun.write(
  join(dist, "index.html"),
  `<!doctype html><meta charset="utf-8"><title>Engine placement spike</title><script type="module" src="app.js"></script>`,
);
console.log("Prepared isolated fixtures and browser bundles");
