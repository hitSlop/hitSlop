<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { previewText } from "../_shared/preview";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { posterView } from "./poster";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();
  const view = $derived(posterView(snapshot));
</script>
<article class="export" aria-label="Exported poster">
  <div class="static-poster">
    <div class="static-marks" aria-hidden="true">{#each view.shapes as shape}<span class="static-shape shape" data-kind={shape.kind} data-tone={shape.tone} style:width={`${Math.max(24, shape.size * 260)}px`} style:height={`${Math.max(24, shape.size * 260)}px`}></span>{/each}</div>
    <h1>{snapshot.title}</h1><p>{previewText(snapshot.body, preview, 800)}</p>
  </div>
</article>
<style>
  .static-poster { width: 520px; box-sizing: border-box; padding: 36px; min-height: 700px; background: var(--slop-paper); color: var(--slop-ink); }
  .static-marks { display: flex; flex-wrap: wrap; gap: 16px; margin-bottom: 28px; align-items: center; }
  .static-shape { position: static; flex: none; }
  h1 { font: 700 46px/1.15 "Poster Lora", serif; margin: 0 0 24px; white-space: pre-wrap; overflow-wrap: anywhere; }
  p { font: 400 17px/1.65 "Poster Lora", serif; margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
</style>
