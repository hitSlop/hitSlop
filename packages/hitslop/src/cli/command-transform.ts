import ts from "typescript";
import { createHash } from "node:crypto";

/** Private to the definition compiler; never part of a command's descriptor or ABI. */
export const commandProvenanceModule = "virtual:hitslop-command-provenance";
const provenanceGlobal = "__slopBuildCommands_6b93e4d2";
export const commandProvenanceSource = `globalThis.${provenanceGlobal} = new class {
  #sites = new WeakMap();
  tag(command, site) { this.#sites.set(command, site); return command; }
  describe(declaration, commands) {
    return {ok:true,declaration,commandSites:Object.entries(commands).map(([name, command]) => {
      const site = this.#sites.get(command);
      if (!site) throw new Error('commands.' + name + ' must use a direct document.command({ description, args, run }) declaration at module scope in a TypeScript module of this project (commands from packages or aliased paths outside it are not registered)');
      return site;
    })};
  }
}();`;
export const describeWithCommandProvenance = (declaration: string, commands: string) =>
  `globalThis.${provenanceGlobal}.describe(${declaration},${commands})`;
const siteId = (moduleId: string, position: number) => createHash("sha256").update(moduleId).update("\0").update(String(position)).digest("hex");

/** Erase our compiler instrumentation after evaluation. AST ranges preserve the
 * minified bundle; user code is never matched by a regex or description. */
export function stripCommandProvenance(source: string) {
  const file = ts.createSourceFile("commands.js", source, ts.ScriptTarget.ESNext, true, ts.ScriptKind.JS);
  const edits: { start: number; end: number; value: string }[] = [];
  const generated = (node: ts.Node): node is ts.PropertyAccessExpression => ts.isPropertyAccessExpression(node) &&
    ts.isIdentifier(node.expression) && node.expression.text === "globalThis" && node.name.text === provenanceGlobal;
  const replace = (node: ts.Node, value: string) => edits.push({ start: node.getStart(file), end: node.end, value });
  const visit = (node: ts.Node) => {
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken && generated(node.left)) {
      replace(node, "void 0");
      return;
    }
    if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && generated(node.expression.expression)) {
      const value = node.arguments[0];
      if (!value || node.arguments.length !== 2) throw new Error("Invalid command provenance in compiled program");
      if (node.expression.name.text === "tag") replace(node, `(${value.getText(file)})`);
      else if (node.expression.name.text === "describe") replace(node, `({ok:true,declaration:${value.getText(file)}})`);
      else throw new Error("Unknown command provenance operation in compiled program");
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  for (const edit of edits.sort((a, b) => b.start - a.start))
    source = source.slice(0, edit.start) + edit.value + source.slice(edit.end);
  if (source.includes(provenanceGlobal)) throw new Error("Command provenance escaped the definition build");
  return source;
}

/** The UI bundle needs a command's name and arguments, never its body: the owner runs the
 * stored program. Parse source syntax, never regex-match bodies (strings, comments and
 * nested braces must remain harmless). Document provenance is resolved before this
 * pass; unsupported declaration shapes fail without touching unrelated APIs. */
export function stripCommandBodies(source: string, fileName: string, declarationEntry = false, sites?: { module: string; found: (site: CommandSite) => void }, members: ReadonlySet<number> = new Set()) {
  return transformCommands(source, fileName, { declarationEntry, sites, members });
}

/** Tag the actual command values registered by the definition build, using the same
 * source positions as the UI scanner. Has no dependency on names or descriptions. */
export function tagCommandDeclarations(source: string, fileName: string, moduleId: string, members: ReadonlySet<number>) {
  return transformCommands(source, fileName, { moduleId, members });
}

