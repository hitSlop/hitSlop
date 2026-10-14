<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { onMount } from "svelte";
  import Layers from "@lucide/svelte/icons/layers";
  import Settings from "@lucide/svelte/icons/settings-2";
  import { getDocument } from "hitslop/svelte";
  import schema from "./schema";
  import { deckView, localDay } from "./deck";
  import Header from "./Header.svelte";
  import Readouts from "./Readouts.svelte";
  import Session from "./Session.svelte";
  import SettingsDialog from "./SettingsDialog.svelte";
  import DeckDialog from "./DeckDialog.svelte";

  const doc = getDocument(schema);

  // The clock is view state: it ticks while the window is visible and on every answer.
  let now = $state(Date.now());
  const offset = $derived(-new Date(now).getTimezoneOffset());
  const clock = () => (now = Date.now());
  onMount(() => {
    const timer = setInterval(() => { if (!document.hidden) clock(); }, 30_000);
    const visible = () => { if (!document.hidden) clock(); };
    document.addEventListener("visibilitychange", visible);
    return () => { clearInterval(timer); document.removeEventListener("visibilitychange", visible); };
  });

  const view = $derived(deckView(doc.current, now, offset));
  const today = $derived(localDay(now, offset));
  let studying = $state(false);
  let settingsOpen = $state(false);
  let cardsOpen = $state(false);
  let startButton = $state<HTMLButtonElement>();

  const planned = $derived(view.forecast?.today.planned ?? 0);
  const canStudy = $derived(!view.problem && doc.current.cards.length > view.unreadable.size);

  function start() {
    clock();
    studying = true;
  }
  function end() {
    studying = false;
    clock();
    queueMicrotask(() => startButton?.focus());
  }
</script>

<main class="deck-sheet">
  <div class="deck-timing" aria-hidden="true"></div>
  <div class="deck-body">
    <Header {view} title={doc.current.title} field={doc.fields.title} onexam={() => (settingsOpen = true)} />

    {#if studying}
      <Session {now} {offset} {clock} onend={end} />
    {:else}
      <Readouts data={doc.current} {view}>
        <button class="deck-primary deck-start" bind:this={startButton} onclick={start} disabled={!canStudy}>
          {#if planned > 0}Study today's {planned} {planned === 1 ? "card" : "cards"}{:else if view.cutByCap > 0}Budget spent · study anyway{:else}All caught up · open session{/if}
        </button>
      </Readouts>
      {#if view.unreadable.size}
        <p class="deck-warning" role="status">{view.unreadable.size} {view.unreadable.size === 1 ? "card is" : "cards are"} unreadable and left out of the plan. See Cards.</p>
      {/if}
    {/if}

    <footer class="deck-foot">
      <button class="deck-tab" onclick={() => (cardsOpen = true)}><Layers size={17} /> Cards <span>{doc.current.cards.length}</span></button>
      <button class="deck-tab" onclick={() => (settingsOpen = true)}><Settings size={17} /> Settings</button>
    </footer>
  </div>
</main>

<SettingsDialog bind:open={settingsOpen} {today} {view} />
<DeckDialog bind:open={cardsOpen} {now} {view} />
