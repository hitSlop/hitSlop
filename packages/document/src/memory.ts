import type { ByteStore, Stored } from "./storage";
export class MemoryStore implements ByteStore {
  private stored: Stored;
  /** Peer replicas of one document share its ID; a new store is a new document. */
  constructor(docId: string = crypto.randomUUID()) {
    this.stored = { checkpoint: null, updates: [], generation: "0", schemaKey: null, docId };
  }
  async load() {
    return structuredClone(this.stored);
  }
  private check(generation: string) {
    if (generation !== this.stored.generation) throw new Error("revision_conflict");
  }
  async append(generation: string, updates: Uint8Array[]) {
    this.check(generation);
    this.stored.updates.push(...updates.map((b) => b.slice()));
    return (this.stored.generation = String(Number(generation) + 1));
  }
  async checkpoint(generation: string, bytes: Uint8Array, schemaKey: string) {
    this.check(generation);
    this.stored = {
      ...this.stored,
      checkpoint: bytes.slice(),
      updates: [],
      schemaKey,
      generation: String(Number(generation) + 1),
    };
    return this.stored.generation;
  }
  async close() {}
}
