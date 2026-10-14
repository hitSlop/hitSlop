import MagicString from "magic-string";
import { parseSync, Visitor } from "vite";

/** Run after Vite resolves new URL(..., import.meta.url). A definition has no
 * browser origin: keep Vite's emitted URL instead of asking QuickJS for one. */
export function portableAssetURLs(source: string, fileName: string) {
  const code = new MagicString(source);
  new Visitor({
    MemberExpression(node) {
      const call = node.object;
      if (
        node.computed ||
        node.property.type !== "Identifier" ||
        node.property.name !== "href" ||
        call.type !== "NewExpression" ||
        call.callee.type !== "Identifier" ||
        call.callee.name !== "URL" ||
        call.arguments.length !== 2
      )
        return;
      const [url] = call.arguments;
      const path =
        url?.type === "Literal"
          ? url.value
          : url?.type === "TemplateLiteral" && !url.expressions.length
            ? url.quasis[0]?.value.cooked
            : undefined;
      if (typeof path === "string" && path.startsWith("/assets/"))
        code.overwrite(node.start, node.end, JSON.stringify(path));
    },
  }).visit(parseSync(fileName, source).program);
  return code.hasChanged()
    ? { code: code.toString(), map: code.generateMap({ hires: "boundary", source: fileName }) }
    : undefined;
}
