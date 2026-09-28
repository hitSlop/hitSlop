<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { Tabs, Collapsible } from "bits-ui";
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import schema from "./schema";
  import { initialView, type View } from "./journal";
  import DatePicker from "./DatePicker.svelte";
  import JournalPage from "./JournalPage.svelte";
  import WritingField from "./WritingField.svelte";
  import Icon from "./Icon.svelte";
  import Export from "./Export.svelte";

  const doc = useDocument(schema);
  let view = $state<View>(initialView(doc.current));
  let thoughtOpen = $state(Boolean(doc.current.quote.trim() || doc.current.quoteAuthor.trim()));
</script>

<Slop>
  <main class={"fmj-book"} data-slop-selection="none" aria-label="Five minute journal">
    <header class={"fmj-masthead"}>
      <h1 class={"fmj-title"}>Five Minute Journal</h1>
      <DatePicker bind:value={() => doc.current.date, (value) => doc.fields.date.set(value)} />
    </header>
    <Tabs.Root value={view} onValueChange={(value) => { if (value === "am" || value === "pm" || value === "all") view = value; }} class={"fmj-journal"}>
      <Tabs.List class={"fmj-tabs"} aria-label="Journal view">
        <Tabs.Trigger value="am" class={"fmj-tab"} data-period="am"><Sun size={17} />Morning{#if doc.current.morningDone}<Check size={14} aria-label="Written" />{/if}</Tabs.Trigger>
        <Tabs.Trigger value="pm" class={"fmj-tab"} data-period="pm"><Moon size={17} />Evening{#if doc.current.eveningDone}<Check size={14} aria-label="Written" />{/if}</Tabs.Trigger>
        <Tabs.Trigger value="all" class={"fmj-dayTab fmj-tab"}>Day</Tabs.Trigger>
      </Tabs.List>
      <Tabs.Content value="am" class={"fmj-panel"}><JournalPage period="am" /></Tabs.Content>
      <Tabs.Content value="pm" class={"fmj-panel"}><JournalPage period="pm" /></Tabs.Content>
      <Tabs.Content value="all" class={"fmj-panel"}>
        <JournalPage period="am" />
        <JournalPage period="pm" />
      </Tabs.Content>
    </Tabs.Root>
    <Collapsible.Root bind:open={thoughtOpen} class={"fmj-thought"}>
      <Collapsible.Trigger class={"fmj-thoughtTrigger"}>
        {#if !thoughtOpen}<Plus size={15} />{/if}
        {thoughtOpen ? "A thought for this day" : (doc.current.quote || doc.current.quoteAuthor ? "Show this day’s thought" : "Add a thought")}
        {#if thoughtOpen}<span class={"fmj-hideLabel"}>Hide</span>{/if}
      </Collapsible.Trigger>
      <Collapsible.Content>
        <WritingField value={doc.current.quote} text={doc.fields.quote} label="Inspiring thought" placeholder="A thought to carry with you…" />
        <input class={"fmj-authorField"} aria-label="Quote author" use:bindText={doc.fields.quoteAuthor} placeholder="Author, if there is one" />
      </Collapsible.Content>
    </Collapsible.Root>
  </main>

  {#snippet exportView()}
    <Export />
  {/snippet}
  {#snippet icon()}
    <Icon />
  {/snippet}
</Slop>
