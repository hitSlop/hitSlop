import { parseSync, type ESTree } from "vite";

/** A `.svelte.ts`/`.svelte.js` module that declares `$state` at its top level: state the
 * view sets and a fresh capture page never sees. */
export function declaresModuleState(code: string, fileName: string) {
  if (!/\.svelte\.[cm]?[jt]s$/.test(fileName)) return false;
  const rune = (node: ESTree.Expression) =>
    node.type === "CallExpression" &&
    (node.callee.type === "Identifier"
      ? node.callee.name === "$state"
      : node.callee.type === "MemberExpression" &&
        !node.callee.computed &&
        node.callee.object.type === "Identifier" &&
        node.callee.object.name === "$state");
  return parseSync(fileName, code).program.body.some((statement) => {
    const declaration =
      statement.type === "ExportNamedDeclaration" ? statement.declaration : statement;
    return (
      declaration?.type === "VariableDeclaration" &&
      declaration.declarations.some(({ init }) => !!init && rune(init))
    );
  });
}
