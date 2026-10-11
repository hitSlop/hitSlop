<script lang="ts">
  import doc from "./schema";
  import CountdownFace from "./CountdownFace.svelte";
  import { countdownView } from "./countdown";
  type Milestone = (typeof doc.current.milestones)[number];

  const view = $derived(countdownView(doc.current, Date.now()));
</script>

<article class="exportShell">
  <header class="header">
    <span class="eyebrow">A date to remember</span>
  </header>
  <CountdownFace data={doc.current} state={view.state} />
  <div class="sectionHead">
    <h2 class="sectionTitle">Along the way</h2>
    <span class="eyebrow">{view.done}/{view.total}</span>
  </div>
  {@render milestoneList(doc.current.milestones)}
</article>

{#snippet milestoneList(items: readonly Milestone[])}
  <ol class="list">
    {#each items as item (item.$id)}
      <li class="row">

          <span class="check" data-state={item.done ? "checked" : "unchecked"} aria-label={item.done ? "Complete" : "Incomplete"}>{item.done ? "✓" : ""}</span>
          <span class="milestone" data-done={item.done}>{item.title}</span>

      </li>
    {:else}
      <li class="empty">No milestones yet.</li>
    {/each}
  </ol>
{/snippet}
