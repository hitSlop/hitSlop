import { build, parseSync, transformWithOxc, Visitor, type ParseResult, type Plugin } from "vite";
import { compile, compileModule, parse } from "svelte/compiler";
import { analyze } from "periscopic";
import { dirname, join, relative } from "node:path";
import postcss from "postcss";
import { readFile } from "node:fs/promises";
import { refusePageRole, refuseRoleImport, type AppRoles } from "./app-vite";
import { bytesOf, typeOf, type Emitted } from "./definition-resources";
import type { DefinitionBuildOptions } from "./definition-config";
import { portableAssetURLs } from "./asset-url-transform";
import { declaresModuleState } from "./capture-state";

/** Each static role builds its own graph, including stylesheets and component CSS. */
export type CaptureInputs = {
  source: string;
  roles: AppRoles;
  sdk: string;
  options: DefinitionBuildOptions;
};
type Binding = [specifier: string, name: string];
type Resolve = (specifier: string, importer: string) => Promise<string | undefined>;
// What authored code may not reference: the page, and the web APIs a capture's JavaScript lacks
// (a bare QuickJS, crates/hitslop-runner/src/realm.rs). Packages are not inspected for these:
// they use some behind guards, and Svelte's own renderer names AbortController and TextEncoder.
const browserGlobals = [
  "window",
  "document",
  "navigator",
  "requestAnimationFrame",
  "fetch",
  "XMLHttpRequest",
  "Image",
  "Worker",
  "URL",
  "URLSearchParams",
  "TextEncoder",
  "TextDecoder",
  "AbortController",
  "Blob",
  "structuredClone",
];

/** Every graph refuses the host's own runtime: the document engine and the SDK's sibling page
 * shell, not an author folder that happens to be named shell. */
export function refuseHostRuntime(id: string, sdk: string, error: (message: string) => never) {
  if (id.includes("/node_modules/loro-crdt/") || id.startsWith(join(dirname(sdk), "shell") + "/"))
    error(`Embedded host runtime rejected: ${id}`);
}

/** Each capture closure is a classic synchronous script; it never mounts the editor. Its
 * build inspects the authored modules it compiles, before SSR erases unsupported calls. */
