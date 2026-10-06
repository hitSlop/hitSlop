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
export function layMines(level: Level, safe: number, random: () => number = Math.random): string {
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
