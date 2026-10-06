<script lang="ts">
  import { bindText } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import doc, { slots, type Slot } from "./schema";
  import { maxOptions, play, slotLabel, spiralPath } from "./fate";
  import Future from "./Future.svelte";
  import { ui } from "./ui.svelte";

  const fate = $derived(play(doc.current.options, doc.current.loops));
  const animate = $derived(ui.playing && !prefersReducedMotion.current);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => () => clearTimeout(timer));

  function spin() {
    const loops = 2 + Math.floor(Math.random() * 8);
    clearTimeout(timer);
    ui.playing = true;
    doc.fields.loops.set(loops);
    timer = setTimeout(() => { ui.playing = false; }, 1500 + (fate.struck.length + 6) * 220);
  }

  function addOption(slot: Slot) {
    doc.fields.options.insert({ slot, text: "" });
  }
</script>

<main class="page" data-slop-selection="none" data-playing={animate ? "" : undefined} aria-label="MASH">
  <div class="coils" aria-hidden="true">{#each Array(14) as _}<i></i>{/each}</div>

  <header class="top">
    <h1 class="logo" aria-label="M.A.S.H.">M<span>·</span>A<span>·</span>S<span>·</span>H</h1>
    <input class="title" aria-label="Whose future" placeholder="Whose future? Write your name" use:bindText={doc.fields.title} />
  </header>

  <section class="spin-zone">
    <svg class="spiral" viewBox="0 0 120 120" role="img" aria-label={doc.current.loops ? `A spiral with ${doc.current.loops} loops` : "No spiral yet"}>
      {#if doc.current.loops}
        {#key doc.current.loops}
          <path d={spiralPath(doc.current.loops)} pathLength="1" />
        {/key}
      {:else}
        <circle cx="60" cy="60" r="44" class="blank" /><text x="60" y="72" text-anchor="middle">?</text>
      {/if}
    </svg>
    <div class="spin-side">
      <button class="spin" onclick={spin}>{doc.current.loops ? "Spin again" : "Spin the spiral"}</button>
      <p class="loops">{doc.current.loops ? `${doc.current.loops} loops = count by ${doc.current.loops}` : "Draws 2–9 loops"}</p>
    </div>
  </section>

  <Future />

  <div class="blocks">
    {#each slots as slot (slot)}
      {@const options = doc.current.options.filter((option) => option.slot === slot)}
      <section class="block" aria-label={slotLabel[slot]}>
        <h3>{slotLabel[slot]}</h3>
        <ul>
          {#each options as option (option.$id)}
            {@const order = fate.struck.indexOf(option.$id)}
            <li class="opt" data-struck={order >= 0 ? "" : undefined} data-pick={fate.picks[slot] === option.$id ? "" : undefined} style:--i={order}>
              <input aria-label={`${slotLabel[slot]} option`} placeholder="…" use:bindText={doc.at(option).text} />
              <button class="x" aria-label="Remove this option" onclick={() => doc.fields.options.remove(option.$id)}><X size={14} /></button>
            </li>
          {/each}
        </ul>
        {#if options.length < maxOptions}
          <button class="add" onclick={() => addOption(slot)}><Plus size={14} />Add option</button>
        {/if}
      </section>
    {/each}
  </div>
</main>
