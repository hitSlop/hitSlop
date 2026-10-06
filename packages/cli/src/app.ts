import { Crust, defineCommand } from "@crustjs/core";
import { didYouMean, help, version } from "@crustjs/extensions";
import { skill } from "@crustjs/skills";
import { ExportFormats, SlopCategories } from "@hitslop/schema/constants";
import metadata from "../package.json";

export const skillExtras = [
  "hitslop",
  "hitslop-authoring",
  "hitslop-design",
  "hitslop-document",
].map((name) => new URL(`../skills/${name}/`, import.meta.url));
export const skillName = "hitslop-cli";
export const cliVersion = metadata.version;

const source = {
  name: "source",
  type: "path",
  required: true,
  description: "Authoring source directory",
} as const;
const document = {
  name: "document",
  type: "string",
  required: true,
  description: "Path to a .slop document",
} as const;
const slopFile = {
  name: "file",
  type: "string",
  required: true,
  description: "Path to a .slop template or document",
} as const;
/** The version an agent's text sets were written against. */
const baseFlag = {
  name: "base",
  type: "string",
  description:
    "The version you read the text at (version from get --snapshot; read again before each rewrite). Text sets then change the text as it was at that version and keep edits made since, such as typing in an open window",
} as const;
const attachFlag = {
  name: "attach",
  type: "string",
  multiple: true,
  description:
    "A file the operations reference, stored in the same batch; repeat for more. slop attachments ref prints its reference. A file nothing references is removed when the document closes",
} as const;
const retrySection = {
  title: "Retries",
  body: "Mutations are never automatically replayed. After an unknown outcome, run slop get before issuing another edit. get saves and returns owner-accepted state; text still being typed in an open window is not included.",
};

/** Document commands use the Rust owner; windows and rendering use the macOS helper. Authoring (init, check, dev, build) needs neither. */
async function native(...argv: string[]) {
  await (await import("./native")).runEngine(argv);
}
/** Reads a template or a closed or open document with the CLI's own file engine, on any
 * platform, as `build` and `check` do: saved state, never an open window's unsaved edits. */
async function readSlop(command: "schema" | "inspect", path: string) {
  return (await import("./engine")).engine([command, path]);
}
const documents = () => import("./documents");

const theme = {
  get: defineCommand("get", { description: "Print the palette: template colors, overrides and effective colors" }, (c) =>
    c.args(document).action(async ({ args }) => (await documents()).themeGet(args.document)),
  ),
  set: defineCommand("set", { description: "Override declared theme colors" }, (c) =>
    c
      .args(document)
      .flags({ name: "values", type: "string", required: true, description: 'Colors as JSON, such as {"accent":"#335577"}' })
      .action(async ({ args, flags }) => (await documents()).themeSet(args.document, flags.values)),
  ),
  reset: defineCommand("reset", { description: "Return one color, or every color, to the template's" }, (c) =>
    c
      .args(document)
      .flags({ name: "token", type: "string", description: "Color to reset; omit to reset all" })
      .action(async ({ args, flags }) => (await documents()).themeReset(args.document, flags.token)),
  ),
  export: defineCommand("export", { description: "Print or write the full palette as a theme file" }, (c) =>
    c
      .args(document)
      .flags({ name: "output", type: "string", description: "File to write; omit to print" })
      .action(async ({ args, flags }) => (await documents()).themeExport(args.document, flags.output)),
  ),
  import: defineCommand("import", { description: "Replace the palette with a theme file made for this template" }, (c) =>
    c
      .args(document, { name: "file", type: "string", required: true, description: "Theme file to import" })
      .action(async ({ args }) => (await documents()).themeImport(args.document, args.file)),
  ),
};

