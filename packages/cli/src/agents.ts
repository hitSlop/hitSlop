import { input, select, resolvePromptIO, type PromptIO } from "@crustjs/prompts";

const handoff =
  "Read slop.ts, AGENTS.md, BRIEF.md, and the local hitSlop authoring/design skills. Build the slop described in BRIEF.md, adapting the checklist starter to its purpose. Update the title, description, and categories in slop.ts to match what you build; the user can edit them later. Install dependencies with bun install, implement the app, then run bun run check and bun run build. If native build needs a compatible hitSlop Mac app, explain that requirement and complete the checks available here.";

const knownAgents = [
  { executable: "codex", label: "Codex", args: [handoff] },
  { executable: "claude", label: "Claude Code", args: [handoff] },
  { executable: "gemini", label: "Gemini CLI", args: ["--prompt-interactive", handoff] },
  { executable: "opencode", label: "OpenCode", args: ["--prompt", handoff] },
];

/** PATH lookup only: discovery never executes an agent or installs software. */
function installedAgents() {
  return knownAgents.flatMap((agent) => {
    const path = Bun.which(agent.executable);
    return path ? [{ ...agent, path }] : [];
  });
}

export async function offerAgentLaunch(destination: string, yes = false, streams?: PromptIO) {
  const io = resolvePromptIO(streams);
  if (yes || process.env.CI || !io.input.isTTY || !io.output.isTTY) return;
  try {
    const agents = installedAgents();
    const selected = await select(
      {
        message: "Which agent CLI should build your slop?",
        choices: [
          ...agents.map((agent) => ({ label: agent.label, value: agent.executable })),
          { label: "Other CLI…", value: "other" },
          { label: "Finish without launching", value: "none" },
        ],
        default: "none",
      },
      io,
    );
    if (selected === "none") return;
    let agent = agents.find((entry) => entry.executable === selected);
    if (selected === "other") {
      const executable = (
        await input(
          {
            message: "Agent executable or path (for example grok)",
            validate(value) {
              if (!value.trim() || !Bun.which(value.trim()))
                throw new Error("Enter an installed executable name or path, without arguments");
            },
          },
          io,
        )
      ).trim();
      const option = (
        await input(
          {
            message:
              "Prompt option (blank for a positional prompt; for example --prompt-interactive)",
            default: "",
            validate(value) {
              if (value.trim() && !/^--?[a-zA-Z][a-zA-Z0-9-]*$/.test(value.trim()))
                throw new Error("Enter one prompt option, or leave blank for a positional prompt");
            },
          },
          io,
        )
      ).trim();
      agent = {
        executable,
        label: executable,
        path: Bun.which(executable)!,
        args: [...(option ? [option] : []), handoff],
      };
    }
    if (!agent) throw new Error("Agent executable is unavailable");
    io.output.write(`Launching ${agent.label} in ${destination}\n`);
    // Pass arguments directly, never through a shell. The user's brief stays in
    // BRIEF.md, and each agent retains its own trust and permission settings.
    const child = Bun.spawn([agent.path, ...agent.args], {
      cwd: destination,
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });
    const code = await child.exited;
    if (code !== 0) throw new Error(`${agent.label} exited with status ${code}`);
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") return;
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}. Your project remains at ${destination}; launch your agent there to continue.`,
      { cause: error },
    );
  }
}
