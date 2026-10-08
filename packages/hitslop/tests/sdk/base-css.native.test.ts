import { expect, test } from "bun:test";
import { webkit } from "playwright";

test("optional base CSS preserves author styles, keyboard focus and reduced-motion completion", async () => {
  const css = await Bun.file(new URL("../../src/sdk/base.css", import.meta.url)).text();
  const browser = await webkit.launch();
  try {
    const page = await browser.newPage();
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.setContent(`<style>${css}</style><style>
      body { color: rgb(12, 34, 56); font: 19px serif; }
      .authored { box-sizing: content-box; color: rgb(65, 43, 21); }
      #pseudo::before { content: "Generated content"; }
      @keyframes arrive { from { opacity: 0; } to { opacity: 1; } }
      .moving { animation-name: arrive; }
    </style><button>Enabled</button><button disabled>Disabled</button><input class="authored"><div id="pseudo"></div><div id="animation"></div>`);
    await page.keyboard.press("Tab");
    const values = await page.evaluate(() => {
      const enabled = document.querySelector("button")!;
      const disabled = document.querySelector("button:disabled")!;
      const authored = document.querySelector("input")!;
      return {
        focused: document.activeElement === enabled,
        outline: getComputedStyle(enabled).outlineStyle,
        font: getComputedStyle(enabled).fontSize,
        cursor: getComputedStyle(enabled).cursor,
        disabledCursor: getComputedStyle(disabled).cursor,
        box: getComputedStyle(authored).boxSizing,
        color: getComputedStyle(authored).color,
        pseudoBox: getComputedStyle(document.querySelector("#pseudo")!, "::before").boxSizing,
      };
    });
    expect(values).toMatchObject({ focused: true, outline: "solid", font: "19px", cursor: "pointer", box: "content-box", color: "rgb(65, 43, 21)", pseudoBox: "border-box" });
    expect(values.disabledCursor).not.toBe("pointer");
    const duration = await page.evaluate(() => new Promise<number>(resolve => {
      const element = document.querySelector("#animation")!;
      element.addEventListener("animationend", () => resolve(parseFloat(getComputedStyle(element).animationDuration)), { once: true });
      element.className = "moving";
    }));
    expect(duration).toBeGreaterThan(0);
    expect(duration).toBeLessThan(0.001);
  } finally {
    await browser.close();
  }
});
