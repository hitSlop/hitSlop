import type { SlopApp } from "../sdk/abi";

/** The generated app must match the file's descriptor: authored schema modules can
 * evaluate differently in the build process and the browser. */
export function checkedApp(app: unknown, descriptor: unknown): SlopApp {
  const view = app as Partial<SlopApp> | undefined;
  if (!view || typeof view.mount !== "function") throw new Error("assets/app.js must export default { mount(ctx, target) }");
  if (!view.descriptor || !sameJSON(view.descriptor, descriptor))
    throw new Error("This app was built for a different document: its schema does not match the file's");
  return view as SlopApp;
}

/** Whether two values are the same JSON: key order and undefined members never matter. */
function sameJSON(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (Array.isArray(a) || Array.isArray(b))
    return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((value, i) => sameJSON(value, b[i]));
  if (!a || !b || typeof a !== "object" || typeof b !== "object") return false;
  const keys = (value: object) => Object.keys(value).filter((key) => (value as Record<string, unknown>)[key] !== undefined);
  const left = keys(a);
  return left.length === keys(b).length && left.every((key) => sameJSON((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key]));
}
