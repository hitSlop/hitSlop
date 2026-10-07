import ts from "typescript";

/** A `.svelte.ts`/`.svelte.js` module that declares `$state` at its top level: state the
 * view sets and a fresh capture page never sees. */
export function declaresModuleState(code: string, fileName: string) {
  if (!/\.svelte\.[cm]?[jt]s$/.test(fileName)) return false;
  const root = ts.createSourceFile(fileName, code, ts.ScriptTarget.ESNext, false);
  const rune = (node: ts.Expression): boolean => {
    if (!ts.isCallExpression(node)) return false;
    const callee = node.expression;
    return ts.isIdentifier(callee) ? callee.text === "$state"
      : ts.isPropertyAccessExpression(callee) && ts.isIdentifier(callee.expression) && callee.expression.text === "$state";
  };
  return root.statements.some(statement => ts.isVariableStatement(statement) &&
    statement.declarationList.declarations.some(declaration => declaration.initializer && rune(declaration.initializer)));
}

/** The first stateful module a capture component imports, directly or through others. */
export function sharedStateIn(component: string, imports: ReadonlyMap<string, readonly string[]>, stateful: ReadonlySet<string>) {
  const seen = new Set<string>();
  const pending = [component];
  while (pending.length) {
    const id = pending.pop()!;
    if (seen.has(id)) continue;
    seen.add(id);
    if (stateful.has(id)) return id;
    pending.push(...imports.get(id) ?? []);
  }
}
