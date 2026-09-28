<script lang="ts">
  import { Dialog } from "bits-ui";
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import Check from "@lucide/svelte/icons/check";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import schema, { type Card, type Lane } from "./schema";

  const doc = useDocument(schema);
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

  const laneCards = $derived.by(() => {
    const grouped = new Map<string, Card[]>();
    for (const lane of doc.current.lanes) grouped.set(lane.laneKey, []);
    for (const card of doc.current.cards) grouped.get(card.laneKey)?.push(card);
    for (const cards of grouped.values()) cards.sort((a, b) => a.order - b.order);
    return grouped;
  });
  const doneCount = $derived(doc.current.doneLaneKey ? cardsFor(doc.current.doneLaneKey).length : 0);
  const openCount = $derived(doc.current.cards.length - doneCount);
  const overLimitCount = $derived(doc.current.lanes.filter(isOverLimit).length);
  const composingLaneTitle = $derived(composingLaneKey ? laneTitle(composingLaneKey) : "lane");

  function cardsFor(laneKey: string): Card[] { return laneCards.get(laneKey) ?? []; }
  function isOverLimit(lane: Lane): boolean { return lane.limit !== undefined && lane.limit > 0 && cardsFor(lane.laneKey).length > lane.limit; }
  function laneIndex(laneKey: string): number { return doc.current.lanes.findIndex((lane) => lane.laneKey === laneKey); }
  function laneTitle(laneKey: string): string { return doc.current.lanes.find((lane) => lane.laneKey === laneKey)?.title.trim() || "lane"; }
  function isDone(laneKey: string): boolean { return doc.current.doneLaneKey === laneKey; }
  function nextOrder(laneKey: string): number { return cardsFor(laneKey).reduce((max, card) => Math.max(max, card.order + 1), 0); }
  function pad(value: number): string { return String(value).padStart(2, "0"); }

  function toggleDoneLane(lane: Lane) {
    if (isDone(lane.laneKey)) doc.fields.doneLaneKey.clear();
    else doc.fields.doneLaneKey.set(lane.laneKey);
    announcement = isDone(lane.laneKey) ? "No completed lane set." : `${lane.title} is now the completed lane.`;
  }

  function addLane() {
    const laneKey = crypto.randomUUID();
    doc.fields.lanes.insert({ laneKey, title: `Lane ${doc.current.lanes.length + 1}` });
    announcement = `Lane added. ${doc.current.lanes.length + 1} lanes on the board.`;
  }

  function removeLane(lane: Lane) {
    const index = laneIndex(lane.laneKey);
    if (doc.current.lanes.length < 2 || index < 0) return;
    const refuge = doc.current.lanes[index === 0 ? 1 : index - 1];
    if (!refuge) return;
    const stranded = cardsFor(lane.laneKey);
    let order = nextOrder(refuge.laneKey);
    doc.change((tx) => {
      for (const card of stranded) {
        const handle = tx.fields.cards.item(card.$id);
        handle.laneKey.set(refuge.laneKey);
        handle.order.set(order++);
      }
      tx.fields.lanes.remove(lane.$id);
      if (doc.current.doneLaneKey === lane.laneKey) tx.fields.doneLaneKey.clear();
    }, { message: "Remove board lane" });
    announcement = `${lane.title || "Lane"} removed. ${stranded.length} orders moved to ${refuge.title}.`;
  }

  function reorderLane(laneKey: string, targetKey: string) {
    const source = doc.current.lanes.find((lane) => lane.laneKey === laneKey);
    const target = doc.current.lanes.find((lane) => lane.laneKey === targetKey);
    if (!source || !target || source.$id === target.$id) return;
    if (laneIndex(laneKey) < laneIndex(targetKey)) doc.fields.lanes.move(source.$id, { after: target.$id });
    else doc.fields.lanes.move(source.$id, { before: target.$id });
  }

  function nudgeLane(lane: Lane, direction: -1 | 1) {
    const target = doc.current.lanes[laneIndex(lane.laneKey) + direction];
    if (!target) return;
    reorderLane(lane.laneKey, target.laneKey);
    announcement = `${lane.title} is now lane ${laneIndex(lane.laneKey) + 1} of ${doc.current.lanes.length}.`;
  }

  function setLaneLimit(lane: Lane, event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    const handle = doc.at(lane).limit;
    if (value === "") handle.clear();
    else handle.set(Math.max(0, Math.floor(Number(value) || 0)));
  }

  function openComposer(laneKey: string) {
    composingLaneKey = laneKey;
    draftTitle = "";
    draftNote = "";
    draftTag = "";
    composing = true;
  }

  function punchTicket() {
    const title = draftTitle.trim();
    const laneKey = composingLaneKey;
    if (!title || !laneKey) return;
    doc.fields.cards.insert({ laneKey, title, note: draftNote.trim(), tag: draftTag.trim(), order: nextOrder(laneKey) });
    announcement = `Work order punched into ${laneTitle(laneKey)}.`;
    composing = false;
    composingLaneKey = null;
  }

  function removeCard(card: Card) {
    doc.fields.cards.remove(card.$id);
    announcement = `${card.title.trim() || "Work order"} removed.`;
  }

  function shiftLane(card: Card, direction: -1 | 1) {
    const target = doc.current.lanes[laneIndex(card.laneKey) + direction];
    if (!target) return;
    const order = nextOrder(target.laneKey);
    doc.change((tx) => {
      const handle = tx.fields.cards.item(card.$id);
      handle.laneKey.set(target.laneKey);
      handle.order.set(order);
    }, { message: "Move work order" });
    announcement = `${card.title.trim() || "Work order"} moved to ${target.title}.`;
  }

  function shiftWithinLane(card: Card, direction: -1 | 1) {
    const siblings = cardsFor(card.laneKey);
    const index = siblings.findIndex((item) => item.$id === card.$id);
    const neighbour = siblings[index + direction];
    if (!neighbour) return;
    doc.change((tx) => {
      tx.fields.cards.item(card.$id).order.set(neighbour.order);
      tx.fields.cards.item(neighbour.$id).order.set(card.order);
    }, { message: "Reorder work order" });
    announcement = `${card.title.trim() || "Work order"} moved to position ${index + direction + 1}.`;
  }

  function dropCard(laneKey: string) {
    const card = doc.current.cards.find((item) => item.$id === draggingCardId);
    draggingCardId = null;
    dropLaneKey = null;
    if (!card || card.laneKey === laneKey) return;
    const order = nextOrder(laneKey);
    doc.change((tx) => {
      const handle = tx.fields.cards.item(card.$id);
      handle.laneKey.set(laneKey);
      handle.order.set(order);
    }, { message: "Move work order" });
    announcement = `${card.title.trim() || "Work order"} moved to ${laneTitle(laneKey)}.`;
  }

  function handleLaneDrop(event: DragEvent, lane: Lane) {
    event.preventDefault();
    if (draggingLaneKey) {
      reorderLane(draggingLaneKey, lane.laneKey);
      announcement = `${laneTitle(draggingLaneKey)} moved to lane ${laneIndex(draggingLaneKey) + 1}.`;
      draggingLaneKey = null;
      return;
    }
    dropCard(lane.laneKey);
  }
