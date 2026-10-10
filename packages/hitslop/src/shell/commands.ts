import type { OwnerIntent } from "../schema/core";
import type { ObjectNode } from "../sdk/schema";
import type { CommandContext } from "../sdk/commands";
import { handleFactory, type Collector } from "./owner/handles";
import { mapPaths, type Locations } from "./owner/path";
import { newID } from "./identity";
export type CommandInput = { value: unknown; descriptor: ObjectNode; now: number; seed: number[]; args: unknown };
/** xoshiro128**: a seeded generator, so one seed gives one command's ids and numbers. */
export function generator([a, b, c, d]: number[]) {
  let s0 = a! >>> 0, s1 = b! >>> 0, s2 = c! >>> 0, s3 = d! >>> 0;
  const next = () => {
    const result = Math.imul(rotl(Math.imul(s1, 5), 7), 9) >>> 0;
    const t = s1 << 9;
    s2 ^= s0; s3 ^= s1; s1 ^= s2; s0 ^= s3; s2 ^= t; s3 = rotl(s3, 11);
    return result;
  };
  return next;
}
const rotl = (x: number, k: number) => ((x << k) | (x >>> (32 - k))) >>> 0;
function freeze<T>(value: T): T {
  if (value && typeof value === "object") {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
}
const refuse = (what: string) => () => { throw new Error(`Commands collect edits with tx; ${what} is not available`); };
export function evaluate<R>(request: CommandInput, command: (ctx: CommandContext<any>, args: any) => R): { intents: OwnerIntent[]; result: R } {
    const next = generator(request.seed);
    const random = () => next() / 2 ** 32;
    const current = freeze(request.value) as object;
    const paths: Locations = new WeakMap();
    mapPaths(paths, current, request.descriptor, []);
    const intents: OwnerIntent[] = [];
    let active = true;
    const collect: Collector = (intent) => {
      if (!active) throw new Error("A transaction handle escaped its command");
      intents.push(JSON.parse(JSON.stringify(intent)) as OwnerIntent);
    };
    const fill = (bytes: Uint8Array) => {
      for (let i = 0; i < bytes.length; i += 4) {
        const word = next();
        for (let j = 0; j < 4 && i + j < bytes.length; j++) bytes[i + j] = (word >>> (8 * j)) & 255;
      }
      return bytes;
    };
    const make = handleFactory(
      {
        handle: (node, path, collector) => make(node, path, collector),
        submit: refuse("an awaited write"),
        write: refuse("an awaited write"),
        preview: refuse("preview"),
        read: refuse("value"),
        assign: refuse("value"),
      },
      () => newID(fill),
    );
    const tx = Object.freeze({
      fields: make(request.descriptor, [], collect),
      at: (value: object) => {
        const location = paths.get(value);
        if (!location) throw new Error("tx.at takes an object from ctx.current");
        return make(location.node, location.path, collect);
      },
    });
    let result: unknown;
    try {
      result = command(Object.freeze({ current, tx, now: request.now, random }) as CommandContext<any>, freeze(request.args));
    } finally {
      active = false;
    }
    if (result && (typeof result === "object" || typeof result === "function") && "then" in result && typeof result.then === "function") throw new Error("Commands are synchronous");
    // Only JSON results cross hosts. Detect unsupported values instead of silently losing them.
    const json = JSON.stringify(result === undefined ? null : result, (_key, value) => {
      if (typeof value === "bigint" || typeof value === "function" || typeof value === "symbol" || (typeof value === "number" && !Number.isFinite(value))) throw new Error("Commands return JSON values");
      return value;
    });
    return { intents, result: JSON.parse(json) as R };
}
