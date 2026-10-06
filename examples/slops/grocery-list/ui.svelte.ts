import type { Category } from "./shared";

// The aisle tab the person is looking at; the export shows the same one.
export const ui = $state({ filter: "All" as "All" | Category });
