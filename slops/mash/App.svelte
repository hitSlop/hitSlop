<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "hitslop/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import schema from "./schema";
  import { maxOptions, spiralPath } from "./fate";
  import { addOption, mashView, removeOption, spin } from "./options";
  import Future from "./Future.svelte";

  const doc = getDocument(schema);
  const view = $derived(mashView(doc.current));
  // UI-only: true while a fresh spin animates. A document that opens never animates.
  let playing = $state(false);
  const animate = $derived(playing && !prefersReducedMotion.current);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => () => clearTimeout(timer));

  async function spinSpiral() {
    clearTimeout(timer);
    const loops = await doc.change(tx => spin(tx, Math.random));
    playing = true;
    const struck = mashView(doc.current).fate.struck.length;
    timer = setTimeout(() => { playing = false; }, 1500 + (struck + 6) * 220);
  }
</script>

<main class="page" data-playing={animate ? "" : undefined} aria-label="MASH">
  <div class="coils" aria-hidden="true">{#each Array(14) as _}<i></i>{/each}</div>

  <header class="top">
    <h1 class="logo" aria-label="M.A.S.H.">M<span>·</span>A<span>·</span>S<span>·</span>H</h1>
    <input class="title" aria-label="Whose future" placeholder="Whose future? Write your name" use:bindText={doc.fields.title} />
  </header>

  <section class="spin-zone">
    <svg class="spiral" viewBox="0 0 120 120" role="img" aria-label={view.loops ? `A spiral with ${view.loops} loops` : "No spiral yet"}>
      {#if view.loops}
        {#key view.loops}
          <path d={spiralPath(view.loops)} pathLength="1" />
        {/key}
      {:else}
        <circle cx="60" cy="60" r="44" class="blank" /><text x="60" y="72" text-anchor="middle">?</text>
      {/if}
    </svg>
    <div class="spin-side">
      <button class="spin" onclick={spinSpiral}>{view.loops ? "Spin again" : "Spin the spiral"}</button>
      <p class="loops">{view.loops ? `${view.loops} loops = count by ${view.loops}` : "Draws 2–9 loops"}</p>
    </div>
  </section>

  <Future snapshot={doc.current} />

  <div class="blocks">
    {#each view.blocks as block (block.slot)}
      <section class="block" aria-label={block.label}>
        <h3>{block.label}</h3>
        <ul>
          {#each block.options as option (option.$id)}
            {@const order = view.fate.struck.indexOf(option.$id)}
            <li class="opt" data-struck={order >= 0 ? "" : undefined} data-pick={view.fate.picks[block.slot] === option.$id ? "" : undefined} style:--i={order}>
              <input aria-label={`${block.label} option`} placeholder="…" use:bindText={doc.at(option).text} />
              <button class="x" aria-label="Remove this option" onclick={() => doc.change(tx => removeOption(tx, option.$id))}><X size={14} /></button>
            </li>
          {/each}
        </ul>
        {#if block.options.length < maxOptions}
          <button class="add" onclick={() => doc.change(tx => addOption(tx, block.slot, doc.current))}><Plus size={14} />Add option</button>
        {/if}
      </section>
    {/each}
  </div>
</main>
