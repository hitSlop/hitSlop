/** The verdict statuses. The schema reads the values; the page reads the labels. */
export const statuses = ["evaluating", "leaning_pro", "leaning_con", "decided_pro", "decided_con"] as const;

export type Status = (typeof statuses)[number];

export const STATUS_LABELS: Record<Status, string> = {
  evaluating: "Evaluating",
  leaning_pro: "Leaning For",
  leaning_con: "Leaning Against",
  decided_pro: "Proceed",
  decided_con: "Pass",
};

export const STATUS_OPTIONS = statuses.map((value) => ({ value, label: STATUS_LABELS[value] }));

export function isStatus(value: string): value is Status {
  return statuses.some((status) => status === value);
}
