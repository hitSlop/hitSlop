export type Weighted = { weight: number };

export function integerWeight(value: number): number {
  return Number.isFinite(value) ? Math.round(value) : 0;
}

export function clampWeight(value: number): number {
  return Math.max(1, Math.min(5, integerWeight(value) || 3));
}

export function totalWeight(items: readonly Weighted[]): number {
  return items.reduce((sum, item) => sum + clampWeight(item.weight), 0);
}

/** Beam rotation in degrees. Heavier For (left) drops counterclockwise. */
export function beamTilt(proTotal: number, conTotal: number): number {
  const total = proTotal + conTotal;
  if (total === 0) return 0;
  return Math.max(-16, Math.min(16, ((conTotal - proTotal) / total) * 32));
}
