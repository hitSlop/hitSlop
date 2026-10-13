import { dateLabel } from "../_shared/format";
// The deck model: one view function for App, Export, Icon and commands, and every edit
// written once over `Tx` so the page and an agent change the document the same way.
// Nothing here reads the clock; callers pass `now` (epoch ms) and a UTC offset.
import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import {
  ASSUMED_EXAM_DAYS, REVIEW_LOG_LIMIT, ScheduleError, addDays, dayStart, forecast, localDay, newSchedule,
  paramsProblem, scheduler, type Forecast, type ForecastDay, type Rating, type Schedule,
} from "./fsrs";

export type Deck = typeof doc.current;
export type Card = Deck["cards"][number];
type Edit = Tx<typeof doc>;
type CardHandle = ReturnType<Edit["fields"]["cards"]["item"]>;

const MINUTE = 60_000;
const DAY = 86_400_000;
/** A learning step due within this window may be shown early when nothing else is left. */
const LEARN_AHEAD = 20 * MINUTE;
const isoPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?Z$/;
const dayPattern = /^\d{4}-\d{2}-\d{2}$/;

export const ratings = [
  { value: 1, label: "Again" }, { value: 2, label: "Hard" }, { value: 3, label: "Good" }, { value: 4, label: "Easy" },
] as const satisfies readonly { value: Rating; label: string }[];

export const errorMessage = (error: unknown) => (error instanceof Error ? error.message : String(error));

// --- Reading -------------------------------------------------------------------------

export interface DeckView {
  /** Why the stored parameters cannot schedule anything, or null. */
  problem: string | null;
  /** Cards ts-fsrs cannot read, by $id, with the reason. They are left out of every count. */
  unreadable: Map<string, string>;
  forecast: Forecast | null;
  /** The bars to draw: min(14, days until the exam), at least today. */
  strip: ForecastDay[];
  /** Due and new cards that do not fit today's budget. */
  cutByCap: number;
  /** The daily card cap, from minutes and seconds per card. */
  cap: number;
}

export function cardCap(data: Pick<Deck, "dailyMinutes" | "secondsPerCard">) {
  return Math.max(1, Math.floor(data.dailyMinutes * 60 / data.secondsPerCard));
}

/** Everything the readouts show, from the snapshot and a supplied clock. */
export function deckView(data: Deck, now: number, utcOffsetMinutes: number): DeckView {
  const cap = cardCap(data);
  const unreadable = new Map<string, string>();
  const empty: DeckView = { problem: null, unreadable, forecast: null, strip: [], cutByCap: 0, cap };
  const problem = paramsProblem(data.scheduler);
  if (problem) return { ...empty, problem };
  const fsrs = scheduler(data.scheduler);
  const readable = data.cards.filter(card => {
    try {
      fsrs.toCard(card, now);
      return true;
    } catch (error) {
      unreadable.set(card.$id, errorMessage(error));
      return false;
    }
  });
  // A malformed log row (only possible through raw CLI edits) cannot count toward today.
  const reviews = data.reviews.filter(entry => isoPattern.test(entry.at) && Number.isFinite(Date.parse(entry.at)));
  let result: Forecast;
  try {
    result = forecast({
      cards: readable, reviews, scheduler: data.scheduler, examDate: validDay(data.examDate) ? data.examDate : undefined,
      dailyMinutes: data.dailyMinutes, secondsPerCard: data.secondsPerCard, newPerDay: data.newPerDay,
      now, utcOffsetMinutes, days: 14,
    });
  } catch (error) {
    return { ...empty, problem: errorMessage(error) };
  }
  const { daysLeft } = result.exam;
  const shown = daysLeft < 0 ? 14 : Math.max(1, Math.min(14, daysLeft));
  const { today } = result;
  const wanted = today.dueToday + today.newToday;
  return { problem: null, unreadable, forecast: result, strip: result.days.slice(0, shown), cutByCap: Math.max(0, wanted - today.planned), cap };
}

export type Next =
  | { kind: "card"; card: Card; left: number }
  | { kind: "done"; nextStepAt: number | null; waiting: number };

/**
 * The next card to study, from the snapshot. Same-day learning steps that are due always
 * come first (they are never capped, as in the forecast); then review cards due today,
 * earliest first, and new cards in deck order, while today's budget lasts (`ignoreCap`
 * lifts it); then a learning step due within 20 minutes. `left` counts the cards in that
 * order that are studyable now.
 */
