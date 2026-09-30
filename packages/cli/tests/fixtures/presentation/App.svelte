<script lang="ts">
  import { useDocument } from "@hitslop/document/svelte";
  import schema from "./schema";
  const document = useDocument(schema);
  // Native tests inject failures without changing the document or the shipped examples.
  function checkRender(kind: "editor" | "export" | "icon") {
    const failure = (globalThis as any).__presentationFailure;
    if (failure?.kind === kind) throw failure.error;
    return "";
  }
</script>

  <main class="editor">{checkRender("editor")}<button onclick={() => document.fields.count.increment().catch(() => {})}>Clicks: {document.current.count}</button></main>
