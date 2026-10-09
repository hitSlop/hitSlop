import ts from "typescript";
import { readFile } from "node:fs/promises";
import { parse } from "svelte/compiler";

const sdk = "/__hitslop_command_sdk__.ts";
type Module = { code: string; imports: Map<string, string> };
type Resolve = (specifier: string, importer: string) => Promise<string | undefined>;

/** Preserve source offsets while exposing Svelte scripts and template calls to the same
 * symbol resolver. Components may use commands, but may never declare them. */
export function commandScript(code: string, id: string): string {
  if (!id.endsWith(".svelte")) return code;
  const component = parse(code, { filename: id, modern: true });
  const chars = Array.from({ length: code.length }, (_, i) => /\s/.test(code[i]!) ? code[i]! : " ");
  const copy = (start: number, end: number) => {
    for (let i = start; i < end; i++) chars[i] = code[i]!;
  };
  for (const script of [component.module, component.instance]) {
    if (script) {
      // Svelte adds source offsets to ESTree's Program; ESTree's type omits them.
      const content = script.content as typeof script.content & { start: number; end: number };
      copy(content.start, content.end);
    }
  }
  const visit = (value: unknown): void => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) { value.forEach(visit); return; }
    const node = value as Record<string, unknown>;
    if (node.type === "CallExpression" && typeof node.start === "number" && typeof node.end === "number") {
      copy(node.start, node.end);
      chars[node.end] = ";";
      return;
    }
    Object.values(node).forEach(visit);
  };
  visit(component.fragment);
  return chars.join("");
}

/** A small source graph, using Vite's resolver rather than guessing from import names.
 * TypeScript supplies lexical scopes and export/alias resolution; no SDK type checking
 * or dependency compilation is needed to establish a document's provenance. */
export class CommandBindings {
  private modules = new Map<string, Module>();
  constructor(private read: (file: string) => Promise<string> = file => readFile(file, "utf8")) {}
  private pending: Promise<unknown> = Promise.resolve();
  clear() { this.modules.clear(); }

  members(code: string, id: string, root: string, resolve: Resolve): Promise<ReadonlySet<number>> {
    const result = this.pending.then(() => this.analyze(code, id, root, resolve));
    this.pending = result.catch(() => { this.modules.clear(); });
    return result;
  }

  private async analyze(code: string, id: string, root: string, resolve: Resolve): Promise<ReadonlySet<number>> {
    const loaded = new Set<string>();
    const load = async (file: string, source?: string): Promise<void> => {
      if (loaded.has(file)) return;
      loaded.add(file);
      let module = this.modules.get(file);
      const script = source === undefined ? undefined : commandScript(source, file);
      if (!module || (script !== undefined && module.code !== script)) {
        const text = script ?? commandScript(await this.read(file), file);
        module = { code: text, imports: new Map() };
        this.modules.set(file, module);
        const ast = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
        for (const statement of ast.statements) {
          if (!(ts.isImportDeclaration(statement) || ts.isExportDeclaration(statement))) continue;
          const spec = statement.moduleSpecifier;
          if (!spec || !ts.isStringLiteral(spec)) continue;
          if (spec.text === "hitslop") { module.imports.set(spec.text, sdk); continue; }
          const found = (await resolve(spec.text, file))?.split("?", 1)[0];
          if (found?.startsWith(root + "/") && !found.includes("/node_modules/") && /\.(?:[cm]?[jt]sx?|svelte)$/.test(found))
            module.imports.set(spec.text, found);
        }
      }
      for (const dependency of module.imports.values()) if (dependency !== sdk) await load(dependency);
    };
    await load(id, code);
    const modules = new Map([...this.modules].filter(([file]) => loaded.has(file)));
    modules.set(sdk, { code: "export declare function defineDocument(value: unknown): unknown;", imports: new Map() });
    return documentMembers(id, modules);
  }
}

function documentMembers(id: string, modules: Map<string, Module>): ReadonlySet<number> {
  const options: ts.CompilerOptions = { allowNonTsExtensions: true, noLib: true, noResolve: false, allowJs: true, target: ts.ScriptTarget.Latest };
  const host = ts.createCompilerHost(options);
  host.fileExists = file => modules.has(file);
  host.readFile = file => modules.get(file)?.code;
  host.getSourceFile = (file, language) => {
    const module = modules.get(file);
    return module && ts.createSourceFile(file, module.code, language, true,
      file.endsWith(".svelte") ? ts.ScriptKind.TS : undefined);
  };
  host.resolveModuleNames = (names, importer) => names.map(name => {
    const file = modules.get(importer)?.imports.get(name);
    return file ? { resolvedFileName: file, extension: ts.Extension.Ts } : undefined;
  });
  const program = ts.createProgram([...modules.keys()], options, host);
  const checker = program.getTypeChecker();
  const file = program.getSourceFile(id)!;
  const unwrapped = (expression: ts.Expression): ts.Expression => {
    while (ts.isParenthesizedExpression(expression) || ts.isAsExpression(expression) ||
      ts.isTypeAssertionExpression(expression) || ts.isSatisfiesExpression(expression) || ts.isNonNullExpression(expression))
      expression = expression.expression;
    return expression;
  };
  const symbol = (node: ts.Node) => {
    const found = checker.getSymbolAtLocation(node);
    return found && (found.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(found) : found);
  };
  const factory = (expression: ts.Expression) => symbol(unwrapped(expression))?.declarations?.some(declaration =>
    declaration.getSourceFile().fileName === sdk && ts.isFunctionDeclaration(declaration) && declaration.name?.text === "defineDocument");
  const document = (expression: ts.Expression, seen = new Set<ts.Symbol>()): boolean => {
    expression = unwrapped(expression);
    if (ts.isCallExpression(expression) && factory(expression.expression)) return true;
    const binding = symbol(expression);
    if (!binding || seen.has(binding)) return false;
    seen.add(binding);
    return binding.declarations?.some(declaration => {
      if (ts.isExportAssignment(declaration)) return document(declaration.expression, seen);
      if (!ts.isVariableDeclaration(declaration) || !declaration.initializer) return false;
      if (!document(declaration.initializer, seen)) return false;
      if (!(declaration.parent.flags & ts.NodeFlags.Const))
        throw new Error(`${id}: document aliases must use const`);
      return true;
    }) ?? false;
  };
  const members = new Set<number>();
  const visit = (node: ts.Node) => {
    const member = ts.isPropertyAccessExpression(node) && node.name.text === "command" ||
      ts.isElementAccessExpression(node) && ts.isStringLiteralLike(node.argumentExpression) && node.argumentExpression.text === "command";
    if (member && document((node as ts.PropertyAccessExpression | ts.ElementAccessExpression).expression)) {
      if (!ts.isCallExpression(node.parent) || node.parent.expression !== node)
        throw new Error(`${id}: use a direct document.command({ description, args, run }) declaration; do not extract the command method`);
      members.add(node.getStart(file));
    }
    if (ts.isVariableDeclaration(node) && ts.isObjectBindingPattern(node.name) && node.initializer && document(node.initializer) &&
      node.name.elements.some(element => (element.propertyName ?? element.name).getText(file) === "command"))
      throw new Error(`${id}: use a direct document.command declaration; do not destructure the command method`);
    ts.forEachChild(node, visit);
  };
  visit(file);
  return members;
}
