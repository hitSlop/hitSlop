import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { colors } from "./schema";
import { addHabit, dayKey, editHabit, removeHabit, setCheckin } from "./calendar";

export default defineCommands(doc, {
  addHabit: {
    description: "Add a habit with no check-ins and return its stable row ID.",
    args: { name: s.string(), color: s.enum(colors) },
    run: ({ tx }, { name, color }) => addHabit(tx, name, color),
  },
  editHabit: {
    description: "Rename a habit and set its color.",
    args: { habit: s.string({ description: "The habit's $id." }), name: s.string(), color: s.enum(colors) },
    run: ({ tx }, { habit, name, color }) => editHabit(tx, habit, name, color),
  },
  checkIn: {
    description: "Mark one day complete for a habit. Refuses days after today.",
    args: { habit: s.string({ description: "The habit's $id." }), day: s.string({ description: "Local calendar day, YYYY-MM-DD." }) },
    run: (ctx, { habit, day }) => setCheckin(ctx.tx, habit, day, true, dayKey(new Date(ctx.now))),
  },
  clearCheckIn: {
    description: "Clear one day's check-in for a habit.",
    args: { habit: s.string({ description: "The habit's $id." }), day: s.string({ description: "Local calendar day, YYYY-MM-DD." }) },
    run: (ctx, { habit, day }) => setCheckin(ctx.tx, habit, day, false, dayKey(new Date(ctx.now))),
  },
  removeHabit: {
    description: "Remove a habit and all its check-in history.",
    args: { habit: s.string({ description: "The habit's $id." }) },
    run: ({ tx }, { habit }) => removeHabit(tx, habit),
  },
});
