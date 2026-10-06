/** Replay the bytes frozen in the document, never rebuilding the author's commands. */
import { appAsset } from "../lib/artifacts";
import { documentEngine } from "./corpus";
import { exec } from "../../packages/hitslop/src/cli/process";
export type CommandScenario = { name: string; args: unknown; now: number; seed: number[]; evaluated: unknown; value: unknown };
export async function evaluateStored(document: string, state: { value: unknown; schema: unknown }, scenario: Pick<CommandScenario, "name" | "args" | "now" | "seed">) {
  const bundle = appAsset(document, "__commands/run.js");
  const request = JSON.stringify({ ...scenario, descriptor: state.schema, value: state.value });
  const result = await exec([documentEngine(), "--evaluate-command"], { stdin: JSON.stringify({ bundle, request }), timeout: 10_000 });
  if (result.code) throw new Error(`Stored command runner failed: ${result.stderr}`);
  const reply = JSON.parse(result.stdout);
  if (!reply.ok) throw new Error(`Stored command failed: ${reply.error}`);
  return reply;
}
