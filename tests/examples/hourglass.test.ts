// The hourglass example's sand and readout: what the glass shows comes from the saved
// start and end alone.
import { expect, test } from "bun:test";
import { base, neck, sandLevels } from "../../examples/slops/hourglass/glass";
import { read } from "../../examples/slops/hourglass/model";

test("sand drains from the top bulb into a rising pile", () => {
  const full = sandLevels(1);
  expect(full.peak).toBeCloseTo(base);
  expect(sandLevels(0).top).toBeCloseTo(neck);
  let previous = full;
  for (const remaining of [0.75, 0.5, 0.25, 0]) {
    const levels = sandLevels(remaining);
    expect(levels.top).toBeGreaterThan(previous.top);
    expect(levels.peak).toBeLessThan(previous.peak);
    previous = levels;
  }
});

// Failure: fallen sand spread flat across the wide bottom bulb, so two minutes of a
// 25-minute glass made a layer about a point and a half thick, hidden at the cap's edge.
test("the first minutes of fallen sand already heap into a visible pile", () => {
  expect(base - sandLevels(1 - 2 / 25).peak).toBeGreaterThan(10);
});

test("the readout counts days for a long wait and a clock under a day", () => {
  const hour = 3_600_000;
  expect(read({ title: "", start: 0, end: 0 }, 5).state).toBe("unset");
  const clock = read({ title: "", start: 0, end: hour }, 1000);
  expect(clock).toMatchObject({ state: "running", value: "59:59", unit: "to go" });
  expect(read({ title: "", start: 0, end: 30 * hour }, 0)).toMatchObject({ value: "1", unit: "day to go", remaining: 1 });
  expect(read({ title: "", start: 0, end: 300 * hour }, 24 * hour)).toMatchObject({ value: "11", unit: "days to go" });
  expect(read({ title: "", start: 0, end: hour }, hour)).toMatchObject({ state: "done", remaining: 0 });
});
