<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import { previewText } from "../_shared/preview";
  import type schema from "./schema";
  import { formatDate, plannedLabel, plannerView, toLabel } from "./schedule";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const view = $derived(plannerView(snapshot));
</script>

<article class="exportPage" aria-label="Exported daily planner">
  <header class="header">
    <span class="eyebrow">DAILY / PLANNER</span>
    <div class="headerMain">
      <h1 class="title">{formatDate(snapshot.date, { weekday: "long" })}</h1>
      <span class="dateNumber">{formatDate(snapshot.date, { day: "2-digit" })}</span>
    </div>
    <p>{formatDate(snapshot.date, { month: "long", day: "numeric", year: "numeric" })}</p>
  </header>
  <section class="priorities" style="max-height:none;overflow:visible">
    <h2 class="heading">Make room for</h2>
    <ol class="priorityList">
      {#each (preview ? (snapshot.priorities).slice(0, 3) : snapshot.priorities) as priority, index (priority.$id)}
        <li class="priority">
          <span class="checkbox">{priority.done ? "✓" : index + 1}</span>
          <span style:text-decoration={priority.done ? "line-through" : "none"}>{priority.text || "—"}</span>
        </li>
      {/each}
    </ol>
  </section>
  <div class="scheduleHead">
    <h2 class="heading">Your day</h2>
    <span class="meta">{plannedLabel(view.booked)}</span>
  </div>
  <div class="exportList">
    {#each (preview ? (view.placed).slice(0, 5) : view.placed) as item (item.block.$id)}
      <section class="exportBlock" data-kind={item.block.kind}>
        <span class="meta">{toLabel(item.start)}–{toLabel(item.end)} · {item.block.kind}</span>
        <h3 class="exportTitle">{item.block.title || "Untitled block"}</h3>
      </section>
    {:else}
      <p>No time blocks yet.</p>
    {/each}
  </div>
  <section class="exportNotes">
    <h2 class="heading">Day notes</h2>
    {previewText(snapshot.notes || "Nothing noted.", preview, 160)}
  </section>
</article>
