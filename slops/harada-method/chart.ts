import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";

type Sheet = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

export const RING: [number, number][] = [
  [0, 0], [0, 1], [0, 2], [1, 0], [1, 2], [2, 0], [2, 1], [2, 2],
];

export function ringIndex(row: number, col: number): number {
  return RING.findIndex(([r, c]) => r === row && c === col);
}

export function keyOf(theme: number, action: number): string {
  return `${theme}:${action}`;
}

export type CellRole = "goal" | "theme" | "action";
export type CellEdge = "none" | "hair" | "block";

export type Cell = {
  row: number;
  col: number;
  block: number;
  role: CellRole;
  theme: number;
  action: number;
  edgeX: CellEdge;
  edgeY: CellEdge;
};

// One list places themes around the goal and blocks around the sheet, so a
// theme sits in the same direction as the 3×3 it opens.
export const CELLS: Cell[] = Array.from({ length: 81 }, (_, index) => {
  const row = Math.floor(index / 9);
  const col = index % 9;
  const blockRow = Math.floor(row / 3);
  const blockCol = Math.floor(col / 3);
  const localRow = row % 3;
  const localCol = col % 3;
  const block = blockRow * 3 + blockCol;
  const isCentreBlock = block === 4;
  const isCentreCell = localRow === 1 && localCol === 1;
  const edgeX: CellEdge = col === 8 ? "none" : localCol === 2 ? "block" : "hair";
  const edgeY: CellEdge = row === 8 ? "none" : localRow === 2 ? "block" : "hair";
  const base = { row, col, block, edgeX, edgeY } as const;
  if (isCentreBlock && isCentreCell) return { ...base, role: "goal", theme: -1, action: -1 };
  if (isCentreBlock) return { ...base, role: "theme", theme: ringIndex(localRow, localCol), action: -1 };
  const theme = ringIndex(blockRow, blockCol);
  if (isCentreCell) return { ...base, role: "theme", theme, action: -1 };
  return { ...base, role: "action", theme, action: ringIndex(localRow, localCol) };
});

/** The caller passes the clock (`Date.now()`), so the view stays a pure function of its inputs. */
export function formatDeadline(value: string, now: number): string {
  const parsed = new Date(`${value}T00:00:00`);
  if (Number.isNaN(parsed.valueOf())) return "No date set";
  const days = Math.ceil((parsed.valueOf() - now) / 86_400_000);
  if (days > 1) return `${days} days out`;
  if (days === 1) return "Tomorrow";
  if (days === 0) return "Today";
  return "Passed";
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

export function formatDate(value: string): string {
  const parsed = new Date(`${value}T00:00:00`);
  if (Number.isNaN(parsed.valueOf())) return "";
  return `${parsed.getDate()} ${MONTHS[parsed.getMonth()]} ${parsed.getFullYear()}`;
}

/** The editor and export read cells, progress and labels from the same snapshot. */
export function sheetView(sheet: Sheet) {
  const namedThemes = sheet.themes.filter(theme => theme.title.trim()).length;
  const writtenActions = sheet.themes.reduce((count, theme) => count + theme.cells.filter(cell => cell.trim()).length, 0);
  return {
    doneCount: Object.values(sheet.done).filter(Boolean).length,
    deadlineText: formatDate(sheet.deadline),
    guide: !sheet.goal.trim() ? "Write the one goal in the centre."
      : namedThemes < 8 ? "Name the eight themes that ring it."
        : writtenActions < 8 ? "Give each theme eight concrete actions."
          : "",
    isDone: (theme: number, action: number) => sheet.done[keyOf(theme, action)] === true,
    titleOf: (theme: number): string => sheet.themes[theme]?.title.trim() || `Theme ${theme + 1}`,
    textOf(cell: Cell): string {
      if (cell.role === "goal") return sheet.goal;
      const theme = sheet.themes[cell.theme];
      if (!theme) return "";
      if (cell.role === "theme") return theme.title;
      return theme.cells[cell.action] ?? "";
    },
  };
}

// Edits are written once: the page runs them with doc.change(tx => ...), a command with its tx.
// Indices are 0 to 7 in ring order, the same slots CELLS lays out.
const SLOTS = RING.length;

function themeRow(current: Sheet, theme: number) {
  const row = theme >= 0 && theme < SLOTS ? current.themes[theme] : undefined;
  if (!row) refuse(`Theme ${theme + 1} does not exist.`);
  return row;
}

function actionRow(current: Sheet, theme: number, action: number) {
  const row = themeRow(current, theme);
  if (!(action >= 0 && action < SLOTS && action < row.cells.length)) refuse(`Action ${action + 1} does not exist on theme ${theme + 1}.`);
  return row;
}

export function setGoal(tx: Edit, text: string): void {
  tx.fields.goal.set(text);
}

export function setTheme(tx: Edit, current: Sheet, theme: number, title: string): void {
  tx.at(themeRow(current, theme)).title.set(title);
}

export function setAction(tx: Edit, current: Sheet, theme: number, action: number, text: string): void {
  tx.at(actionRow(current, theme, action)).cells.set(action, text);
}

/** Ticks or unticks an action; returns the new state. */
export function setDone(tx: Edit, current: Sheet, theme: number, action: number, done: boolean): boolean {
  actionRow(current, theme, action);
  const key = keyOf(theme, action);
  if (done) tx.fields.done.put(key, true);
  else tx.fields.done.delete(key);
  return done;
}

/** "" clears the date; anything else must be a calendar date as YYYY-MM-DD. */
export function setDeadline(tx: Edit, value: string): void {
  if (value !== "" && !/^\d{4}-\d{2}-\d{2}$/.test(value)) refuse("Use a date as YYYY-MM-DD, or leave it empty.");
  tx.fields.deadline.set(value);
}
