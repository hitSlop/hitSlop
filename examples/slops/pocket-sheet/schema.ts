import { defineDocument, s, type Value } from "@hitslop/document";

export const tints = ["butter", "mint", "sky", "rose", "lilac"] as const;
export type Tint = (typeof tints)[number];
export const stamps = ["🍕", "⭐", "✅", "❤️", "💸", "🎉", "⚠️", "🌱"] as const;

const schema = defineDocument({
  title: s.text(),
  // Sparse cells keyed "B3": edits to different cells merge independently.
  cells: s.record(s.object({
    input: s.string({ maxLength: 500 }),
    tint: s.optional(s.enum(tints)),
    stamp: s.optional(s.string({ maxLength: 16 })),
  })),
  // Column letter to pixel width, present only after a resize.
  widths: s.record(s.integer({ min: 56, max: 320 })),
});
export type Cell = Value<typeof schema.fields.node>["cells"][string];
export default schema;
