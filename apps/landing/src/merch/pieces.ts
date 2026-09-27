// Merch pieces shown on /merch. Images come from merch/_tools/publish.py.
export type MerchCategory = "tops" | "headwear" | "other";
export type MerchAssetKind = "thumb" | "product" | "product-back" | "model" | "model-back" | "detail-back";
export type MerchView = { kind: MerchAssetKind; label: string; alt: string };

export type MerchPiece = {
  slug: string;
  name: string;
  file: string;
  category: MerchCategory;
  blurb: string;
  method: string;
  /** Set only after a model photo has been reviewed and published. */
  hasModel?: boolean;
  defaultView: "product" | "model";
  views?: MerchView[];
};

export const MERCH_CATEGORIES: { id: MerchCategory; label: string }[] = [
  { id: "tops", label: "Tops" },
  { id: "headwear", label: "Headwear" },
  { id: "other", label: "Other" },
];

export const MERCH_PIECES: MerchPiece[] = [
  { slug: "01-daily-driver-tee", name: "Daily Driver", file: "daily-driver.slop", category: "tops", blurb: "Washed black boxy tee with a small chest logo.", method: "Embroidered icon + wordmark", hasModel: true, defaultView: "model" },
  { slug: "02-happy-accident-tee", name: "Happy Accident", file: "happy-accident.slop", category: "tops", blurb: "Cream tee. Big back print. It works somehow.", method: "Screenprint", hasModel: true, defaultView: "model" },
  { slug: "03-full-slop-tee", name: "Full Slop", file: "full-slop.slop", category: "tops", blurb: "Long sleeve for when you have too many ideas.", method: "Multi-layer print + sleeve graphics", hasModel: true, defaultView: "model" },
  { slug: "14-tiny-apps-tee", name: "Tiny Apps", file: "tiny-apps.slop", category: "tops", blurb: "Tiny apps up front. Big personality on the back.", method: "Screenprint", hasModel: true, defaultView: "product", views: [{ kind: "product", label: "Front", alt: "Grey Tiny Apps tee, front view" }, { kind: "product-back", label: "Back", alt: "Tiny Apps tee back with Big personality. print" }] },
  { slug: "17-just-a-file-tee", name: "It’s Just a File", file: "just-a-file.slop", category: "tops", blurb: "A little sunshine. A local file. Butter-yellow heavyweight tee.", method: "Halftone screenprint", hasModel: true, defaultView: "model", views: [{ kind: "product", label: "Product", alt: "Butter-yellow tee with a blue folder in a wildflower meadow and It’s just a file. print" }, { kind: "model", label: "On model", alt: "Model wearing the It’s Just a File tee with blue jeans and cream sneakers" }] },
  { slug: "04-daily-driver-toque", name: "Daily Toque", file: "daily-toque.slop", category: "headwear", blurb: "Black rib knit with a tiny woven icon.", method: "Woven badge", hasModel: true, defaultView: "product" },
  { slug: "05-happy-accident-toque", name: "Soft Crash", file: "soft-crash.slop", category: "headwear", blurb: "Fuzzy purple knit, loud .slop across the cuff.", method: "Brushed jacquard knit + woven tag", hasModel: true, defaultView: "model" },
  { slug: "09-full-slop-cap", name: "Cursor Storm", file: "cursor-storm.slop", category: "headwear", blurb: "Black 5-panel covered in cursors.", method: "All-over embroidery", hasModel: true, defaultView: "product" },
  { slug: "13-works-offline-cap", name: "Works Offline", file: "works-offline.slop", category: "headwear", blurb: "Washed navy cap. No account required.", method: "Embroidery", hasModel: true, defaultView: "product", views: [{ kind: "product", label: "Front", alt: "Navy Works Offline cap, front view" }, { kind: "detail-back", label: "Back", alt: "Works Offline cap rear embroidery: no account required" }] },
  { slug: "08-happy-accident-cap", name: "Probably Works", file: "probably-works.slop", category: "headwear", blurb: "Washed purple cap. Quietly optimistic.", method: "Front + side embroidery", hasModel: true, defaultView: "product" },
  { slug: "10-daily-driver-bag", name: "Daily Tote", file: "daily-tote.slop", category: "other", blurb: "Heavy canvas tote. Carries the essentials.", method: "Screenprint", hasModel: true, defaultView: "product" },
  { slug: "11-happy-accident-bag", name: "Keep Changes", file: "keep-changes.slop", category: "other", blurb: "Canvas tote with a very important question.", method: "Layered screenprint", hasModel: true, defaultView: "model" },
  { slug: "15-cursor-clicker", name: "Cursor Clicker", file: "cursor-clicker.slop", category: "other", blurb: "A little click for your pocket.", method: "Molded shell + mechanical switch", defaultView: "product" },
  { slug: "16-ok-again", name: "OK, Again", file: "ok-again.slop", category: "other", blurb: "One more click. Just to be sure.", method: "Translucent case + mechanical key", defaultView: "product" },
];

export const merchAsset = (slug: string, kind: MerchAssetKind) => `/assets/merch/${slug}/${kind}.webp`;
