<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Slider, RadioGroup, Button } from "bits-ui";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import schema from "./schema";

  const orbs = [
    { value: 1, color: "var(--slop-orb1)", label: "Heavy / Reflective" },
    { value: 2, color: "var(--slop-orb2)", label: "Low / Subdued" },
    { value: 3, color: "var(--slop-orb3)", label: "Steady / Centered" },
    { value: 4, color: "var(--slop-orb4)", label: "Warm / Serene" },
    { value: 5, color: "var(--slop-orb5)", label: "Radiant / Energized" },
  ];

  const doc = useDocument(schema);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);
  let draftMood = $state(4);
  let draftEnergy = $state(4);
  let draftNote = $state("");

  function orbColor(value: number) {
    return orbs.find((orb) => orb.value === value)?.color ?? "var(--slop-orb3)";
  }

  function logEntry() {
    const note = draftNote.trim();
    if (!note) return;
    const now = new Date();
    const day = `${now.toLocaleDateString("en-US", { weekday: "short" }).toUpperCase()} ${now.getDate()}`;
    const first = doc.current.entries[0];
    doc.fields.entries.insert(
      { day, mood: draftMood, energy: draftEnergy, note },
      first ? { before: first.$id } : undefined,
    );
    draftNote = "";
  }
</script>

<Slop>
  <main class="sheet" data-slop-selection="none" aria-label="Mood journal">
    <input class="prompt" aria-label="Journal prompt" use:bindText={doc.fields.prompt} />
    <section class="checkin" data-slop-export="hide" aria-label="Today check in">
      <div class="orbs">
        <RadioGroup.Root class="orb-group" value={String(draftMood)} onValueChange={(value) => { const next = Number(value); if (next >= 1 && next <= 5) draftMood = next; }} aria-label="Select mood">
          {#each orbs as orb}
            <RadioGroup.Item value={String(orb.value)} class="orb" style="background:{orb.color}" aria-label={orb.label}></RadioGroup.Item>
          {/each}
        </RadioGroup.Root>
        <label class="energy">
          <span>Energy</span>
          <Slider.Root type="single" value={draftEnergy} onValueChange={(value) => draftEnergy = value} min={1} max={5} step={1} class="slider" aria-label="Energy level 1 to 5">
            {#snippet children({ thumbs })}
              <span class="track"><Slider.Range class="range" /></span>
              {#each thumbs as index}<Slider.Thumb {index} class="thumb" aria-label="Energy level" />{/each}
            {/snippet}
          </Slider.Root>
        </label>
      </div>
      <form class="composer" onsubmit={(event) => { event.preventDefault(); logEntry(); }}>
        <input placeholder="One line reflection…" aria-label="Reflection note" bind:value={draftNote} />
        <Button.Root class="log" type="submit">Log</Button.Root>
      </form>
    </section>
    <ul class="list" aria-label="Check-in history">
      {#each doc.current.entries as item (item.$id)}
        <li class="row" animate:flip={{ duration: flipMs }}>
          <span class="entry-orb" style:background={orbColor(item.mood)} aria-hidden="true"></span>
          <div class="meta"><span>{item.day}</span><small>⚡ {item.energy}/5</small></div>
          <p class="note">{item.note}</p>
          <button class="remove" data-slop-export="hide" aria-label="Delete entry" onclick={() => doc.fields.entries.remove(item.$id)}><Trash2 size={12} /></button>
        </li>
      {:else}
        <li class="empty">Nothing written yet. Log one line.</li>
      {/each}
    </ul>
    <footer class="foot"><span>{doc.current.entries.length} reflections recorded</span><span>Quiet journal</span></footer>
  </main>

  {#snippet exportView()}
    <article class="export-sheet" aria-label="Exported mood log">
      <h1 class="prompt">{doc.current.prompt}</h1>
      <ul class="list">
        {#each doc.current.entries as item (item.$id)}
          <li class="row">
            <span class="entry-orb" style:background={orbColor(item.mood)}></span>
            <div class="meta"><span>{item.day}</span><small>⚡ {item.energy}/5</small></div>
            <p class="note">{item.note}</p>
          </li>
        {/each}
      </ul>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="icon-surface" aria-hidden="true">
      <div class="icon-sheet">
        <div class="icon-title"></div>
        <div class="icon-orbs">
          {#each orbs as orb, index}<span style:background={orb.color} data-active={index === 4}></span>{/each}
        </div>
        <div class="icon-line"></div>
        <div class="icon-line"></div>
        <div class="icon-line"></div>
      </div>
    </div>
  {/snippet}
</Slop>
