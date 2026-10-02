import { input, resolvePromptIO, type PromptIO } from "@crustjs/prompts";
import { SlopManifestSchema, parseManifest, type SlopCategory } from "@hitslop/schema";
import { validate } from "@hitslop/schema/validation";
import { ManifestText } from "@hitslop/schema/constants";
import { cp, lstat, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import metadata from "../package.json";

export interface InitOptions {
  brief?: string;
  title?: string;
  slug?: string;
  category?: SlopCategory[];
  author?: string;
  description?: string;
  yes?: boolean;
}

const cliRoot = fileURLToPath(new URL("../", import.meta.url));
const fields = SlopManifestSchema.properties;

export async function initProject(target: string, options: InitOptions = {}, streams?: PromptIO) {
  const destination = resolve(target);
  const existing = await lstat(destination).catch((error) => {
    if (error.code !== "ENOENT") throw error;
  });
  if (existing) throw new Error("Choose a new source directory");
  const template = parseManifest(
    JSON.parse(await readFile(join(cliRoot, "templates/checklist/manifest.json"), "utf8")),
  );
  const io = resolvePromptIO(streams);
  const interactive = !options.yes && !process.env.CI && io.input.isTTY && io.output.isTTY;
  let slug =
    basename(destination)
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, ManifestText.slug.maxLength)
      .replace(/-+$/g, "") || "my-slop";
  if (slug.length === 1) slug += "-slop";
  slug = options.slug ?? slug;
  try {
    validate(fields.slug, slug);
  } catch {
    throw new Error("Slug must be 2–64 lowercase letters or digits, separated by single hyphens");
  }

  async function text(
    message: string,
    supplied: string | undefined,
    fallback: string,
    schema: typeof fields.title,
    limits: { minLength: number; maxLength: number; pattern?: string },
    prompt = interactive,
  ) {
    const check = (value: string) => {
      try {
        validate(schema, value);
      } catch {
        throw new Error(
          `${message} must be ${limits.minLength}–${limits.maxLength} characters${limits.pattern ? " and contain non-whitespace text" : ""}`,
        );
      }
    };
    if (supplied !== undefined || !prompt) {
      const value = supplied ?? fallback;
      check(value);
      return value;
    }
    return input({ message, default: fallback, validate: check }, io);
  }

  const brief =
    options.brief ??
    (interactive
      ? await input(
          {
            message: "What should your slop do?",
            validate(value) {
              if (!value.trim()) throw new Error("Describe what you want the agent to build");
            },
          },
          io,
        )
      : "");
  // Title, categories, and description are placeholders the building agent
  // replaces to match the brief; only explicit flags override them here.
  const title = await text(
    "Title",
    options.title,
    basename(destination).slice(0, ManifestText.title.maxLength).trim() || "My slop",
    fields.title,
    ManifestText.title,
    false,
  );
  const categories: SlopCategory[] = options.category ?? ["productivity"];
  try {
    validate(fields.categories, categories);
  } catch {
    throw new Error("Choose one or two distinct manifest categories");
  }
  const author = await text("Author", options.author, "Anonymous", fields.author.properties.name, ManifestText.authorName);
  const description = await text(
    "Description",
    options.description,
    "A hitSlop mini app.",
    fields.description,
    ManifestText.description,
    false,
  );
  const manifest = parseManifest({
    ...template,
    slug,
    title,
    categories,
    author: { name: author },
    description,
  });
  const project = JSON.parse(
    await readFile(join(cliRoot, "templates/checklist/package.json"), "utf8"),
  );
  project.name = slug;
  project.dependencies["@hitslop/document"] = metadata.dependencies["@hitslop/document"];
  project.devDependencies = { "@hitslop/cli": metadata.version };
  project.scripts = {
    dev: "slop dev .",
    check: "slop check .",
    build: "slop build .",
    register: "slop register .",
  };

  // Collect and validate everything before writing. Exclusive mkdir claims only
  // our new directory; a racing creator is never overwritten or cleaned up.
  await mkdir(dirname(destination), { recursive: true });
  await mkdir(destination);
  try {
    const starter = join(cliRoot, "templates/checklist");
    for (const file of await readdir(starter))
      await cp(join(starter, file), join(destination, file), {
        recursive: true,
        errorOnExist: true,
        force: false,
      });
    await writeFile(join(destination, "package.json"), JSON.stringify(project, null, 2) + "\n");
    await writeFile(join(destination, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
    await writeFile(
      join(destination, "BRIEF.md"),
      `# Build brief\n\n${brief.trim() || description}\n`,
    );
    await cp(join(cliRoot, "skills"), join(destination, ".agents/skills"), { recursive: true });
    await writeFile(
      join(destination, "AGENTS.md"),
      "Read manifest.json, BRIEF.md, .agents/skills/hitslop-authoring/SKILL.md, and .agents/skills/hitslop-design/SKILL.md first. Build the slop described in BRIEF.md. Update manifest.json's title, description, and categories to match what you build; the user can edit them later. Choose a visual direction suited to its purpose; the checklist is a functional starting point whose layout and appearance should be adapted to the task. Use plain CSS, defineTheme colors (only colors a person may change; fonts in CSS), and typed document handles. Install dependencies with bun install, then run bun run check and bun run build.\n\nThe project includes portable copies of its agent guides. Installed agent skills update with the global CLI; these copies do not, so review guide changes when you upgrade the project's @hitslop/cli and @hitslop/document versions. CLI and SDK versions may differ; use the SDK version declared by the CLI's @hitslop/document dependency.\n",
    );
  } catch (error) {
    await rm(destination, { recursive: true, force: true });
    throw error;
  }
  return destination;
}
