import { beforeAll, expect, test } from "bun:test";
import { webkit, type Locator, type Page } from "playwright";

let presentation: string;
beforeAll(async () => {
  const build = await Bun.build({
    entrypoints: [new URL("../../src/shell/presentation.ts", import.meta.url).pathname],
    target: "browser",
  });
  if (!build.success) throw new AggregateError(build.logs, "Could not build presentation defaults");
  presentation = await build.outputs[0]!.text();
});

// Use pointer selection: programmatic Range selection can bypass user-select.
async function dragText(page: Page, target: Locator): Promise<string> {
  await target.scrollIntoViewIfNeeded();
  await page.evaluate(() => {
    (document.activeElement as HTMLElement)?.blur();
    getSelection()?.removeAllRanges();
  });
  const box = await target.boundingBox();
  if (!box) throw new Error("Selection target is not visible");
  await page.mouse.move(box.x + 1, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 1, box.y + box.height / 2, { steps: 10 });
  await page.mouse.up();
  return page.evaluate(() => getSelection()?.toString() ?? "");
}

for (const mode of ["standard", "transparent", "glass", "skin"] as const) {
  test(`${mode} UI defaults to no selection while editing, author overrides and captures remain selectable`, async () => {
    const browser = await webkit.launch();
    try {
      const page = await browser.newPage();
      await page.setContent(`
        <!doctype html>
        <style>
          body { font: 20px monospace; }
          p { margin: 12px; }
          input, textarea { display: block; font: inherit; }
          aside { position: fixed; top: 12px; left: 450px; }
          .copyable { -webkit-user-select: text; user-select: text; }
        </style>
        <main data-hitslop-root>
          <p><span id="label">Ordinary label</span></p>
          <button id="button">Ordinary button</button>
          <input aria-label="Input" value="Editable input">
          <input aria-label="Readonly input" readonly value="Readonly input">
          <textarea aria-label="Textarea">Editable textarea</textarea>
          <textarea aria-label="Readonly textarea" readonly>Readonly textarea</textarea>
          <div contenteditable="true"><p><span id="editable">Editable content</span></p></div>
          <div contenteditable><p><span id="empty-editable">Empty attribute</span></p></div>
          <div contenteditable="plaintext-only"><p><span id="plain-editable">Plain text editor</span></p></div>
          <section class="copyable"><p><span id="content">Useful output</span></p></section>
        </main>
        <aside>
          <p><span id="portal-label">Portal label</span></p>
          <p class="copyable"><span id="portal-content">Portal output</span></p>
        </aside>
      `);
      await page.evaluate(async ({ source, mode }) => {
        const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
        try {
          const { installPresentationStage } = await import(url);
          installPresentationStage({ mode, width: 800, height: 600, resizable: true });
        } finally {
          URL.revokeObjectURL(url);
        }
      }, { source: presentation, mode });

      for (const id of ["label", "button", "portal-label"])
        expect(await dragText(page, page.locator(`#${id}`)), id).toBe("");
      for (const id of ["content", "portal-content", "editable", "empty-editable", "plain-editable"]) {
        const target = page.locator(`#${id}`);
        expect(await dragText(page, target), id).toBe((await target.textContent())!);
      }
      for (const label of ["Input", "Readonly input", "Textarea", "Readonly textarea"]) {
        const field = page.getByRole("textbox", { name: label, exact: true });
        await field.dblclick();
        const selectedText = () => field.evaluate((element) => {
          const input = element as HTMLInputElement | HTMLTextAreaElement;
          return input.value.slice(input.selectionStart!, input.selectionEnd!);
        });
        expect(await selectedText(), `${label}: pointer selection`).not.toBe("");
        await field.press("ControlOrMeta+a");
        expect(await selectedText(), `${label}: keyboard selection`).toBe(await field.inputValue());
        if (!label.startsWith("Readonly")) {
          await field.press("R");
          expect(await field.inputValue()).toBe("R");
        }
      }
      const editable = page.locator('[contenteditable="true"]');
      await editable.click();
      await editable.press("ControlOrMeta+a");
      expect(await page.evaluate(() => getSelection()?.toString())).toBe("Editable content");
      await editable.press("R");
      expect(await editable.textContent()).toBe("R");

      // Capture suspends the editor default; returning to the editor reinstates it.
      await page.evaluate(() => document.documentElement.setAttribute("data-slop-capture", "static"));
      expect(await dragText(page, page.locator("#label"))).toBe("Ordinary label");
      await page.evaluate(() => document.documentElement.removeAttribute("data-slop-capture"));
      expect(await dragText(page, page.locator("#label"))).toBe("");

      // A whole-app override also covers portal content outside the app root.
      await page.locator("style").first().evaluate((style) => {
        style.textContent += "body { -webkit-user-select: text; user-select: text; }";
      });
      expect(await dragText(page, page.locator("#label"))).toBe("Ordinary label");
      expect(await dragText(page, page.locator("#portal-label"))).toBe("Portal label");
    } finally {
      await browser.close();
    }
  }, 30000);
}

// A glass window's page background tints the frost; transparent and skin windows show
// what the host draws behind the page instead.
test("glass keeps an authored page background that transparent and skin windows clear", async () => {
  const browser = await webkit.launch();
  try {
    const page = await browser.newPage();
    const backgrounds: Record<string, string> = {};
    for (const mode of ["standard", "transparent", "glass", "skin"] as const) {
      await page.setContent(
        `<!doctype html><style>body { background: rgba(10, 20, 30, 0.5); }</style><main data-hitslop-root></main>`,
      );
      backgrounds[mode] = await page.evaluate(async ({ source, mode }) => {
        const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
        try {
          const { installPresentationStage } = await import(url);
          installPresentationStage({ mode, width: 800, height: 600, resizable: true });
        } finally {
          URL.revokeObjectURL(url);
        }
        return getComputedStyle(document.body).backgroundColor;
      }, { source: presentation, mode });
    }
    expect(backgrounds).toEqual({
      standard: "rgba(10, 20, 30, 0.5)",
      transparent: "rgba(0, 0, 0, 0)",
      glass: "rgba(10, 20, 30, 0.5)",
      skin: "rgba(0, 0, 0, 0)",
    });
  } finally {
    await browser.close();
  }
}, 30000);
