import { defineDocument, s, type Value } from "@hitslop/document";

export const themes = ["swiss", "dark", "navy"] as const;
export const layouts = ["title", "split", "metric", "quote", "cards"] as const;

const schema = defineDocument({
  deckTitle: s.text(),
  theme: s.enum(themes),
  activeSlideIndex: s.integer({ min: 0 }),
  slides: s.list(s.object({
    layout: s.enum(layouts),
    title: s.optional(s.text()),
    subtitle: s.optional(s.text()),
    tag: s.optional(s.text()),
    points: s.list(s.string()),
    highlightLabel: s.optional(s.text()),
    highlightValue: s.optional(s.text()),
    highlightDesc: s.optional(s.text()),
    metricValue: s.optional(s.text()),
    metricLabel: s.optional(s.text()),
    quoteText: s.optional(s.text()),
    author: s.optional(s.text()),
    cards: s.list(s.object({
      title: s.text(),
      desc: s.text(),
    })),
    notes: s.text(),
  })),
});

export type Deck = Value<typeof schema.fields.node>;
export type Slide = Deck["slides"][number];
export type SlideLayout = (typeof layouts)[number];
export type DeckTheme = (typeof themes)[number];
export default schema;
