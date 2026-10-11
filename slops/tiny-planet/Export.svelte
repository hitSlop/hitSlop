<script lang="ts">
  import { capture } from "hitslop/svelte";
  import { onMount, tick } from "svelte";
  import doc from "./schema";
  import { PlanetScene, readPalette } from "./planet";
  import { worldView } from "./world";

  const view = $derived(worldView(doc.current));
  let figure: HTMLElement;
  let host = $state<HTMLDivElement>();
  let image = $state<HTMLImageElement>();
  let still = $state("");

  // Export is a fresh page: it builds a still, unspun scene from the saved seed, season and
  // things, keeps a PNG of it, and discards the WebGL renderer before capture.
  async function renderStill(): Promise<void> {
    if (still || !host) return;
    await tick();
    const scene = new PlanetScene({
      host,
      palette: readPalette(figure),
      seed: doc.current.seed,
      season: doc.current.season,
      reducedMotion: true,
      onPlace: () => {},
      onRemove: () => {},
      onMessage: () => {},
    });
    try {
      scene.setProps(doc.current.props.map((prop) => ({ id: prop.$id, kind: prop.kind, lat: prop.lat, lon: prop.lon })));
      still = scene.snapshot();
    } finally {
      scene.destroy();
    }
    await tick();
    await image?.decode().catch(() => {});
  }

  onMount(() => {
    const ready = renderStill();
    const stop = capture.onPrepare(() => ready);
    return stop;
  });
</script>

<figure bind:this={figure} class="planet-export">
  {#if still}
    <img bind:this={image} src={still} alt={`${view.title}, a low-poly world in ${view.season}`} />
  {:else}
    <div bind:this={host} class="planet-export-stage"></div>
  {/if}
  <figcaption><strong>{view.title}</strong><span>{view.things} · {view.season}</span></figcaption>
</figure>
