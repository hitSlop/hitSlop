export const currencies = ["USD", "EUR", "GBP", "JPY"] as const;
export const tones = ["sky", "leaf", "sun", "rose", "violet", "slate"] as const;
export type Tone = (typeof tones)[number];
export const maxSources = 4;
export const maxBuckets = 7;

export const money = (amount: number, currency: string): string =>
  new Intl.NumberFormat(undefined, { style: "currency", currency, maximumFractionDigits: 0 }).format(amount);

export const share = (part: number, whole: number): string => (whole > 0 ? `${Math.round((part / whole) * 100)}%` : "0%");
