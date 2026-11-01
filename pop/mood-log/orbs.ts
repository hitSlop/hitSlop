export const orbs = [
  { value: 1, color: "var(--pop-orb1)", label: "Heavy / Reflective" },
  { value: 2, color: "var(--pop-orb2)", label: "Low / Subdued" },
  { value: 3, color: "var(--pop-orb3)", label: "Steady / Centered" },
  { value: 4, color: "var(--pop-orb4)", label: "Warm / Serene" },
  { value: 5, color: "var(--pop-orb5)", label: "Radiant / Energized" },
];

export function orbColor(value: number) {
  return orbs.find((orb) => orb.value === value)?.color ?? "var(--pop-orb3)";
}
