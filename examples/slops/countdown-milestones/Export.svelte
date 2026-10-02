<script lang="ts">
  import doc, { type Milestone } from "./schema";
  import CountdownFace from "./CountdownFace.svelte";
  import { countdownState } from "./countdown";

  const done = $derived(doc.current.milestones.filter((item) => item.done).length);
  const view = $derived(countdownState(doc.current.targetDate, doc.current.targetTime, Date.now()));
</script>

<article class="exportShell">
  <header class="header">
    <span class="eyebrow">A date to remember</span>
  </header>
  <CountdownFace data={doc.current} state={view} />
  <div class="sectionHead">
    <h2 class="sectionTitle">Along the way</h2>
    <span class="eyebrow">{done}/{doc.current.milestones.length}</span>
  </div>
  {@render milestoneList(doc.current.milestones, false)}
</article>

{#snippet milestoneList(items: readonly Milestone[], editable: boolean)}
  <ol class="list">
    {#each items as item, index (item.$id)}
      <li class="row">

          <span class="check" data-state={item.done ? "checked" : "unchecked"} aria-label={item.done ? "Complete" : "Incomplete"}>{item.done ? "✓" : ""}</span>
          <span class="milestone" data-done={item.done}>{item.title}</span>

      </li>
    {:else}
      <li class="empty">No milestones yet.</li>
    {/each}
  </ol>
{/snippet}
