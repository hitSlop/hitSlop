/** Replay the bytes frozen in the document, never rebuilding the author's commands. */
import { Database } from "bun:sqlite";
import { appAsset } from "../lib/artifacts";
import { documentEngine } from "./corpus";
import { exec } from "../../packages/hitslop/src/cli/process";
export type CommandScenario = { name: string; args: unknown; now: number; seed: number[]; evaluated: unknown; value: unknown };
export async function evaluateStored(document: string, state: { value: unknown; schema: unknown }, scenario: Pick<CommandScenario, "name" | "args" | "now" | "seed">) {
  const database = new Database(document, { readonly: true });
  let runtimeABI: number;
  try { runtimeABI = (database.query("SELECT runtime_abi FROM app WHERE id = 1").get() as { runtime_abi: number }).runtime_abi; }
  finally { database.close(); }
  const bundle = appAsset(document, "commands.js");
  const request = JSON.stringify({ ...scenario, descriptor: state.schema, value: state.value });
  const result = await exec([documentEngine(), "--evaluate-command"], { stdin: JSON.stringify({ runtimeABI, bundle, request }), timeout: 10_000 });
  if (result.code) throw new Error(`Stored command runner failed: ${result.stderr}`);
  const reply = JSON.parse(result.stdout);
  if (!reply.ok) throw new Error(`Stored command failed: ${reply.error}`);
  return reply;
}
