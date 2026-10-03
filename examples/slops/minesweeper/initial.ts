import type { Input } from "@hitslop/document";
import schema from "./schema";
import { hiddenBoard } from "./game";

export default {
  level: "easy",
  status: "ready",
  cells: hiddenBoard("easy"),
  mines: "",
  seconds: 0,
  best: { easy: 0, medium: 0, hard: 0 },
  wins: 0,
  losses: 0,
} satisfies Input<typeof schema.descriptor>;
