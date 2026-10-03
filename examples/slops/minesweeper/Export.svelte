<script lang="ts">
  import doc from "./schema";
  import { LEVELS, adjacent, clock, type Level } from "./game";

  const game = $derived(doc.current);
  const level = $derived(game.level as Level);
  const size = $derived(LEVELS[level]);
  const verdict = $derived(game.status === "won" ? "Channel clear!" : game.status === "lost" ? "Hit a mine." : game.status === "playing" ? "In progress" : "Fresh board");
</script>

<article class="sea export" aria-label="Exported minesweeper board">
  <div class="chart">
    <header class="head">
      <p class="kicker">Harbor chart · {level}</p>
      <h1>Buoy Sweep</h1>
    </header>
    <p class="readout"><span class="state" data-status={game.status}>{verdict}</span><span><b>{clock(game.seconds)}</b>{#if game.best[level]} · best {clock(game.best[level])}{/if}</span></p>
    <div class="board" style:--cols={size.cols}>
      {#each game.cells as mark, index (index)}
        {@const mine = game.mines[index] === "1"}
        {@const near = mark === "r" && !mine ? adjacent(level, game.mines, index) : 0}
        <span class="cell" data-open={mark === "r"} data-flag={mark === "f"} data-mine={mine && (mark === "r" || game.status === "lost")} data-n={near}>
          {#if mark === "f"}⚑{:else if mine && (mark === "r" || game.status === "lost")}✹{:else if near}{near}{/if}
        </span>
      {/each}
    </div>
    <p class="foot"><span class="record">{game.wins} won · {game.losses} lost</span></p>
  </div>
</article>
