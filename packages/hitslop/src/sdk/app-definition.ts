import { commandSet } from "./command-set";
import type { BuildDeclaration, ArtworkInput } from "../wire/app.generated";

/** Projects the convention-discovered pieces; Rust owns app acceptance. */
export function describeApp(
  app: any,
  document: any,
  commands: unknown,
  views: BuildDeclaration["views"],
): BuildDeclaration & { artwork?: ArtworkInput } {
  const { theme, window, artwork, capabilities, widget, ...metadata } = app;
  if (!document?.descriptor)
    throw new Error("schema.ts must default-export a defineDocument value");
  const set = commands === undefined ? undefined : commandSet(commands);
  if (commands !== undefined && !set)
    throw new Error("commands.ts must default-export defineCommands(document, specs)");
  if (set && set.document !== document)
    throw new Error("commands.ts must pass the schema.ts document to defineCommands");
  if (widget !== undefined && !views.widget)
    throw new Error("widget metadata requires a root Widget.svelte");
  return {
    metadata,
    window,
    document: document.descriptor,
    initial: document.initial,
    theme: Object.entries(theme ?? {}).map(([token, color]) => ({ token, color: color as string })),
    commands: Object.entries(set?.specs ?? {}).map(([name, spec]) => ({
      name,
      description: spec.description,
      args: { kind: "object", properties: spec.args },
    })),
    views: { ...views, ...(views.widget ? { widget: widget === undefined ? {} : widget } : {}) },
    capabilities: capabilities ?? [],
    artwork,
  };
}
