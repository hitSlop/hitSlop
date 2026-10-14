import { getContext } from "svelte";
import type { Definition, LiveDocument, ObjectNode } from "../schema";

const key = Symbol("hitslop.document");
type Mounted = { schema: Definition<ObjectNode>; document: LiveDocument<ObjectNode> };

/** The Svelte context a mount gives its components: the definition it mounted with and
 * that mount's own document. */
export function documentContext(schema: Mounted["schema"], document: Mounted["document"]) {
  return new Map<symbol, Mounted>([[key, { schema, document }]]);
}

/**
 * The document an editor component reads and edits: `const doc = getDocument(schema)`, with
 * `schema` the default export of the app's schema.ts. Call it while a component initializes
 * (at the top of its `<script>`, or in a function that runs then) and keep the result. Each
 * mount has its own document. Preview, Export, Icon and Widget read their `snapshot` prop
 * instead (`CaptureProps`).
 */
export function getDocument<N extends ObjectNode>(schema: Definition<N>): LiveDocument<N> {
  let mounted: Mounted | undefined;
  try {
    mounted = getContext<Mounted | undefined>(key);
  } catch {
    throw new Error(
      "getDocument(schema) must run while a component initializes: call it at the top of the component's <script>, not in an event handler, a timer or at module scope",
    );
  }
  if (!mounted)
    throw new Error(
      "getDocument(schema) needs a component of a mounted hitSlop app; Preview, Export, Icon and Widget read their snapshot prop instead",
    );
  if (mounted.schema !== schema)
    throw new Error("getDocument(schema) takes the schema.ts definition the app mounted with");
  return mounted.document as LiveDocument<N>;
}
