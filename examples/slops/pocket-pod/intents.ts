/** What the click wheel (or the keyboard) asks the player to do. */
export type Intent =
  | { type: "scroll"; delta: 1 | -1 }
  | { type: "select" }
  | { type: "hold-select" }
  | { type: "menu" }
  | { type: "playpause" }
  | { type: "next" }
  | { type: "prev" }
  | { type: "seek"; delta: 1 | -1 };
