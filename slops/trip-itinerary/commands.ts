import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { stopTags } from "./schema";
import { addDay, addPackingItem, addStop, removeStop, setPackingDone, setStopDone } from "./shared";

export default defineCommands(doc, {
  addStop: {
    description: "Add a stop to a day's timeline and return its stable row ID.",
    args: {
      day: s.string({ description: "The day's dayKey." }),
      time: s.string({ description: "The start time as HH:MM; blank becomes 10:00." }),
      title: s.string({ description: "What the stop is." }),
      location: s.string({ description: "The place or note; may be blank." }),
      tag: s.enum(stopTags, { description: "The stop type." }),
    },
    run: ({ tx, current }, { day, time, title, location, tag }) => addStop(tx, current, day, { time, title, location, tag }),
  },
  setStopDone: {
    description: "Check or uncheck a stop by its $id.",
    args: {
      stop: s.string({ description: "The stop's $id." }),
      done: s.boolean({ description: "True when the stop is done." }),
    },
    run: ({ tx, current }, { stop, done }) => setStopDone(tx, current, stop, done),
  },
  removeStop: {
    description: "Remove a stop from its day by its $id.",
    args: { stop: s.string({ description: "The stop's $id." }) },
    run: ({ tx, current }, { stop }) => removeStop(tx, current, stop),
  },
  addDay: {
    description: "Add a day after the last one, select it, and return its dayKey.",
    args: { subtitle: s.string({ description: "The day's theme line; blank uses the default." }) },
    run: (ctx, { subtitle }) => addDay(ctx.tx, ctx.current, `d-${Math.floor(ctx.random() * 2 ** 52).toString(36)}`, subtitle),
  },
  addPackingItem: {
    description: "Add an unpacked item to the packing stub and return its stable row ID.",
    args: { text: s.string({ description: "The item's text." }) },
    run: ({ tx }, { text }) => addPackingItem(tx, text),
  },
  setPackingDone: {
    description: "Check or uncheck a packing item by its $id.",
    args: {
      item: s.string({ description: "The item's $id." }),
      done: s.boolean({ description: "True when the item is packed." }),
    },
    run: ({ tx }, { item, done }) => setPackingDone(tx, item, done),
  },
});
