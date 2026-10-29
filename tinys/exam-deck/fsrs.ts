// Pure scheduling over ts-fsrs 5.4.2. Every function takes time explicitly (`now`, epoch
// milliseconds) and never reads the clock or Math.random, so the page and a command get the
// same answer from the same snapshot. Stored state is plain data (see SCHEMA.md); ts-fsrs
// `Card` objects exist only inside these functions.
import { clipParameters, fsrs as createFsrs, State, type Card, type FSRS, type Grade, type StepUnit } from "ts-fsrs";

const DAY = 86_400_000;
const MINUTE = 60_000;

/** The FSRS fields of a stored card; extra row fields ($id, front, back) are ignored. */
export interface Schedule {
  readonly state: StateName;
  readonly due?: string;
  readonly stability: number;
  readonly difficulty: number;
  readonly scheduledDays: number;
  readonly learningStep: number;
  readonly reps: number;
  readonly lapses: number;
  readonly lastReview?: string;
}

/** The stored `scheduler` object. */
export interface SchedulerParams {
  readonly algorithm: "fsrs-6";
  readonly w: readonly number[];
  readonly requestRetention: number;
  readonly maximumInterval: number;
  readonly enableFuzz: boolean;
  readonly enableShortTerm: boolean;
  readonly learningSteps: readonly string[];
  readonly relearningSteps: readonly string[];
}

export interface Review { readonly card: string; readonly rating: number; readonly at: string }
export type Rating = 1 | 2 | 3 | 4;
export type StateName = "new" | "learning" | "review" | "relearning";

const stateNames: readonly StateName[] = ["new", "learning", "review", "relearning"];
const stateValues: Record<StateName, State> = { new: State.New, learning: State.Learning, review: State.Review, relearning: State.Relearning };

/** A fresh, never-reviewed card. */
export const newSchedule: Schedule = Object.freeze({
  state: "new", stability: 0, difficulty: 0, scheduledDays: 0, learningStep: 0, reps: 0, lapses: 0,
});

/** The most reviews the log keeps; SCHEMA.md states the policy. */
export const REVIEW_LOG_LIMIT = 20_000;
/** With no exam date, readiness and new-card pacing assume an exam this many days out. */
export const ASSUMED_EXAM_DAYS = 28;

const isoPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?Z$/;
const dayPattern = /^\d{4}-\d{2}-\d{2}$/;
const stepPattern = /^\d+(?:\.\d+)?[mhd]$/;

export class ScheduleError extends Error {}

function instant(value: string, field: string): Date {
  const ms = isoPattern.test(value) ? Date.parse(value) : NaN;
  if (!Number.isFinite(ms)) throw new ScheduleError(`${field} is not an ISO 8601 UTC time: ${value}`);
  return new Date(ms);
}

/** Why these parameters cannot be used as stored, or null when they can. ts-fsrs silently
 * clamps weights it considers out of range, so a stored weight that would be changed is
 * refused here instead of being used under a different value than the one saved. */
export function paramsProblem(params: SchedulerParams): string | null {
  if (params.algorithm !== "fsrs-6") return `Unknown scheduling algorithm "${params.algorithm}".`;
  if (params.w.length !== 21) return `FSRS-6 needs 21 weights, found ${params.w.length}.`;
  if (!params.w.every(Number.isFinite)) return "A weight is not a finite number.";
  const clipped = clipParameters([...params.w], params.relearningSteps.length, params.enableShortTerm);
  const changed = clipped.findIndex((value, index) => value !== params.w[index]);
  if (changed >= 0) return `Weight w[${changed}] = ${params.w[changed]} is outside the range FSRS-6 accepts.`;
  const steps = [...params.learningSteps, ...params.relearningSteps];
  const bad = steps.find(step => !stepPattern.test(step));
  if (bad !== undefined) return `Step "${bad}" must be a number followed by m, h or d.`;
  if (!(params.requestRetention > 0 && params.requestRetention <= 1)) return "Target retention must be between 0 and 1.";
  if (!(Number.isInteger(params.maximumInterval) && params.maximumInterval >= 1)) return "Maximum interval must be a whole number of days.";
  return null;
}

