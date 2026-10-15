import { rustBindings } from "./rust-bindings";

/** Rust owns contracts. The evaluator prelude is bundled SDK code, not a validator. */
export async function generateContracts(check = false) {
  await rustBindings(check);
}
if (import.meta.main) await generateContracts(process.argv.includes("--check"));
