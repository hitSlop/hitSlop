import ts from "typescript";

/** Run after Vite resolves new URL(..., import.meta.url). A definition has no
 * browser origin: keep Vite's emitted URL instead of asking QuickJS for one. */
export function portableAssetURLs(source: string, fileName: string) {
  let changed = false;
  const result = ts.transpileModule(source, {
    fileName,
    compilerOptions: {target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,sourceMap:true},
    transformers: {before:[context => root => {
      const visit: ts.Visitor = node => {
        if (ts.isPropertyAccessExpression(node) && node.name.text === "href" && ts.isNewExpression(node.expression)) {
          const call = node.expression;
          const [url] = call.arguments ?? [];
          if (ts.isIdentifier(call.expression) && call.expression.text === "URL" && call.arguments?.length === 2 &&
            url && (ts.isStringLiteral(url) || ts.isNoSubstitutionTemplateLiteral(url)) && url.text.startsWith("/assets/")) {
            changed = true;
            return context.factory.createStringLiteral(url.text);
          }
        }
        return ts.visitEachChild(node, visit, context);
      };
      return ts.visitEachChild(root, visit, context);
    }]},
  });
  return changed ? {code:result.outputText.replace(/\n\/\/# sourceMappingURL=.*\n?$/, "\n"),map:result.sourceMapText} : undefined;
}
