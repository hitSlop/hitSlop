// The runtime's internal module, loaded by boot.js and headless.js. Its exports are
// not an app contract: built slops receive only the ctx defined in abi.ts.
export { boot, bootHeadless, createContext, initialize } from "./boot";
export { Document } from "./document";
export { Session } from "./session";
export { MemoryStore } from "./memory";
export { fromDescriptor } from "./schema";
export { default as runtimeIdentity } from "./runtime-identity.json";
// Begin fetching and compiling WASM as soon as the runtime evaluates.
// Callers observe failures through initialize().
import { initialize } from "./boot";
initialize().catch(() => {});
