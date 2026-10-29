import { realpath } from "node:fs/promises";
import { resolve } from "node:path";
import { buildGraphs, type DefinitionBuildOptions } from "./definition-config";
import { evaluateDefinition } from "./definition-evaluate";
import { stageResources } from "./definition-resources";
export { resourceKey } from "./definition-resources";
export type { DefinitionBuildOptions } from "./definition-config";

/** Compile, evaluate in the restricted runner, then stage the accepted inventory. */
export async function buildDefinition(
  source: string,
  stage: string,
  options: DefinitionBuildOptions = {},
) {
  source = await realpath(resolve(source));
  const graphs = await buildGraphs(source, options);
  const program = graphs.outputs.definition.get("commands.js");
  if (!program) throw new Error("Definition build emitted no command program");
  const declaration = await evaluateDefinition(program.bytes);
  const input = await stageResources(source, resolve(stage), graphs, declaration);
  return {
    input,
    dependencies: {
      definition: [...graphs.inputs.definition],
      ui: [...graphs.inputs.ui],
      capture: [...graphs.inputs.capture],
    },
    uiBytes: graphs.outputs.ui.get("ui.js")!.bytes.length,
  };
}
