import { bindCommands, commandInfo } from "./commands";
import type { BuildDeclaration, ArtworkInput } from "../wire/app.generated";

/** Plain declaration projection used in the restricted evaluator. Acceptance belongs
 * to Rust; these checks establish component/command registration, not metadata rules. */
export function describeApp(app: any): BuildDeclaration & { artwork?: ArtworkInput } {
  const { document, view, export: exportView, icon, commands = {}, theme, initial, window, artwork, ...metadata } = app;
  for (const [role, component] of Object.entries({ view, export: exportView, icon })) {
    if (role !== "view" && component === undefined) continue;
    if (!component || (component as any)["~hitslop"] !== "component")
      throw new Error(`${role} must reference an imported Svelte component`);
  }
  if (!document?.descriptor) throw new Error("document must be a defineDocument value");
  bindCommands(commands, document);
  return {
    metadata, window, document: document.descriptor, initial,
    theme: Object.entries(theme ?? {}).map(([token, color]) => ({ token, color: color as string })),
    commands: Object.entries(commands).map(([name, command]) => {
      const { spec } = commandInfo(command)!;
      if (typeof spec.run !== "function") throw new Error(`commands.${name} needs run(ctx, args)`);
      return { name, description: spec.description, args: { kind: "object", properties: spec.args } };
    }),
    views: { export: exportView !== undefined, icon: icon !== undefined },
    artwork,
  };
}
