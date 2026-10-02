// The page only applies themes: the native owner validates and saves every write.
import { test, expect } from "bun:test";
import { ThemeController } from "../src/theme-runtime";
import { defineTheme } from "../src/theme";

test("saved overrides apply over defaults without validation on load", () => {
  const defaults = defineTheme({ accent: "red", paper: "white" });
  let visible = {};
  const theme = new ThemeController(defaults.defaults, (values) => { visible = values; });
  theme.load({ accent: "blue" });
  expect(visible).toEqual({ accent: "blue", paper: "white" });
  // Loading never fails on theme; the browser ignores CSS it cannot parse.
  theme.load({ accent: "not a colour;" });
  expect(visible).toEqual({ accent: "not a colour;", paper: "white" });
  // A reset override shows its default again.
  theme.load({});
  expect(visible).toEqual({ accent: "red", paper: "white" });
});
