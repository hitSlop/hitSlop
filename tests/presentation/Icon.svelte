<script lang="ts">
import type { CaptureProps } from "hitslop/svelte";
import type schema from "./schema";
let { snapshot }: CaptureProps<typeof schema> = $props();

// Saved state makes the fresh renderer fail, independently of editor-local globals.
function checkRender() {
  if (snapshot.count === -2001) throw new Error("Snippet failed");
  return "";
}
</script>

<div class="icon">{checkRender()}</div>
