<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { boardView, clock } from "./game";

  let { snapshot }: CaptureProps<typeof schema> = $props();

  const game = $derived(snapshot);
  const board = $derived(boardView(game));
  const verdict = $derived(game.status === "won" ? "Channel clear!" : game.status === "lost" ? "Hit a mine." : game.status === "playing" ? "In progress" : "Fresh board");
</script>

<article class="sea export" aria-label="Exported minesweeper board">
  <div class="chart">
    <header class="head">
      <p class="kicker">Harbor chart · {board.level}</p>
      <h1>Buoy Sweep</h1>
    </header>
    <p class="readout"><span class="state" data-status={game.status}>{verdict}</span><span><b>{clock(game.seconds)}</b>{#if board.best} · best {clock(board.best)}{/if}</span></p>
    <div class="board" style:--cols={board.size.cols}>
      {#each board.cells as cell (cell.index)}
        <span class="cell" data-open={cell.open} data-flag={cell.mark === "f"} data-mine={cell.boom} data-n={cell.near}>
          {#if cell.mark === "f"}⚑{:else if cell.showMine}✹{:else if cell.near}{cell.near}{/if}
        </span>
      {/each}
    </div>
    <p class="foot"><span class="record">{game.wins} won · {game.losses} lost</span></p>
  </div>
</article>