export const app = new Crust("slop", {
  description: "Author hitSlop mini apps and work with local documents",
  version: cliVersion,
  sections: [
    {
      title: "Document workflow",
      body: "Run bunx @hitslop/cli or install globally with bun install -g @hitslop/cli. Read slop.ts first. Inspect schema before editing. Built and registered template masters are immutable: create a writable copy before editing. Native macOS commands route to the live session or acquire exclusive ownership when closed.",
    },
  ],
})
  .extend(version())
  .extend(help())
  .extend(didYouMean())
  .add(
    defineCommand(
      "attachments",
      { description: "Reference, inspect, and export document attachments" },
      (c) =>
        c
          .add(
            defineCommand("list", { description: "List attachment IDs and sizes" }, (c) =>
              c.args(document).action(async ({ args }) => (await documents()).attachmentsList(args.document)),
            ),
          )
          .add(
            defineCommand(
              "ref",
              { description: "Print a file's reference as JSON, for operations that store it with --attach" },
              (c) =>
                c
                  .args({ name: "file", type: "string", required: true })
                  .action(async ({ args }) => (await documents()).attachmentsRef(args.file)),
            ),
          )
          .add(
            defineCommand(
              "export",
              { description: "Export an attachment without overwriting an existing file" },
              (c) =>
                c
                  .args(document, { name: "id", type: "string", required: true })
                  .flags({ name: "output", type: "string", required: true })
                  .action(async ({ args, flags }) =>
                    (await documents()).attachmentsExport(args.document, args.id, flags.output),
                  ),
            ),
          ),
    ),
  )
  .add(
    defineCommand(
      "init",
      {
        description: "Create an authoring project from the checklist starter",
        sections: [
          {
            title: "Build with your agent",
            body: "Interactive setup asks what your slop should do and saves it in BRIEF.md. After creation, choose a detected agent CLI, Other CLI to enter an executable and its prompt option, or Finish without launching. The agent starts in the project with BRIEF.md, AGENTS.md, and local hitSlop guidance, using its normal permissions. Launch failure keeps the project. --yes, CI, and non-TTY runs never prompt or launch another agent.",
          },
          {
            title: "Project metadata",
            body: "Interactive terminals also ask for the author. Title, categories, and description start as the directory name, productivity, and A hitSlop mini app.; the launched agent updates them in slop.ts to match what it builds, and you can edit them there anytime. Flags set any of these explicitly; --category accepts one or two distinct values. An omitted brief uses the description. The directory's name is the slug: 2–64 lowercase letters or digits, separated by single hyphens. All metadata is validated before creating files; existing destinations are refused. Project agent guides are portable copies, not links managed by skills repair.",
          },
        ],
      },
      (c) =>
        c
          .args(source)
          .flags(
            {
              name: "brief",
              type: "string",
              description: "What the slop should do; saved in BRIEF.md for your agent",
            },
            {
              name: "title",
              type: "string",
              description: "App title (defaults to directory name)",
            },
            {
              name: "category",
              type: "string",
              multiple: true,
              choices: [...SlopCategories],
              description: "Catalog category; repeat for a second category",
            },
            { name: "author", type: "string", description: "Author name (default: Anonymous)" },
            {
              name: "description",
              type: "string",
              description: "App description (default: A hitSlop mini app.)",
            },
            {
              name: "yes",
              type: "boolean",
              description: "Skip prompts and agent launch; use defaults for missing metadata",
            },
          )
          .action(async ({ args, flags, stdout }) => {
            const destination = await (await import("./init")).initProject(args.source, flags);
            stdout(`Created ${destination}. Run bun install in that directory, then bun run dev.`);
            await (await import("./agents")).offerAgentLaunch(destination, flags.yes);
          }),
    ),
  )
  .add(
    defineCommand("check", { description: "Check Svelte and TypeScript authoring source, and slop.ts as a build does" }, (c) =>
      c.args(source).action(async ({ args }) => {
        const { createRequire } = await import("node:module");
        const require = createRequire(import.meta.url);
        const { exec } = await import("./process");
        const svelte = await exec([process.execPath, require.resolve("svelte-check/bin/svelte-check"), "--workspace", args.source], {
          inherit: ["stdout", "stderr"],
        });
        await (await import("./build")).checkProject(args.source);
        if (svelte.code) throw new Error("Authoring checks failed");
      }),
    ),
  )
  .add(
    defineCommand("dev", { description: "Serve a disposable browser preview" }, (c) =>
      c
        .args(source)
        .flags({
          name: "port",
          type: "number",
          default: 5173,
          description: "HTTP port (1–65535)",
        })
        .action(async ({ args, flags }) => {
          if (!Number.isInteger(flags.port) || flags.port < 1 || flags.port > 65535)
            throw new Error("Port must be an integer from 1 to 65535");
          await (await import("./authoring")).dev(args.source, flags.port);
        }),
    ),
  )
  .add(
    defineCommand("build", { description: "Build a runtime template on any platform" }, (c) =>
      c
        .args(source)
        .flags({
          name: "artwork",
          type: "string",
          description: "native: render Finder artwork not in artwork/ with hitSlop.app (macOS)",
        })
        .action(async ({ args, flags }) => {
          await (await import("./authoring")).build(args.source, flags.artwork);
        }),
    ),
  )
  .add(
    defineCommand(
      "register",
      { description: "Build with native artwork and register an immutable local template (macOS)" },
      (c) =>
        c.args(source).action(async ({ args }) => {
          await (await import("./authoring")).register(args.source);
        }),
    ),
  )
  .add(
    defineCommand(
      "templates",
      { description: "List the templates the app's catalog shows, bundled and installed, as JSON" },
      (c) => c.action(() => native("templates")),
    ),
  )
  .add(
    defineCommand(
      "create",
      { description: "Create a writable document from a template" },
      (c) =>
        c
          .flags(
            {
              name: "from",
              type: "string",
              required: true,
              description: "Template to copy: a slug from slop templates, or a template .slop path",
            },
            {
              name: "output",
              type: "string",
              required: true,
              description: "Path for the new writable document; .slop is added if it is missing",
            },
          )
          // As the app's save panel does: a document's name ends in .slop.
          .action(({ flags }) =>
            native("create", "--from", flags.from, "--output", flags.output.endsWith(".slop") ? flags.output : `${flags.output}.slop`),
          ),
    ),
  )
  .add(
    defineCommand("open", { description: "Open a document in the hitSlop app" }, (c) =>
      c.args(document).action(({ args }) => native("open", args.document)),
    ),
  )
  .add(
    defineCommand("schema", { description: "Print the document schema descriptor" }, (c) =>
      c.args(document).action(async ({ args }) => process.stdout.write(await readSlop("schema", args.document))),
    ),
  )
  .add(
    defineCommand(
      "inspect",
      { description: "Print what a .slop file holds: its kind, app, artwork, attachments and saved state sizes" },
      (c) => c.args(slopFile).action(async ({ args }) => process.stdout.write(await readSlop("inspect", args.file))),
    ),
  )
  .add(
    defineCommand("get", { description: "Print current document state as JSON" }, (c) =>
      c
        .args(document)
        .flags({
          name: "snapshot",
          type: "boolean",
          description: "Print the schema, version, value and colors ({schema, defaults, version, value, theme})",
        })
        .action(async ({ args, flags }) => (await documents()).get(args.document, flags.snapshot === true)),
    ),
  )
  .add(
    defineCommand(
      "apply",
      { description: "Apply one document operation", sections: [retrySection] },
      (c) =>
        c
          .args(document)
          .flags(
            {
              name: "op",
              type: "string",
              required: true,
              description: "Operation object as JSON",
            },
            baseFlag,
            attachFlag,
          )
          .action(async ({ args, flags }) => (await documents()).apply(args.document, flags.op, flags.base, flags.attach)),
    ),
  )
  .add(
    defineCommand(
      "batch",
      { description: "Apply an atomic batch of document operations", sections: [retrySection] },
      (c) =>
        c
          .args(document)
          .flags(
            {
              name: "ops",
              type: "string",
              required: true,
              description: "Array of operations as JSON",
            },
            baseFlag,
            attachFlag,
          )
          .action(async ({ args, flags }) => (await documents()).batch(args.document, flags.ops, flags.base, flags.attach)),
    ),
  )
  .add(
    defineCommand(
      "import",
      {
        description: "Replace document data with a JSON file's value, writing only the differences",
        sections: [retrySection],
      },
      (c) =>
        c
          .args(document, { name: "file", type: "string", required: true, description: "JSON file holding the new value" })
          .flags({
            name: "path",
            type: "string",
            description: 'Where to replace, as a JSON path (default: the whole document), e.g. \'["rows"]\'',
          })
          .action(async ({ args, flags }) => (await documents()).importValue(args.document, args.file, flags.path)),
    ),
  )
  .add(
    defineCommand(
      "export",
      {
        description: "Export a document as PNG or PDF on macOS",
        sections: [
          {
            title: "Capture behavior",
            body: "Open documents export their live selected view; closed documents export the saved state with the initial view. Output must not be the document itself. A lost acknowledgement has an uncertain outcome: inspect the destination before retrying.",
          },
        ],
      },
      (c) =>
        c
          .args(document)
          .flags(
            {
              name: "format",
              type: "string",
              choices: ExportFormats,
              required: true,
              description: "Export format: png or pdf",
            },
            {
              name: "output",
              type: "string",
              required: true,
              description: "Destination file",
            },
          )
          .action(async ({ args, flags }) => (await documents()).exportDocument(args.document, flags.format, flags.output)),
    ),
  )
  .add(
    defineCommand("theme", { description: "Inspect, override and share a document's palette" }, (c) =>
      c.add(theme.get).add(theme.set).add(theme.reset).add(theme.export).add(theme.import),
    ),
  )
  .extend(skill({ name: skillName, extras: skillExtras, defaultScope: "global" }));
