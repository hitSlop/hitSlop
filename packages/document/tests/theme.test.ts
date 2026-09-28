// Guards persisted token overrides, failed writes and invalid values.
import { test, expect } from "bun:test";
import { ThemeController } from "../src/theme-runtime";
import { defineTheme } from "../src/theme";

test("theme overrides persist before application and failed writes preserve the visible theme", async () => {
  const defaults = defineTheme({ accent: "red", paper: "white" });
  let persisted = {},
    visible = {},
    fail = false;
  const theme = new ThemeController(
    defaults.defaults,
    async (values) => {
      if (fail) throw new Error("disk full");
      persisted = structuredClone(values);
    },
    (values) => {
      visible = values;
    },
  );
  theme.load({});
  await theme.set({ accent: "blue" });
  expect(persisted).toEqual({ accent: "blue" });
  expect(visible).toEqual({ accent: "blue", paper: "white" });
  fail = true;
  await expect(theme.set({ accent: "green" })).rejects.toThrow("disk full");
  expect(theme.get().overrides).toEqual({ accent: "blue" });
  expect(visible).toEqual({ accent: "blue", paper: "white" });
  fail = false;
  const reopened = new ThemeController(defaults.defaults, async () => {});
  reopened.load(persisted);
  expect(reopened.get()).toEqual(theme.get());
  await theme.reset("accent");
  expect(visible).toEqual(defaults.defaults);
  await theme.set({ accent: "blue", paper: "black" });
  await theme.reset();
  expect(persisted).toEqual({});
});

test("theme rejects undeclared tokens, structural CSS and oversized values without saving", async () => {
  let saves = 0;
  const theme = new ThemeController({ accent: "red" }, async () => {
    saves++;
  });
  for (const values of [
    { missing: "blue" },
    { accent: "red;display:none" },
    { accent: "}" },
    { accent: "x".repeat(4097) },
    { accent: "var(--slop-unknown)" },
  ] as Record<string, string>[]) {
    await expect(theme.set(values)).rejects.toThrow();
  }
  await expect(theme.reset("missing")).rejects.toThrow();
  expect(saves).toBe(0);
  expect(() => defineTheme({ "invalid token": "red" })).toThrow();
});
