import ts from "typescript";

/** The explicit command form is part of the authoring API. Parse source syntax, never
 * regex-match bodies (strings, comments and nested braces must remain harmless). */
export function stripCommandBodies(source: string, fileName: string, stubModule: string, declarationEntry = false) {
  let changed = false;
  let needsStub = false;
  const result = ts.transpileModule(source, {
    fileName,
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext, sourceMap: true },
    transformers: { before: [context => root => {
      const f = context.factory;
      const stub = f.createUniqueName("hitslopCommandStub");
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
        if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === "command") {
          const spec = node.arguments[0];
          if (node.arguments.length !== 1 || !spec || !ts.isObjectLiteralExpression(spec))
            throw new Error(`${fileName}: command declarations must use doc.command({ description, args, run })`);
          if (spec.properties.some(p => ts.isSpreadAssignment(p) || (p.name && ts.isComputedPropertyName(p.name))))
            throw new Error(`${fileName}: command declarations cannot spread or compute their properties`);
          const name = (p: ts.ObjectLiteralElementLike) => p.name && (ts.isIdentifier(p.name) || ts.isStringLiteral(p.name)) ? p.name.text : undefined;
          const runs = spec.properties.filter(p => name(p) === "run");
          if (runs.length !== 1 || (!ts.isMethodDeclaration(runs[0]!) && !(ts.isPropertyAssignment(runs[0]!) && (ts.isArrowFunction(runs[0]!.initializer) || ts.isFunctionExpression(runs[0]!.initializer)))))
            throw new Error(`${fileName}: run must be an inline synchronous method or function`);
          const run = ts.isMethodDeclaration(runs[0]!) ? runs[0]! : (runs[0]! as ts.PropertyAssignment).initializer as ts.ArrowFunction | ts.FunctionExpression;
          if (run.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword) || ("asteriskToken" in run && run.asteriskToken))
            throw new Error(`${fileName}: run must be synchronous and cannot be a generator`);
          changed = true;
          needsStub = true;
          return f.createCallExpression(stub, undefined, [node.expression.expression,
            f.updateObjectLiteralExpression(spec, spec.properties.filter(p => name(p) !== "run"))]);
        }
        return ts.visitEachChild(node, visit, context);
      };
      const next = ts.visitEachChild(root, visit, context);
      return needsStub ? f.updateSourceFile(next, [f.createImportDeclaration(undefined,
        f.createImportClause(false, undefined, f.createNamedImports([f.createImportSpecifier(false, f.createIdentifier("commandStub"), stub)])),
        f.createStringLiteral(stubModule)), ...next.statements]) : next;
    }] },
  });
  return changed ? { code: result.outputText.replace(/\n\/\/# sourceMappingURL=.*\n?$/, "\n"), map: result.sourceMapText } : undefined;
}
