// Guards authored titles escaping the preview title/iframe attribute without injecting elements.
import { expect, test } from "bun:test";
import { previewFrame } from "../../src/cli/preview";
import { capabilityFeatures, capabilityPermissionsPolicy } from "../../src/schema/policy";

const window = {
  width: 480, height: 620, fullscreenable: false,
  frame: { kind: "standard", frame: {
    fullscreenFit: false, resizable: true, lockAspect: false, background: null, radius: "22px",
    shape: { kind: "radii", horizontal: Array(4).fill({ value: 22, percent: false }), vertical: Array(4).fill({ value: 22, percent: false }) },
  } },
} as const;

test("preview titles cannot inject executable markup or iframe attributes", async () => {
  const title = '</title><script>alert(1)</script><title>" onload="alert(2)';
  const frame = previewFrame({ title, window, capabilities: [] });
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

// The preview admits devices as the native host does: only those the app declared.
test("the preview delegates only declared devices to the app frame", async () => {
  const allowOf = async (capabilities: Parameters<typeof previewFrame>[0]["capabilities"]): Promise<string | null> => {
    let allow = null as string | null;
    await new HTMLRewriter()
      .on("iframe", { element(element) { allow = element.getAttribute("allow"); } })
      .transform(new Response(previewFrame({ title: "Devices", window, capabilities })))
      .text();
    return allow;
  };
  expect(await allowOf([])).toBeNull();
  expect(capabilityPermissionsPolicy([])).toBe("camera=(), microphone=()");
  expect(await allowOf(["speech", "microphone"])).toBe("microphone");
  expect(capabilityPermissionsPolicy(["speech", "microphone"])).toBe("camera=(), microphone=(self)");
  expect(await allowOf(["microphone", "camera"])).toBe("camera; microphone");
});

test.each([
  [[], [], "camera=(), microphone=()"],
  [["speech"], [], "camera=(), microphone=()"],
  [["camera"], ["camera"], "camera=(self), microphone=()"],
  [["microphone"], ["microphone"], "camera=(), microphone=(self)"],
  [["camera", "speech"], ["camera"], "camera=(self), microphone=()"],
  [["microphone", "speech"], ["microphone"], "camera=(), microphone=(self)"],
  [["microphone", "camera"], ["camera", "microphone"], "camera=(self), microphone=(self)"],
  [
    ["speech", "microphone", "camera"],
    ["camera", "microphone"],
    "camera=(self), microphone=(self)",
  ],
] as const)("browser permission bytes for %j", (capabilities, features, header) => {
  expect(capabilityFeatures(capabilities)).toEqual([...features]);
  expect(capabilityPermissionsPolicy(capabilities)).toBe(header);
});
