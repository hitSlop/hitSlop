import { build, transformWithOxc, type Plugin } from "vite";
import { compile, compileModule, parse } from "svelte/compiler";
import { join } from "node:path";
import postcss from "postcss";
import ts from "typescript";
import { readFile } from "node:fs/promises";
import { discoverRoles, refusePageRole, refuseRoleImport } from "./app-vite";
import { bytesOf, typeOf, type Emitted } from "./definition-resources";
import type { DefinitionBuildOptions } from "./definition-config";
import { portableAssetURLs } from "./asset-url-transform";

/** Each capture closure is a classic synchronous script; it never mounts the editor. */
export async function buildCapture(
  role: "widget" | "icon" | "preview" | "export",
  source: string,
  sdk: string,
  ui: Map<string, Emitted>,
  options: DefinitionBuildOptions,
  globalStyles: string[],
) {
  const roles = await discoverRoles(source);
  if (!roles[role]) return;
  const functionName = role === "widget" ? "__slopWidget" : "__slopCapture";
  const renderer = role === "widget" ? "widgetRenderer" : "viewRenderer";
  const entry = `\0hitslop-${role}-entry`;
  const styles = new Map<string, string>();
  const dependencies = new Set<string>();
  const plugin: Plugin = {
    name: "hitslop-capture-ssr",
    enforce: "pre",
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
      if (id === entry)
        return `${globalStyles.map((path) => `import ${JSON.stringify(path)};`).join("\n")}
import doc from ${JSON.stringify(roles.schema)};
import Widget from ${JSON.stringify(roles[role])};
import {${renderer}} from ${JSON.stringify(join(sdk, role === "widget" ? "app/widget-render.ts" : "app/capture-render.ts"))};
globalThis.${functionName} = json => ${renderer}(doc,Widget,globalThis.__slopCaptureCSS)(json);`;
    },
    async transform(code, id) {
      if (id.endsWith(".svelte")) {
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
        // The compiler AST distinguishes real await syntax from comments and strings.
        const visit = (node: unknown): void => {
          if (!node || typeof node !== "object") return;
          const value = node as { type?: string };
          if (
            !id.startsWith(sdk) &&
            (value.type === "AwaitBlock" || value.type === "AwaitExpression")
          )
            this.error(`Native capture rendering is synchronous: remove await from ${id}`);
          Object.values(node).forEach((value) =>
            Array.isArray(value) ? value.forEach(visit) : visit(value),
          );
        };
        visit(result.ast);
        if (!id.startsWith(sdk) && !id.includes("/node_modules/")) {
          const reasons = retainedCaptureReasons(result.js.code, id, [
            "window",
            "document",
            "navigator",
            "requestAnimationFrame",
            "fetch",
            "XMLHttpRequest",
            "Image",
            "Worker",
          ]);
          if (reasons.length) this.error(reasons.join("\n"));
        }
        const style = id.replace(/\.svelte$/, ".widget.css");
        if (result.css) styles.set(style, result.css.code);
        return {
          code: result.js.code + (result.css ? `\nimport ${JSON.stringify(style)};` : ""),
          map: result.js.map,
        };
      }
      if (/\.svelte\.[jt]s$/.test(id)) {
        const stripped = id.endsWith(".ts") ? (await transformWithOxc(code, id)).code : code;
        return compileModule(stripped, { filename: id, generate: "server", dev: false }).js;
      }
    },
    generateBundle() {
      for (const id of this.getModuleIds())
        if (!id.startsWith("\0") && !styles.has(id)) dependencies.add(id.split("?", 1)[0]!);
    },
  };
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
          if (code.includes("/assets/")) return portableAssetURLs(code, chunk.fileName);
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
  });
  if ("on" in result) throw new Error("Capture builds cannot watch");
  let program = "";
  let css = "";
  const resources = new Map(ui);
  for (const output of Array.isArray(result) ? result : [result])
    for (const item of output.output) {
      if (item.type === "chunk") program += item.code;
      else if (item.fileName.endsWith(".css"))
        css += "\n" + new TextDecoder().decode(bytesOf(item.source));
      else
        resources.set(item.fileName, {
          key: item.fileName,
          bytes: bytesOf(item.source),
          mediaType: typeOf(item.fileName)[0],
        });
    }
  // Resources are immutable bytes from the build inventory, never runtime filesystem URLs.
  const inline = (text: string) =>
    text.replace(/\/assets\/([a-zA-Z0-9_./-]+)/g, (_, key: string) => {
      const resource = resources.get(key);
      if (!resource) throw new Error(`Capture references an asset no build emitted: ${key}`);
      return `data:${resource.mediaType};base64,${Buffer.from(resource.bytes).toString("base64")}`;
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
  // The font bytes are registered once for the batch, not repeated for each family.
  return {
    bytes:
      bytesOf(`"use strict";globalThis.__slopCaptureCSS=${JSON.stringify(sheet.toString())};\n${inline(program)}\n{
const render = globalThis.${functionName};
globalThis.${functionName} = json => JSON.stringify({frames:JSON.parse(render(json)),fonts:${JSON.stringify(fonts)}});
}`),
    dependencies,
    unavailable,
  };
}

/** Diagnose unsupported authored lifecycle calls before SSR can erase them.
 * Inspect source ASTs through the already resolved UI graph, before compiling SSR.
 * Strings and comments never choose the rendering engine.
 */
export async function browserCaptureReasons(
  entry: string,
  imports: Map<string, string[]>,
  sdk: string,
) {
  const pending = [entry],
    seen = new Set<string>(),
    reasons = new Set<string>();
  const names = /^(onMount|onPrepare|capture|motion|draft|notify)$/;
  while (pending.length) {
    const id = pending.pop()!;
    if (seen.has(id) || id.startsWith("\0") || id.startsWith(sdk) || id.includes("/node_modules/"))
      continue;
    seen.add(id);
    pending.push(...(imports.get(id) ?? []));
    if (!/\.(svelte|[cm]?[jt]s)$/.test(id)) continue;
    const code = await readFile(id, "utf8");
    const identifier = (name: string) => {
      if (names.test(name)) reasons.add(`${id}: ${name}`);
    };
    if (id.endsWith(".svelte")) {
      const visit = (node: unknown): void => {
        if (!node || typeof node !== "object") return;
        const value = node as { type?: string; name?: string };
        if (value.type === "Identifier" && value.name) identifier(value.name);
        if (value.type === "AwaitBlock" || value.type === "AwaitExpression")
          reasons.add(`${id}: await`);
        Object.values(node).forEach((value) =>
          Array.isArray(value) ? value.forEach(visit) : visit(value),
        );
      };
      visit(parse(code, { modern: true }));
    } else {
      const visit = (node: ts.Node) => {
        if (ts.isIdentifier(node)) identifier(node.text);
        if (node.kind === ts.SyntaxKind.AwaitExpression) reasons.add(`${id}: await`);
        ts.forEachChild(node, visit);
      };
      visit(ts.createSourceFile(id, code, ts.ScriptTarget.Latest, false));
      for (const reason of retainedCaptureReasons(code, id, [
        "window",
        "document",
        "navigator",
        "requestAnimationFrame",
        "fetch",
        "XMLHttpRequest",
        "Image",
        "Worker",
      ]))
        reasons.add(reason);
    }
  }
  return [...reasons];
}

/** Inspect retained code, not unused helpers in a shared source module. Binding the
 * program distinguishes browser globals from local names such as `document`. */
function retainedCaptureReasons(code: string, filename: string, names: string[]): string[] {
  const scriptName = `${filename}.js`;
  const file = ts.createSourceFile(
    scriptName,
    code,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.JS,
  );
  const host: ts.CompilerHost = {
    getSourceFile: (name) => (name === scriptName ? file : undefined),
    getDefaultLibFileName: () => "",
    writeFile() {},
    getCurrentDirectory: () => "",
    getDirectories: () => [],
    fileExists: (name) => name === scriptName,
    readFile: (name) => (name === scriptName ? code : undefined),
    getCanonicalFileName: (name) => name,
    useCaseSensitiveFileNames: () => true,
    getNewLine: () => "\n",
  };
  const checker = ts
    .createProgram([scriptName], { allowJs: true, noLib: true }, host)
    .getTypeChecker();
  const globals = new Set(names);
  const reasons = new Set<string>();
  const inspect = (node: ts.Node) => {
    if (ts.isIdentifier(node) && globals.has(node.text)) {
      const parent = node.parent;
      const property =
        (ts.isPropertyAccessExpression(parent) && parent.name === node) ||
        ((ts.isPropertyAssignment(parent) || ts.isMethodDeclaration(parent)) &&
          parent.name === node);
      const globalProperty =
        ts.isPropertyAccessExpression(parent) &&
        parent.name === node &&
        ts.isIdentifier(parent.expression) &&
        ["globalThis", "self", "window"].includes(parent.expression.text);
      if (globalProperty || (!property && !checker.getSymbolAtLocation(node)))
        reasons.add(
          `${filename}: retained dependency uses ${node.text}, unavailable in synchronous capture rendering`,
        );
    }
    if (
      names.includes("Intl") &&
      ts.isCallExpression(node) &&
      ts.isPropertyAccessExpression(node.expression) &&
      /^(toLocaleString|toLocaleDateString|toLocaleTimeString)$/.test(node.expression.name.text)
    )
      reasons.add(
        `${filename}: ${node.expression.name.text} requires locale services unavailable in QuickJS`,
      );
    ts.forEachChild(node, inspect);
  };
  inspect(file);
  return [...reasons];
}
