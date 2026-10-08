import ts from "typescript";

/** The UI bundle needs a command's name and arguments, never its body: the owner runs the
 * stored program. Parse source syntax, never regex-match bodies (strings, comments and
 * nested braces must remain harmless). A declaration with `args` and `run` in any other
 * shape is an error, so no body reaches the page unnoticed. */
export function stripCommandBodies(source: string, fileName: string, declarationEntry = false) {
  let changed = false;
  const result = ts.transpileModule(source, {
    fileName,
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext, sourceMap: true },
    transformers: { before: [context => root => {
      const f = context.factory;
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
        if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && declarations.has(node.expression.text)) {
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
        // Only that exact shape loses its body; a `.command(...)` without `args` and `run` is not ours.
        const spec = commandDeclaration(node, fileName);
        if (spec) {
          const run = spec.properties.find(p => propertyName(p) === "run")!;
          const body = ts.isMethodDeclaration(run) ? run : ts.isPropertyAssignment(run) ? run.initializer : undefined;
          if (body && (ts.isMethodDeclaration(body) || ts.isArrowFunction(body) || ts.isFunctionExpression(body)) &&
            (body.modifiers?.some(m => m.kind === ts.SyntaxKind.AsyncKeyword) || ("asteriskToken" in body && body.asteriskToken)))
            throw new Error(`${fileName}: run must be synchronous and cannot be a generator`);
          changed = true;
          const call = node as ts.CallExpression;
          return f.updateCallExpression(call, ts.visitNode(call.expression, visit) as ts.Expression, call.typeArguments,
            [f.updateObjectLiteralExpression(spec, spec.properties.filter(p => p !== run))]);
        }
        return ts.visitEachChild(node, visit, context);
      };
      return ts.visitEachChild(root, visit, context);
    }] },
  });
  return changed ? { code: result.outputText.replace(/\n\/\/# sourceMappingURL=.*\n?$/, "\n"), map: result.sourceMapText } : undefined;
}

const commandKeys = new Set(["description", "args", "run"]);
const propertyName = (p: ts.ObjectLiteralElementLike) =>
  p.name && (ts.isIdentifier(p.name) || ts.isStringLiteral(p.name)) ? p.name.text : undefined;
/** The spec of `x.command({ description, args, run })`: one object literal whose keys are
 * exactly those three. */
function commandDeclaration(node: ts.Node, fileName: string) {
  if (!ts.isCallExpression(node) || !ts.isPropertyAccessExpression(node.expression) ||
    node.expression.name.text !== "command" || node.arguments.length !== 1) return;
  const spec = node.arguments[0]!;
  if (!ts.isObjectLiteralExpression(spec)) return;
  const names = spec.properties.map(propertyName);
  if (names.length === commandKeys.size && names.every(name => name && commandKeys.has(name)) && new Set(names).size === names.length) return spec;
  if (names.includes("args") && names.includes("run"))
    throw new Error(`${fileName}: declare a command as <document>.command({ description, args, run }) with exactly those fields written out, no spread`);
}
