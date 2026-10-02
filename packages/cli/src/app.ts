import { Crust, defineCommand } from "@crustjs/core";
import { didYouMean, help, version } from "@crustjs/extensions";
import { skill } from "@crustjs/skills";
import { SlopCategories } from "@hitslop/schema/constants";
import metadata from "../package.json";
import { isGlobalInstall } from "./paths";

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
const retrySection = {
  title: "Retries",
  body: "Mutations are never automatically replayed. After an unknown outcome, run slop get before issuing another edit. get saves and returns owner-accepted state; text still being typed in an open window is not included.",
};

/** Document commands run in the macOS helper, which reaches a live window or owns a
 * closed document. */
async function native(...argv: string[]) {
  if (process.platform !== "darwin")
    throw new Error("Document commands require macOS and the installed hitSlop app.");
  await (await import("./native")).runNative(argv);
}
/** CLI flags as helper arguments: `--name value`, or `--name` for a set boolean. */
const flagArgs = (flags: Record<string, unknown>) =>
  Object.entries(flags).flatMap(([name, value]) =>
    typeof value === "string" || typeof value === "number"
      ? [`--${name}`, String(value)]
      : value === true
        ? [`--${name}`]
        : [],
  );
const forward = (command: string, target: string, flags: Record<string, unknown>) =>
  native(command, target, ...flagArgs(flags));

const themeDescriptions = {
  get: "Print theme defaults, overrides and effective values",
  set: "Override declared theme tokens",
  reset: "Remove one theme override, or all of them",
};

function themeCommand(command: "get" | "set" | "reset") {
  return defineCommand(command, { description: themeDescriptions[command] }, (sub) => {
    const configured = sub.args(document).flags(
      ...(command === "set"
        ? [
            {
              name: "values",
              type: "string" as const,
              required: true as const,
              description: "Theme token values as JSON",
            },
          ]
        : []),
      ...(command === "reset"
        ? [
            {
              name: "token",
              type: "string" as const,
              description: "Token to reset; omit to reset all",
            },
          ]
        : []),
    );
    return configured.action(({ args, flags }) => native("theme", command, args.document, ...flagArgs(flags)));
  });
}

