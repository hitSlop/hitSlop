import type { Input } from "@hitslop/document";
import schema from "./schema";

export default { boardShape: "landscape", strokes: [] } satisfies Input<typeof schema.fields.node>;
