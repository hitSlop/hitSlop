import { defineDocument, s } from "@hitslop/tinys";

export const tints = ["butter", "mint", "sky", "rose", "lilac"] as const;
export const stamps = ["🍕", "⭐", "✅", "❤️", "💸", "🎉", "⚠️", "🌱"] as const;

export default defineDocument(
  {
    title: s.text({ description: "The sheet's title." }),
    // Sparse cells keyed "B3": edits to different cells merge independently.
    cells: s.record(
      s.object({
        input: s.string({ maxLength: 500, description: "What was typed: a number, words or a =formula." }),
        tint: s.optional(s.enum(tints)),
        stamp: s.optional(s.string({ maxLength: 16 })),
      }),
    ),
    // Column letter to pixel width, present only after a resize.
    widths: s.record(s.integer({ min: 56, max: 320 })),
  },
  {
    initial: {
      title: "Pizza night",
      cells: {
        A1: { input: "Item" }, B1: { input: "Cost" }, C1: { input: "Paid by" },
        A2: { input: "Pizza", stamp: "🍕" }, B2: { input: "24" }, C2: { input: "Sam" },
        A3: { input: "Drinks" }, B3: { input: "18.50" }, C3: { input: "Ari" },
        A4: { input: "Garlic knots" }, B4: { input: "7.25" }, C4: { input: "Sam" },
        A5: { input: "Tip" }, B5: { input: "=ROUND(SUM(B2:B4)*18%, 2)" },
        A6: { input: "Total", tint: "butter" }, B6: { input: "=SUM(B2:B5)", tint: "butter" },
        A8: { input: "Friends" }, B8: { input: "4" },
        A9: { input: "Each pays", tint: "mint" }, B9: { input: "=ROUND(B6/B8, 2)", tint: "mint", stamp: "💸" },
      },
      widths: { A: 128 },
    },
  },
);
