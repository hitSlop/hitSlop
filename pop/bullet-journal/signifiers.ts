export const signifierTypes = ["task", "complete", "migrated", "scheduled", "event", "note"] as const;

export const SIGNIFIERS = [
  { type: "task", symbol: "•", label: "Task" },
  { type: "complete", symbol: "×", label: "Completed" },
  { type: "migrated", symbol: "›", label: "Migrated" },
  { type: "scheduled", symbol: "‹", label: "Scheduled" },
  { type: "event", symbol: "○", label: "Event" },
  { type: "note", symbol: "—", label: "Note" },
] as const;

export type Signifier = typeof SIGNIFIERS[number]["type"];
export const CYCLE: Signifier[] = SIGNIFIERS.map(item => item.type);
export const signifierItems = SIGNIFIERS.map(item => ({
  value: item.type,
  label: `${item.symbol} ${item.label}`,
}));

export function isSignifier(value: string): value is Signifier {
  return SIGNIFIERS.some(item => item.type === value);
}

export function symbolFor(type: string): string {
  return SIGNIFIERS.find(item => item.type === type)?.symbol ?? "•";
}

export function labelFor(type: string): string {
  return SIGNIFIERS.find(item => item.type === type)?.label ?? "Task";
}

export function nextSignifier(type: string): Signifier {
  const index = CYCLE.indexOf(type as Signifier);
  return CYCLE[(index + 1) % CYCLE.length] ?? "task";
}