/** A scheduler bound to stored parameters; throws ScheduleError when they are unusable. */
export function scheduler(params: SchedulerParams) {
  const problem = paramsProblem(params);
  if (problem) throw new ScheduleError(problem);
  const engine: FSRS = createFsrs({
    w: [...params.w],
    request_retention: params.requestRetention,
    maximum_interval: params.maximumInterval,
    enable_fuzz: params.enableFuzz,
    enable_short_term: params.enableShortTerm,
    learning_steps: [...params.learningSteps] as StepUnit[],
    relearning_steps: [...params.relearningSteps] as StepUnit[],
  });

  /** Stored fields → a ts-fsrs Card. A new card has no stored due; ts-fsrs does not read it
   * for new cards, so `now` stands in. `elapsed_days` is deprecated and recomputed by the
   * library from `last_review` at every review, so it is not stored. */
  function toCard(schedule: Schedule, now: number): Card {
    if (!stateNames.includes(schedule.state)) throw new ScheduleError(`Unknown card state "${schedule.state}".`);
    const isNew = schedule.state === "new";
    if (!isNew && (schedule.due === undefined || schedule.lastReview === undefined))
      throw new ScheduleError("A reviewed card needs due and lastReview.");
    return {
      due: schedule.due === undefined ? new Date(now) : instant(schedule.due, "due"),
      stability: schedule.stability,
      difficulty: schedule.difficulty,
      elapsed_days: 0,
      scheduled_days: schedule.scheduledDays,
      learning_steps: schedule.learningStep,
      reps: schedule.reps,
      lapses: schedule.lapses,
      state: stateValues[schedule.state],
      last_review: schedule.lastReview === undefined ? undefined : instant(schedule.lastReview, "lastReview"),
    };
  }

  /** A ts-fsrs Card → the fields to store. */
  function fromCard(card: Card): Schedule {
    const state = stateNames[card.state];
    if (state === "new") return newSchedule;
    return {
      state,
      due: card.due.toISOString(),
      stability: card.stability,
      difficulty: card.difficulty,
      scheduledDays: card.scheduled_days,
      learningStep: card.learning_steps,
      reps: card.reps,
      lapses: card.lapses,
      lastReview: (card.last_review ?? card.due).toISOString(),
    };
  }

  function grade(rating: number): Grade {
    if (rating !== 1 && rating !== 2 && rating !== 3 && rating !== 4) throw new ScheduleError(`Rating must be 1–4, got ${rating}.`);
    return rating as Grade;
  }

  /** The stored fields after rating `schedule` at `now`. */
  function review(schedule: Schedule, rating: number, now: number): Schedule {
    return fromCard(engine.next(toCard(schedule, now), new Date(now), grade(rating)).card);
  }

  /** What each rating would do, for the four answer buttons. */
  function preview(schedule: Schedule, now: number): Record<Rating, { schedule: Schedule; intervalMs: number }> {
    const card = toCard(schedule, now);
    const out = {} as Record<Rating, { schedule: Schedule; intervalMs: number }>;
    for (const rating of [1, 2, 3, 4] as const) {
      const next = engine.next(card, new Date(now), rating as Grade).card;
      out[rating] = { schedule: fromCard(next), intervalMs: next.due.getTime() - now };
    }
    return out;
  }

  /** Probability of recall at `at` (0 for a new card), as ts-fsrs computes it. */
  function retrievability(schedule: Schedule, at: number): number {
    if (schedule.state === "new") return 0;
    return engine.get_retrievability(toCard(schedule, at), new Date(at), false);
  }

  /** Rebuild a card's state from its reviews, oldest first. With the stored parameters this
   * reproduces the stored state; with new parameters it reschedules from history. */
  function replay(reviews: readonly Review[]): Schedule {
    return reviews.reduce<Schedule>((schedule, entry) => review(schedule, entry.rating, instant(entry.at, "at").getTime()), newSchedule);
  }

  return { engine, toCard, fromCard, review, preview, retrievability, replay };
}

export type Scheduler = ReturnType<typeof scheduler>;

/** True when the log still holds every review of the card, so `replay` can rebuild it. */
export function logComplete(cardId: string, schedule: Schedule, reviews: readonly Review[]): boolean {
  let count = 0;
  for (const entry of reviews) if (entry.card === cardId) count++;
  return count === schedule.reps;
}

// Local calendar days. The caller passes the UTC offset in minutes (east positive, so
// `-new Date(now).getTimezoneOffset()` in a page). A fixed offset can be an hour off across
// a daylight-saving change inside the horizon; it never affects stored data.

