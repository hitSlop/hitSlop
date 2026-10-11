/**
 * What the click wheel (or the keyboard) asks the player to do. These are page gestures, read by
 * controller.svelte.ts; document changes go through the edits in pod.ts, which commands.ts shares.
 */
export type Intent =
  | { type: "scroll"; delta: 1 | -1 }
  | { type: "select" }
  | { type: "hold-select" }
  | { type: "menu" }
  | { type: "playpause" }
  | { type: "next" }
  | { type: "prev" }
  | { type: "seek"; delta: 1 | -1 };
