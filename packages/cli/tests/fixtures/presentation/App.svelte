<script lang="ts">
    import doc from "./schema";
  // Native tests inject failures and refused writes without changing the shipped examples.
  function checkRender(kind: "editor" | "export" | "icon") {
    const failure = (globalThis as any).__presentationFailure;
    if (failure?.kind === kind) throw failure.error;
    return "";
  }
</script>

  <main class="editor">{checkRender("editor")}<button onclick={() => doc.fields.count.increment((globalThis as any).__presentationIncrement ?? 1)}>Clicks: {doc.current.count}</button></main>