export const app = new Crust("slop", {
  description: "Author hitSlop mini apps and work with local documents",
  version: cliVersion,
  sections: [
    {
      title: "Document workflow",
      body: "Run bunx @hitslop/cli or install globally with bun install -g @hitslop/cli. Read manifest.json first. Inspect schema before editing. Built and registered template masters are immutable: create a writable copy before editing. Native macOS commands route to the live session or acquire exclusive ownership when closed.",
    },
  ],
})
  .extend(version())
  .extend(help())
  .extend(didYouMean())
  .add(
    defineCommand(
      "attachments",
      { description: "Import, inspect, and export document attachments" },
      (c) =>
        c
          .add(
            defineCommand("list", { description: "List attachment IDs and sizes" }, (c) =>
              c.args(document).action(({ args }) => native("attachments", "list", args.document)),
            ),
          )
          .add(
            defineCommand(
              "import",
              { description: "Save a file and print its reference as JSON" },
              (c) =>
                c
                  .args(document, { name: "file", type: "string", required: true })
                  .action(({ args }) => native("attachments", "import", args.document, args.file)),
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
                  .action(({ args, flags }) =>
                    native("attachments", "export", args.document, args.id, ...flagArgs(flags)),
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
            body: "Interactive terminals also ask for the author. Title, categories, and description start as the directory name, productivity, and A hitSlop mini app.; the launched agent updates them in manifest.json to match what it builds, and you can edit them there anytime. Flags set any of these explicitly; --category accepts one or two distinct values. An omitted brief uses the description. The slug is derived from the directory name unless --slug is supplied. All metadata is validated before creating files; existing destinations are refused. Project agent guides are portable copies, not links managed by skills repair.",
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
              name: "slug",
              type: "string",
              description: "App slug: 2–64 lowercase letters/digits with single hyphens",
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
    defineCommand("check", { description: "Check Svelte and TypeScript authoring source" }, (c) =>
      c.args(source).action(async ({ args }) => {
        const { createRequire } = await import("node:module");
        const require = createRequire(import.meta.url);
        const child = Bun.spawn(
          [
            process.execPath,
            require.resolve("svelte-check/bin/svelte-check"),
            "--workspace",
            args.source,
          ],
          { stdout: "inherit", stderr: "inherit" },
        );
        if (await child.exited) throw new Error("Authoring checks failed");
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
    defineCommand("build", { description: "Build a runtime template with native previews" }, (c) =>
      c.args(source).action(async ({ args }) => {
        await (await import("./authoring")).build(args.source);
      }),
    ),
  )
  .add(
    defineCommand(
      "register",
      { description: "Build and register an immutable local template" },
      (c) =>
        c.args(source).action(async ({ args }) => {
          await (await import("./authoring")).register(args.source);
        }),
    ),
  )
  .add(
    defineCommand(
      "create",
      { description: "Create a writable document from a built or registered template" },
      (c) =>
        c
          .flags(
            {
              name: "from",
              type: "string",
              required: true,
              description: "Template .slop to copy",
            },
            {
              name: "output",
              type: "string",
              required: true,
              description: "Path for the new writable document",
            },
          )
          .action(({ flags }) => native("create", ...flagArgs(flags))),
    ),
  )
  .add(
    defineCommand("open", { description: "Open a document in the hitSlop app" }, (c) =>
      c.args(document).action(({ args }) => native("open", args.document)),
    ),
  )
  .add(
    defineCommand("schema", { description: "Print the document schema descriptor" }, (c) =>
      c.args(document).action(({ args }) => forward("schema", args.document, {})),
    ),
  )
  .add(
    defineCommand("get", { description: "Print current document state as JSON" }, (c) =>
      c
        .args(document)
        .flags({
          name: "snapshot",
          type: "boolean",
          description: "Print the schema with the current state ({schema, state})",
        })
        .action(({ args, flags }) => forward("get", args.document, flags)),
    ),
  )
  .add(
    defineCommand(
      "apply",
      { description: "Apply one document operation", sections: [retrySection] },
      (c) =>
        c
          .args(document)
          .flags({
            name: "op",
            type: "string",
            required: true,
            description: "Operation object as JSON",
          })
          .action(({ args, flags }) => forward("apply", args.document, flags)),
    ),
  )
  .add(
    defineCommand(
      "batch",
      { description: "Apply an atomic batch of document operations", sections: [retrySection] },
      (c) =>
        c
          .args(document)
          .flags({
            name: "ops",
            type: "string",
            required: true,
            description: "Array of operations as JSON",
          })
          .action(({ args, flags }) => forward("batch", args.document, flags)),
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
          .action(({ args, flags }) => native("import", args.document, args.file, ...flagArgs(flags))),
    ),
  )
  .add(
    defineCommand(
      "compact",
      { description: "Checkpoint document storage", sections: [retrySection] },
      (c) => c.args(document).action(({ args, flags }) => forward("compact", args.document, flags)),
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
            body: "Open documents export their live selected view; closed documents export the saved state with the initial view. Output must be outside the source package. A lost acknowledgement has an uncertain outcome: inspect the destination before retrying.",
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
              choices: ["png", "pdf"],
              required: true,
              description: "Export format: png or pdf",
            },
            {
              name: "output",
              type: "string",
              required: true,
              description: "Destination outside the document package",
            },
          )
          .action(({ args, flags }) => forward("export", args.document, flags)),
    ),
  )
  .add(
    defineCommand("theme", { description: "Inspect and override declared theme tokens" }, (c) =>
      c.add(themeCommand("get")).add(themeCommand("set")).add(themeCommand("reset")),
    ),
  )
  // Only the global install repairs links: other copies would point agents at a bunx
  // cache or a project's node_modules. Repairs stay global, where the links follow the
  // global CLI; projects keep init's guide copies, which repair must not report.
  .extend(
    skill({
      name: skillName,
      extras: skillExtras,
      defaultScope: "global",
      autoUpdate: isGlobalInstall,
    }),
  );
