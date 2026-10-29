import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";

type Edit = Tx<typeof schema>;
export type Game = Value<typeof schema.descriptor>;

export const LEVELS = {
  easy: { cols: 6, rows: 6, mines: 5 },
  medium: { cols: 8, rows: 8, mines: 10 },
  hard: { cols: 10, rows: 10, mines: 18 },
} as const;
export type Level = keyof typeof LEVELS;

export const hiddenBoard = (level: Level) => "h".repeat(LEVELS[level].cols * LEVELS[level].rows);

function neighbors(level: Level, index: number): number[] {
  const { cols, rows } = LEVELS[level];
  const x = index % cols;
  const y = Math.floor(index / cols);
  const out: number[] = [];
  for (let dy = -1; dy <= 1; dy++) {
    for (let dx = -1; dx <= 1; dx++) {
      if (!dx && !dy) continue;
      const nx = x + dx;
      const ny = y + dy;
      if (nx >= 0 && nx < cols && ny >= 0 && ny < rows) out.push(ny * cols + nx);
    }
  }
  return out;
}

/** Scatter mines anywhere except the first-revealed cell and its neighbors. */
export function layMines(level: Level, safe: number, random: () => number): string {
  const total = LEVELS[level].cols * LEVELS[level].rows;
  const banned = new Set([safe, ...neighbors(level, safe)]);
  const pool = Array.from({ length: total }, (_, index) => index).filter((index) => !banned.has(index));
  for (let i = pool.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [pool[i], pool[j]] = [pool[j]!, pool[i]!];
  }
  const mines = new Set(pool.slice(0, LEVELS[level].mines));
  return Array.from({ length: total }, (_, index) => (mines.has(index) ? "1" : "0")).join("");
}

export const adjacent = (level: Level, mines: string, index: number): number =>
  neighbors(level, index).filter((n) => mines[n] === "1").length;

/** Reveal a cell; a zero floods outward. Flagged cells are left alone. Returns the new cells and whether a mine was hit. */
export function reveal(level: Level, cells: string, mines: string, index: number): { cells: string; boom: boolean } {
  if (cells[index] !== "h") return { cells, boom: false };
  const next = cells.split("");
  if (mines[index] === "1") {
    next[index] = "r";
    return { cells: next.join(""), boom: true };
  }
  const queue = [index];
  while (queue.length) {
    const at = queue.pop()!;
    if (next[at] !== "h") continue;
    next[at] = "r";
    if (adjacent(level, mines, at) === 0) queue.push(...neighbors(level, at));
  }
  return { cells: next.join(""), boom: false };
}

export const cleared = (cells: string, mines: string): boolean =>
  [...mines].every((mine, index) => mine === "1" || cells[index] === "r");

export const flagsLeft = (level: Level, cells: string): number =>
  LEVELS[level].mines - [...cells].filter((cell) => cell === "f").length;

export const clock = (seconds: number): string =>
  `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;

/** What the editor and its captures draw: one record per cell, derived from the snapshot. */
export function boardView(game: Game) {
  const level = game.level as Level;
  const over = game.status === "won" || game.status === "lost";
  const cells = [...game.cells].map((mark, index) => {
    const mine = game.mines[index] === "1";
    const open = mark === "r";
    return {
      index,
      mark,
      mine,
      open,
      near: open && !mine ? adjacent(level, game.mines, index) : 0,
      showMine: mine && (open || game.status === "lost"),
      boom: (open && mine) || (game.status === "lost" && mine && mark !== "f"),
    };
  });
  return { level, size: LEVELS[level], over, best: game.best[level], cells, flagsLeft: flagsLeft(level, game.cells) };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

/** Start a fresh hidden board at a size. */
export function newGame(tx: Edit, level: Level) {
  if (!(level in LEVELS)) refuse(`Unknown level "${level}".`);
  tx.fields.level.set(level);
  tx.fields.status.set("ready");
  tx.fields.cells.set(hiddenBoard(level));
  tx.fields.mines.set("");
  tx.fields.seconds.set(0);
}

/** Reveal a cell. The first reveal lays the mines, never on that cell or its neighbors. */
export function dig(tx: Edit, game: Game, index: number, random: () => number, seconds: number) {
  const level = game.level as Level;
  if (game.status === "won" || game.status === "lost") refuse("The game is over; start a new board.");
  if (!Number.isInteger(index) || index < 0 || index >= game.cells.length) refuse("That cell is not on the board.");
  if (game.cells[index] === "f") refuse("Remove the buoy before digging there.");
  if (game.cells[index] !== "h") refuse("That cell is already clear.");
  const mines = game.mines || layMines(level, index, random);
  const result = reveal(level, game.cells, mines, index);
  const won = !result.boom && cleared(result.cells, mines);
  const time = game.status === "playing" ? seconds : 0;
  tx.fields.mines.set(mines);
  tx.fields.cells.set(result.cells);
  tx.fields.seconds.set(time);
  tx.fields.status.set(result.boom ? "lost" : won ? "won" : "playing");
  if (result.boom) tx.fields.losses.increment();
  if (won) {
    tx.fields.wins.increment();
    const best = game.best[level];
    if (!best || time < best) tx.fields.best[level].set(Math.max(time, 1));
  }
  return result.boom ? "lost" : won ? "won" : "playing";
}

/** Toggle a buoy on a hidden cell. */
export function toggleFlag(tx: Edit, game: Game, index: number) {
  if (game.status === "won" || game.status === "lost") refuse("The game is over; start a new board.");
  if (!Number.isInteger(index) || index < 0 || index >= game.cells.length) refuse("That cell is not on the board.");
  const mark = game.cells[index];
  if (mark === "r") refuse("A cleared cell cannot take a buoy.");
  tx.fields.cells.set(game.cells.slice(0, index) + (mark === "f" ? "h" : "f") + game.cells.slice(index + 1));
  return mark === "f" ? "removed" : "placed";
}
