<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { Dialog } from "bits-ui";
  import { bindText } from "hitslop/svelte";
  import Check from "@lucide/svelte/icons/check";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import doc from "./schema";
  import {
    addCard, addLane, boardView, moveCard, pad, refugeFor, removeCard, removeLane, setDoneLane, setLaneLimit,
    type Card, type Lane,
  } from "./model";

  let draggingCardId = $state<string | null>(null);
  let draggingLaneKey = $state<string | null>(null);
  let dropLaneKey = $state<string | null>(null);
  let announcement = $state("");
  let composing = $state(false);
  let composingLaneKey = $state<string | null>(null);
  let draftTitle = $state("");
  let draftNote = $state("");
  let draftTag = $state("");
  let titleField = $state<HTMLInputElement>();

  const { cardsFor, isOverLimit, isDone, doneCount, openCount, overLimitCount } = $derived(boardView(doc.current));
  const composingLaneTitle = $derived(composingLaneKey ? laneTitle(composingLaneKey) : "lane");

  function laneIndex(laneKey: string): number { return doc.current.lanes.findIndex((lane) => lane.laneKey === laneKey); }
  function laneTitle(laneKey: string): string { return doc.current.lanes.find((lane) => lane.laneKey === laneKey)?.title.trim() || "lane"; }
  function cardName(card: Card): string { return card.title.trim() || "Work order"; }

  // Writes resolve once the snapshot shows them, so announcements read the new state.
  async function toggleDoneLane(lane: Lane) {
    await doc.change((tx) => setDoneLane(tx, doc.current, isDone(lane.laneKey) ? "" : lane.laneKey));
    announcement = isDone(lane.laneKey) ? `${lane.title} is now the completed lane.` : "No completed lane set.";
  }

  async function addLaneToBoard() {
    await doc.change((tx) => addLane(tx, doc.current, { title: `Lane ${doc.current.lanes.length + 1}`, laneKey: crypto.randomUUID() }));
    announcement = `Lane added. ${doc.current.lanes.length} lanes on the board.`;
  }

  function deleteLane(lane: Lane) {
    const refuge = refugeFor(doc.current, lane.laneKey);
    if (!refuge) return;
    const stranded = cardsFor(lane.laneKey).length;
    void doc.change((tx) => removeLane(tx, doc.current, lane.laneKey));
    announcement = `${lane.title || "Lane"} removed. ${stranded} orders moved to ${refuge.title}.`;
  }

  function reorderLane(laneKey: string, targetKey: string) {
    const source = doc.current.lanes.find((lane) => lane.laneKey === laneKey);
    const target = doc.current.lanes.find((lane) => lane.laneKey === targetKey);
    if (!source || !target || source.$id === target.$id) return;
    const destination = laneIndex(laneKey) < laneIndex(targetKey) ? { after: target.$id } : { before: target.$id };
    return doc.fields.lanes.move(source.$id, destination);
  }

  async function nudgeLane(lane: Lane, direction: -1 | 1) {
    const target = doc.current.lanes[laneIndex(lane.laneKey) + direction];
    if (!target) return;
    await reorderLane(lane.laneKey, target.laneKey);
    announcement = `${lane.title} is now lane ${laneIndex(lane.laneKey) + 1} of ${doc.current.lanes.length}.`;
  }

  function changeLimit(lane: Lane, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    // A refused or cleared value shows the saved limit again.
    const limit = input.value === "" ? undefined : Number(input.value) || 0;
    void doc.change((tx) => setLaneLimit(tx, doc.current, lane.laneKey, limit));
  }

  function openComposer(laneKey: string) {
    composingLaneKey = laneKey;
    draftTitle = "";
    draftNote = "";
    draftTag = "";
    composing = true;
  }

  async function punchTicket() {
    const laneKey = composingLaneKey;
    if (!draftTitle.trim() || !laneKey) return;
    // The dialog keeps the draft until the ticket is accepted.
    await doc.change((tx) => addCard(tx, doc.current, { laneKey, title: draftTitle, note: draftNote, tag: draftTag }));
    announcement = `Work order punched into ${laneTitle(laneKey)}.`;
    composing = false;
    composingLaneKey = null;
  }

  function deleteCard(card: Card) {
    void doc.change((tx) => removeCard(tx, card.$id)).then(() => (announcement = `${cardName(card)} removed.`));
  }

  function shiftLane(card: Card, direction: -1 | 1) {
    const target = doc.current.lanes[laneIndex(card.laneKey) + direction];
    if (!target) return;
    void doc.change((tx) => moveCard(tx, doc.current, card.$id, target.laneKey));
    announcement = `${cardName(card)} moved to ${target.title}.`;
  }

  function shiftWithinLane(card: Card, direction: -1 | 1) {
    const siblings = cardsFor(card.laneKey);
    const index = siblings.findIndex((item) => item.$id === card.$id);
    const neighbour = siblings[index + direction];
    if (!neighbour) return;
    doc.fields.cards.move(card.$id, direction < 0 ? { before: neighbour.$id } : { after: neighbour.$id });
    announcement = `${cardName(card)} moved to position ${index + direction + 1}.`;
  }

  function dropCard(laneKey: string) {
    const card = doc.current.cards.find((item) => item.$id === draggingCardId);
    draggingCardId = null;
    dropLaneKey = null;
    if (!card || card.laneKey === laneKey) return;
    void doc.change((tx) => moveCard(tx, doc.current, card.$id, laneKey));
    announcement = `${cardName(card)} moved to ${laneTitle(laneKey)}.`;
  }

  function handleLaneDrop(event: DragEvent, lane: Lane) {
    event.preventDefault();
    if (draggingLaneKey) {
      const moving = draggingLaneKey;
      draggingLaneKey = null;
      void reorderLane(moving, lane.laneKey)?.then(() => {
        announcement = `${laneTitle(moving)} moved to lane ${laneIndex(moving) + 1}.`;
      });
      return;
    }
    dropCard(lane.laneKey);
  }
