import { OperationRejectedError } from "./errors";
import {
  canonicalJSON,
  checkRecordKey,
  unwrap,
  validate,
  validateMark,
  type Node,
  type ObjectNode,
  type Path,
  type RichTextValue,
} from "./schema";
import type { Commands } from "./operations";
import { diff } from "./bind-text";
import { isID } from "./identity";

const pointer = (base: string, key: string | number) =>
  base + "/" + String(key).replaceAll("~", "~0").replaceAll("/", "~1");
const object = (value: any): value is Record<string, any> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const equal = (a: unknown, b: unknown) => canonicalJSON(a) === canonicalJSON(b);
const fail = (at: string, message: string): never => {
  throw new OperationRejectedError(`${at || "/"}: ${message}`);
};
function checked<T>(at: string, work: () => T): T {
  try {
    return work();
  } catch (error) {
    if (error instanceof OperationRejectedError && error.message.startsWith(`${at || "/"}: `))
      throw error;
    return fail(at, error instanceof Error ? error.message : String(error));
  }
}
function unicode(value: string, at: string) {
  for (const point of value) {
    const code = point.charCodeAt(0);
    if (point.length === 1 && code >= 0xd800 && code <= 0xdfff)
      fail(at, "Unpaired Unicode surrogate");
  }
}

