<script lang="ts">
import "./styles.css";
import type { CaptureProps } from "@hitslop/tinys/svelte";
import type schema from "./schema";
let { snapshot }: CaptureProps<typeof schema> = $props();

// Saved state makes the fresh renderer fail, independently of editor-local globals.
function checkRender() {
  if (snapshot.count === -2001) throw new Error("Snippet failed");
  return "";
}
</script>

<div class="icon">{checkRender()}</div>