</script>


  <main class="board-canvas" aria-label="Work-order board">
    <div class="board-chassis">
      <header class="board-rail">
        <div class="board-rail-mark" aria-hidden="true"><span></span><span></span></div>
        <div class="board-rail-name">
          <h1 class="board-sr-only">{doc.current.title}</h1>
          <input class="board-rail-title" aria-label="Board name" use:bindText={doc.fields.title} />
          <p class="board-rail-hint" data-slop-export="hide">Drag tickets between rails. Amber marks work over WIP.</p>
        </div>
        <dl class="board-meters">
          <div><dt>Open</dt><dd>{pad(openCount)}</dd></div>
          <div><dt>Done</dt><dd>{pad(doneCount)}</dd></div>
          <div class="board-meter-wip" data-alert={overLimitCount > 0}><dt>Over WIP</dt><dd>{pad(overLimitCount)}</dd></div>
        </dl>
      </header>

      <div class="board-deck">
        {#each doc.current.lanes as lane, index (lane.$id)}
          {@const cards = cardsFor(lane.laneKey)}
          <section class="board-lane" data-drop={dropLaneKey === lane.laneKey} data-over={isOverLimit(lane)} data-done={isDone(lane.laneKey)}
            aria-label="{lane.title} lane, {cards.length} work orders{lane.limit ? `, WIP ${cards.length} of ${lane.limit}` : ""}{isOverLimit(lane) ? ", over limit" : ""}"
            ondragover={(event) => { if (draggingCardId || draggingLaneKey) event.preventDefault(); if (draggingCardId) dropLaneKey = lane.laneKey; }}
            ondragleave={() => { if (dropLaneKey === lane.laneKey) dropLaneKey = null; }}
            ondrop={(event) => handleLaneDrop(event, lane)}>
            <header class="board-lane-head">
              <button class="board-lane-grip" type="button" draggable="true" data-slop-export="hide" aria-label="Reorder {lane.title} lane, position {index + 1} of {doc.current.lanes.length}"
                ondragstart={(event) => { draggingLaneKey = lane.laneKey; event.dataTransfer?.setData("text/plain", lane.laneKey); }}
                ondragend={() => { draggingLaneKey = null; dropLaneKey = null; }}
                onkeydown={(event) => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); nudgeLane(lane, event.key === "ArrowLeft" ? -1 : 1); } }}><GripVertical size={13} /></button>
              <input class="board-lane-title" aria-label="Lane {index + 1} name" use:bindText={doc.at(lane).title} />
              <button class="board-lane-remove" type="button" data-slop-export="hide" aria-label="Remove {lane.title} lane" title="Remove lane and move its orders to a neighbour" disabled={doc.current.lanes.length < 2} onclick={() => deleteLane(lane)}><X size={12} /></button>
              <div class="board-lane-meta">
                <label class="board-lane-limit" title="Maximum work orders in this lane">
                  <span class="board-wip-lamp" data-alert={isOverLimit(lane)} aria-hidden="true"></span><span>Max</span>
                  <input aria-label="{lane.title} maximum work orders" type="number" min="0" placeholder="–" value={lane.limit ?? ""} onchange={(event) => changeLimit(lane, event)} />
                </label>
                <button class="board-lane-done" type="button" data-slop-export={isDone(lane.laneKey) ? undefined : "hide"} aria-pressed={isDone(lane.laneKey)} aria-label="Mark {lane.title} as the completed lane" title="Completed lane" onclick={() => toggleDoneLane(lane)}><Check size={10} strokeWidth={3.5} /><span>Done</span></button>
              </div>
            </header>

            <div class="board-lane-slot">
              {#each cards as card, position (card.$id)}
                <article class="board-ticket" data-dragging={draggingCardId === card.$id} draggable="true"
                  ondragstart={(event) => { event.stopPropagation(); draggingCardId = card.$id; event.dataTransfer?.setData("text/plain", card.$id); }}
                  ondragend={() => { draggingCardId = null; dropLaneKey = null; }}>
                  <div class="board-ticket-stub" aria-hidden="true"><span class="board-punch"></span><span>{pad(position + 1)}</span></div>
                  <div class="board-ticket-body">
                    <textarea class="board-ticket-title" rows="1" aria-label="Work order title" use:bindText={doc.at(card).title}></textarea>
                    <textarea class="board-ticket-note" rows="2" placeholder="Notes" aria-label="Work order notes" use:bindText={doc.at(card).note}></textarea>
                    <div class="board-ticket-foot">
                      <input class="board-ticket-tag" placeholder="tag" aria-label="Work order tag" use:bindText={doc.at(card).tag} />
                      <div class="board-ticket-actions" data-slop-export="hide">
                        <button type="button" aria-label="Move to previous lane" disabled={index === 0} onclick={() => shiftLane(card, -1)}><ArrowLeft size={11} /></button>
                        <button type="button" aria-label="Move up in lane" disabled={position === 0} onclick={() => shiftWithinLane(card, -1)}><ArrowUp size={11} /></button>
                        <button type="button" aria-label="Move down in lane" disabled={position === cards.length - 1} onclick={() => shiftWithinLane(card, 1)}><ArrowDown size={11} /></button>
                        <button type="button" aria-label="Move to next lane" disabled={index === doc.current.lanes.length - 1} onclick={() => shiftLane(card, 1)}><ArrowRight size={11} /></button>
                        <button type="button" aria-label="Remove work order" onclick={() => deleteCard(card)}><Trash2 size={11} /></button>
                      </div>
                    </div>
                  </div>
                </article>
              {/each}
              {#if cards.length === 0}<p class="board-lane-empty">Empty slot<span>Punch a work order to load this rail.</span></p>{/if}
              <button type="button" class="board-lane-add" data-slop-export="hide" onclick={() => openComposer(lane.laneKey)}><Plus size={12} />Punch ticket</button>
            </div>
          </section>
        {/each}
        <button type="button" class="board-deck-add" data-slop-export="hide" aria-label="Add lane" onclick={addLaneToBoard}><Plus size={14} /><span>Lane</span></button>
      </div>
      <p class="board-announcement" role="status" aria-live="polite">{announcement}</p>
    </div>
  </main>


<Dialog.Root bind:open={composing} onOpenChange={(open) => { if (!open) composingLaneKey = null; }}>
  <Dialog.Portal>
    <Dialog.Overlay class="board-overlay" data-slop-export="hide" />
    <Dialog.Content class="board-dialog" data-slop-export="hide" onOpenAutoFocus={(event) => { event.preventDefault(); queueMicrotask(() => titleField?.focus()); }}>
      <div class="board-dialog-head"><div><p class="board-dialog-eyebrow">Work order</p><Dialog.Title>Punch a ticket</Dialog.Title><Dialog.Description>Slot this order into {composingLaneTitle}. Drag it to another rail when work moves.</Dialog.Description></div><Dialog.Close class="board-dialog-close" aria-label="Close"><X size={14} /></Dialog.Close></div>
      <form class="board-dialog-form" onsubmit={(event) => { event.preventDefault(); void punchTicket(); }}>
        <label><span>Job title</span><input bind:this={titleField} bind:value={draftTitle} placeholder="Rebuild the spindle jig" required /></label>
        <label><span>Notes</span><textarea rows="3" bind:value={draftNote} placeholder="Setup, parts, or hold-ups"></textarea></label>
        <label><span>Tag</span><input bind:value={draftTag} placeholder="shop" /></label>
        <div class="board-dialog-actions"><button class="board-btn-secondary" type="button" onclick={() => { composing = false; composingLaneKey = null; }}>Cancel</button><button class="board-btn-primary" type="submit" disabled={!draftTitle.trim()}>Slot ticket</button></div>
      </form>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