export function localDay(ms: number, utcOffsetMinutes: number): string {
  return new Date(ms + utcOffsetMinutes * MINUTE).toISOString().slice(0, 10);
}
export function dayStart(day: string, utcOffsetMinutes: number): number {
  if (!dayPattern.test(day)) throw new ScheduleError(`Not a YYYY-MM-DD date: ${day}`);
  return Date.parse(`${day}T00:00:00Z`) - utcOffsetMinutes * MINUTE;
}
export function addDays(day: string, days: number): string {
  return new Date(Date.parse(`${day}T00:00:00Z`) + days * DAY).toISOString().slice(0, 10);
}
export function daysBetween(from: string, to: string): number {
  return Math.round((Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / DAY);
}

/** Seeded PRNG (mulberry32): the forecast's only source of randomness. */
export function mulberry32(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface ForecastInput {
  readonly cards: readonly (Schedule & { readonly $id: string })[];
  readonly reviews: readonly Review[];
  readonly scheduler: SchedulerParams;
  readonly examDate?: string;
  readonly dailyMinutes: number;
  readonly secondsPerCard: number;
  readonly newPerDay?: number;
  readonly now: number;
  readonly utcOffsetMinutes: number;
  /** Days in the load strip (default 14). */
  readonly days?: number;
  /** Simulated futures averaged together (default 8). */
  readonly runs?: number;
  readonly seed?: number;
}

export interface ForecastDay { date: string; reviews: number; newCards: number; overBudget: boolean }

export interface Forecast {
  today: {
    date: string;
    /** Reviewed cards due by `now`. */
    dueNow: number;
    /** Reviewed cards due before the end of today. */
    dueToday: number;
    /** New cards still to introduce today. */
    newToday: number;
    /** Reviews already logged today. */
    doneToday: number;
    /** Cards the daily budget allows. */
    cap: number;
    /** The headline "N cards today": due and new work that still fits today's budget. */
    planned: number;
    /** Due cards that do not fit today's budget. */
    backlog: number;
  };
  days: ForecastDay[];
  exam: {
    date: string;
    /** True when no exam date is stored and ASSUMED_EXAM_DAYS stands in. */
    assumed: boolean;
    daysLeft: number;
    /** Mean probability of recall across every card on exam morning (unseen cards count 0). */
    expectedRecall: number;
    /** Mean number of cards at or above the target retention on exam morning. */
    readyCards: number;
    /** Mean number of cards still never studied by the exam. */
    unseen: number;
    total: number;
  };
}

/** Even pacing: the remaining new cards spread until two days before the exam. */
function autoNew(remaining: number, daysLeft: number) {
  if (remaining <= 0) return 0;
  const spread = daysLeft > 4 ? daysLeft - 2 : Math.max(1, daysLeft);
  return Math.ceil(remaining / spread);
}

interface SimCard { schedule: Schedule; dueMs: number; index: number }

/**
 * Today's work from the stored state, and a forward simulation of the next `days` days and
 * the exam. Each simulated day takes due cards (earliest first) up to the budget, then new
 * cards in deck order, then same-day learning steps (always finished, never capped). A new
 * or learning card is rated Good; a review card is recalled (Good) with the probability
 * ts-fsrs gives at that moment, else forgotten (Again), drawn from the seeded PRNG. The
 * result is the mean over `runs` futures, so equal input always gives equal output.
 */
export function forecast(input: ForecastInput): Forecast {
  const fsrs = scheduler(input.scheduler);
  const { now, utcOffsetMinutes: offset } = input;
  const days = input.days ?? 14;
  const runs = input.runs ?? 8;
  const seed = input.seed ?? 0x5eed;
  const today = localDay(now, offset);
  const examDate = input.examDate ?? addDays(today, ASSUMED_EXAM_DAYS);
  const examIndex = daysBetween(today, examDate);
  const cap = Math.max(1, Math.floor(input.dailyMinutes * 60 / input.secondsPerCard));

  // Today, from the log: reviews done and cards first studied today.
  const firstSeen = new Map<string, string>();
  let doneToday = 0;
  for (const entry of input.reviews) {
    const day = localDay(instant(entry.at, "at").getTime(), offset);
    if (day === today) doneToday++;
    if (!firstSeen.has(entry.card)) firstSeen.set(entry.card, day);
  }
  let introducedToday = 0;
  for (const day of firstSeen.values()) if (day === today) introducedToday++;

  const endToday = dayStart(addDays(today, 1), offset);
  const reviewed = input.cards.filter(card => card.state !== "new");
  const dueMs = (card: Schedule) => instant(card.due!, "due").getTime();
  const dueNow = reviewed.filter(card => dueMs(card) <= now).length;
  const dueToday = reviewed.filter(card => dueMs(card) < endToday).length;
  const remainingNew = input.cards.length - reviewed.length;
  const newQuota = (daysLeft: number, remaining: number, introduced: number) =>
    Math.min(remaining, Math.max(0, (input.newPerDay ?? autoNew(remaining + introduced, daysLeft)) - introduced));
  const capToday = Math.max(0, cap - doneToday);
  const newToday = newQuota(examIndex, remainingNew, introducedToday);
  const plannedReviews = Math.min(dueToday, capToday);
  const planned = plannedReviews + Math.min(newToday, capToday - plannedReviews);

  // The simulation runs through the exam (bounded) so readiness sees every review before it.
  const horizon = Math.max(days, Math.min(examIndex + 1, 366));
  const load = Array.from({ length: days }, () => ({ reviews: 0, newCards: 0, overBudget: 0 }));
  let expectedRecall = 0, readyCards = 0, unseen = 0;
  const target = input.scheduler.requestRetention;

  for (let run = 0; run < runs; run++) {
    const random = mulberry32(seed + Math.imul(run + 1, 0x9e3779b9));
    const sims: SimCard[] = input.cards.map((card, index) => ({
      schedule: card, index,
      dueMs: card.state === "new" ? Infinity : dueMs(card),
    }));
    const fresh = sims.filter(sim => sim.schedule.state === "new");
    let measured = false;

    const study = (sim: SimCard, at: number) => {
      const s = sim.schedule;
      const rating = s.state === "review" ? (random() < fsrs.retrievability(s, at) ? 3 : 1) : 3;
      sim.schedule = fsrs.review(s, rating, at);
      sim.dueMs = Date.parse(sim.schedule.due!);
    };
    const measure = (at: number) => {
      let recall = 0, ready = 0;
      for (const sim of sims) {
        const r = fsrs.retrievability(sim.schedule, at);
        recall += r;
        if (r >= target) ready++;
      }
      expectedRecall += sims.length ? recall / sims.length : 0;
      readyCards += ready;
      unseen += fresh.length;
      measured = true;
    };
    if (examIndex < 0) measure(now);

    for (let d = 0; d < horizon; d++) {
      const day = addDays(today, d);
      const start = dayStart(day, offset);
      const end = start + DAY;
      const session = d === 0 ? now : start + 12 * 60 * MINUTE;
      if (d === examIndex) measure(start + 9 * 60 * MINUTE);
      if (d >= days && measured) break;

      let budget = d === 0 ? capToday : cap;
      let reviews = 0, newCards = 0, at = session;
      const due = sims.filter(sim => sim.dueMs < end).sort((a, b) => a.dueMs - b.dueMs || a.index - b.index);
      const steps: SimCard[] = [];
      for (const sim of due) {
        if (budget <= 0) break;
        study(sim, at = Math.max(sim.dueMs, session));
        budget--; reviews++;
        if (sim.dueMs < end) steps.push(sim);
      }
      const overBudget = due.length > reviews;
      const quota = d === 0 ? newToday : newQuota(examIndex - d, fresh.length, 0);
      for (const sim of fresh.splice(0, Math.min(quota, Math.max(0, budget)))) {
        study(sim, session);
        budget--; reviews++; newCards++;
        if (sim.dueMs < end) steps.push(sim);
      }
      // Same-day learning steps, in due order; bounded so a step list can never loop forever.
      for (let guard = 0; steps.length && guard < 10_000; guard++) {
        steps.sort((a, b) => a.dueMs - b.dueMs || a.index - b.index);
        const sim = steps.shift()!;
        study(sim, at = Math.max(sim.dueMs, at));
        reviews++;
        if (sim.dueMs < end) steps.push(sim);
      }
      if (d < days) {
        load[d].reviews += reviews;
        load[d].newCards += newCards;
        if (overBudget) load[d].overBudget++;
      }
    }
    // An exam beyond the bounded horizon is measured from the state the horizon reached.
    if (!measured) measure(dayStart(examDate, offset) + 9 * 60 * MINUTE);
  }

  return {
    today: { date: today, dueNow, dueToday, newToday, doneToday, cap, planned, backlog: Math.max(0, dueToday - capToday) },
    days: load.map((entry, d) => ({
      date: addDays(today, d),
      reviews: entry.reviews / runs,
      newCards: entry.newCards / runs,
      overBudget: entry.overBudget * 2 > runs,
    })),
    exam: {
      date: examDate,
      assumed: input.examDate === undefined,
      daysLeft: examIndex,
      expectedRecall: expectedRecall / runs,
      readyCards: readyCards / runs,
      unseen: unseen / runs,
      total: input.cards.length,
    },
  };
}
