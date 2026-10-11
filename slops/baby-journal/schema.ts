import { defineDocument, s } from "hitslop";

export const colors = ["peach", "pink", "blue", "lilac", "mint", "yellow", "apricot"] as const;
export const kinds = ["feeding", "diaper", "sleep", "note"] as const;

export default defineDocument({
  name: s.text(),
  birthday: s.string(),
  color: s.enum(colors, { default: "peach" }),
  photo: s.optional(s.object({ id: s.string(), mimeType: s.string() })),
  lastCaregiverId: s.string(),
  bottleUnit: s.enum(["mL", "oz"], { default: "mL" }),
  caregivers: s.list(s.object({ name: s.text(), archived: s.boolean({ default: false }) })),
  entries: s.list(
    s.object({
      kind: s.enum(kinds),
      timestamp: s.number(),
      caregiverId: s.string(),
      notes: s.text(),
      feedingType: s.enum(["bottle", "nursing"], { default: "bottle" }),
      amount: s.optional(s.number({ min: 0 })),
      unit: s.enum(["mL", "oz"], { default: "mL" }),
      side: s.enum(["left", "right", "both"], { default: "both" }),
      minutes: s.optional(s.number({ min: 0 })),
      diaper: s.enum(["wet", "dirty", "both"], { default: "wet" }),
      endTimestamp: s.optional(s.number()),
    }),
  ),
}, { initial: {
  name: "",
  birthday: "",
  lastCaregiverId: "",
  caregivers: [{ name: "You" }],
  entries: [],
} });
