<script lang="ts">
  import { mashView, type Mash } from "./options";
  import { slotLabel } from "./fate";

  let { snapshot }: { snapshot: Mash } = $props();
  const view = $derived(mashView(snapshot));
</script>

<aside class="sticky" aria-label="Your future">
  <span class="tape" aria-hidden="true"></span>
  <h2>{view.title ? `${view.title}’s future` : "Your future"}</h2>
  {#if view.loops === 0}
    <p class="waiting">Spin the spiral to find out…</p>
  {:else}
    <dl>
      {#each view.picked as row (row.slot)}
        <div><dt>{slotLabel[row.slot]}</dt><dd>{row.text}</dd></div>
      {/each}
    </dl>
  {/if}
</aside>
