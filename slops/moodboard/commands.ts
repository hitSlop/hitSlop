import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { removeTile, setCaption, setTitle } from "./tiles";

// Adding an image needs its bytes: agents insert a tile with `slop batch --attach` instead.
export default defineCommands(doc, {
  setTitle: {
    description: "Rename the board; an empty title shows as Moodboard.",
    args: { title: s.string({ description: "The board's new title." }) },
    run: ({ tx }, { title }) => setTitle(tx, title),
  },
  setCaption: {
    description: "Set the caption of an image on the board, up to 60 characters.",
    args: {
      tile: s.string({ description: "The tile's $id." }),
      caption: s.string({ description: "The caption; empty to clear it." }),
    },
    run: ({ tx, current }, { tile, caption }) => setCaption(tx, current, tile, caption),
  },
  removeTile: {
    description: "Remove an image from the board by its tile $id.",
    args: { tile: s.string({ description: "The tile's $id." }) },
    run: ({ tx, current }, { tile }) => removeTile(tx, current, tile),
  },
});
