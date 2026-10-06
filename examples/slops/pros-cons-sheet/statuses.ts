export const STATUSES = [
  { value: "evaluating", label: "Evaluating" },
  { value: "leaning_pro", label: "Leaning For" },
  { value: "leaning_con", label: "Leaning Against" },
  { value: "decided_pro", label: "Proceed" },
  { value: "decided_con", label: "Pass" },
] as const;

export type Status = (typeof STATUSES)[number]["value"];

export function isStatus(value: string): value is Status {
  return STATUSES.some((item) => item.value === value);
}
