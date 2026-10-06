/** Real WebKit pointer and resize diagnostics; screenshots alone cannot establish parity. */
import { webkit } from "playwright";
import { startDev } from "../../packages/hitslop/src/cli/dev";
import { resolve } from "node:path";
import { mkdir, writeFile } from "node:fs/promises";
import { buildShapeLabVariant, shapeLabVariants, type ShapeLabVariant } from "../lib/native-fixtures";
const evidence = resolve(".hitslop/evidence/shape-lab");
await mkdir(evidence, { recursive: true });
const results: {
  kind: string;
  editorClick: boolean;
  textInput: boolean;
  cornerReceiver: boolean;
  holeReceiver: boolean | null;
  resizedHoleReceiver: boolean | null;
  aspect: boolean;
}[] = [];
const browser = await webkit.launch();
try {
  for (const kind of Object.keys(shapeLabVariants) as ShapeLabVariant[]) {
    await buildShapeLabVariant(kind);
    const dev = await startDev(resolve("generated/shape-lab/sources", kind), 5198);
    try {
      const page = await browser.newPage({ viewport: { width: 800, height: 650 } });
      await page.goto(dev.url);
      const frame = page.frameLocator("iframe");
      await frame.getByRole("heading", { name: "Shape Lab", exact: true }).waitFor();
      await frame.getByRole("button", { name: "W ←", exact: true }).click();
      await frame.getByRole("button", { name: "E →", exact: true }).click();
      await frame
        .locator(".shape-lab-readout strong")
        .filter({ hasText: "2" })
        .waitFor({ state: "attached" });
      await frame.getByRole("textbox", { name: "Capture note" }).fill("Browser input");
      const nativeFrame = page.frames().find((f) => f.url().includes("/app.html"))!;
      await nativeFrame.evaluate(async () => {
        await (globalThis as any).__slop.flush();
      });
      await page.screenshot({ path: `${evidence}/${kind}.png` });
      const receiver = async (fx: number, fy: number) => {
        const box = (await page.locator(".window").boundingBox())!;
        const x = box.x + box.width * fx,
          y = box.y + box.height * fy;
        await page.evaluate(
          ({ x, y }) => {
            document.querySelector("#receiver")?.remove();
            const button = document.createElement("button");
            button.textContent = "Receiver";
            button.id = "receiver";
            Object.assign(button.style, {
              position: "absolute",
              left: `${x - 20}px`,
              top: `${y - 20}px`,
              width: "40px",
              height: "40px",
              zIndex: "0",
            });
            button.onclick = () => {
              button.dataset.hit = "yes";
            };
            document.body.prepend(button);
          },
          { x, y },
        );
        await page.mouse.click(x, y);
        return (await page.locator("#receiver").getAttribute("data-hit")) === "yes";
      };
      const cornerReceiver = await receiver(0.002, 0.002);
      const hole =
        kind === "hole" || kind === "locked"
          ? [350 / 480, 88 / 360]
          : kind === "concave"
            ? [465 / 480, 156 / 360]
            : kind === "washer"
              ? [0.5, 0.5]
              : null;
      const holeReceiver = hole ? await receiver(hole[0]!, hole[1]!) : null;
      let aspect = true,
        resizedHoleReceiver: boolean | null = null;
      if (kind !== "washer") {
        const before = (await page.locator(".window").boundingBox())!;
        await page.getByRole("button", { name: "Resize preview", exact: true }).focus();
        await page.keyboard.press("Shift+ArrowRight");
        const after = (await page.locator(".window").boundingBox())!;
        aspect =
          Math.abs(after.width - before.width - 50) < 1 &&
          (kind === "locked"
            ? Math.abs(after.width / after.height - 4 / 3) < 0.005
            : Math.abs(after.height - before.height) < 1);
        if (hole) resizedHoleReceiver = await receiver(hole[0]!, hole[1]!);
      }
      results.push({
        kind,
        editorClick: true,
        textInput:
          (await frame.getByRole("textbox", { name: "Capture note" }).inputValue()) ===
          "Browser input",
        cornerReceiver,
        holeReceiver,
        resizedHoleReceiver,
        aspect,
      });
      await page.close();
    } finally {
      await dev.close();
    }
  }
} finally {
  await browser.close();
}
await writeFile(
  `${evidence}/browser.json`,
  JSON.stringify({ browser: browser.version(), results }, null, 2),
);
console.log(results);
if (
  results.some(
    (r) =>
      !r.cornerReceiver ||
      r.holeReceiver === false ||
      r.resizedHoleReceiver === false ||
      !r.aspect ||
      !r.textInput,
  )
) {
  console.error(
    "FAIL: pointer/resize behavior did not match the silhouette. See browser.json; pixels alone are not evidence of input routing.",
  );
  process.exitCode = 1;
}
