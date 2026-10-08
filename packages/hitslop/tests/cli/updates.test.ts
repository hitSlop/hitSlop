import { exec } from "../../../../scripts/lib/test-process";
// Registry I/O is replaced only in isolated subprocesses. Verify observable
// machine output, interactive notices/cache reuse, and successful offline work.
import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

test("update checks are interactive, cached, optional, and soft-failing", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-updates-"));
  try {
    for (const scenario of ["piped", "stderr-piped", "ci", "disabled", "offline", "notice"]) {
      const home = join(root, scenario);
      await mkdir(home);
      const script = `
        import {Crust} from ${JSON.stringify(resolve("packages/hitslop/node_modules/@crustjs/core/dist/index.js"))};
        import {interactiveUpdates} from ${JSON.stringify(resolve("packages/hitslop/src/cli/updates.ts"))};
        Object.defineProperty(process.stdout,"isTTY",{value:${scenario !== "piped"}});
        Object.defineProperty(process.stderr,"isTTY",{value:${scenario !== "stderr-piped"}});
        let calls=0;
        globalThis.fetch=async()=>{calls++;${scenario === "offline" ? 'throw Error("offline")' : 'return Response.json({"dist-tags":{latest:"999.0.0"}})'}};
        const app=new Crust("probe",{version:"1.1.0"}).extend(interactiveUpdates()).action(({stdout})=>stdout('{"ok":true}'));
        await app.execute({argv:[]});await app.execute({argv:[]});
        if(calls!==${["notice", "offline"].includes(scenario) ? 1 : 0})throw Error("Unexpected registry activity: "+calls);
      `;
      const result = await exec([process.execPath, "-e", script], {
        env: {
          ...process.env,
          HOME: home,
          XDG_STATE_HOME: home,
          XDG_CACHE_HOME: home,
          CI: scenario === "ci" ? "1" : "",
          HITSLOP_NO_UPDATE_CHECK: scenario === "disabled" ? "1" : "0",
        },
      });
      const { stdout: out, stderr: err, code } = result;
      expect(code).toBe(0);
      expect(out).toBe('{"ok":true}\n{"ok":true}\n');
      if (scenario === "notice") {
        expect(err.match(/Update available/g)).toHaveLength(1);
        expect(err).toContain("cli-workflows/#upgrade-the-cli");
      } else expect(err).toBe("");
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
