import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { evaluateGuess, getDailyWord, getRandomWord, isValidWord } from "./words";

export type Data = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;
type Mode = Data["mode"];
type Status = Data["daily"]["status"];

export const ROWS = 6;
export const COLS = 5;

export interface Game {
  targetWord: string;
  guesses: readonly string[];
  status: Status;
  date?: string | undefined;
}

/**
 * The game being played in a mode. The daily board saved for an earlier day reads as a fresh
 * puzzle for `today`; the first guess then writes that reset, so nothing writes on its own.
 */
export function gameOf(data: Data, mode: Mode, today: string): Game {
  if (mode === "practice") return data.practice;
  if (data.daily.date === today) return data.daily;
  return { targetWord: getDailyWord(today).word, guesses: [], status: "playing", date: today };
}

/** The editor, its captures and the share text derive everything from the same snapshot. */
export function wordleView(data: Data, today: string) {
  const game = gameOf(data, data.mode, today);
  const { played, won } = data.stats;
  return {
    mode: data.mode,
    game,
    isOver: game.status !== "playing",
    winPercent: played ? Math.round((won / played) * 100) : 0,
    score: game.status === "won" ? `${game.guesses.length}/${ROWS}` : game.status === "lost" ? `X/${ROWS}` : `${Math.min(ROWS, game.guesses.length + 1)} of ${ROWS}`,
    statusLine: game.status === "won"
      ? `Solved in ${game.guesses.length}/${ROWS}`
      : game.status === "lost"
        ? "Out of guesses"
        : `Guess ${Math.min(ROWS, game.guesses.length + 1)} of ${ROWS}`,
  };
}

/** Spoiler-free result text for the active game. */
export function shareText(data: Data, today: string): string {
  const { mode, game } = wordleView(data, today);
  const score = game.status === "won" ? game.guesses.length : "X";
  const header = mode === "daily" ? `hitSlop Wordle (${game.date ?? today}) ${score}/${ROWS}` : `hitSlop Wordle (Practice) ${score}/${ROWS}`;
  const rows = game.guesses.map(guess =>
    evaluateGuess(guess, game.targetWord).map(state => (state === "correct" ? "🟩" : state === "present" ? "🟨" : "⬛")).join(""));
  return `${header}\n\n${rows.join("\n")}`;
}

function board(tx: Edit, mode: Mode) {
  return mode === "daily" ? tx.fields.daily : tx.fields.practice;
}

/** Clear a board for a new target word. */
function resetBoard(tx: Edit, data: Data, mode: Mode, targetWord: string, date?: string) {
  const b = board(tx, mode);
  const count = data[mode].guesses.length;
  b.targetWord.set(targetWord);
  if (count) b.guesses.remove(0, count);
  b.status.set("playing");
  if (date !== undefined) b.date.set(date);
  else if (data[mode].date !== undefined) b.date.clear();
}

/** Play a five-letter guess in the active mode; updates status and stats when the game ends. */
export function playGuess(tx: Edit, data: Data, today: string, raw: string) {
  const guess = raw.trim().toUpperCase();
  if (guess.length < COLS) refuse("Not enough letters.");
  if (!isValidWord(guess)) refuse("Not in word list.");
  const mode = data.mode;
  const game = gameOf(data, mode, today);
  if (game.status !== "playing") refuse("This game is over.");
  if (mode === "daily" && data.daily.date !== today) resetBoard(tx, data, "daily", game.targetWord, today);
  const b = board(tx, mode);
  const count = game.guesses.length + 1;
  const won = guess === game.targetWord;
  const lost = !won && count >= ROWS;
  b.guesses.insert(guess);
  if (!won && !lost) return "playing" as Status;
  b.status.set(won ? "won" : "lost");
  const stats = tx.fields.stats;
  stats.played.increment();
  if (won) {
    stats.won.increment();
    const streak = data.stats.currentStreak + 1;
    stats.currentStreak.set(streak);
    if (streak > data.stats.maxStreak) stats.maxStreak.set(streak);
    const key = String(count);
    stats.guessDistribution.put(key, (data.stats.guessDistribution[key] ?? 0) + 1);
  } else {
    stats.currentStreak.set(0);
  }
  return (won ? "won" : "lost") as Status;
}

/** Start a new practice puzzle with a word from the supplied random source. */
export function newPractice(tx: Edit, data: Data, random: () => number) {
  const word = getRandomWord(random);
  resetBoard(tx, data, "practice", word);
}

export function setMode(tx: Edit, mode: Mode) {
  tx.fields.mode.set(mode);
}
