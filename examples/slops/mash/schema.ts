import { defineDocument, s, type Value } from "@hitslop/document";

export const slots = ["home", "spouse", "kids", "car", "job", "city", "pet"] as const;

const schema = defineDocument({
  title: s.text(),
  // How many loops the last spiral had; 0 means it has not been drawn yet.
  loops: s.integer({ min: 0, max: 20 }),
  options: s.list(s.object({
    slot: s.enum(slots),
    text: s.text(),
  })),
});

export type Mash = Value<typeof schema.descriptor>;
export type Option = Mash["options"][number];
export type Slot = (typeof slots)[number];
export default schema;
