<script lang="ts">
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  let { snapshot, attachmentURL }: CaptureProps<typeof schema> = $props();
</script>

<div class="accent-patch" aria-label="Theme accent"></div>
{#if snapshot.photo}<img src={attachmentURL(snapshot.photo)} alt="Attached solid patch" width="32" height="32" />{/if}
<p class="cjk">中文 😀</p>
<p class="hand">Hand 中文 😀</p>
<p data-slop-export="hide">Hidden in capture</p>
<p>{snapshot.note ?? "No note"} / {snapshot.memo ?? "No memo"} / {snapshot.place?.name ?? "No place"}</p>
<p>{snapshot.enabled ?? false} / {snapshot.weight ?? 0} / {snapshot.stage ?? "unset"}</p>
{#each snapshot.rows as row (row.$id)}
  <section><h2>{row.text}</h2><p>{row.tags.join(", ")} / {Object.entries(row.notes).map(([key, value]) => `${key}: ${value}`).join(", ")}</p><p>Row counter: {row.hits}</p></section>
{/each}
<p>Colors: {snapshot.colors.join(", ")}</p>
<p>Checkins: {Object.entries(snapshot.checkins).map(([key, value]) => `${key}: ${value}`).join(", ")}</p>
{#each Object.entries(snapshot.cells) as [key, cell]}<p>{key}: {cell.input} / {cell.width ?? 0} / {cell.hits}</p>{/each}
{#each snapshot.slides as slide (slide.$id)}
  <section aria-label={slide.title}><h2>{slide.title}</h2>{#each slide.blocks as block (block.$id)}<p>{block.text}</p>{/each}</section>
{/each}
