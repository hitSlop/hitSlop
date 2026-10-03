export const statuses = ["on-time", "boarding", "delayed", "gone"] as const;
export type Status = (typeof statuses)[number];
export const maxRows = 8;
export const widths = { time: 5, text: 14, status: 8 } as const;

export const statusLabel: Record<Status, string> = {
  "on-time": "ON TIME",
  boarding: "BOARDING",
  delayed: "DELAYED",
  gone: "DEPARTED",
};

export const nextStatus = (status: Status): Status => statuses[(statuses.indexOf(status) + 1) % statuses.length]!;

/** Board text is upper case and padded to its column, one character per flap. */
export const cells = (text: string, width: number): string[] => [...text.toUpperCase().padEnd(width).slice(0, width)];

export const clean = (text: string, width: number): string => text.toUpperCase().trim().slice(0, width);
