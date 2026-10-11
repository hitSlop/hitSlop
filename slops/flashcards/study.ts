import { refuse, type Tx } from "hitslop";
import type doc from "./schema";

export type Edit = Tx<typeof doc>;
export type Data = typeof doc.current;
type Deck = Data["decks"][number];
type Card = Deck["cards"][number];

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

/** A good answer climbs one box and a miss falls one, both clamped to boxes 1 to 4. */
export function nextBox(box: number, good: boolean): LeitnerBox {
  return clampBox(good ? box + 1 : box - 1);
}

/** The deck, cards and position the study view shows for the saved decks and the page's selection. */
export function studyView(data: Data, selection: { deckKey: string; box: number; cardIndex: number }) {
  const decks = data.decks;
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

/** A deck key for a new deck; the caller supplies the randomness. */
export function newDeckKey(random: () => number): string {
  return `d-${Math.floor(random() * 2 ** 48).toString(36)}`;
}

function shuffleIds(ids: string[], random: () => number): string[] {
  const next = [...ids];
  for (let index = next.length - 1; index > 0; index -= 1) {
    const swap = Math.floor(random() * (index + 1));
    const left = next[index]!;
    const right = next[swap]!;
    next[index] = right;
    next[swap] = left;
  }
  return next;
}

function findDeck(data: Data, deckId: string): Deck {
  const deck = data.decks.find((row) => row.$id === deckId);
  if (!deck) refuse("That deck is no longer here.");
  return deck;
}

function findCard(deck: Deck, cardId: string): Card {
  const card = deck.cards.find((row) => row.$id === cardId);
  if (!card) refuse("That card is no longer in this deck.");
  return card;
}

function cardText(front: string, back: string) {
  const text = { front: front.trim(), back: back.trim() };
  if (!text.front || !text.back) refuse("Enter both a front and a back for the card.");
  return text;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addDeck(tx: Edit, name: string, deckKey: string) {
  const trimmed = name.trim();
  if (!trimmed) refuse("Name the deck.");
  return tx.fields.decks.insert({ deckKey, name: trimmed, cards: [] });
}
export function addCard(tx: Edit, data: Data, deckId: string, front: string, back: string) {
  const deck = findDeck(data, deckId);
  return tx.at(deck).cards.insert(cardText(front, back));
}
export function editCard(tx: Edit, data: Data, deckId: string, cardId: string, front: string, back: string) {
  const card = findCard(findDeck(data, deckId), cardId);
  const text = cardText(front, back);
  tx.at(card).front.set(text.front);
  tx.at(card).back.set(text.back);
}
/** Grades a card and stamps lastReviewed from the explicit time; returns the card's new box. */
export function gradeCard(tx: Edit, data: Data, deckId: string, cardId: string, good: boolean, now: number): LeitnerBox {
  const card = findCard(findDeck(data, deckId), cardId);
  const box = nextBox(card.box, good);
  tx.at(card).box.set(box);
  tx.at(card).lastReviewed.set(new Date(now).toISOString());
  return box;
}
export function removeCard(tx: Edit, data: Data, deckId: string, cardId: string) {
  const deck = findDeck(data, deckId);
  tx.at(deck).cards.remove(findCard(deck, cardId).$id);
}
export function shuffleDeck(tx: Edit, data: Data, deckId: string, random: () => number): number {
  const deck = findDeck(data, deckId);
  const order = shuffleIds(deck.cards.map((card) => card.$id), random);
  const firstId = deck.cards[0]?.$id;
  let anchor: string | null = null;
  for (const id of order) {
    if (anchor === null) {
      if (firstId && id !== firstId) tx.at(deck).cards.move(id, { before: firstId });
    } else tx.at(deck).cards.move(id, { after: anchor });
    anchor = id;
  }
  return order.length;
}
