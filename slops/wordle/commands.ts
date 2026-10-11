import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { modes } from "./schema";
import { newPractice, playGuess, setMode } from "./wordle";
import { dayOf } from "./words";

export default defineCommands(doc, {
  guess: {
    description: "Play a five-letter guess in the current mode and return the game status afterwards: playing, won or lost.",
    args: { word: s.string({ description: "A five-letter word from the word list." }) },
    run: ({ tx, current, now }, { word }) => playGuess(tx, current, dayOf(now), word),
  },
  newPractice: {
    description: "Start a new practice puzzle with a random word.",
    args: {},
    run: ({ tx, current, random }) => newPractice(tx, current, random),
  },
  setMode: {
    description: "Switch between the daily puzzle and practice mode.",
    args: { mode: s.enum(modes) },
    run: ({ tx }, { mode }) => setMode(tx, mode),
  },
});
