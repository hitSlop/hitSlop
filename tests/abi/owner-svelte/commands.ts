import { defineCommands } from "hitslop/commands";
import { refuse, s } from "hitslop";
import schema from "./schema";

export default defineCommands(schema, {
  bump: {
    description: "Add to the hit counter, record the result and report the total.",
    args: { by: s.integer({ min: 1, max: 10 }) },
    run({ current, tx }, { by }) {
      tx.fields.hits.increment(by);
      tx.fields.lastHit.set(current.hits + by);
      return current.hits + by;
    },
  },
  shapes: {
    description:
      "Emit every document intent and anchored row destination through the stored prelude.",
    args: {},
    run({ current, tx, now, random }) {
      const anchor = tx.fields.rows.insert({ text: "Command anchor" }).id;
      const before = tx.fields.rows.insert({ text: "Before" }, { before: anchor }).id;
      const after = tx.fields.rows.insert({ text: "After" }, { after: anchor }).id;
      tx.fields.rows.move(before, { after });
      tx.fields.rows.move(after);
      tx.fields.rows.remove(before);
      tx.fields.note.clear();
      tx.fields.place.$clear();
      tx.fields.colors.replace(["red", "green", "blue"]);
      tx.fields.colors.insert("white", 0);
      tx.fields.colors.set(0, "black");
      tx.fields.colors.remove(1, 2);
      tx.fields.checkins.put("command", 1);
      tx.fields.checkins.delete("command");
      tx.at(current.settings).hits.increment();
      tx.at(current).title.set("Command title");
      tx.fields.hits.increment();
      return { anchor, after, now, random: random() };
    },
  },
  arguments: {
    description: "Optional and defaulted arguments for every scalar kind; return an array.",
    args: {
      boolean: s.boolean({ default: true }),
      string: s.string({ default: "default" }),
      number: s.number({ default: 0.5 }),
      integer: s.integer({ default: 7 }),
      enum: s.enum(["todo", "done"], { default: "todo" }),
      optionalBoolean: s.optional(s.boolean()),
      optionalString: s.optional(s.string()),
      optionalNumber: s.optional(s.number()),
      optionalInteger: s.optional(s.integer()),
      optionalEnum: s.optional(s.enum(["todo", "done"])),
    },
    run(_ctx, args) {
      return [
        args.boolean,
        args.string,
        args.number,
        args.integer,
        args.enum,
        args.optionalBoolean ?? null,
        args.optionalString ?? null,
        args.optionalNumber ?? null,
        args.optionalInteger ?? null,
        args.optionalEnum ?? null,
      ];
    },
  },
  nothing: {
    description: "Return null.",
    args: {},
    run() {
      return null;
    },
  },
  decline: {
    description: "Refuse with a message for the person.",
    args: {},
    run() {
      refuse("Not today.");
    },
  },
});
