// Prove the handoff at the process boundary, without starting a paid agent:
// executable discovery, cwd, prompt arguments, opt-out, and failure retention.
import { expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

test("agent picker launches known and user-entered CLIs in the project with a brief handoff", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-agents-"));
  try {
    const bin = join(root, "bin"),
      project = join(root, "my project");
    await mkdir(bin);
    await mkdir(project);
    await writeFile(join(project, "BRIEF.md"), "Build a budget tracker; $(not-a-command)");
    for (const name of ["codex", "claude", "gemini", "opencode", "grok"]) {
      const path = join(bin, name);
      await writeFile(
        path,
        `#!${process.execPath}\nawait Bun.write(process.env.PROBE_RESULT, JSON.stringify({cwd:process.cwd(),args:process.argv.slice(2),brief:await Bun.file("BRIEF.md").text()}));process.exit(Number(process.env.PROBE_EXIT || 0));\n`,
      );
      await chmod(path, 0o755);
    }
    for (const scenario of [
      "codex",
      "claude",
      "gemini",
      "opencode",
      "custom",
      "none",
      "yes",
      "ci",
      "non-tty",
      "cancel",
      "failure",
    ]) {
      const resultFile = join(root, scenario + ".json");
      const script = `
        import {offerAgentLaunch,installedAgents} from ${JSON.stringify(resolve("packages/cli/src/agents.ts"))};
        import {createPromptIO} from ${JSON.stringify(resolve("packages/cli/node_modules/@crustjs/prompts/dist/testing.js"))};
        import {PassThrough} from "node:stream";
        const scenario=${JSON.stringify(scenario)};
        const tty=createPromptIO({isTTY:scenario!=="non-tty"});
        const output=Object.assign(new PassThrough(),{isTTY:scenario!=="non-tty"});
        output.pipe(tty.io.output);
        function shown(text){return new Promise(resolve=>{const listen=chunk=>{if(chunk.toString().includes(text)){output.off("data",listen);resolve()}};output.on("data",listen)})}
        if(installedAgents().length!==4) throw Error("Unexpected discovery");
        const ready=shown("Which agent CLI");
        const result=offerAgentLaunch(${JSON.stringify(project)},scenario==="yes",{input:tty.io.input,output}).catch(e=>e);
        if(!["yes","ci","non-tty"].includes(scenario)) {
          await ready;
          if(scenario==="cancel") tty.keys("ctrl+c");
          else if(scenario==="custom") {
            tty.keys("up");const executable=shown("Agent executable");tty.keys("return");await executable;
            tty.type("grok");const option=shown("Prompt option");tty.keys("return");await option;tty.type("-i");tty.keys("return");
          } else {
            const up={codex:5,claude:4,gemini:3,opencode:2,none:0,failure:5}[scenario];
            for(let i=0;i<up;i++)tty.keys("up");tty.keys("return");
          }
        }
        const outcome=await result;
        if(scenario==="failure") {if(!(outcome instanceof Error)||!outcome.message.includes("project remains"))throw Error("Missing recovery guidance")}
        else if(outcome instanceof Error) throw outcome;
        output.destroy();
      `;
      const child = Bun.spawn([process.execPath, "-e", script], {
        env: {
          ...process.env,
          PATH: bin,
          CI: scenario === "ci" ? "1" : "",
          PROBE_RESULT: resultFile,
          PROBE_EXIT: scenario === "failure" ? "7" : "0",
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [code, stderr] = await Promise.all([
        child.exited,
        new Response(child.stderr).text(),
        new Response(child.stdout).text(),
      ]);
      expect(stderr).toBe("");
      expect(code).toBe(0);
      const launched = await Bun.file(resultFile).exists();
      expect(launched).toBe(!["none", "yes", "ci", "non-tty", "cancel"].includes(scenario));
      if (launched) {
        const actual = JSON.parse(await readFile(resultFile, "utf8"));
        expect(actual.cwd).toBe(await realpath(project));
        expect(actual.args.length).toBe(
          ["gemini", "opencode", "custom"].includes(scenario) ? 2 : 1,
        );
        if (scenario === "gemini") expect(actual.args[0]).toBe("--prompt-interactive");
        if (scenario === "opencode") expect(actual.args[0]).toBe("--prompt");
        if (scenario === "custom") expect(actual.args[0]).toBe("-i");
        expect(actual.args.at(-1)).toContain("Read slop.ts, AGENTS.md, BRIEF.md");
        expect(actual.args.at(-1)).toContain("bun run check");
      }
      expect(await readFile(join(project, "BRIEF.md"), "utf8")).toBe(
        "Build a budget tracker; $(not-a-command)",
      );
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 20_000);
