import type { OwnerIntent, OwnerPath as Path } from "@hitslop/schema/core";
import type { Node } from "@hitslop/document";
import { isScalar } from "@hitslop/document/internal";
import { newID } from "../identity";
export type Collector = (intent: OwnerIntent) => void;
interface HandleHost {
  handle(node: Node, path: Path, collect: Collector | undefined): any;
  submit<R>(intents: OwnerIntent[], result: R): Promise<R>;
  /** A scalar write, shown at once and reverted if refused. */
  write(intent: OwnerIntent): Promise<void>;
  preview(path: Path, value: unknown): void;
  /** The shown value at `path`, recording a framework dependency. */
  read(path: Path): unknown;
  /** Shows `value` now and commits it once it settles; `undefined` clears an optional. */
  assign(path: Path, value: unknown, optional: boolean): void;
}
/** Typed authoring handles; execution and ordering stay with the owner document. */
export function handleFactory(host: HandleHost) {
  const make = (node: Node, path: Path, collect: Collector | undefined): any => {
    const send = <R = void>(intent: OwnerIntent, result: R = undefined as R) => {
      if (collect) {
        collect(intent);
        return result;
      }
      return host.submit([intent], result);
    };
    const shown = (intent: OwnerIntent) => (collect ? send(intent) : host.write(intent));
    const set = (value: unknown) => shown({ type: "set", path, value });
    const scalar = () => ({
      set,
      ...(!collect ? { preview: (value: unknown) => host.preview(path, value) } : {}),
    });
    // Live scalar handles expose `value` for Svelte `bind:`; transaction handles do not.
    const bindable = <T extends object>(handle: T, optional: boolean) =>
      collect
        ? handle
        : Object.defineProperty(handle, "value", {
            enumerable: true,
            get: () => host.read(path),
            set: (value: unknown) => host.assign(path, value, optional),
          });
    if (node.kind === "optional") {
      const clear = () => shown({ type: "clear", path });
      if (isScalar(node.inner)) return Object.freeze(bindable({ ...scalar(), clear }, true));
      if (node.inner.kind === "text")
        return Object.freeze({ set: (value: string) => send({ type: "set", path, value }), clear });
      // An optional object: its fields, plus `set` to create or replace it and `clear`.
      const fields = make(node.inner, path, collect);
      const handle = Object.create(null);
      for (const name of Object.keys(fields))
        Object.defineProperty(handle, name, { enumerable: true, get: () => fields[name] });
      Object.defineProperty(handle, "set", {
        value: (value: unknown) => send({ type: "set", path, value }),
      });
      Object.defineProperty(handle, "clear", { value: clear });
      return Object.freeze(handle);
    }
    if (isScalar(node)) return Object.freeze(bindable(scalar(), false));
    switch (node.kind) {
      case "object": {
        // Children are built on first access and kept by this handle.
        const children: Record<string, any> = {};
        for (const key of Object.keys(node.properties)) {
          let child: any;
          Object.defineProperty(children, key, {
            enumerable: true,
            get: () => (child ??= host.handle(node.properties[key]!, [...path, key], collect)),
          });
        }
        return Object.freeze(children);
      }
      case "counter": {
        const increment = (by = 1) => send({ type: "increment", path, by });
        return Object.freeze({ increment });
      }
      case "record": {
        const entry = (key: string) => [...path, key];
        return Object.freeze({
          entry: (key: string) => host.handle(node.value, entry(key), collect),
          put: (key: string, value: unknown) => send({ type: "set", path: entry(key), value }),
          delete: (key: string) => send({ type: "clear", path: entry(key) }),
        });
      }
      case "list": {
        if (node.item.kind !== "object") {
          const at = (index: number) => [...path, { index }];
          return Object.freeze({
            insert: (value: unknown, index?: number) =>
              send({ type: "insert", path, value, ...(index === undefined ? {} : { index }) }),
            set: (index: number, value: unknown) => shown({ type: "set", path: at(index), value }),
            ...(!collect
              ? { preview: (index: number, value: unknown) => host.preview(at(index), value) }
              : {}),
            remove: (index: number, count = 1) => send({ type: "remove", path, index, count }),
            // Rewrites the whole list; the owner keeps unchanged positions.
            replace: (values: unknown[]) => send({ type: "set", path, value: values }),
          });
        }
        const item = node.item;
        return Object.freeze({
          item: (id: string) => host.handle(item, [...path, { id }], collect),
          insert: (value: unknown, at?: { before: string } | { after: string }) => {
            const id = newID();
            return send(
              { type: "insert", path, value, id, ...(at ? { at } : {}) },
              Object.freeze({ id }),
            );
          },
          remove: (id: string) => send({ type: "remove", path, id }),
          move: (id: string, at?: { before: string } | { after: string }) =>
            send({ type: "move", path, id, ...(at ? { at } : {}) }),
        });
      }
      case "text":
        // Whole-field replacement of the text as it is when the owner runs it.
        return Object.freeze({ set: (value: string) => send({ type: "set", path, value }) });
      default:
        throw new Error(`Unsupported descriptor: ${(node as Node).kind}`);
    }
  };
  return make;
}
