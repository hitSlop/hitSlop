import type { ForecastDay } from "./fsrs";
export function capInScale(days: readonly ForecastDay[], cap: number) {
  return cap <= Math.max(4, ...days.map(day => day.reviews)) * 1.6;
}
