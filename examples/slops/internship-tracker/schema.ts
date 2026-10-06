import { defineDocument, s, type Value } from "@hitslop/document";

export const stages = ["wishlist", "applied", "assessment", "interview", "offer", "rejected"] as const;

const schema = defineDocument({
  title: s.text(),
  applications: s.list(s.object({
    company: s.text(),
    role: s.text(),
    stage: s.enum(stages),
    followUp: s.optional(s.string({ maxLength: 10 })),
  })),
  chats: s.list(s.object({
    name: s.text(),
    place: s.text(),
    when: s.optional(s.string({ maxLength: 10 })),
    thanked: s.boolean(),
  })),
});

export type Tracker = Value<typeof schema.descriptor>;
export type Application = Tracker["applications"][number];
export type Chat = Tracker["chats"][number];
export default schema;
