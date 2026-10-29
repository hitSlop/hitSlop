<script lang="ts">
  import { Dialog } from "bits-ui";
  import X from "@lucide/svelte/icons/x";
  import Trash from "@lucide/svelte/icons/trash-2";
  import { EditableText, getDocument, notify } from "@hitslop/tinys/svelte";
  import schema from "./schema";
  import { addCard, cardStatus, removeCard, resetCard, type DeckView } from "./deck";

  interface Props { open: boolean; now: number; view: DeckView }
  let { open = $bindable(), now, view }: Props = $props();
  const doc = getDocument(schema);

  let front = $state("");
  let back = $state("");
  let adding = $state(false);
  let frontInput = $state<HTMLTextAreaElement>();
  const ready = $derived(front.trim() !== "" && back.trim() !== "" && !adding);

  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (!ready) return;
    adding = true;
    const [f, b] = [front, back];
    try {
      await doc.change(tx => addCard(tx, f, b));
      if (front === f) front = "";
      if (back === b) back = "";
      frontInput?.focus();
    } finally {
      adding = false;
    }
  }
  function submitOnEnter(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      (event.currentTarget as HTMLElement).closest("form")?.requestSubmit();
    }
  }
  async function remove(id: string) {
    await doc.change(tx => removeCard(tx, doc.current, id));
    notify("Card and its history removed. Undo with ⌘Z.");
  }
  async function repair(id: string) {
    await doc.change(tx => resetCard(tx, doc.current, id));
    notify("Card reset to new.");
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="deck-overlay" />
    <Dialog.Content class="deck-dialog deck-dialog-tall">
      <div class="deck-dialog-head">
        <Dialog.Title class="deck-dialog-title">Cards <span class="deck-count">{doc.current.cards.length}</span></Dialog.Title>
        <Dialog.Close class="deck-icon-button" aria-label="Close cards"><X size={20} /></Dialog.Close>
      </div>
      <Dialog.Description class="deck-dialog-description">New cards are introduced from the top. Click a card's text to edit it.</Dialog.Description>

      <form class="deck-composer" onsubmit={add}>
        <textarea bind:this={frontInput} bind:value={front} rows="2" aria-label="Front of a new card" placeholder="Front: the question" onkeydown={submitOnEnter}></textarea>
        <textarea bind:value={back} rows="2" aria-label="Back of a new card" placeholder="Back: the answer" onkeydown={submitOnEnter}></textarea>
        <button class="deck-primary" type="submit" disabled={!ready}>Add card</button>
      </form>

      <ol class="deck-cards">
        {#each doc.current.cards as card, index (card.$id)}
          {@const row = doc.at(card)}
          {@const status = cardStatus(card, view, now)}
          <li class="deck-card-row" data-tone={status.tone}>
            <span class="deck-card-number" aria-hidden="true">{index + 1}</span>
            <div class="deck-card-texts">
              <EditableText class="deck-card-front" field={row.front} label="Front of card {index + 1}" placeholder="Front" />
              <EditableText class="deck-card-back" field={row.back} label="Back of card {index + 1}" placeholder="Back" />
              <p class="deck-card-status"><b>{status.label}</b> · {status.detail}</p>
              {#if status.tone === "error"}
                <button class="deck-text-button" onclick={() => repair(card.$id)}>Reset this card to new</button>
              {/if}
            </div>
            <button class="deck-icon-button" aria-label="Remove card {index + 1}" onclick={() => remove(card.$id)}><Trash size={17} /></button>
          </li>
        {:else}
          <li class="deck-card-empty">No cards yet. Add the first one above.</li>
        {/each}
      </ol>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
