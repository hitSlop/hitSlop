import { findEngine } from "./engine";
import { exec } from "./process";
import { RuntimeABI } from "../schema/constants";

export async function evaluateDefinition(program: Uint8Array) {
    const evaluated = await exec([await findEngine(), "--evaluate-command"], {
      cwd:"/", env:{}, timeout:4000,
      stdin:JSON.stringify({runtimeABI:RuntimeABI,mode:"definition",bundle:new TextDecoder().decode(program),request:"{}"}),
    });
    if (evaluated.code) throw new Error(`Definition evaluator failed: ${evaluated.stderr}`);
    const reply = JSON.parse(evaluated.stdout) as
      | { ok: true; declaration: ReturnType<typeof import("../sdk/app-definition").describeApp> }
      | { ok: false; error: string };
    if (reply.ok !== true) throw new Error(`Definition initialization failed: ${reply.error}`);
    return reply.declaration;
}
