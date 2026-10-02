// Generates the landing page's template wall data from the real template sources.
// Run from the repository root after templates change:
//   bun apps/landing/scripts/templates.ts
// Outputs are committed: the Cloudflare build installs only apps/landing and never
// builds templates. theme.ts files are read as text, never executed.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const landing = fileURLToPath(new URL("..", import.meta.url));
const repo = resolve(landing, "../..");
const sources = ["examples/slops"].map((dir) => join(repo, dir));
const publicDir = join(landing, "public/assets/templates");
const output = join(landing, "src/data/templates.json");

// A hand-picked sticker for every template; new templates fall back to a sparkle.
const emoji: Record<string, string> = {
  "alien-radio": "👽", "ambient-sound-mixer": "🌧️", "assignment-tracker": "📖", "baby-journal": "🍼",
  "bullet-journal": "📒", "choice-point": "🧭", "codex-pet": "🐾", "contact-card": "🪪",
  "cornell-notes": "📝", "countdown-milestones": "⏳", "daily-planner": "📅", "doodle-board": "🖍️",
  "eisenhower-matrix": "📌", "expense-log": "🧾", "five-minute-journal": "💛", "flashcards": "🗂️",
  "focus-timer": "🍅", "grade-calculator": "🎓", "grocery-list": "🥑", "habit-heatmap": "🌿",
  "harada-method": "🎯", "invoice": "💸", "ivy-lee-method": "✅", "kanban-board": "📋",
  "koi-pond": "🐟", "markdown-editor": "✍️", "meeting-notes": "💬", "metronome-tapper": "🎵",
  "mood-log": "😊", "morning-pages": "🌅", "packing-list": "🧳", "personal-budget": "💰",
  "pixel-art": "🎨", "pocket-sheet": "📊", "pros-cons-sheet": "⚖️", "reading-tracker": "📚",
  "recipe": "🍴", "resume": "💼", "school-schedule": "🏫", "semester-planner": "🗓️",
  "side-quest": "🗺️", "slide-deck": "🖥️", "small-expenses": "🪙", "soma-amp": "📻",
  "subscription-tracker": "🔁", "three-three-three": "3️⃣", "trip-itinerary": "✈️", "water-tracker": "💧",
  "weekly-planner": "🧲", "wordle": "🔤", "workout-planner": "💪", "quick-checklist": "☑️",
};

// Transparent or skinned apps have no opaque surface token; use their real plate colors.
const tileOverrides: Record<string, Partial<Template["colors"]>> = {
  "alien-radio": { background: "#16190c", accent: "#caff42", ink: "#eaffb3" },
  "codex-pet": { background: "#3b2821", accent: "#ffd281", ink: "#fff7eb" },
  "koi-pond": { background: "#5f9fa3", accent: "#a9d3c6", ink: "#ffffff" },
};

// Theme tokens differ per template; take the first key that exists in each role.
const backgroundKeys = ["surface", "paper", "chassis", "shell", "frame", "wood", "water", "graphite", "field", "desk", "studio", "board", "canvas", "onSurface"];
const accentKeys = ["accent", "lime", "brass", "highlight", "primary", "margin", "line", "lilac", "surfaceLight", "letterhead", "surfaceCon", "glass", "tray", "shallow", "woodHi"];
const inkKeys = ["ink", "chassisInk", "onSurface", "glass"];

type Template = {
  slug: string;
  title: string;
  description: string;
  categories: string[];
  shape: "rounded" | "ellipse" | "rectangle";
  width: number;
  height: number;
  emoji: string;
  colors: { background: string; accent: string; ink: string };
  icon?: string;
  preview?: string;
};

function themeColors(dir: string): Record<string, string> {
  const file = join(dir, "theme.ts");
  if (!existsSync(file)) return {};
  const colors: Record<string, string> = {};
  for (const [, key, value] of readFileSync(file, "utf8").matchAll(/(\w+):\s*["'](#[0-9a-fA-F]{3,8})["']/g)) colors[key!] ??= value!;
  return colors;
}
const pick = (colors: Record<string, string>, keys: string[], fallback: string) =>
  keys.map((key) => colors[key]).find(Boolean) ?? fallback;
// Relative luminance decides whether the tile label is dark or light.
function luminance(hex: string): number {
  const full = hex.length === 4 ? hex.replace(/^#(.)(.)(.)$/, "#$1$1$2$2$3$3") : hex.slice(0, 7);
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(full.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
}

await rm(publicDir, { recursive: true, force: true });
const templates: Template[] = [];
const missingPreview: string[] = [];
for (const source of sources) {
  if (!existsSync(source)) continue;
  for (const name of readdirSync(source).sort()) {
    const dir = join(source, name);
    const manifestPath = join(dir, "manifest.json");
    if (!existsSync(manifestPath)) continue;
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    // Developer acceptance workload; never advertise it as a product template.
    if (manifest.slug === "shape-lab") continue;
    if (templates.some((t) => t.slug === manifest.slug)) continue;
    const colors = themeColors(dir);
    const background = pick(colors, backgroundKeys, "#f4efff");
    const accent = pick(colors, accentKeys, "#6c16ed");
    let ink = pick(colors, inkKeys, luminance(background) > 0.35 ? "#10132c" : "#ffffff");
    // Guard against a theme whose "ink" is meant for a different panel.
    if (Math.abs(luminance(ink) - luminance(background)) < 0.3) ink = luminance(background) > 0.35 ? "#10132c" : "#ffffff";
    const presentation = manifest.presentation ?? {};
    const template: Template = {
      slug: manifest.slug,
      title: manifest.title,
      description: manifest.description ?? "",
      categories: manifest.categories ?? [],
      shape: presentation.shape === "50%" ? "ellipse" :
        presentation.skin || typeof presentation.shape === "object" ? "rectangle" : "rounded",
      width: presentation.width ?? 480,
      height: presentation.height ?? 600,
      emoji: emoji[manifest.slug] ?? "✨",
      colors: { background, accent, ink, ...tileOverrides[manifest.slug] },
    };
    // Real QuickLook artwork, generated by the template build, when a build exists.
    const quickLook = join(dir, "dist", `${manifest.slug}.slop`, "QuickLook");
    for (const [file, key] of [["Icon.png", "icon"], ["Preview.png", "preview"]] as const) {
      const from = join(quickLook, file);
      if (!existsSync(from)) continue;
      const to = join(publicDir, manifest.slug, file.toLowerCase());
      await mkdir(dirname(to), { recursive: true });
      await cp(from, to);
      template[key] = `/assets/templates/${manifest.slug}/${file.toLowerCase()}`;
    }
    if (!template.preview) missingPreview.push(manifest.slug);
    templates.push(template);
  }
}
templates.sort((a, b) => a.title.localeCompare(b.title));
await mkdir(dirname(output), { recursive: true });
await writeFile(output, JSON.stringify(templates, null, 2) + "\n");
console.log(`${templates.length} templates → ${output}`);
if (missingPreview.length) console.log(`No QuickLook build (emoji tile used): ${missingPreview.length} — build templates to add artwork.`);
