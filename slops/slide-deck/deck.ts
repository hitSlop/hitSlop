import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { layouts, themes } from "./schema";

export type Edit = Tx<typeof schema>;
export type Deck = Value<typeof schema.descriptor>;
export type Slide = Deck["slides"][number];
export type SlideLayout = (typeof layouts)[number];
export type DeckTheme = (typeof themes)[number];

/** The editor, the presenter and the capture read slides from the same snapshot. The selected index is view state. */
export function deckView(data: Deck, selected: number) {
  const slides = data.slides;
  const index = Math.max(0, Math.min(slides.length - 1, selected));
  return { slides, index, current: slides[index] as Slide | undefined, last: slides.length - 1 };
}

function slideOf(current: Deck, id: RowRef) {
  const key = typeof id === "string" ? id : id.$id;
  const slide = current.slides.find(item => item.$id === key);
  if (!slide) refuse("No slide with that $id.");
  return slide;
}

export function addSlide(tx: Edit) {
  return tx.fields.slides.insert({
    layout: "split",
    title: "New Slide Title",
    points: ["First key insight", "Second supporting point", "Third impactful takeaway"],
    highlightLabel: "METRIC",
    highlightValue: "100%",
    highlightDesc: "Description of the achievement or focus.",
    cards: [],
    notes: "Speaker talking points go here.",
  });
}

export function removeSlide(tx: Edit, current: Deck, id: RowRef) {
  const slide = slideOf(current, id);
  if (current.slides.length <= 1) refuse("A deck needs at least one slide.");
  tx.fields.slides.remove(slide.$id);
}

/** Move a slide one place up (-1) or down (1) the deck. */
export function moveSlide(tx: Edit, current: Deck, id: RowRef, direction: -1 | 1) {
  const slide = slideOf(current, id);
  const neighbor = current.slides[current.slides.indexOf(slide) + direction];
  if (!neighbor) refuse(direction < 0 ? "That slide is already first." : "That slide is already last.");
  tx.fields.slides.move(slide.$id, direction < 0 ? { before: neighbor.$id } : { after: neighbor.$id });
}

/** Switch a slide's layout, filling only the fields the new layout needs and keeping existing text. */
export function setLayout(tx: Edit, current: Deck, id: RowRef, layout: SlideLayout) {
  const slide = slideOf(current, id);
  if (!layouts.includes(layout)) refuse(`Layout must be one of: ${layouts.join(", ")}.`);
  if (slide.layout === layout) return;
  const item = tx.fields.slides.item(slide.$id);
  item.layout.set(layout);
  const fill = (value: string | undefined, handle: { set(value: string): void }, fallback: string) => {
    if (value === undefined) handle.set(fallback);
  };
  if (layout === "title") {
    fill(slide.tag, item.tag, "");
    fill(slide.title, item.title, "");
    fill(slide.subtitle, item.subtitle, "");
  } else if (layout === "split") {
    fill(slide.title, item.title, "");
    if (!slide.points.length) for (const point of ["Point 1", "Point 2", "Point 3"]) item.points.insert(point);
    fill(slide.highlightLabel, item.highlightLabel, "HIGHLIGHT");
    fill(slide.highlightValue, item.highlightValue, "Key Metric");
    fill(slide.highlightDesc, item.highlightDesc, "");
  } else if (layout === "metric") {
    fill(slide.tag, item.tag, "");
    fill(slide.metricValue, item.metricValue, "99.9%");
    fill(slide.metricLabel, item.metricLabel, "RELIABILITY");
    fill(slide.subtitle, item.subtitle, "");
  } else if (layout === "quote") {
    fill(slide.quoteText, item.quoteText, "Inspiration leads to innovation.");
    fill(slide.author, item.author, "Author Name");
  } else {
    fill(slide.title, item.title, "");
    if (!slide.cards.length) {
      item.cards.insert({ title: "Pillar 1", desc: "First pillar description." });
      item.cards.insert({ title: "Pillar 2", desc: "Second pillar description." });
      item.cards.insert({ title: "Pillar 3", desc: "Third pillar description." });
    }
  }
}

export function setTheme(tx: Edit, theme: DeckTheme) {
  if (!themes.includes(theme)) refuse(`Theme must be one of: ${themes.join(", ")}.`);
  tx.fields.theme.set(theme);
}

export function nextTheme(theme: DeckTheme): DeckTheme {
  return themes[(themes.indexOf(theme) + 1) % themes.length] ?? "swiss";
}
