// Guards persisted token overrides and failed writes. Value rules live in the native owner.
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

test("theme definitions refuse invalid token names", () => {
  expect(() => defineTheme({ "invalid token": "red" })).toThrow();
});