/** A one-shot, schema-directed edit. Never a persisted JSON mirror or a second engine. */
export function importJSON(
  root: ObjectNode,
  current: any,
  input: unknown,
  tx: Commands,
  fresh = false,
): void {
  const suppliedIDs = new Set<string>();
  const targets = new Set<string>();
  const references: { at: string; target: string }[] = [];
  const existingIDs = new Map<string, string>();
  const resolved = new Map<string, string>();
  const deferred: (() => void)[] = [];

  function replaceText(path: Path, before: string, after: string) {
    if (before === after) return;
    // Share the binding's linear, Unicode-safe splice. LoroText.update's general
    // diff can take minutes for large unrelated imports.
    tx.execute({ type: "text.splice", path, ...diff(before, after) });
  }

  function rich(node: Extract<Node, { kind: "richtext" }>, value: any, at: string): RichTextValue {
    if (typeof value === "string") value = { text: value, delta: value ? [{ insert: value }] : [] };
    if (
      !object(value) ||
      typeof value.text !== "string" ||
      !Array.isArray(value.delta) ||
      Object.keys(value).some((k) => k !== "text" && k !== "delta")
    )
      return fail(at, "Expected rich text string or {text, delta}");
    unicode(value.text, at);
    const delta: { insert: string; attributes?: Record<string, any> }[] = [];
    for (const [index, span] of value.delta.entries()) {
      const location = pointer(pointer(at, "delta"), index);
      if (
        !object(span) ||
        typeof span.insert !== "string" ||
        Object.keys(span).some((k) => k !== "insert" && k !== "attributes") ||
        (span.attributes !== undefined && !object(span.attributes))
      )
        return fail(location, "Expected a text insert and optional mark attributes");
      const attributes: Record<string, any> = {};
      for (const [key, mark] of Object.entries(span.attributes ?? {})) {
        checked(location, () => validateMark(node, key, mark));
        if (typeof mark === "string") unicode(mark, location);
        if (mark !== null) attributes[key] = mark;
      }
      if (!span.insert) continue;
      const previous = delta.at(-1);
      if (previous && equal(previous.attributes ?? {}, attributes)) previous.insert += span.insert;
      else
        delta.push({
          insert: span.insert,
          ...(Object.keys(attributes).length ? { attributes } : {}),
        });
    }
    if (delta.map((span) => span.insert).join("") !== value.text)
      return fail(at, "Rich text and delta disagree");
    let offset = 0;
    for (const span of delta) {
      offset += span.insert.length;
      if (
        offset < value.text.length &&
        /[\uD800-\uDBFF]/.test(value.text[offset - 1]) &&
        /[\uDC00-\uDFFF]/.test(value.text[offset])
      )
        return fail(at, "Rich text marks must fall on whole code-point boundaries");
    }
    return { text: value.text, delta };
  }

  function normalize(
    node: Node,
    value: any,
    at: string,
    depth = 0,
    row = false,
    tree = false,
  ): any {
    if (depth > 128) return fail(at, "Import exceeds maximum nesting depth (128)");
    if (typeof value === "string") unicode(value, at);
    if (node.kind === "optional")
      return value === undefined ? undefined : normalize(node.inner, value, at, depth + 1);
    if (node.kind === "string" && object(value)) {
      if (Object.keys(value).length !== 1 || typeof value.$ref !== "string")
        return fail(at, "Expected string or {$ref: JSON Pointer}");
      references.push({ at, target: value.$ref });
      return { $ref: value.$ref };
    }
    if (node.kind === "richtext") return rich(node, value, at);
    if (node.kind === "object") {
      if (!object(value)) return fail(at, "Expected object");
      for (const key of Object.keys(value))
        if (
          !Object.hasOwn(node.properties, key) &&
          !(row && key === "$id") &&
          !(tree && key === "children")
        )
          fail(pointer(at, key), "Unknown field");
      const result: Record<string, any> = {};
      if (row) {
        targets.add(at);
        if (value.$id !== undefined) {
          if (!isID(value.$id) || suppliedIDs.has(value.$id))
            fail(pointer(at, "$id"), "Expected a unique row ID of 1-64 letters, digits, - or _");
          suppliedIDs.add(value.$id);
          result.$id = value.$id;
        }
      }
      for (const [key, child] of Object.entries(node.properties)) {
        const next = normalize(child, value[key], pointer(at, key), depth + 1);
        if (next !== undefined) result[key] = next;
      }
      return result;
    }
    if (node.kind === "record") {
      if (!object(value)) return fail(at, "Expected record");
      return Object.fromEntries(
        Object.entries(value).map(([key, entry]) => {
          unicode(key, pointer(at, key));
          checked(pointer(at, key), () => checkRecordKey(key));
          return [key, normalize(node.value, entry, pointer(at, key), depth + 1)];
        }),
      );
    }
    if (node.kind === "list" || node.kind === "tree") {
      if (!Array.isArray(value)) return fail(at, "Expected list");
      return value.map((entry, index) => {
        const location = pointer(at, index);
        const next = normalize(
          node.item,
          entry,
          location,
          depth + 1,
          node.item.kind === "object",
          node.kind === "tree",
        );
        if (node.kind === "tree")
          next.children = normalize(
            node,
            entry.children ?? [],
            pointer(location, "children"),
            depth + 1,
          );
        return next;
      });
    }
    checked(at, () => validate(node, value));
    return value;
  }

  function indexExisting(node: Node, value: any, path: Path): void {
    if (value === undefined) return;
    node = unwrap(node);
    if (node.kind === "object")
      for (const [key, child] of Object.entries(node.properties))
        indexExisting(child, value[key], [...path, key]);
    else if (node.kind === "record")
      for (const [key, entry] of Object.entries(value))
        indexExisting(node.value, entry, [...path, { key }]);
    else if ((node.kind === "list" && node.item.kind === "object") || node.kind === "tree") {
      const rows = (entries: any[]) => {
        for (const row of entries) {
          existingIDs.set(row.$id, JSON.stringify(path));
          indexExisting(node.item, row, [...path, { id: row.$id }]);
          if (node.kind === "tree") rows(row.children);
        }
      };
      rows(value);
    }
  }
  const data = normalize(root, input, "");
  for (const ref of references)
    if (!targets.has(ref.target))
      fail(ref.at, "Reference must point to an imported row or tree node");
  indexExisting(root, current, []);

  // New containers start with valid scalar fields and empty identity-bearing collections.
  // This permits forward/cyclic links without exposing temporary values to the live document.
  function seed(node: Node, value: any, snapshot = false): any {
    if (value === undefined) return undefined;
    node = unwrap(node);
    if (node.kind === "list" || node.kind === "tree") return [];
    if (node.kind === "record") return {};
    if (node.kind === "richtext") return snapshot ? { text: "", delta: [] } : "";
    if (node.kind === "string" && object(value)) return "";
    if (node.kind === "object")
      return Object.fromEntries(
        Object.entries(node.properties).flatMap(([key, child]) =>
          value[key] === undefined ? [] : [[key, seed(child, value[key], snapshot)]],
        ),
      );
    return value;
  }

  function walk(node: Node, before: any, value: any, path: Path, at: string): void {
    if (value === undefined) {
      if (before !== undefined) tx.execute({ type: "clear", path });
      return;
    }
    node = unwrap(node);
    if (before === undefined) {
      checked(at, () => tx.execute({ type: "assign", path, value: seed(node, value) }));
      before = seed(node, value, true);
    }
    if (node.kind === "string" && object(value)) {
      const target = value.$ref;
      deferred.push(() => {
        const id = resolved.get(target)!;
        checked(at, () => validate(node, id));
        if (before !== id) checked(at, () => tx.execute({ type: "set", path, value: id }));
      });
    } else if (node.kind === "object") {
      for (const [key, child] of Object.entries(node.properties))
        walk(child, before[key], value[key], [...path, key], pointer(at, key));
    } else if (node.kind === "record") {
      for (const key of Object.keys(before))
        if (!Object.hasOwn(value, key)) tx.execute({ type: "clear", path: [...path, { key }] });
      for (const [key, entry] of Object.entries(value))
        walk(node.value, before[key], entry, [...path, { key }], pointer(at, key));
    } else if ((node.kind === "list" && node.item.kind === "object") || node.kind === "tree") {
      collection(
        node as Extract<Node, { kind: "list" | "tree" }> & { item: ObjectNode },
        before,
        value,
        path,
        at,
      );
    } else if (node.kind === "list") {
      // Reference values in scalar lists are resolved after every row has its ID.
      deferred.push(() => {
        const next = value.map((v: any, index: number) => {
          const result = object(v) ? resolved.get(v.$ref)! : v;
          checked(pointer(at, index), () => validate(node.item, result));
          return result;
        });
        if (!equal(before, next)) tx.execute({ type: "assign", path, value: next });
      });
    } else if (node.kind === "richtext") {
      const previous = rich(node, before, at);
      if (equal(previous, value)) return;
      checked(at, () => {
        replaceText(path, before.text, value.text);
        if (value.text.length)
          for (const key of Object.keys(node.marks))
            tx.execute({ type: "text.unmark", path, key, start: 0, end: value.text.length });
        let start = 0;
        for (const span of value.delta) {
          const end = start + span.insert.length;
          for (const [key, mark] of Object.entries(span.attributes ?? {}))
            tx.execute({ type: "text.mark", path, key, value: mark as any, start, end });
          start = end;
        }
      });
    } else if (before !== value) {
      checked(at, () => {
        if (node.kind === "counter") {
          // Loro counters merge increments. Reject an unrepresentable absolute target
          // instead of silently rounding it (even two increments can coalesce).
          const delta = value - before;
          if (!Number.isFinite(delta) || before + delta !== value)
            fail(at, "Counter target exceeds available numeric precision");
          tx.execute({ type: "increment", path, value: delta });
        } else if (node.kind === "text") replaceText(path, before, value);
        else tx.execute({ type: "set", path, value });
      });
    }
  }

  function collection(
    node: { kind: "list" | "tree"; item: ObjectNode },
    before: any[],
    value: any[],
    path: Path,
    at: string,
  ): void {
    const old = new Map<string, any>();
    const parents = new Map<string, string | null>();
    const siblings = new Map<string | null, string[]>();
    function index(rows: any[], parent: string | null) {
      siblings.set(
        parent,
        rows.map((row) => row.$id),
      );
      for (const row of rows) {
        old.set(row.$id, row);
        parents.set(row.$id, parent);
        if (node.kind === "tree") index(row.children, row.$id);
      }
    }
    index(before, null);
    const desired: { row: any; id: string; parent: string | null; at: string; before: any }[] = [];
    function allocate(rows: any[], parent: string | null, location: string) {
      for (const [i, row] of rows.entries()) {
        const rowAt = pointer(location, i);
        const existingPath = fresh ? undefined : existingIDs.get(row.$id);
        if (existingPath !== undefined && existingPath !== JSON.stringify(path))
          fail(pointer(rowAt, "$id"), "Cannot reuse an ID from another collection");
        const previous = !fresh && old.get(row.$id);
        // Supplied IDs are kept (a round trip into a new document preserves references);
        // fresh imports and rows without one receive new IDs.
        const keep = !fresh && row.$id !== undefined ? { id: row.$id } : {};
        const id = previous
          ? row.$id
          : checked(rowAt, () => tx.execute({ type: "insert", path, value: seed(node.item, row), ...keep }))!
              .id;
        resolved.set(rowAt, id);
        if (!previous) {
          parents.set(id, null);
          siblings.get(null)!.push(id);
          if (node.kind === "tree") siblings.set(id, []);
        }
        desired.push({
          row,
          id,
          parent,
          at: rowAt,
          before: previous || seed(node.item, row, true),
        });
        if (node.kind === "tree") allocate(row.children, id, pointer(rowAt, "children"));
      }
    }
    allocate(value, null, at);
    const retained = new Set(desired.map((row) => row.id));
    function moveParent(id: string, parent: string | null) {
      const previous = parents.get(id)!;
      tx.execute({ type: "move", path, id, destination: { parent } });
      const list = siblings.get(previous)!;
      list.splice(list.indexOf(id), 1);
      siblings.get(parent)!.push(id);
      parents.set(id, parent);
    }
    if (node.kind === "tree") {
      // Detach changing edges first: permits reversing ancestry and rescuing descendants.
      for (const row of desired)
        if (parents.get(row.id) !== row.parent && parents.get(row.id) !== null)
          moveParent(row.id, null);
      for (const row of desired)
        if (parents.get(row.id) !== row.parent) moveParent(row.id, row.parent);
    }
    for (const [id] of old)
      if (!retained.has(id)) {
        const parent = parents.get(id)!;
        if (parent === null || retained.has(parent)) tx.execute({ type: "remove", path, id });
        const list = siblings.get(parent)!;
        list.splice(list.indexOf(id), 1);
      }
    const positions = new Map<string | null, number>();
    for (const row of desired) {
      const position = positions.get(row.parent) ?? 0,
        list = siblings.get(row.parent)!;
      if (list[position] !== row.id) {
        tx.execute({ type: "move", path, id: row.id, destination: { before: list[position]! } });
        list.splice(list.indexOf(row.id), 1);
        list.splice(position, 0, row.id);
      }
      positions.set(row.parent, position + 1);
      walk(node.item, row.before, row.row, [...path, { id: row.id }], row.at);
    }
  }
  walk(root, current, data, [], "");
  for (const resolve of deferred) resolve();
}