</script>

<Slop>
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
              <button class="board-lane-remove" type="button" data-slop-export="hide" aria-label="Remove {lane.title} lane" title="Remove lane and move its orders to a neighbour" disabled={doc.current.lanes.length < 2} onclick={() => removeLane(lane)}><X size={12} /></button>
              <div class="board-lane-meta">
                <label class="board-lane-limit" title="Maximum work orders in this lane">
                  <span class="board-wip-lamp" data-alert={isOverLimit(lane)} aria-hidden="true"></span><span>Max</span>
                  <input aria-label="{lane.title} maximum work orders" type="number" min="0" placeholder="–" value={lane.limit ?? ""} onchange={(event) => setLaneLimit(lane, event)} />
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
                        <button type="button" aria-label="Remove work order" onclick={() => removeCard(card)}><Trash2 size={11} /></button>
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
        <button type="button" class="board-deck-add" data-slop-export="hide" aria-label="Add lane" onclick={addLane}><Plus size={14} /><span>Lane</span></button>
      </div>
      <p class="board-announcement" role="status" aria-live="polite">{announcement}</p>
    </div>
  </main>

  {#snippet exportView()}
    <main class="board-canvas board-export">
      <div class="board-chassis">
        <header class="board-rail"><div class="board-rail-mark" aria-hidden="true"><span></span><span></span></div><div class="board-rail-name"><h1 class="board-rail-title">{doc.current.title}</h1><p>Work-order schedule</p></div><dl class="board-meters"><div><dt>Open</dt><dd>{pad(openCount)}</dd></div><div><dt>Done</dt><dd>{pad(doneCount)}</dd></div><div class="board-meter-wip" data-alert={overLimitCount > 0}><dt>Over WIP</dt><dd>{pad(overLimitCount)}</dd></div></dl></header>
        <div class="board-deck">
          {#each doc.current.lanes as lane (lane.$id)}
            {@const cards = cardsFor(lane.laneKey)}
            <section class="board-lane" data-over={isOverLimit(lane)} data-done={isDone(lane.laneKey)}>
              <header class="board-lane-head"><span class="board-lane-title">{lane.title}</span>{#if lane.limit}<span class="board-lane-limit"><span class="board-wip-lamp" data-alert={isOverLimit(lane)}></span>Max {lane.limit}</span>{/if}</header>
              <div class="board-lane-slot">
                {#each cards as card, position (card.$id)}
                  <article class="board-ticket"><div class="board-ticket-stub"><span class="board-punch"></span><span>{pad(position + 1)}</span></div><div class="board-ticket-body"><strong class="board-ticket-title">{card.title}</strong><p class="board-ticket-note">{card.note}</p><span class="board-ticket-tag">{card.tag}</span></div></article>
                {/each}
                {#if cards.length === 0}<p class="board-lane-empty">Empty slot</p>{/if}
              </div>
            </section>
          {/each}
        </div>
      </div>
    </main>
  {/snippet}

  {#snippet icon()}
    <div class="board-icon" aria-hidden="true"><div class="board-icon-body"><i></i><i></i><i></i><span></span></div></div>
  {/snippet}
</Slop>

<Dialog.Root bind:open={composing} onOpenChange={(open) => { if (!open) composingLaneKey = null; }}>
  <Dialog.Portal>
    <Dialog.Overlay class="board-overlay" data-slop-export="hide" />
    <Dialog.Content class="board-dialog" data-slop-export="hide" onOpenAutoFocus={(event) => { event.preventDefault(); queueMicrotask(() => titleField?.focus()); }}>
      <div class="board-dialog-head"><div><p class="board-dialog-eyebrow">Work order</p><Dialog.Title>Punch a ticket</Dialog.Title><Dialog.Description>Slot this order into {composingLaneTitle}. Drag it to another rail when work moves.</Dialog.Description></div><Dialog.Close class="board-dialog-close" aria-label="Close"><X size={14} /></Dialog.Close></div>
      <form class="board-dialog-form" onsubmit={(event) => { event.preventDefault(); punchTicket(); }}>
        <label><span>Job title</span><input bind:this={titleField} bind:value={draftTitle} placeholder="Rebuild the spindle jig" required /></label>
        <label><span>Notes</span><textarea rows="3" bind:value={draftNote} placeholder="Setup, parts, or hold-ups"></textarea></label>
        <label><span>Tag</span><input bind:value={draftTag} placeholder="shop" /></label>
        <div class="board-dialog-actions"><button class="board-btn-secondary" type="button" onclick={() => { composing = false; composingLaneKey = null; }}>Cancel</button><button class="board-btn-primary" type="submit" disabled={!draftTitle.trim()}>Slot ticket</button></div>
      </form>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
