// Fixture generation (scripts/lib/native-fixtures.ts) replaces this source metadata, never document
// state or host geometry.
export default {
  slug: "shape-lab",
  kind: "rounded",
  name: "Rounded baseline",
  title: "Shape Lab",
  window: { kind: "standard" as const, width: 480, height: 360 },
  skin: false,
  expectation: "Corners clip; the centre receives input.",
};
