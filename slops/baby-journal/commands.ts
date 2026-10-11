import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { kinds } from "./schema";
import { addCaregiver, addEntry, removeEntry, setCaregiverArchived, updateEntry, type EntryInput } from "./journal";

const entryArgs = {
  kind: s.enum(kinds, { description: "feeding, diaper, sleep or note." }),
  caregiver: s.string({ description: "The caregiver's $id." }),
  timestamp: s.optional(s.number({ description: "Unix milliseconds of the moment; defaults to now." })),
  notes: s.optional(s.string({ description: "Notes, or the memory text for a note." })),
  feeding: s.optional(s.enum(["bottle", "nursing"], { description: "Feeding type; defaults to bottle." })),
  amount: s.optional(s.number({ description: "Bottle amount above zero; blank to omit." })),
  unit: s.optional(s.enum(["mL", "oz"], { description: "Bottle unit; defaults to the journal's bottle unit." })),
  minutes: s.optional(s.number({ description: "Nursing minutes above zero; blank to omit." })),
  side: s.optional(s.enum(["left", "right", "both"], { description: "Nursing side; defaults to both." })),
  diaper: s.optional(s.enum(["wet", "dirty", "both"], { description: "Diaper contents; defaults to wet." })),
  endTimestamp: s.optional(s.number({ description: "Sleep only: Unix milliseconds of waking up." })),
};

type EntryArgs = {
  kind: EntryInput["kind"];
  caregiver: string;
  timestamp?: number;
  notes?: string;
  feeding?: EntryInput["feedingType"];
  amount?: number;
  unit?: EntryInput["unit"];
  minutes?: number;
  side?: EntryInput["side"];
  diaper?: EntryInput["diaper"];
  endTimestamp?: number;
};

/** Fills the optional arguments with the same defaults the form uses. */
function entryInput(args: EntryArgs, bottleUnit: EntryInput["unit"], now: number): EntryInput {
  return {
    kind: args.kind,
    timestamp: args.timestamp ?? now,
    caregiverId: args.caregiver,
    notes: args.notes ?? "",
    feedingType: args.feeding ?? "bottle",
    unit: args.unit ?? bottleUnit,
    side: args.side ?? "both",
    diaper: args.diaper ?? "wet",
    amount: args.amount,
    minutes: args.minutes,
    endTimestamp: args.endTimestamp,
  };
}

export default defineCommands(doc, {
  addEntry: {
    description: "Record a feeding, diaper change, sleep or memory and return its stable row ID; the time defaults to now.",
    args: entryArgs,
    run: ({ tx, current, now }, args) => addEntry(tx, current, entryInput(args, current.bottleUnit, now), now),
  },
  updateEntry: {
    description: "Replace the details of an entry by its $id, keeping its kind; omitted optional details are cleared.",
    args: { entry: s.string({ description: "The entry's $id." }), ...entryArgs },
    run: ({ tx, current, now }, { entry, ...args }) => updateEntry(tx, current, entry, entryInput(args, current.bottleUnit, now), now),
  },
  removeEntry: {
    description: "Remove an entry from the journal by its $id.",
    args: { entry: s.string({ description: "The entry's $id." }) },
    run: ({ tx, current }, { entry }) => removeEntry(tx, current, entry),
  },
  addCaregiver: {
    description: "Add an active caregiver by name and return its stable row ID; a repeated name is refused.",
    args: { name: s.string({ description: "The caregiver's name, such as Amanda or Grandma." }) },
    run: ({ tx, current }, { name }) => addCaregiver(tx, current, name),
  },
  setCaregiverArchived: {
    description: "Archive or restore a caregiver by $id; archived caregivers stay on past entries, and one must stay active.",
    args: {
      caregiver: s.string({ description: "The caregiver's $id." }),
      archived: s.boolean({ description: "True to archive, false to restore." }),
    },
    run: ({ tx, current }, { caregiver, archived }) => setCaregiverArchived(tx, current, caregiver, archived),
  },
});
