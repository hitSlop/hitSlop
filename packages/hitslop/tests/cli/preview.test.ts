// Guards authored titles escaping the preview title/iframe attribute without injecting elements.
import { expect, test } from "bun:test";
import { previewFrame } from "../../src/cli/preview";

test("preview titles cannot inject executable markup or iframe attributes", async () => {
  const title = '</title><script>alert(1)</script><title>" onload="alert(2)';
  const frame = previewFrame({ title, window: {
    width: 480, height: 620, fullscreenable: false,
    frame: { kind: "standard", frame: {
      fullscreenFit: false, resizable: true, lockAspect: false, background: null, radius: "22px",
      shape: { kind: "radii", horizontal: Array(4).fill({ value: 22, percent: false }), vertical: Array(4).fill({ value: 22, percent: false }) },
    } },
  } });
  const scripts: (string | null)[] = [];
  let inlineScript = "";
  let iframeTitle: string | null = null;
  let onload: string | null = null;
  const response = new HTMLRewriter()
    .on("script", {
      element(element) {
        scripts.push(element.getAttribute("src"));
      },
      text(chunk) { inlineScript += chunk.text; },
    })
    .on("iframe", {
      element(element) {
        iframeTitle = element.getAttribute("title");
        onload = element.getAttribute("onload");
      },
    })
    .transform(new Response(frame));
  await response.text();
  expect(scripts).toEqual(["/__preview__/resize.js"]);
  expect(inlineScript).toBe("");
  expect(onload).toBeNull();
  expect(iframeTitle).not.toBeNull();
});
