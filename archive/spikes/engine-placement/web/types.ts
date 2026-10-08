import type { Operation } from "../contract";
export interface FixtureRow {
  readonly $id: string;
  readonly text: string;
  readonly done: boolean;
}
export interface FixtureView {
  readonly title: string;
  readonly rows: readonly FixtureRow[];
  readonly body?: {
    readonly text: string;
    readonly delta: readonly {
      readonly insert: string;
      readonly attributes?: Readonly<Record<string, string | number | boolean | null>>;
    }[];
  };
  readonly taps?: number | null;
}
export interface Adapter {
  current(): FixtureView;
  submit(operations: Operation[]): Promise<{ engineMS: number }>;
  flush(): Promise<void>;
  close(): Promise<void>;
}
export interface ViewController {
  submit(operations: Operation[]): Promise<{ engineMS: number }>;
  state(): FixtureView;
  setDraft(value: string): void;
  drain(): Promise<void>;
  pending(): number;
  type(text: string): void;
}
