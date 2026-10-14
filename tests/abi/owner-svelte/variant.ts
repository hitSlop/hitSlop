import type { AppDeclaration } from "../../../packages/hitslop/src/sdk/slop";
// The control declares no services; the fixture builder (scripts/lib/native-fixtures.ts) builds a
// copy of this app that declares every capability (`declaredCapabilities` in scripts/lib/presentation-variants.ts).
export default {} as Pick<AppDeclaration, "capabilities">;
