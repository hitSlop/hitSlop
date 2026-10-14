export const orbs = [
  { value: 1, color: "var(--slop-orb1)", label: "Heavy / Reflective" },
  { value: 2, color: "var(--slop-orb2)", label: "Low / Subdued" },
  { value: 3, color: "var(--slop-orb3)", label: "Steady / Centered" },
  { value: 4, color: "var(--slop-orb4)", label: "Warm / Serene" },
  { value: 5, color: "var(--slop-orb5)", label: "Radiant / Energized" },
];

export function orbColor(value: number) {
  return orbs.find((orb) => orb.value === value)?.color ?? "var(--slop-orb3)";
}
