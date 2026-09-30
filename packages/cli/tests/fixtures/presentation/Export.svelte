<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
const document = useDocument(schema);
function checkRender(kind: "editor" | "export" | "icon") {
    const failure = (globalThis as any).__presentationFailure;
    if (failure?.kind === kind) throw failure.error;
    return "";
  }
let { mode }: { mode: "preview" | "export" } = $props();
</script>

<article class="export">{checkRender("export")}<h1>Presentation</h1><p>Clicks: {document.current.count}</p></article>
