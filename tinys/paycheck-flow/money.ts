import { moneyLabel } from "../_shared/format";
export const currencies = ["USD", "EUR", "GBP", "JPY"] as const;
export const tones = ["sky", "leaf", "sun", "rose", "violet", "slate"] as const;
export type Currency = (typeof currencies)[number];
export type Tone = (typeof tones)[number];
export const maxSources = 4;
export const maxBuckets = 7;

/** Digits after the decimal point in each currency's minor unit. */
const minorDigits: Record<Currency, number> = { USD: 2, EUR: 2, GBP: 2, JPY: 0 };
export const minorScale = (currency: Currency): number => 10 ** minorDigits[currency];

/** Amounts are stored as integer minor units; labels show whole major units. */
export const money = (minor: number, currency: Currency): string =>
  moneyLabel(minor / minorScale(currency), currency, 0);

/** The major-unit number shown in an amount field. */
export const toMajor = (minor: number, currency: Currency): number => minor / minorScale(currency);

/** What someone typed in an amount field, as minor units; blank, invalid or negative becomes 0. */
export const parseMajor = (value: string, currency: Currency): number => {
  const parsed = parseFloat(value);
  return Number.isFinite(parsed) ? Math.max(0, Math.round(parsed * minorScale(currency))) : 0;
};

export const share = (part: number, whole: number): string => (whole > 0 ? `${Math.round((part / whole) * 100)}%` : "0%");
