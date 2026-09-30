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

export function nextOctober(now = new Date()): string {
  const year = now.getMonth() >= 9 ? now.getFullYear() + 1 : now.getFullYear();
  return `${year}-10-12`;
}

export function formatDeadline(value: string, now = Date.now()): string {
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
