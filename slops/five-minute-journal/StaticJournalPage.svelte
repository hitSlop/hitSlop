<script lang="ts">
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import Check from "@lucide/svelte/icons/check";
  import { journalView, type Data } from "./journal";
  import WritingField from "./StaticWritingField.svelte";

  let { period, snapshot }: { period: "am" | "pm"; snapshot: Data } = $props();
  const id = $props.id();
  const view = $derived(journalView(snapshot));
  const morning = $derived(period === "am");
  const done = $derived(morning ? view.morningDone : view.eveningDone);
  const gratitudes = $derived(view.gratitudes);
  const intentions = $derived(view.intentions);
  const highlights = $derived(view.highlights);
</script>

<section class={"fmj-page"} data-period={period} aria-labelledby="{id}-heading">
  <header class={"fmj-pageHeading"}>
    <div>
      <h2 id="{id}-heading" class={"fmj-heading"}>{morning ? "Morning" : "Evening"}</h2>
      <p class={"fmj-intro"}>{morning ? "A little intention for the day ahead." : "A moment to gather the good."}</p>
    </div>
    <span class={"fmj-periodMark"} aria-hidden="true">
      {#if morning}<Sun size={42} strokeWidth={1.25} />{:else}<Moon size={42} strokeWidth={1.25} />{/if}
    </span>
  </header>

  {#if morning}
    <div class={"fmj-prompt"} role="group" aria-labelledby="{id}-gratitudes">
      <h3 class={"fmj-promptLabel"} id="{id}-gratitudes">I am grateful for…</h3>
      {#each gratitudes as line, index}
        <WritingField value={line.value} label="Gratitude {index + 1}" prefix={String(index + 1)} placeholder={line.placeholder} />
      {/each}
    </div>
    <div class={"fmj-prompt"} role="group" aria-labelledby="{id}-intentions">
      <h3 class={"fmj-promptLabel"} id="{id}-intentions">What would make today great?</h3>
      {#each intentions as line, index}
        <WritingField value={line.value} label="Intention {index + 1}" prefix={String(index + 1)} placeholder={line.placeholder} />
      {/each}
    </div>
    <div class={"fmj-prompt"}>
      <h3 class={"fmj-promptLabel"}>Daily affirmation</h3>
      <WritingField value={snapshot.affirmation} label="Daily affirmation" prefix="I am" placeholder="Give yourself a few kind words…" />
    </div>
  {:else}
    <div class={"fmj-prompt"} role="group" aria-labelledby="{id}-highlights">
      <h3 class={"fmj-promptLabel"} id="{id}-highlights">Three amazing things today…</h3>
      {#each highlights as line, index}
        <WritingField value={line.value} label="Highlight {index + 1}" prefix={String(index + 1)} placeholder={line.placeholder} />
      {/each}
    </div>
    <div class={"fmj-prompt"}>
      <h3 class={"fmj-promptLabel"}>How could today have been even better?</h3>
      <WritingField value={snapshot.lesson} label="How could today have been even better?" placeholder="A lesson, a boundary, a gentler next time…" />
    </div>
  {/if}

  <footer class={"fmj-pageFooter"}>
    <span class="fmj-writtenNote">{#if done}<Check size={16} />{/if}{done ? "Written" : "Still unfolding"}</span>
    <span class={"fmj-closing"}>{morning ? "Carry this into your day." : "Let the rest wait until tomorrow."}</span>
  </footer>
</section>
