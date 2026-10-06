<script lang="ts">
  import { onMount } from "svelte";
  import { bindText, resizeWindow } from "hitslop/svelte";
  import doc from "./schema";
  import variant from "./variant";
  let root: HTMLElement;
  let size = $state({ width: 0, height: 0 });
  let pointer = $state("Move over the grid");
  let error = $state("");
  onMount(() => {
    const observer = new ResizeObserver(([entry]) => {
      if (entry) size = { width: Math.round(entry.contentRect.width), height: Math.round(entry.contentRect.height) };
    });
    observer.observe(root);
    return () => observer.disconnect();
  });
  async function hit(target: string) {
    try {
      await doc.change(tx => { tx.fields.hits.increment(); tx.fields.lastTarget.set(target); });
      error = "";
    } catch (cause) { error = String(cause); }
  }
  async function resize(width: number, height: number) {
    try { await resizeWindow({ width, height }); error = ""; }
    catch (cause) { error = String(cause); }
  }
  function locate(event: PointerEvent) {
    const rect = root.getBoundingClientRect();
    pointer = `${Math.round(event.clientX - rect.left)}, ${Math.round(event.clientY - rect.top)}`;
  }
</script>

<main data-variant={variant.kind} bind:this={root} class:shape-lab-skin={variant.skin} class="shape-lab" onpointermove={locate}>
  <header class="shape-lab-heading"><span>01 / TOP</span><h1>Shape Lab</h1><p>{variant.name}</p><output class="shape-lab-skin-count">{doc.current.hits} clicks · {doc.current.lastTarget}</output></header>
  <button class="shape-lab-edge shape-lab-north" onclick={() => hit("North")}>N ↑</button>
  <button class="shape-lab-edge shape-lab-west" onclick={() => hit("West")}>W ←</button>
  <button class="shape-lab-edge shape-lab-east" onclick={() => hit("East")}>E →</button>
  <button class="shape-lab-edge shape-lab-south" onclick={() => hit("South")}>S ↓</button>
  <section class="shape-lab-readout" aria-label="Observed input">
    <span class="shape-lab-caption">ACCEPTED CLICKS</span><strong>{doc.current.hits}</strong>
    <p>Last: {doc.current.lastTarget}</p><p class="shape-lab-expectation">{variant.expectation}</p>
  </section>
  <label class="shape-lab-note">Edit / capture check<input aria-label="Capture note" use:bindText={doc.fields.note} /></label>
  <footer class="shape-lab-tools">
    <output aria-label="Window dimensions">{size.width} × {size.height}</output>
    <output class="shape-lab-pointer">{pointer}</output>
    {#if !variant.skin}<div class="shape-lab-resize" data-slop-export="hide"><button onclick={() => resize(480, 360)}>480 × 360</button><button onclick={() => resize(600, 400)}>600 × 400</button></div>{/if}
    <span>02 / BOTTOM</span>
  </footer>
  {#if error}<p role="alert" class="shape-lab-error">{error}</p>{/if}
</main>
