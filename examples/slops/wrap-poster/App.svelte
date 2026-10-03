<script lang="ts">
  import { bindText } from "@hitslop/document/svelte";
  import { onMount } from "svelte";
  import { Button } from "bits-ui";
  import Circle from "@lucide/svelte/icons/circle";
  import Square from "@lucide/svelte/icons/square";
  import Type from "@lucide/svelte/icons/type";
  import Check from "@lucide/svelte/icons/check";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash from "@lucide/svelte/icons/trash-2";
  import doc, { maxShapes, tones } from "./schema";
  import Poster from "./Poster.svelte";
  import { PAGE } from "./fonts.svelte";
  import type { PosterShape } from "./types";

  let stage: HTMLElement;
  let scale = $state(1);
  let selected = $state<string | null>(null);
  let drag = $state<{ id: string; x: number; y: number } | null>(null);
  let editing = $state(false);
  let fits = $state(true);

  const clamp = (value: number, low = 0, high = 1) => Math.min(high, Math.max(low, value));
  const shapes = $derived<PosterShape[]>(doc.current.shapes.map((shape) => {
    const live = drag?.id === shape.$id ? drag : null;
    return { id: shape.$id, kind: shape.kind, tone: shape.tone, x: live?.x ?? shape.x, y: live?.y ?? shape.y, size: shape.size };
  }));
  const chosen = $derived(doc.current.shapes.find((shape) => shape.$id === selected));
  const full = $derived(doc.current.shapes.length >= maxShapes);

  onMount(() => {
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      scale = Math.max(0.3, Math.min((entry.contentRect.width - 24) / PAGE.width, (entry.contentRect.height - 24) / PAGE.height, 1.6));
    });
    observer.observe(stage);
    return () => observer.disconnect();
  });

  function down(event: PointerEvent, id: string) {
    selected = id;
    const shape = doc.current.shapes.find((candidate) => candidate.$id === id);
    if (!shape) return;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const start = { x: event.clientX, y: event.clientY, at: { x: shape.x, y: shape.y } };
    const move = (next: PointerEvent) => {
      drag = { id, x: clamp(start.at.x + (next.clientX - start.x) / scale / PAGE.width), y: clamp(start.at.y + (next.clientY - start.y) / scale / PAGE.height) };
    };
    const up = async () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
      target.removeEventListener("pointercancel", up);
      const final = drag;
      try {
        if (final && final.id === id) await doc.change((tx) => { const row = tx.at(shape); row.x.set(final.x); row.y.set(final.y); });
      } finally {
        drag = null;
      }
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
    target.addEventListener("pointercancel", up);
  }

  function key(event: KeyboardEvent, id: string) {
    const shape = doc.current.shapes.find((candidate) => candidate.$id === id);
    if (!shape) return;
    const step = event.shiftKey ? 0.05 : 0.01;
    const row = doc.at(shape);
    const handled = ({
      ArrowLeft: () => row.x.set(clamp(shape.x - step)),
      ArrowRight: () => row.x.set(clamp(shape.x + step)),
      ArrowUp: () => row.y.set(clamp(shape.y - step)),
      ArrowDown: () => row.y.set(clamp(shape.y + step)),
      "+": () => resize(1),
      "=": () => resize(1),
      "-": () => resize(-1),
      Delete: () => remove(),
      Backspace: () => remove(),
    } as Record<string, () => void>)[event.key];
    if (!handled) return;
    event.preventDefault();
    selected = id;
    handled();
  }

  async function add(kind: "circle" | "square") {
    if (full) return;
    const { id } = await doc.fields.shapes.insert({ kind, tone: tones[doc.current.shapes.length % tones.length]!, x: 0.5, y: 0.55, size: kind === "circle" ? 0.28 : 0.22 });
    selected = id;
  }
  function resize(direction: 1 | -1) {
    if (chosen) doc.at(chosen).size.set(clamp(Math.round((chosen.size + direction * 0.04) * 100) / 100, 0.12, 0.6));
  }
  function remove() {
    if (!selected) return;
    doc.fields.shapes.remove(selected);
    selected = null;
  }
</script>

<main class="desk" data-slop-selection="none" aria-label="Wrap poster">
  <div bind:this={stage} class="stage" onpointerdown={(event) => { if (event.target === event.currentTarget) selected = null; }} role="presentation">
    <div class="fit" style:width="{PAGE.width * scale}px" style:height="{PAGE.height * scale}px">
      <div class="scale" style:transform="scale({scale})">
        <Poster title={doc.current.title} body={doc.current.body} {shapes} {selected} interactive bind:fits onshapedown={down} onshapekey={key} />
      </div>
    </div>
  </div>

  {#if editing}
    <section class="sheet" aria-label="Edit text">
      <input class="title-input" aria-label="Poster title" placeholder="Title" use:bindText={doc.fields.title} />
      <textarea aria-label="Poster text" rows="6" placeholder="Write the story…" use:bindText={doc.fields.body}></textarea>
    </section>
  {/if}

  <div class="bar" role="toolbar" aria-label="Poster tools">
    {#if !fits}<p class="warn" role="status">Some text doesn’t fit. Shrink a shape or trim the words.</p>{/if}
    <div class="group">
      <Button.Root class="chip" onclick={() => add("circle")} disabled={full}><Circle size={16} />Circle</Button.Root>
      <Button.Root class="chip" onclick={() => add("square")} disabled={full}><Square size={16} />Square</Button.Root>
      <Button.Root class="chip" aria-pressed={editing} onclick={() => (editing = !editing)}>{#if editing}<Check size={16} />Done{:else}<Type size={16} />Edit text{/if}</Button.Root>
    </div>
    {#if chosen}
      <div class="group" aria-label="Selected shape">
        {#each tones as tone}
          <button class="swatch" data-tone={tone} aria-label={`Make it ${tone}`} aria-pressed={chosen.tone === tone} onclick={() => doc.at(chosen).tone.set(tone)}></button>
        {/each}
        <Button.Root class="chip icon" aria-label="Smaller" onclick={() => resize(-1)}><Minus size={16} /></Button.Root>
        <Button.Root class="chip icon" aria-label="Bigger" onclick={() => resize(1)}><Plus size={16} /></Button.Root>
        <Button.Root class="chip icon" aria-label="Remove shape" onclick={remove}><Trash size={16} /></Button.Root>
      </div>
    {/if}
  </div>
</main>
