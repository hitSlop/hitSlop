import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { levels } from "./schema";
import { dig, newGame, toggleFlag } from "./game";

export default defineCommands(schema, {
  newGame: {
    description: "Start a fresh hidden board at the given size, keeping wins, losses and best times.",
    args: { level: s.enum(levels, { description: "Board size: easy 6x6, medium 8x8, hard 10x10." }) },
    run: ({ tx }, { level }) => newGame(tx, level),
  },
  dig: {
    description: "Clear one cell by its row-major index (row * columns + column); the first dig lays the mines. Returns the game status afterwards.",
    args: { index: s.integer({ min: 0, description: "Cell index, counted row by row from 0." }) },
    run: ({ tx, current, random }, { index }) => dig(tx, current, index, random, current.seconds),
  },
  toggleFlag: {
    description: "Place or remove a buoy on a hidden cell by its row-major index. Returns placed or removed.",
    args: { index: s.integer({ min: 0, description: "Cell index, counted row by row from 0." }) },
    run: ({ tx, current }, { index }) => toggleFlag(tx, current, index),
  },
});
