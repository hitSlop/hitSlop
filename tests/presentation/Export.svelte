<script lang="ts">
import doc from "./schema";
// Saved state makes the fresh renderer fail, independently of editor-local globals.
function checkRender() {
  if (doc.current.count === -1001) throw new Error("Snippet failed");
  return "";
}
let { mode }: { mode: "preview" | "export" } = $props();
</script>

<article class="export">{checkRender()}<h1>Presentation</h1><p>Clicks: {doc.current.count}</p></article>
