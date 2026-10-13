<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { displayDate, journalView } from "./journal";
  import { previewText } from "../_shared/preview";
  import PreviewNote from "../_shared/PreviewNote.svelte";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const view = $derived(journalView(snapshot));
</script>

<article class="fmj-exportBook fmj-book">
  <header class="fmj-masthead"><h1 class="fmj-title">Five Minute Journal</h1><p class="fmj-exportDate">{displayDate(snapshot.date)}</p></header>
  <div class="preview-periods">
    <section>
      <h2 class="fmj-heading">Morning</h2>
      <h3 class="fmj-promptLabel">I am grateful for…</h3>
      {#each view.gratitudes.filter(line => line.value.trim()).slice(0, 2) as line}<p>{previewText(line.value, true, 100)}</p>{:else}<p class="quiet">A little gratitude to come.</p>{/each}
      <h3 class="fmj-promptLabel">What would make today great?</h3>
      {#each view.intentions.filter(line => line.value.trim()).slice(0, 2) as line}<p>{previewText(line.value, true, 100)}</p>{:else}<p class="quiet">Room for a good intention.</p>{/each}
      {#if snapshot.affirmation.trim()}<h3 class="fmj-promptLabel">I am…</h3><p>{previewText(snapshot.affirmation, true, 100)}</p>{/if}
      <small>{view.morningDone ? "Written" : "Still unfolding"}</small>
    </section>
    <section>
      <h2 class="fmj-heading">Evening</h2>
      <h3 class="fmj-promptLabel">Amazing things today</h3>
      {#each view.highlights.filter(line => line.value.trim()).slice(0, 2) as line}<p>{previewText(line.value, true, 100)}</p>{:else}<p class="quiet">A moment to gather the good.</p>{/each}
      <h3 class="fmj-promptLabel">A lesson for tomorrow</h3>
      <p>{previewText(snapshot.lesson, true, 140) || "Let the rest wait until tomorrow."}</p>
      <small>{view.eveningDone ? "Written" : "Still unfolding"}</small>
    </section>
  </div>
</article>
<PreviewNote>Open the journal for every reflection.</PreviewNote>

<style>
  .preview-periods { display: grid; grid-template-columns: 1fr 1fr; gap: 28px; padding: 24px 28px 32px; }
  section { min-width: 0; }
  h2 { margin: 0 0 20px; }
  h3 { margin: 18px 0 8px; }
  p { margin: 8px 0; font-size: 14px; line-height: 1.55; white-space: pre-wrap; overflow-wrap: anywhere; }
  small, .quiet { opacity: 0.65; }
</style>