function transformCommands(source: string, fileName: string, { declarationEntry = false, moduleId, sites, members }: {
  members: ReadonlySet<number>; declarationEntry?: boolean; moduleId?: string; sites?: { module: string; found: (site: CommandSite) => void };
}) {
  let changed = false;
  const result = ts.transpileModule(source, {
    fileName,
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext, sourceMap: true },
    transformers: { before: [context => root => {
      const f = context.factory;
      const tag = f.createPropertyAccessExpression(f.createPropertyAccessExpression(f.createIdentifier("globalThis"), provenanceGlobal), "tag");
      const declarations = new Set<string>();
      for (const statement of root.statements) {
        if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier) || statement.moduleSpecifier.text !== "hitslop") continue;
        const imports = statement.importClause?.namedBindings;
        if (imports && ts.isNamedImports(imports)) {
          for (const member of imports.elements)
            if ((member.propertyName ?? member.name).text === "defineSlop") declarations.add(member.name.text);
        }
      }
      if (declarationEntry && !root.statements.some(statement => ts.isExportAssignment(statement) && !statement.isExportEquals &&
        ts.isCallExpression(statement.expression) && ts.isIdentifier(statement.expression.expression) && declarations.has(statement.expression.expression.text)))
        throw new Error(`${fileName}: the entry must export default defineSlop({ ... }) imported from hitslop`);
      const visit: ts.Visitor = node => {
        // Metadata belongs to the definition build. Project only the UI roles;
        // otherwise passing the whole object keeps artwork-only URLs in ui.js.
        if (moduleId === undefined && ts.isCallExpression(node) && ts.isIdentifier(node.expression) && declarations.has(node.expression.text)) {
          const declaration = node.arguments[0];
          if (node.arguments.length !== 1 || !declaration || !ts.isObjectLiteralExpression(declaration) ||
            declaration.properties.some(p => ts.isSpreadAssignment(p) || (p.name && ts.isComputedPropertyName(p.name))))
            throw new Error(`${fileName}: defineSlop requires an object literal with explicit field names; field values may be computed`);
          changed = true;
          return ts.visitNode(f.updateObjectLiteralExpression(declaration, declaration.properties.filter(p =>
            p.name && (ts.isIdentifier(p.name) || ts.isStringLiteral(p.name)) &&
            ["document", "view", "export", "icon", "commands"].includes(p.name.text))), visit);
        }
        // A document command declaration: `<doc>.command({ description, args, run })`.
        // Provenance, not the spelling or shape of another library's API, identifies it.
        const spec = commandDeclaration(node, fileName, members);
        if (spec) {
          sites?.found(site(node, root, fileName, sites.module));
          const run = spec.properties.find(p => propertyName(p) === "run")!;
          const body = ts.isMethodDeclaration(run) ? run : ts.isPropertyAssignment(run) ? run.initializer : undefined;
          if (body && (ts.isMethodDeclaration(body) || ts.isArrowFunction(body) || ts.isFunctionExpression(body)) &&
            (body.modifiers?.some(m => m.kind === ts.SyntaxKind.AsyncKeyword) || ("asteriskToken" in body && body.asteriskToken)))
            throw new Error(`${fileName}: run must be synchronous and cannot be a generator`);
          changed = true;
          const call = node as ts.CallExpression;
          if (moduleId !== undefined)
            return f.createCallExpression(tag, undefined, [call, f.createStringLiteral(siteId(moduleId, node.getStart(root)))]);
          return f.updateCallExpression(call, ts.visitNode(call.expression, visit) as ts.Expression, call.typeArguments,
            [f.updateObjectLiteralExpression(spec, spec.properties.filter(p => p !== run))]);
        }
        return ts.visitEachChild(node, visit, context);
      };
      const transformed = ts.visitEachChild(root, visit, context);
      if (!changed || moduleId === undefined) return transformed;
      return f.updateSourceFile(transformed, [
        f.createImportDeclaration(undefined, undefined, f.createStringLiteral(commandProvenanceModule)),
        ...transformed.statements,
      ]);
    }] },
  });
  return changed ? { code: result.outputText.replace(/\n\/\/# sourceMappingURL=.*\n?$/, "\n"), map: result.sourceMapText } : undefined;
}

/** A source declaration, with a deterministic build-only identity shared by both builds. */
export type CommandSite = { file: string; id: string; name?: string };
/** Every `<doc>.command({ description, args, run })` in `source`. */
export function commandSites(source: string, fileName: string, moduleId = fileName, members: ReadonlySet<number> = new Set()): CommandSite[] {
  const file = ts.createSourceFile(fileName, source, ts.ScriptTarget.ESNext, true);
  const sites: CommandSite[] = [];
  const visit = (node: ts.Node) => {
    if (commandDeclaration(node, fileName, members)) sites.push(site(node, file, fileName, moduleId));
    ts.forEachChild(node, visit);
  };
  visit(file);
  return sites;
}
function site(node: ts.Node, file: ts.SourceFile, fileName: string, moduleId: string): CommandSite {
  const binding = node.parent;
  const exported = ts.isVariableDeclaration(binding) && ts.isIdentifier(binding.name) &&
    ts.isVariableStatement(binding.parent.parent) &&
    binding.parent.parent.modifiers?.some(m => m.kind === ts.SyntaxKind.ExportKeyword);
  return { file: fileName, id: siteId(moduleId, node.getStart(file)), name: exported ? (binding.name as ts.Identifier).text : undefined };
}
const commandKeys = new Set(["description", "args", "run"]);
const propertyName = (p: ts.ObjectLiteralElementLike) =>
  p.name && (ts.isIdentifier(p.name) || ts.isStringLiteral(p.name)) ? p.name.text : undefined;
/** The spec of `x.command({ description, args, run })`: one object literal whose keys are
 * exactly those three. A proven document command in any other form is an authoring error. */
function commandDeclaration(node: ts.Node, fileName: string, members: ReadonlySet<number>) {
  if (!ts.isCallExpression(node)) return;
  const callee = node.expression;
  if (!(ts.isPropertyAccessExpression(callee) && callee.name.text === "command") &&
      !(ts.isElementAccessExpression(callee) && ts.isStringLiteralLike(callee.argumentExpression) && callee.argumentExpression.text === "command")) return;
  if (!members.has(callee.getStart())) return;
  const spec = node.arguments[0];
  if (node.arguments.length !== 1 || !spec || !ts.isObjectLiteralExpression(spec)) {
    throw new Error(`${fileName}: pass a command's spec to ${callee.expression.getText()}.command as an object literal: { description, args, run }`);
  }
  const names = spec.properties.map(propertyName);
  if (names.length === commandKeys.size && names.every(name => name && commandKeys.has(name)) && new Set(names).size === names.length) {
    // One declaration site must produce one command value. A factory, loop or
    // class initializer could otherwise register one instance and hide another.
    for (let parent = node.parent; parent; parent = parent.parent)
      if (ts.isFunctionLike(parent) || ts.isIterationStatement(parent, false) || ts.isClassLike(parent))
        throw new Error(`${fileName}: declare commands at module scope, outside functions, loops and classes; import the callable where it is used`);
    return spec;
  }
  throw new Error(`${fileName}: declare a command as <document>.command({ description, args, run }) with exactly those fields written out, no spread`);
}
