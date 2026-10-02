import type { Deck, Flashcards } from "./schema";

export type LeitnerBox = 1 | 2 | 3 | 4;
export const BOXES: Array<{ id: LeitnerBox; label: string }> = [
  { id: 1, label: "New" },
  { id: 2, label: "Review" },
  { id: 3, label: "Known" },
  { id: 4, label: "Mastered" },
];

export function clampBox(value: number): LeitnerBox {
  if (value <= 1) return 1;
  if (value >= 4) return 4;
  return value as LeitnerBox;
}

/** The deck, cards and position the study view shows for the saved decks and the session's selection. */
export function studyView(decks: Flashcards["decks"], selection: { deckKey: string; box: number; cardIndex: number }) {
  const activeDeck: Deck | null = decks.find((deck) => deck.deckKey === selection.deckKey) ?? decks[0] ?? null;
  const cards = activeDeck?.cards ?? [];
  const visibleCards = selection.box === 0 ? cards : cards.filter((card) => card.box === selection.box);
  const cardIndex = visibleCards.length === 0 ? 0 : Math.min(Math.max(0, selection.cardIndex), visibleCards.length - 1);
  const currentCard = visibleCards[cardIndex] ?? null;
  const boxCounts: Record<LeitnerBox, number> = { 1: 0, 2: 0, 3: 0, 4: 0 };
  for (const card of cards) boxCounts[clampBox(card.box)] += 1;
  const boxLabel = currentCard ? (BOXES[clampBox(currentCard.box) - 1]?.label ?? "New") : "New";
  const stages = [
    { id: 0, label: "All", count: cards.length },
    ...BOXES.map(({ id, label }) => ({ id, label, count: boxCounts[id] })),
  ];
  return { activeDeck, visibleCards, cardIndex, currentCard, boxCounts, boxLabel, stages };
}
