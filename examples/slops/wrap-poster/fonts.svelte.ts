import { clearCache } from "@chenglou/pretext";

export const PAGE = { width: 520, height: 700, side: 36, top: 32, bottom: 34 } as const;
export const BODY = { font: '400 17px "Poster Lora"', lineHeight: 26 } as const;
export const TITLE = { font: '700 46px "Poster Lora"', lineHeight: 52 } as const;

// Pretext measures with the real font, so nothing is laid out until the face is registered and loaded.
// (In dev the stylesheet can arrive after this module, so wait for the face to exist first.)
export const fonts = $state({ ready: false });

async function load(): Promise<void> {
  const registered = () => [...document.fonts].some((face) => face.family.replace(/["']/g, "") === "Poster Lora");
  for (let i = 0; i < 60 && !registered(); i++) await new Promise((resolve) => setTimeout(resolve, 50));
  await Promise.all([document.fonts.load(BODY.font), document.fonts.load(TITLE.font)]).catch(() => undefined);
  clearCache();
  fonts.ready = true;
}

if (typeof document !== "undefined") void load();
