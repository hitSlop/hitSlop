<script lang="ts">
  import { Checkbox } from "bits-ui";
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import Check from "@lucide/svelte/icons/check";
  import doc from "./schema";
  import { journalView, setWritten } from "./journal";
  import WritingField from "./WritingField.svelte";

  let { period, staticView = false }: { period: "am" | "pm"; staticView?: boolean } = $props();
  const id = $props.id();
  const view = $derived(journalView(doc.current));
  const morning = $derived(period === "am");
  const done = $derived(morning ? view.morningDone : view.eveningDone);
  const gratitudes = $derived(view.gratitudes.map(line => ({ ...line, text: doc.fields[line.key] })));
  const intentions = $derived(view.intentions.map(line => ({ ...line, text: doc.fields[line.key] })));
  const highlights = $derived(view.highlights.map(line => ({ ...line, text: doc.fields[line.key] })));
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
        <WritingField value={line.value} text={staticView ? undefined : line.text} label="Gratitude {index + 1}" prefix={String(index + 1)} placeholder={line.placeholder} {staticView} />
      {/each}
    </div>
    <div class={"fmj-prompt"} role="group" aria-labelledby="{id}-intentions">
      <h3 class={"fmj-promptLabel"} id="{id}-intentions">What would make today great?</h3>
      {#each intentions as line, index}
        <WritingField value={line.value} text={staticView ? undefined : line.text} label="Intention {index + 1}" prefix={String(index + 1)} placeholder={line.placeholder} {staticView} />
      {/each}
    </div>
    <div class={"fmj-prompt"}>
      <h3 class={"fmj-promptLabel"}>Daily affirmation</h3>
      <WritingField value={doc.current.affirmation} text={staticView ? undefined : doc.fields.affirmation} label="Daily affirmation" prefix="I am" placeholder="Give yourself a few kind words…" {staticView} />
    </div>
  {:else}
    <div class={"fmj-prompt"} role="group" aria-labelledby="{id}-highlights">
      <h3 class={"fmj-promptLabel"} id="{id}-highlights">Three amazing things today…</h3>
      {#each highlights as line, index}
        <WritingField value={line.value} text={staticView ? undefined : line.text} label="Highlight {index + 1}" prefix={String(index + 1)} placeholder={line.placeholder} {staticView} />
      {/each}
    </div>
    <div class={"fmj-prompt"}>
      <h3 class={"fmj-promptLabel"}>How could today have been even better?</h3>
      <WritingField value={doc.current.lesson} text={staticView ? undefined : doc.fields.lesson} label="How could today have been even better?" placeholder="A lesson, a boundary, a gentler next time…" {staticView} />
    </div>
  {/if}

  <footer class={"fmj-pageFooter"}>
    {#if staticView}
      <span class={"fmj-writtenNote"}>{#if done}<Check size={16} />{/if}{done ? "Written" : "Still unfolding"}</span>
    {:else}
      <Checkbox.Root class={"fmj-completion"} checked={done} onCheckedChange={(checked) => doc.change(tx => setWritten(tx, period, checked === true))} aria-label="{done ? 'Written' : 'Mark as written'}: {morning ? 'Morning' : 'Evening'}">
        {#snippet children({ checked })}
          <span class={"fmj-checkBox"}>{#if checked}<Check size={15} strokeWidth={2.5} />{/if}</span>
          {checked ? "Written" : "Mark as written"}
        {/snippet}
      </Checkbox.Root>
    {/if}
    <span class={"fmj-closing"}>{morning ? "Carry this into your day." : "Let the rest wait until tomorrow."}</span>
  </footer>
</section>
