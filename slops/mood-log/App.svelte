<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, draft } from "hitslop/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Slider, RadioGroup, Button } from "bits-ui";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import doc from "./schema";
  import { addEntry, moodView, removeEntry } from "./entries";
  import { orbs, orbColor } from "./orbs";

  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);
  const view = $derived(moodView(doc.current));
  let draftMood = $state(4);
  let draftEnergy = $state(4);
  const newNote = draft((note) => doc.change((tx) => addEntry(tx, doc.current, { mood: draftMood, energy: draftEnergy, note }, new Date())));
  let noteInput = $state<HTMLInputElement>();

  async function logEntry(event: SubmitEvent) {
    if (await newNote.submit(event)) noteInput?.focus();
  }
</script>

  <main class="sheet" aria-label="Mood journal">
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
      <form class="composer" onsubmit={logEntry}>
        <input bind:this={noteInput} bind:value={newNote.value} placeholder="One line reflection…" aria-label="Reflection note" />
        <Button.Root class="log" type="submit" disabled={!newNote.ready}>Log</Button.Root>
      </form>
    </section>
    <ul class="list" aria-label="Check-in history">
      {#each view.entries as item (item.$id)}
        <li class="row" animate:flip={{ duration: flipMs }}>
          <span class="entry-orb" style:background={orbColor(item.mood)} aria-hidden="true"></span>
          <div class="meta"><span>{item.day}</span><small>⚡ {item.energy}/5</small></div>
          <p class="note">{item.note}</p>
          <button class="remove" data-slop-export="hide" aria-label="Delete entry" onclick={() => doc.change((tx) => removeEntry(tx, item.$id))}><Trash2 size={12} /></button>
        </li>
      {:else}
        <li class="empty">Nothing written yet. Log one line.</li>
      {/each}
    </ul>
    <footer class="foot"><span>{view.count} reflections recorded</span><span>Quiet journal</span></footer>
  </main>
