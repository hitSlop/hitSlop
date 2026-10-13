<script lang="ts">
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  import { trayView } from "./dice";
  import History from "./History.svelte";
  import Readout from "./Readout.svelte";
  const view = $derived(trayView(doc.current));
  const now = Date.now();
</script>

<main class="tray-shell tray-export">
  <Readout roll={view.lastView}>
    {#snippet title()}<h1 class="plate-title">{view.title}</h1>{/snippet}
  </Readout>
  <section class="well">
    <div class="static-dice">
      {#each view.lastView?.values ?? [] as die}
        <div class="static-die"><strong>{die.value}</strong><span>{die.kind}</span></div>
      {:else}<p>The tray is ready for a roll.</p>{/each}
    </div>
  </section>
  <section class="slip">
    <div class="slip-head"><h2>Recent rolls</h2></div>
    <History rolls={preview ? view.history.slice(0, 4) : view.history} {now} />
  </section>
</main>

<style>
.static-dice { display: flex; flex-wrap: wrap; justify-content: center; align-items: center; gap: 20px; min-height: 240px; padding: 28px; }
.static-die { display: flex; flex-direction: column; align-items: center; justify-content: center; width: 84px; height: 84px; border: 3px solid var(--slop-pip); border-radius: 16px; background: var(--slop-die); color: var(--slop-pip); box-shadow: 4px 5px 0 #00000028; }
.static-die strong { font: 700 32px/1.1 serif; } .static-die span { font: 12px/1.5 sans-serif; }
</style>
