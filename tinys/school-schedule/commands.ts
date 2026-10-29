import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { COLORS, DAYS } from "./schema";
import {
  copyDay, moveClass, periodKeyFrom, removeActivity, removeClass, removePeriod, saveActivity, saveClass, saveDetails, savePeriod,
} from "./schedule";

const day = s.enum(DAYS, { description: "Weekday: MON to FRI." });
const color = s.enum(COLORS, { description: "Subject color." });
const time = (what: string) => s.string({ description: `${what} as 24-hour HH:MM.` });

export default defineCommands(schema, {
  addClass: {
    description: "Put a class in an empty timetable slot and return its row ID.",
    args: {
      day,
      period: s.string({ description: "The period's periodKey, from the periods list." }),
      subject: s.string(),
      room: s.optional(s.string()),
      teacher: s.optional(s.string()),
      color: s.optional(color),
    },
    run: ({ tx, current }, a) =>
      saveClass(tx, current, { day: a.day, periodKey: a.period, subject: a.subject, room: a.room ?? "", teacher: a.teacher ?? "", color: a.color ?? "slate" }),
  },
  editClass: {
    description: "Change a class's subject, room, teacher and color, and return its row ID.",
    args: {
      class: s.string({ description: "The class's $id." }),
      subject: s.string(),
      room: s.string(),
      teacher: s.string(),
      color,
    },
    run: ({ tx, current }, a) => {
      const row = current.classes.find((c) => c.$id === a.class);
      return saveClass(tx, current, { day: row?.day ?? "", periodKey: row?.periodKey ?? "", subject: a.subject, room: a.room, teacher: a.teacher, color: a.color }, a.class);
    },
  },
  moveClass: {
    description: "Move a class to another day and period, swapping with any class already there.",
    args: {
      class: s.string({ description: "The class's $id." }),
      day,
      period: s.string({ description: "The target period's periodKey." }),
    },
    run: ({ tx, current }, a) => moveClass(tx, current, a.class, a.day, a.period),
  },
  removeClass: {
    description: "Remove a class from the timetable.",
    args: { class: s.string({ description: "The class's $id." }) },
    run: ({ tx, current }, a) => removeClass(tx, current, a.class),
  },
  addPeriod: {
    description: "Add a bell-time period for every weekday and return its row ID.",
    args: { name: s.string(), start: time("Start"), end: time("End") },
    run: ({ tx, current, random }, a) => savePeriod(tx, current, { periodKey: periodKeyFrom(random()), ...a }),
  },
  removePeriod: {
    description: "Remove a period and every class in it; return how many classes were removed.",
    args: { period: s.string({ description: "The period's $id." }) },
    run: ({ tx, current }, a) => removePeriod(tx, current, a.period),
  },
  addActivity: {
    description: "Add an after-class plan on a weekday and return its row ID.",
    args: { day, title: s.string(), start: time("Start"), end: time("End"), location: s.optional(s.string()) },
    run: ({ tx, current }, a) => saveActivity(tx, current, { ...a, location: a.location ?? "" }),
  },
  removeActivity: {
    description: "Remove an after-class plan.",
    args: { activity: s.string({ description: "The plan's $id." }) },
    run: ({ tx, current }, a) => removeActivity(tx, current, a.activity),
  },
  copyDay: {
    description: "Repeat one weekday's classes across the other four days, replacing theirs; return the number copied.",
    args: { day },
    run: ({ tx, current }, a) => copyDay(tx, current, a.day),
  },
  setDetails: {
    description: "Set the student name, term, homeroom and locker.",
    args: { studentName: s.string(), term: s.string(), homeroom: s.string(), locker: s.string() },
    run: ({ tx }, a) => saveDetails(tx, a),
  },
});
