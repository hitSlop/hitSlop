export const percentOf = (current: number, target: number) =>
  target > 0 ? Math.min(100, Math.round((current / target) * 100)) : 0;
