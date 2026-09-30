
export type Point = { column: number; row: number };
export const ui = $state({
  resizing: null as { letter: string; width: number } | null,
  editing: null as { key: string; from: "cell" | "bar" } | null,
  cursor: { column: 1, row: 8 } as Point,
  anchor: { column: 1, row: 8 } as Point,
  draft: "",
  stampOpen: false,
  notice: "",
  bouncing: new Set() as ReadonlySet<string>,
  dragging: false,
});
