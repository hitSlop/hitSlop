import type {
  Field,
  Scalar,
  Text,
  RichText,
  CounterNode,
  Input,
} from "../../packages/document/src/schema";
import type { Operation } from "./contract";

/** Candidate authoring surface: callbacks only collect intent. Host acceptance is async. */
export class EditBatch {
  private active = true;
  private operations: Operation[] = [];
  private add(operation: Operation) {
    if (!this.active) throw new Error("Edit batch has ended");
    this.operations.push(operation);
  }
  set<N extends Scalar>(field: Field<N>, value: Input<NoInfer<N>>) {
    this.add({ type: "set", path: [...field.path], value });
  }
  splice(field: Field<Text | RichText>, index: number, deleteCount: number, text: string) {
    this.add({ type: "splice", path: [...field.path], index, deleteCount, text });
  }
  increment(field: Field<CounterNode>, amount = 1) {
    this.add({ type: "increment", path: [...field.path], amount });
  }
  finish() {
    this.active = false;
    return this.operations;
  }
}
export class NativeDocumentClient {
  private queue: Promise<unknown> = Promise.resolve();
  constructor(
    private submit: (operations: Operation[]) => Promise<unknown>,
    private save: () => Promise<void>,
  ) {}
  edit(build: (batch: EditBatch) => void): Promise<void> {
    const batch = new EditBatch();
    try {
      const result: unknown = build(batch);
      if (result && typeof (result as Promise<unknown>).then === "function") {
        void Promise.resolve(result).catch(() => {});
        throw new Error("Build edits synchronously; await edit acceptance outside the callback");
      }
    } catch (error) {
      batch.finish();
      return Promise.reject(error);
    }
    const operations = batch.finish();
    const result = this.queue.then(async () => {
      if (operations.length) await this.submit(operations);
    });
    this.queue = result.catch(() => {});
    return result;
  }
  async flush() {
    await this.queue;
    await this.save();
  }
}
