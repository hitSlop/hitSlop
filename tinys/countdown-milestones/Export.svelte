<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { Value } from "@hitslop/tinys";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import CountdownFace from "./CountdownFace.svelte";
  import { countdownView } from "./countdown";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();
  type Milestone = Value<typeof schema.descriptor>["milestones"][number];

  const view = $derived(countdownView(snapshot, Date.now()));
</script>

<article class="exportShell">
  <header class="header">
    <span class="eyebrow">A date to remember</span>
  </header>
  <CountdownFace data={snapshot} state={view.state} />
  <div class="sectionHead">
    <h2 class="sectionTitle">Along the way</h2>
    <span class="eyebrow">{view.done}/{view.total}</span>
  </div>
  {@render milestoneList(snapshot.milestones)}
</article>

{#snippet milestoneList(items: readonly Milestone[])}
  <ol class="list">
    {#each (preview ? (items).slice(0, 5) : items) as item (item.$id)}
      <li class="row">

          <span class="check" data-state={item.done ? "checked" : "unchecked"} aria-label={item.done ? "Complete" : "Incomplete"}>{item.done ? "✓" : ""}</span>
          <span class="milestone" data-done={item.done}>{item.title}</span>

      </li>
    {:else}
      <li class="empty">No milestones yet.</li>
    {/each}
  </ol>
{/snippet}
