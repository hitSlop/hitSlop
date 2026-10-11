<script lang="ts">
  import { capture } from "hitslop/svelte";
  import { onMount, tick } from "svelte";
  import doc from "./schema";
  import { trayView } from "./dice";
  import History from "./History.svelte";
  import Readout from "./Readout.svelte";
  import Tray from "./Tray.svelte";

  const view = $derived(trayView(doc.current));
  const shown = $derived(view.last ? { id: view.last.$id, seed: view.last.seed, dice: view.last.dice } : undefined);
  const now = Date.now();
  let still = $state("");
  let image = $state<HTMLImageElement>();
  let finish: () => void = () => {};
  const ready = new Promise<void>((resolve) => (finish = resolve));

  // Export is a fresh page: it renders the last roll at rest once with WebGL, keeps that frame
  // as a PNG and drops the renderer before capture. Without WebGL the flat dice stay.
  async function took(url: string | null) {
    if (url) {
      still = url;
      await tick();
      await image?.decode().catch(() => {});
    }
    finish();
  }

  onMount(() => {
    const stop = capture.onPrepare(() => ready);
    const giveUp = setTimeout(finish, 8000);
    return () => { clearTimeout(giveUp); stop(); };
  });
</script>

<main class="tray-shell tray-export">
  <Readout roll={view.lastView}>
    {#snippet title()}<h1 class="plate-title">{view.title}</h1>{/snippet}
  </Readout>
  <section class="well">
    {#if still}
      <img bind:this={image} class="well-still" src={still} alt={view.lastView ? `The dice at rest: ${view.lastView.breakdown}` : "An empty dice tray"} />
    {:else}
      <Tray roll={shown} reduced still label="Dice tray" onStill={took} />
    {/if}
  </section>
  <section class="slip">
    <div class="slip-head"><h2>Recent rolls</h2></div>
    <History rolls={view.history.slice(0, 12)} {now} />
  </section>
</main>
