import { input, resolvePromptIO, type PromptIO } from "@crustjs/prompts";
import type { Category as SlopCategory, AppMetadata } from "../wire/app.generated";
import { execute } from "./engine";
import { ManifestText } from "../schema/constants";
import { cp, lstat, mkdir, readdir, readFile, rm, writeFile, symlink } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import metadata from "../../package.json";
import { cliRoot } from "./paths";
import { projectSlug } from "./build";

interface InitOptions {
  brief?: string;
  title?: string;
  category?: SlopCategory[];
  author?: string;
  description?: string;
  yes?: boolean;
}


/** The starter's `slop.ts` with one top-level field's line replaced. */
function setField(slop: string, key: string, value: string) {
  const line = new RegExp(`^  ${key}: .*,$`, "m");
  if (!line.test(slop)) throw new Error(`The starter's slop.ts has no ${key} line`);
  return slop.replace(line, () => `  ${key}: ${value},`);
}

export async function initProject(target: string, options: InitOptions = {}, streams?: PromptIO) {
  const destination = resolve(target);
  const existing = await lstat(destination).catch((error) => {
    if (error.code !== "ENOENT") throw error;
  });
  if (existing) throw new Error("Choose a new source directory");
  const slug = projectSlug(destination);
  const candidate: AppMetadata = { slug, title: "My slop", description: "A hitSlop mini app.", author: { name: "Anonymous" }, categories: ["productivity"] };
  const checkMetadata = async (value: AppMetadata) => { await execute({ method: "validateMetadata", metadata: value }); };
  await checkMetadata(candidate);
  const io = resolvePromptIO(streams);
  const interactive = !options.yes && !process.env.CI && io.input.isTTY && io.output.isTTY;

  async function text(
    message: string,
    supplied: string | undefined,
    fallback: string,
    field: "title" | "description" | "author",
    limits: { minLength: number; maxLength: number; pattern?: string },
    prompt = interactive,
  ) {
    const check = async (value: string) => {
      try {
        await checkMetadata({ ...candidate, [field]: field === "author" ? { name: value } : value });
      } catch {
        throw new Error(
          `${message} must be ${limits.minLength}–${limits.maxLength} characters${limits.pattern ? " and contain non-whitespace text" : ""}`,
        );
      }
    };
    if (supplied !== undefined || !prompt) {
      const value = supplied ?? fallback;
      await check(value);
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
    "title",
    ManifestText.title,
    false,
  );
  const categories: SlopCategory[] = options.category ?? ["productivity"];
  try {
    await checkMetadata({ ...candidate, categories });
  } catch {
    throw new Error("Choose one or two distinct categories");
  }
  const author = await text("Author", options.author, "Anonymous", "author", ManifestText.authorName);
  const description = await text(
    "Description",
    options.description,
    "A hitSlop mini app.",
    "description",
    ManifestText.description,
    false,
  );
  let slop = await readFile(join(cliRoot, "templates/checklist/slop.ts"), "utf8");
  slop = setField(slop, "slug", JSON.stringify(slug));
  slop = setField(slop, "title", JSON.stringify(title));
  slop = setField(slop, "description", JSON.stringify(description));
  slop = setField(slop, "author", `{ name: ${JSON.stringify(author)} }`);
  slop = setField(slop, "categories", JSON.stringify(categories));
  const project = JSON.parse(
    await readFile(join(cliRoot, "templates/checklist/package.json"), "utf8"),
  );
  project.name = slug;
  project.devDependencies = { "hitslop": metadata.version };
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
    await writeFile(join(destination, "slop.ts"), slop);
    await writeFile(
      join(destination, "BRIEF.md"),
      `# Build brief\n\n${brief.trim() || description}\n`,
    );
    await mkdir(join(destination, ".agents/skills"), { recursive: true });
    for (const name of await readdir(join(cliRoot, "skills")))
      await symlink(`../../node_modules/hitslop/skills/${name}`, join(destination, ".agents/skills", name));
    await writeFile(
      join(destination, "AGENTS.md"),
      "Run bun install, then read slop.ts, BRIEF.md, .agents/skills/hitslop-authoring/SKILL.md, and .agents/skills/hitslop-design/SKILL.md. Build the slop described in BRIEF.md. Update the title, description, and categories in slop.ts to match what you build. Choose a visual direction suited to its purpose. Use plain CSS, slop.ts theme colors, and typed document handles. Run bun run check and bun run build.\n\nThe CLI, SDK and project guides come from the pinned hitslop package. Project skill links follow node_modules/hitslop when dependencies are upgraded. Use the project's bun run scripts for authoring.\n",
    );
  } catch (error) {
    await rm(destination, { recursive: true, force: true });
    throw error;
  }
  return destination;
}