export function nextCard(data: Deck, view: DeckView, now: number, utcOffsetMinutes: number, ignoreCap = false): Next {
  const result = view.forecast;
  if (!result) return { kind: "done", nextStepAt: null, waiting: 0 };
  const endToday = dayStart(addDays(result.today.date, 1), utcOffsetMinutes);
  const cards = data.cards.filter(card => !view.unreadable.has(card.$id));
  const due = (card: Card) => Date.parse(card.due!);
  const byDue = (a: Card, b: Card) => due(a) - due(b);
  const steps = cards.filter(card => (card.state === "learning" || card.state === "relearning") && due(card) <= now).sort(byDue);
  const reviews = cards.filter(card => card.state === "review" && due(card) < endToday).sort(byDue);
  const fresh = cards.filter(card => card.state === "new").slice(0, result.today.newToday);
  const budget = ignoreCap ? Infinity : Math.max(0, result.today.cap - result.today.doneToday);
  const allowedReviews = reviews.slice(0, budget);
  const allowedNew = fresh.slice(0, Math.max(0, budget - allowedReviews.length));
  const queue = [...steps, ...allowedReviews, ...allowedNew];
  const ahead = cards
    .filter(card => (card.state === "learning" || card.state === "relearning") && due(card) > now)
    .sort(byDue);
  if (!queue.length && ahead.length && due(ahead[0]) <= now + LEARN_AHEAD) queue.push(ahead[0]);
  if (queue.length) return { kind: "card", card: queue[0], left: queue.length };
  const later = ahead.find(card => due(card) < endToday);
  return {
    kind: "done",
    nextStepAt: later ? due(later) : null,
    waiting: reviews.length - allowedReviews.length + fresh.length - allowedNew.length,
  };
}

/** What each answer button would schedule, or the reason the card cannot be scheduled. */
export function answerPreview(data: Deck, card: Card, now: number) {
  try {
    return { ok: true as const, options: scheduler(data.scheduler).preview(card, now) };
  } catch (error) {
    return { ok: false as const, reason: errorMessage(error) };
  }
}

export function formatInterval(ms: number) {
  const minutes = Math.max(1, Math.round(ms / MINUTE));
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h`;
  const days = Math.round(ms / DAY);
  if (days < 45) return `${days} ${days === 1 ? "day" : "days"}`;
  if (days < 365) return `${Math.round(days / 30)} mo`;
  return `${(days / 365).toFixed(days < 3650 ? 1 : 0)} yr`;
}

export function formatDay(day: string, style: "short" | "long" = "short") {
  const date = new Date(`${day}T12:00:00Z`);
  return dateLabel(date, style === "long"
    ? { weekday: "short", month: "short", day: "numeric", timeZone: "UTC" }
    : { month: "short", day: "numeric", timeZone: "UTC" });
}

/** How a card's place in the schedule reads in the deck list. */
export function cardStatus(card: Card, view: DeckView, now: number) {
  const reason = view.unreadable.get(card.$id);
  if (reason) return { label: "Unreadable", detail: reason, tone: "error" as const };
  if (card.state === "new") return { label: "New", detail: "Not studied yet", tone: "new" as const };
  const wait = Date.parse(card.due!) - now;
  const label = card.state === "review" ? "Review" : card.state === "learning" ? "Learning" : "Relearning";
  return { label, detail: wait <= 0 ? "Due now" : `Due in ${formatInterval(wait)}`, tone: wait <= 0 ? "due" as const : "later" as const };
}

export function validDay(value: string | undefined): value is string {
  if (value === undefined || !dayPattern.test(value)) return false;
  const ms = Date.parse(`${value}T00:00:00Z`);
  return Number.isFinite(ms) && new Date(ms).toISOString().slice(0, 10) === value;
}

/** A compact summary for agents (`status` command). */
export function deckStatus(data: Deck, now: number, utcOffsetMinutes: number) {
  const view = deckView(data, now, utcOffsetMinutes);
  const result = view.forecast;
  return {
    problem: view.problem,
    unreadable: [...view.unreadable].map(([card, reason]) => ({ card, reason })),
    today: result ? { ...result.today, cutByCap: view.cutByCap } : null,
    forecast: view.strip.map(day => ({ date: day.date, cards: Math.round(day.reviews), newCards: Math.round(day.newCards), overBudget: day.overBudget })),
    exam: result ? {
      date: result.exam.date,
      assumed: result.exam.assumed,
      daysLeft: result.exam.daysLeft,
      estimatedRecall: Math.round(result.exam.expectedRecall * 1000) / 1000,
      estimatedReadyCards: Math.round(result.exam.readyCards),
      estimatedUnseen: Math.round(result.exam.unseen),
      total: result.exam.total,
      note: result.exam.assumed
        ? `No exam date is set, so an exam ${ASSUMED_EXAM_DAYS} days out is assumed. These are model estimates, not a promise.`
        : "Model estimates from FSRS, assuming the planned cards are studied. Not a promise.",
    } : null,
  };
}

// --- Editing -------------------------------------------------------------------------

function findCard(data: Deck, id: string): Card {
  const card = data.cards.find(row => row.$id === id);
  if (!card) refuse("There is no card with that id.");
  return card;
}

/** Writes all nine FSRS fields of one card. */
function writeSchedule(row: CardHandle, schedule: Schedule) {
  row.state.set(schedule.state);
  if (schedule.due === undefined) row.due.clear(); else row.due.set(schedule.due);
  row.stability.set(schedule.stability);
  row.difficulty.set(schedule.difficulty);
  row.scheduledDays.set(schedule.scheduledDays);
  row.learningStep.set(schedule.learningStep);
  row.reps.set(schedule.reps);
  row.lapses.set(schedule.lapses);
  if (schedule.lastReview === undefined) row.lastReview.clear(); else row.lastReview.set(schedule.lastReview);
}

/**
 * Answer a card at `at`: the card's nine FSRS fields and one log row in one change. When the
 * log is full the oldest rows go in the same change (SCHEMA.md's growth policy). A card or
 * parameters ts-fsrs cannot read are refused, never rewritten.
 */
export function reviewCard(tx: Edit, data: Deck, cardId: string, rating: number, at: number): Schedule {
  if (rating !== 1 && rating !== 2 && rating !== 3 && rating !== 4) refuse("Rate 1 (Again), 2 (Hard), 3 (Good) or 4 (Easy).");
  const card = findCard(data, cardId);
  if (card.lastReview !== undefined && Date.parse(card.lastReview) > at)
    refuse("This card was last reviewed after the review time; check the clock.");
  let next: Schedule;
  try {
    next = scheduler(data.scheduler).review(card, rating, at);
  } catch (error) {
    if (error instanceof ScheduleError) refuse(`This card can't be scheduled: ${error.message}`);
    throw error;
  }
  writeSchedule(tx.fields.cards.item(cardId), next);
  const overflow = data.reviews.length + 1 - REVIEW_LOG_LIMIT;
  for (let index = 0; index < overflow; index++) tx.fields.reviews.remove(data.reviews[index]);
  tx.fields.reviews.insert({ card: cardId, rating, at: new Date(at).toISOString() });
  return next;
}

