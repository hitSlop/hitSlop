<script lang="ts">
import doc from "./schema";

// Saved state makes the fresh renderer fail, independently of editor-local globals.
function checkRender() {
  if (doc.current.count === -2001) throw new Error("Snippet failed");
  return "";
}
</script>

<div class="icon">{checkRender()}</div>
