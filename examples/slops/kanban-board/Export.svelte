<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
import { boardView, pad } from "./model";
const doc = useDocument(schema);
const { cardsFor, isOverLimit, isDone, doneCount, openCount, overLimitCount } = $derived(boardView(doc.current));
</script>

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
