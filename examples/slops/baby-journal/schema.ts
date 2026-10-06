import { defineDocument, s, type Input, type Value } from "@hitslop/document";

export const colors = ["peach", "pink", "blue", "lilac", "mint", "yellow", "apricot"] as const;
export const kinds = ["feeding", "diaper", "sleep", "note"] as const;
const schema = defineDocument({
  name: s.text(),
  birthday: s.string(),
  color: s.enum(colors),
  photo: s.optional(s.object({ id: s.string(), mimeType: s.string() })),
  lastCaregiverId: s.string(),
  bottleUnit: s.enum(["mL", "oz"]),
  caregivers: s.list(s.object({ name: s.text(), archived: s.boolean() })),
  entries: s.list(
    s.object({
      kind: s.enum(kinds),
      timestamp: s.number(),
      caregiverId: s.string(),
      notes: s.text(),
      feedingType: s.enum(["bottle", "nursing"]),
      amount: s.optional(s.number({ min: 0 })),
      unit: s.enum(["mL", "oz"]),
      side: s.enum(["left", "right", "both"]),
      minutes: s.optional(s.number({ min: 0 })),
      diaper: s.enum(["wet", "dirty", "both"]),
      endTimestamp: s.optional(s.number()),
    }),
  ),
});
export type Journal = Value<typeof schema.descriptor>;
export type Entry = Journal["entries"][number];
export type EntryInput = Input<typeof schema.descriptor>["entries"][number];
export type Kind = Entry["kind"];
export default schema;
