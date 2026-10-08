// Generates the landing page's template wall data from the built templates. Run from the
// repository root after bun run build:templates:
//   bun apps/landing/scripts/templates.ts
// Outputs are committed: the Cloudflare build installs only apps/landing and never
// builds templates. Metadata and stored artwork are read through the Rust engine.
/// <reference types="bun" />
import { execute } from "../../../packages/hitslop/src/cli/engine";
import { mkdir, mkdtemp, readdir, rename, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { repository } from "../../../scripts/lib/artifacts";
import { builtTemplates } from "../../../scripts/templates/discover";

const landing = fileURLToPath(new URL("..", import.meta.url));
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

const pick = (colors: Record<string, string>, keys: string[], fallback: string) =>
  keys.map((key) => colors[key]).find(Boolean) ?? fallback;
// Relative luminance decides whether the tile label is dark or light.
function luminance(hex: string): number {
  const full = hex.length === 4 ? hex.replace(/^#(.)(.)(.)$/, "#$1$1$2$2$3$3") : hex.slice(0, 7);
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(full.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
}

export async function generateTemplates(sources: { slug: string; file: string }[], destination = landing) {
  const publicDir = join(destination, "public/assets/templates");
  const output = join(destination, "src/data/templates.json");
  await mkdir(destination, { recursive: true });
  const stage = await mkdtemp(join(destination, ".templates-"));
  try {
    const templates: Template[] = [];
    const missingPreview: string[] = [];
    for (const { slug, file } of sources) {
      // Developer acceptance workload; never advertise it as a product template.
      if (slug === "shape-lab") continue;
      const { info } = await execute({ method: "inspect", file });
      if (info.metadata.slug !== slug) throw new Error(`Template slug mismatch: ${slug}`);
      const { metadata: manifest, defaults: colors, window: presentation } = info;
      const background = pick(colors, backgroundKeys, "#f4efff");
      const accent = pick(colors, accentKeys, "#6c16ed");
      let ink = pick(colors, inkKeys, luminance(background) > 0.35 ? "#10132c" : "#ffffff");
      // Guard against a theme whose "ink" is meant for a different panel.
      if (Math.abs(luminance(ink) - luminance(background)) < 0.3) ink = luminance(background) > 0.35 ? "#10132c" : "#ffffff";
      const template: Template = {
        slug,
        title: manifest.title,
        description: manifest.description,
        categories: manifest.categories,
        shape: presentation.kind === "standard" && presentation.shape === "50%" ? "ellipse" :
          presentation.kind === "skin" || (presentation.kind === "standard" && typeof presentation.shape === "object") ? "rectangle" : "rounded",
        width: presentation.width,
        height: presentation.height,
        emoji: emoji[slug] ?? "✨",
        colors: { background, accent, ink, ...tileOverrides[slug] },
      };
      for (const key of ["icon", "preview"] as const) {
        const to = join(stage, slug, `${key}.png`);
        await mkdir(dirname(to), { recursive: true });
        const { output: image } = await execute({ method: "artwork.export", file, target: key, output: to });
        if (!image) continue;
        template[key] = `/assets/templates/${slug}/${key}.png`;
      }
      if (!template.preview) missingPreview.push(slug);
      templates.push(template);
    }
    templates.sort((a, b) => a.title.localeCompare(b.title));
    await writeFile(join(stage, "templates.json"), JSON.stringify(templates, null, 2) + "\n");
    // All reads and generation succeeded. Publish assets before the JSON that refers to them.
    for (const template of templates) {
      for (const role of ["icon", "preview"] as const) {
        if (!template[role]) continue;
        const to = join(publicDir, template.slug, `${role}.png`);
        await mkdir(dirname(to), { recursive: true });
        await rename(join(stage, template.slug, `${role}.png`), to);
      }
    }
    await mkdir(dirname(output), { recursive: true });
    await rename(join(stage, "templates.json"), output);
    // Only remove obsolete artwork after the new inventory has been published.
    for (const entry of await readdir(publicDir, { withFileTypes: true }).catch(() => [])) {
      const template = templates.find(template => template.slug === entry.name);
      if (!template) await rm(join(publicDir, entry.name), { recursive: true, force: true });
      else for (const role of ["icon", "preview"] as const)
        if (!template[role]) await rm(join(publicDir, entry.name, `${role}.png`), { force: true });
    }
    console.log(`${templates.length} templates → ${output}`);
    if (missingPreview.length) console.log(`No template artwork (emoji tile used): ${missingPreview.length} — their builds have no preview artwork.`);
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  const { templates } = await builtTemplates();
  await generateTemplates(templates.map(({ slug }) => ({ slug, file: join(repository, "generated/templates", `${slug}.slop`) })));
}
