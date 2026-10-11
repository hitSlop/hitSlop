<script lang="ts">
  import { capture } from "hitslop/svelte";
  import { onMount, tick } from "svelte";
  import doc from "./schema";
  import { pondView } from "./shared";
  import { mulberry, PondScene, readPalette } from "./pond";

  const view = $derived(pondView(doc.current));
  let figure: HTMLElement;
  let host = $state<HTMLDivElement>();
  let image = $state<HTMLImageElement>();
  let still = $state("");

  // Export is a fresh page: it paints a still pond from the saved seed and koi (fish placed
  // from the seed, so the frame repeats), keeps a PNG of it and discards both p5 sketches.
  async function renderStill(): Promise<void> {
    if (still || !host) return;
    await tick();
    const scene = new PondScene({
      host,
      palette: readPalette(figure),
      seed: doc.current.seed,
      reducedMotion: true,
      random: mulberry(doc.current.seed),
      onFeed: () => {},
    });
    try {
      scene.setKoi(doc.current.koi.map((k) => ({ id: k.$id, name: k.name, pattern: k.pattern, size: k.size })));
      still = await scene.snapshot();
    } finally {
      scene.destroy();
    }
    await tick();
    await image?.decode().catch(() => {});
  }

  onMount(() => {
    const ready = renderStill();
    return capture.onPrepare(() => ready);
  });
</script>

<figure bind:this={figure} class="koi-export">
  {#if still}
    <img bind:this={image} src={still} alt={`Watercolor koi pond with ${view.count ? view.names : "no koi"}`} />
  {:else}
    <div bind:this={host} class="koi-export-stage"></div>
  {/if}
  <figcaption><strong>Koi Pond</strong><span>{view.count ? view.names : "A quiet pond"} · {view.feedingLabel}</span></figcaption>
</figure>