export function addCard(tx: Edit, front: string, back: string) {
  const prompt = front.trim(), answer = back.trim();
  if (!prompt) refuse("Write the front of the card.");
  if (!answer) refuse("Write the back of the card.");
  return tx.fields.cards.insert({ front: prompt, back: answer }).id;
}

/** Remove a card and its review history together. */
export function removeCard(tx: Edit, data: Deck, cardId: string) {
  findCard(data, cardId);
  for (const entry of data.reviews) if (entry.card === cardId) tx.fields.reviews.remove(entry);
  tx.fields.cards.remove(cardId);
}

/** Return one card to new and drop its history (the repair for an unreadable card). */
export function resetCard(tx: Edit, data: Deck, cardId: string) {
  findCard(data, cardId);
  for (const entry of data.reviews) if (entry.card === cardId) tx.fields.reviews.remove(entry);
  writeSchedule(tx.fields.cards.item(cardId), newSchedule);
}

/** Every card back to new and an empty log. Returns the number of cards reset. */
export function resetProgress(tx: Edit, data: Deck) {
  for (const card of data.cards) writeSchedule(tx.at(card), newSchedule);
  for (const entry of data.reviews) tx.fields.reviews.remove(entry);
  return data.cards.length;
}

/** Set the exam date (YYYY-MM-DD, today or later) or clear it with `undefined`. */
export function setExamDate(tx: Edit, date: string | undefined, today: string) {
  if (date === undefined || date === "") return tx.fields.examDate.clear();
  if (!validDay(date)) refuse("Use a real date written YYYY-MM-DD.");
  if (date < today) refuse("Pick today or a later date for the exam.");
  tx.fields.examDate.set(date);
}

export function setRetention(tx: Edit, retention: number) {
  if (!Number.isFinite(retention) || retention < 0.7 || retention > 0.99)
    refuse("Target retention must be between 70% and 99% (0.70 to 0.99).");
  tx.fields.scheduler.requestRetention.set(retention);
}

export interface Budget { dailyMinutes?: number; secondsPerCard?: number; newPerDay?: number | null }

/** Change the study budget; `newPerDay: null` returns to automatic pacing. */
export function setBudget(tx: Edit, budget: Budget) {
  const whole = (value: number, min: number, max: number) => Number.isInteger(value) && value >= min && value <= max;
  if (budget.dailyMinutes !== undefined) {
    if (!whole(budget.dailyMinutes, 1, 600)) refuse("Daily minutes must be a whole number from 1 to 600.");
    tx.fields.dailyMinutes.set(budget.dailyMinutes);
  }
  if (budget.secondsPerCard !== undefined) {
    if (!whole(budget.secondsPerCard, 3, 600)) refuse("Seconds per card must be a whole number from 3 to 600.");
    tx.fields.secondsPerCard.set(budget.secondsPerCard);
  }
  if (budget.newPerDay === null) tx.fields.newPerDay.clear();
  else if (budget.newPerDay !== undefined) {
    if (!whole(budget.newPerDay, 1, 1000)) refuse("New cards per day must be a whole number from 1 to 1000.");
    tx.fields.newPerDay.set(budget.newPerDay);
  }
}

export { localDay, ASSUMED_EXAM_DAYS };
