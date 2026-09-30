// Guards authored titles escaping the preview title/iframe attribute without injecting elements.
import { expect, test } from "bun:test";
import { previewFrame } from "../src/preview";

test("preview titles cannot inject executable markup or iframe attributes", async () => {
  const title = '</title><script>alert(1)</script><title>" onload="alert(2)';
  const frame = previewFrame({ title, presentation: { width: 480, height: 620 } });
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