export async function buildCapture(
  role: "widget" | "icon" | "preview" | "export",
  { source, roles, sdk, options }: CaptureInputs,
) {
  const component = roles[role];
  if (!component) return;
  const functionName = role === "widget" ? "__slopWidget" : "__slopCapture";
  const renderer = role === "widget" ? "widgetRenderer" : "viewRenderer";
  const entry = `\0hitslop-${role}-entry`;
  const styles = new Map<string, string>();
  const dependencies = new Set<string>();
  const reasons = new Set<string>();
  // Captures render saved state in a fresh page, so a module the editor sets shows only
  // its initial value there, even when the editor is reused as the export component.
  const stateful = new Set<string>();
  // Imported names a capture may not use, by resolved module (see buildStart).
  const editorOnly = new Map<string, (name: string) => boolean>();
  const reexports = new Map<string, Promise<ReturnType<typeof bindings>>>();
  const authored = (id: string) =>
    /\.(svelte|[cm]?[jt]s)$/.test(id) &&
    !id.startsWith("\0") &&
    !id.startsWith(sdk) &&
    !id.includes("/node_modules/");
  /** Whether importing `name` from `id` reaches editor-only runtime through authored
   * re-exports. A namespace import ("*") reaches everything its module exports. */
  const forbidden = async (
    id: string | undefined,
    name: string,
    resolve: Resolve,
    seen = new Set<string>(),
  ): Promise<boolean> => {
    if (!id) return false;
    const rule = editorOnly.get(id);
    if (rule) return rule(name);
    // A component's default export is the component itself.
    if (
      !authored(id) ||
      seen.has(`${id}\0${name}`) ||
      (id.endsWith(".svelte") && name === "default")
    )
      return false;
    seen.add(`${id}\0${name}`);
    if (!reexports.has(id))
      reexports.set(
        id,
        readFile(id, "utf8").then((code) =>
          id.endsWith(".svelte")
            ? bindings(
                parseSync(id, compile(code, { filename: id, generate: "server" }).js.code, {
                  lang: "js",
                }).module,
              )
            : bindings(parseSync(id, code).module),
        ),
      );
    const { exports, stars } = await reexports.get(id)!;
    const binding = exports.get(name);
    const next: Binding[] =
      name === "*"
        ? [...exports.values(), ...stars.map((star): Binding => [star, "*"])]
        : binding
          ? [binding]
          : name === "default"
            ? []
            : stars.map((star): Binding => [star, name]);
    for (const [specifier, imported] of next)
      if (await forbidden(await resolve(specifier, id), imported, resolve, seen)) return true;
    return false;
  };
  const plugin: Plugin = {
    name: "hitslop-capture-ssr",
    enforce: "pre",
    async buildStart() {
      // Editor-only runtime: every value hitslop/svelte and hitslop/embed export, and Svelte's
      // onMount, which SSR never runs. A capture reads attachment URLs from its `attachmentURL`
      // prop and a video's thumbnail from hitslop/youtube, which only works on strings.
      for (const specifier of ["hitslop/svelte", "hitslop/embed"]) {
        const found = await this.resolve(specifier, component);
        if (!found) continue;
        const values = new Set(
          parseSync(found.id, await readFile(found.id, "utf8")).module.staticExports.flatMap(
            ({ entries }) =>
              entries.flatMap(({ isType, exportName }) =>
                !isType && exportName.name ? [exportName.name] : [],
              ),
          ),
        );
        editorOnly.set(found.id, (name) => name === "*" || values.has(name));
      }
      const svelte = await this.resolve("svelte", component);
      if (svelte) editorOnly.set(svelte.id, (name) => name === "*" || name === "onMount");
    },
    async resolveId(id, importer, resolveOptions) {
      if (id === entry || styles.has(id)) return id;
      if (id.startsWith("node:")) return `\0capture-unavailable:${id}`;
      refuseRoleImport(id, importer, roles, false, (message) => this.error(message));
      if (!importer || importer === entry) return;
      const found = await this.resolve(id, importer, { ...resolveOptions, skipSelf: true });
      if (found) refusePageRole(found.id, roles, (message) => this.error(message));
      return found;
    },
    load(id) {
      if (id.startsWith("\0capture-unavailable:"))
        return 'throw Error("Node services are unavailable in synchronous capture rendering");';
      if (styles.has(id)) return styles.get(id);
      if (id !== entry) return;
      return `import Component from ${JSON.stringify(component)};
import {${renderer}} from ${JSON.stringify(join(sdk, role === "widget" ? "app/widget-render.ts" : "app/capture-render.ts"))};
globalThis.${functionName} = json => ${renderer}(Component,globalThis.__slopCaptureCSS)(json);`;
    },
    async transform(code, id) {
      const resolve: Resolve = async (specifier, importer) =>
        (await this.resolve(specifier, importer))?.id;
      /** Awaits, editor-only imports and browser globals in an authored module's JavaScript. */
      const inspect = async (js: string) => {
        const { program, module } = parseSync(id, js, { lang: "js" });
        new Visitor({
          AwaitExpression() {
            reasons.add(`${id}: await`);
          },
        }).visit(program);
        for (const reason of retainedCaptureReasons(js, id, browserGlobals)) reasons.add(reason);
        for (const [specifier, name] of bindings(module).imports)
          if (await forbidden(await resolve(specifier, id), name, resolve))
            reasons.add(`${id}: ${name === "*" ? `* from ${specifier}` : name}`);
      };
      if (id.endsWith(".svelte")) {
        // Before compiling, which refuses top-level await without experimental options.
        if (!id.startsWith(sdk) && awaits(parse(code, { modern: true })))
          reasons.add(`${id}: await`);
        const result = compile(code, {
          filename: id,
          generate: "server",
          css: "external",
          dev: false,
          cssHash: ({ hash, css }) => `svelte-${hash(css)}`,
        });
        if (result.ast.css && !result.css)
          this.error(
            `Remove css="injected" from ${id}; capture CSS is collected by the build for native rendering`,
          );
        if (authored(id)) await inspect(result.js.code);
        const style = id.replace(/\.svelte$/, ".widget.css");
        if (result.css) styles.set(style, result.css.code);
        return {
          code: result.js.code + (result.css ? `\nimport ${JSON.stringify(style)};` : ""),
          map: result.js.map,
        };
      }
      const runes = /\.svelte\.[jt]s$/.test(id);
      if (!runes && !authored(id)) return;
      // TypeScript is erased first, as the build erases it, so a type is never a use.
      const js = /\.[cm]?ts$/.test(id) ? (await transformWithOxc(code, id)).code : code;
      if (authored(id)) {
        await inspect(js);
        if (id.startsWith(source + "/") && declaresModuleState(code, id)) stateful.add(id);
      }
      if (!runes) return;
      return compileModule(js, { filename: id, generate: "server", dev: false }).js;
    },
    generateBundle() {
      for (const id of this.getModuleIds()) {
        if (!id.startsWith("\0") && !styles.has(id)) dependencies.add(id.split("?", 1)[0]!);
        refuseHostRuntime(id, sdk, (message) => this.error(message));
      }
    },
  };
  const refuse = (found: Iterable<string>) => {
    const sorted = [...found].sort();
    return sorted.length
      ? new Error(
          `${component} (${role} capture) must render synchronously with Takumi:\n${sorted.join("\n")}`,
        )
      : undefined;
  };
  const refusal = () =>
    stateful.size
      ? new Error(
          `${component} (the ${role} view) imports ${relative(source, [...stateful].sort()[0]!)}, whose $state is always its initial value in a capture. Captures render saved state in a fresh page: read the snapshot prop there, and give a component shared with the editor that value as a prop.`,
        )
      : refuse(reasons);
  const result = await build({
    root: source,
    configFile: false,
    envFile: false,
    publicDir: false,
    logLevel: "error",
    base: "/assets/",
    plugins: [
      plugin,
      {
        name: "hitslop-capture-asset-urls",
        enforce: "post",
        renderChunk(code, chunk) {
          if (!code.includes("/assets/")) return;
          return portableAssetURLs(code, chunk.fileName);
        },
      },
    ],
    resolve: {
      alias: options.alias,
      dedupe: ["svelte"],
      conditions: ["svelte", "node"],
      tsconfigPaths: true,
    },
    ssr: { noExternal: true, resolve: { conditions: ["svelte", "node"] } },
    build: {
      ssr: true,
      emitAssets: true,
      write: false,
      minify: "oxc",
      target: "safari17",
      assetsInlineLimit: 0,
      cssCodeSplit: false,
      rolldownOptions: {
        input: entry,
        output: { format: "iife", codeSplitting: false, entryFileNames: `${role}.js` },
      },
    },
  }).catch((error) => {
    // A refused module can also fail to compile: report the cause.
    throw refusal() ?? error;
  });
  const refused = refusal();
  if (refused) throw refused;
  if ("on" in result) throw new Error("Capture builds cannot watch");
  let program = "";
  let css = "";
  const resources = new Map<string, Emitted>();
  for (const output of Array.isArray(result) ? result : [result])
    for (const item of output.output) {
      if (item.type === "chunk") program += item.code;
      // Raw CSS URL imports are resources, not part of the role's cascade. Vite names
      // only its combined stylesheet with this synthetic source name.
      else if (
        typeof item.source === "string" &&
        item.names.includes("style.css") &&
        item.originalFileNames.includes("style.css")
      )
        css += "\n" + new TextDecoder().decode(bytesOf(item.source));
      else
        resources.set(item.fileName, {
          key: item.fileName,
          bytes: bytesOf(item.source),
          mediaType: typeOf(item.fileName)[0],
        });
    }
  // Resources are immutable bytes from the build inventory, never runtime filesystem URLs.
  const urls = new Map<string, string>();
  const visiting = new Set<string>();
  const inline = (text: string): string =>
    text.replace(/\/assets\/([a-zA-Z0-9_./-]+)/g, (_, key: string) => {
      const cached = urls.get(key);
      if (cached) return cached;
      const resource = resources.get(key);
      if (!resource) throw new Error(`Capture references an asset no build emitted: ${key}`);
      if (visiting.has(key)) throw new Error(`Circular capture resource reference: ${key}`);
      visiting.add(key);
      // Vite also rewrites dependencies inside raw CSS assets. Resolve those before
      // encoding the asset, so a nested URL never needs a filesystem or browser origin.
      const bytes =
        resource.mediaType === "text/css"
          ? bytesOf(inline(new TextDecoder().decode(resource.bytes)))
          : resource.bytes;
      const url = `data:${resource.mediaType};base64,${Buffer.from(bytes).toString("base64")}`;
      urls.set(key, url);
      visiting.delete(key);
      return url;
    });
  const sheet = postcss.parse(inline(css));
  // A package can introduce a missing global even when authored modules do not
  // mention it. Inspect the retained SSR code after dependency tree-shaking.
  const unavailable = retainedCaptureReasons(program, `${role}.js`, ["Intl"]);
  const fonts: { family: string; bytes: string; weight?: number; italic: boolean }[] = [];
  sheet.walkAtRules("font-face", (rule) => {
    const properties: Record<string, string> = {};
    rule.walkDecls((declaration) => {
      properties[declaration.prop] = declaration.value;
    });
    const family = properties["font-family"]?.replace(/^(['"])(.*)\1$/, "$2");
    const source = properties.src?.match(
      /url\(\s*["']?data:[^;,]+;base64,([A-Za-z0-9+/=]+)["']?\s*\)/,
    );
    if (!family || !source)
      throw new Error(
        "Capture @font-face must name a family and an imported, bundled font URL; local() and remote resources are unavailable",
      );
    const style = properties["font-style"] ?? "normal";
    if (style !== "normal" && style !== "italic")
      throw new Error(`Capture font ${family}: use normal or italic font-style`);
    const weight = properties["font-weight"] ?? "normal";
    const numeric =
      weight === "normal"
        ? 400
        : weight === "bold"
          ? 700
          : /^\d+$/.test(weight)
            ? Number(weight)
            : undefined;
    if (numeric === undefined && !/^\d+\s+\d+$/.test(weight))
      throw new Error(`Capture font ${family}: unsupported font-weight ${weight}`);
    fonts.push({
      family,
      bytes: source[1]!,
      ...(numeric === undefined ? {} : { weight: numeric }),
      italic: style === "italic",
    });
    rule.remove();
  });
  const missing = refuse(unavailable);
  if (missing) throw missing;
  // The font bytes are registered once for the batch, not repeated for each family.
  return {
    bytes:
      bytesOf(`"use strict";globalThis.__slopCaptureCSS=${JSON.stringify(sheet.toString())};\n${inline(program)}\n{
const render = globalThis.${functionName};
globalThis.${functionName} = json => JSON.stringify({frames:JSON.parse(render(json)),fonts:${JSON.stringify(fonts)}});
}`),
    dependencies,
  };
}

/** Real await syntax, not comments or strings: the compiler AST distinguishes them. */
function awaits(node: unknown): boolean {
  if (!node || typeof node !== "object") return false;
  const { type } = node as { type?: unknown };
  return (
    type === "AwaitBlock" ||
    type === "AwaitExpression" ||
    Object.values(node).some((value) => (Array.isArray(value) ? value.some(awaits) : awaits(value)))
  );
}

/** A module's value imports, the bindings it re-exports by exported name, and its
 * `export *` sources. A namespace binding is named "*". */
function bindings({ staticImports, staticExports }: ParseResult["module"]) {
  const imports: Binding[] = [];
  const locals = new Map<string, Binding>();
  for (const { moduleRequest, entries } of staticImports)
    for (const { importName, localName, isType } of entries) {
      if (isType) continue;
      const binding: Binding = [
        moduleRequest.value,
        importName.name ?? (importName.kind === "Default" ? "default" : "*"),
      ];
      imports.push(binding);
      locals.set(localName.value, binding);
    }
  const exports = new Map<string, Binding>();
  const stars: string[] = [];
  for (const { entries } of staticExports)
    for (const { moduleRequest, importName, exportName, localName, isType } of entries) {
      if (isType) continue;
      if (moduleRequest && importName.kind === "AllButDefault") stars.push(moduleRequest.value);
      const binding: Binding | undefined = moduleRequest
        ? [moduleRequest.value, importName.name ?? "*"]
        : locals.get(localName.name ?? "");
      const name = exportName.kind === "Default" ? "default" : exportName.name;
      if (binding && name) exports.set(name, binding);
    }
  return { imports, exports, stars };
}

/** Inspect retained code, not unused helpers in a shared source module. Binding the
 * program distinguishes browser globals from local names such as `document`. */
function retainedCaptureReasons(code: string, filename: string, names: string[]): string[] {
  const { program, module } = parseSync(filename, code, { lang: "js" });
  const reasons = new Set<string>();
  const uses = (name: string) =>
    reasons.add(
      `${filename}: retained dependency uses ${name}, unavailable in synchronous capture rendering`,
    );
  // Periscopic leaves namespace imports undeclared.
  const namespaces = new Set(
    module.staticImports.flatMap(({ entries }) =>
      entries.flatMap(({ importName, localName }) =>
        importName.kind === "NamespaceObject" ? [localName.value] : [],
      ),
    ),
  );
  for (const name of analyze(program as unknown as Parameters<typeof analyze>[0]).globals.keys())
    if (names.includes(name) && !namespaces.has(name)) uses(name);
  new Visitor({
    MemberExpression(node) {
      if (
        !node.computed &&
        node.object.type === "Identifier" &&
        ["globalThis", "self", "window"].includes(node.object.name) &&
        node.property.type === "Identifier" &&
        names.includes(node.property.name)
      )
        uses(node.property.name);
    },
    CallExpression({ callee }) {
      if (
        names.includes("Intl") &&
        callee.type === "MemberExpression" &&
        !callee.computed &&
        callee.property.type === "Identifier" &&
        /^(toLocaleString|toLocaleDateString|toLocaleTimeString)$/.test(callee.property.name)
      )
        reasons.add(
          `${filename}: ${callee.property.name} requires locale services unavailable in QuickJS`,
        );
    },
  }).visit(program);
  return [...reasons];
}
