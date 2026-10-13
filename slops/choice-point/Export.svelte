<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  const data = $derived(doc.current);
</script>

<main class="cp-sheet cp-export">
  <header class="cp-head"><div><h1>Choice Point</h1><p>Notice what hooks you, then choose your next move.</p></div><span class="cp-date">{data.date}</span></header>
  <section class="cp-card"><h2>Situation, thoughts & feelings</h2><p>{previewText(data.situation || "Nothing noted.", preview, 150)}</p><h3>What’s hooking me</h3><ul>{#each (preview ? (data.hooks).slice(0, 2) : data.hooks) as item}<li>{previewText(item.text, preview, 75)}</li>{/each}</ul></section>
  <div class="cp-export-moves">
    <section class="cp-card cp-away"><h2>Away moves</h2><ul>{#each (preview ? (data.awayMoves).slice(0, 2) : data.awayMoves) as item}<li>{previewText(item.text, preview, 75)}</li>{/each}</ul></section>
    <section class="cp-card cp-towards"><h2>Towards moves</h2><ul>{#each (preview ? (data.towardsMoves).slice(0, 2) : data.towardsMoves) as item}<li>{previewText(item.text, preview, 75)}</li>{/each}</ul></section>
  </div>
  <section class="cp-card"><h2>Helps me unhook</h2><ul>{#each (preview ? (data.helpers).slice(0, 2) : data.helpers) as item}<li>{previewText(item.text, preview, 75)}</li>{/each}</ul></section>
  <section class="cp-card cp-next"><h2>One towards move</h2><p>{previewText(data.nextMove || "A next move is still to come.", preview, 110)}</p><p>{data.nextWhen}</p><strong>{data.nextDone ? "✓ Done" : "Not done yet"}</strong></section>
</main>

<style>
  .cp-export { display: flex; flex-direction: column; gap: 18px; height: auto; overflow: visible; }
  .cp-export-moves { display: grid; grid-template-columns: 1fr 1fr; gap: 18px; }
  .cp-export :global(p), .cp-export :global(li) { white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.5; }
  .cp-export :global(ul) { margin: 10px 0 0; padding-left: 20px; }
  .cp-export :global(.cp-next) { margin: 0; }
</style>
