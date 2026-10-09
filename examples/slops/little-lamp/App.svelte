<script lang="ts">
  import doc from "./schema";
  import Lamp from "./Lamp.svelte";
  let lookX = $state(0);
  let lookY = $state(0);
  function look(event: PointerEvent) {
    const box = event.currentTarget instanceof HTMLElement ? event.currentTarget.getBoundingClientRect() : null;
    if (!box) return;
    lookX = Math.max(-1, Math.min(1, (event.clientX - box.left) / box.width * 2 - 1));
    lookY = Math.max(-1, Math.min(1, (event.clientY - box.top) / box.height * 2 - 1));
  }
</script>

<button class="lamp-companion" aria-label="Little Lamp" aria-pressed={doc.current.on}
  onclick={() => doc.fields.on.set(!doc.current.on)}
  onpointermove={look} onpointerleave={() => { lookX = 0; lookY = 0; }}>
  <Lamp on={doc.current.on} {lookX} {lookY} />
</button>
