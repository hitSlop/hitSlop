import { test, expect } from "bun:test";
import { src, thumbnail } from "../../src/sdk/app/youtube";

test("the thumbnail is the video's image on YouTube's image host", () => {
  expect(thumbnail("dQw4w9WgXcQ")).toBe("https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg");
});

test("the relay URL carries the video and options in the fragment", () => {
  expect(src("dQw4w9WgXcQ")).toBe("https://hitslop.com/embed/youtube.html#v=dQw4w9WgXcQ");
  expect(src("dQw4w9WgXcQ", { controls: true, autoplay: false, captions: true, start: 30 })).toBe(
    "https://hitslop.com/embed/youtube.html#v=dQw4w9WgXcQ&controls=1&autoplay=0&cc=1&start=30",
  );
  expect(src("dQw4w9WgXcQ", { relay: "https://example.com/relay.html", start: -4 })).toBe(
    "https://example.com/relay.html#v=dQw4w9WgXcQ",
  );
  expect(() => src("not an id")).toThrow();
});
