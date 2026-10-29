import { input, resolvePromptIO, type PromptIO } from "@crustjs/prompts";
import type { Category as TinyCategory, AppMetadata } from "../wire/app.generated";
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
  category?: TinyCategory[];
  author?: string;
  description?: string;
  yes?: boolean;
}

/** The starter's `tiny.ts` with one top-level field's line replaced. */
function setField(tiny: string, key: string, value: string) {
  const line = new RegExp(`^  ${key}: .*,$`, "m");
  if (!line.test(tiny)) throw new Error(`The starter's tiny.ts has no ${key} line`);
  return tiny.replace(line, () => `  ${key}: ${value},`);
}

export async function initProject(target: string, options: InitOptions = {}, streams?: PromptIO) {
  const destination = resolve(target);
  const existing = await lstat(destination).catch((error) => {
    if (error.code !== "ENOENT") throw error;
  });
  if (existing) throw new Error("Choose a new source directory");
  const slug = projectSlug(destination);
  const candidate: AppMetadata = {
    slug,
    title: "My tiny",
    description: "A tinys mini app.",
    author: { name: "Anonymous" },
    categories: ["productivity"],
  };
  const checkMetadata = async (value: AppMetadata) => {
    await execute({ method: "validateMetadata", metadata: value });
  };
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
        await checkMetadata({
          ...candidate,
          [field]: field === "author" ? { name: value } : value,
        });
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
            message: "What should your tiny do?",
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
    basename(destination).slice(0, ManifestText.title.maxLength).trim() || "My tiny",
    "title",
    ManifestText.title,
    false,
  );
  const categories: TinyCategory[] = options.category ?? ["productivity"];
  try {
    await checkMetadata({ ...candidate, categories });
  } catch {
    throw new Error("Choose one or two distinct categories");
  }
  const author = await text(
    "Author",
    options.author,
    "Anonymous",
    "author",
    ManifestText.authorName,
  );
  const description = await text(
    "Description",
    options.description,
    "A tinys mini app.",
    "description",
    ManifestText.description,
    false,
  );
  let tiny = await readFile(join(cliRoot, "templates/checklist/tiny.ts"), "utf8");
  tiny = setField(tiny, "slug", JSON.stringify(slug));
  tiny = setField(tiny, "title", JSON.stringify(title));
  tiny = setField(tiny, "description", JSON.stringify(description));
  tiny = setField(tiny, "author", `{ name: ${JSON.stringify(author)} }`);
  tiny = setField(tiny, "categories", JSON.stringify(categories));
  const project: {
    name: string;
    devDependencies: Record<string, string>;
    scripts?: Record<string, string>;
  } = JSON.parse(await readFile(join(cliRoot, "templates/checklist/package.json"), "utf8"));
  project.name = slug;
  // Beside this CLI, the starter pins the checker and TypeScript that `tiny check` runs.
  project.devDependencies["@hitslop/tinys"] = metadata.version;
  project.scripts = {
    dev: "tiny dev .",
    check: "tiny check .",
    build: "tiny build .",
    register: "tiny register .",
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
    await writeFile(join(destination, "tiny.ts"), tiny);
    await writeFile(
      join(destination, "BRIEF.md"),
      `# Build brief\n\n${brief.trim() || description}\n`,
    );
    await mkdir(join(destination, ".agents/skills"), { recursive: true });
    for (const name of await readdir(join(cliRoot, "skills")))
      await symlink(
        `../../node_modules/@hitslop/tinys/skills/${name}`,
        join(destination, ".agents/skills", name),
      );
    await writeFile(
      join(destination, "AGENTS.md"),
      "Run bun install, then read tiny.ts, BRIEF.md, .agents/skills/tinys-authoring/SKILL.md, and .agents/skills/tinys-design/SKILL.md. Build the tiny described in BRIEF.md. Update the title, description, and categories in tiny.ts to match what you build. Choose a visual direction suited to its purpose. Use plain CSS, tiny.ts theme colors, and typed document handles. For mobile support, follow the Mobile editors guidance in tinys-design: reserve the native actions corner, pad the root by --tiny-safe-top and --tiny-safe-bottom (the editor runs edge to edge), and review the iPhone preset and native keyboard. Run bun run check and bun run build.\n\n**Do not create an automated test or test suite for each tiny.** Creating, styling, animating, or updating a tiny does not require new tests. Use existing checks/builds and hands-on preview/export review. Shared SDK, storage, or host regressions belong in existing tests at their owning boundary; do not duplicate that coverage in example-specific tests.\n\nThe CLI, SDK and project guides come from the pinned tinys package. Project skill links follow node_modules/@hitslop/tinys when dependencies are upgraded. Use the project's bun run scripts for authoring.\n",
    );
  } catch (error) {
    await rm(destination, { recursive: true, force: true });
    throw error;
  }
  return destination;
}
