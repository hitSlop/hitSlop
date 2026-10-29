<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { getDocument } from "@hitslop/tinys/svelte";
  import { untrack } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { Button } from "bits-ui";
  import Flag from "@lucide/svelte/icons/flag";
  import Bomb from "@lucide/svelte/icons/bomb";
  import Rotate from "@lucide/svelte/icons/rotate-ccw";
  import schema from "./schema";
  import { LEVELS, boardView, clock, dig as digCell, newGame as startGame, toggleFlag, type Level } from "./game";

  const doc = getDocument(schema);
  const game = $derived(doc.current);
  const board = $derived(boardView(game));
  const level = $derived(board.level);
  const size = $derived(board.size);
  const over = $derived(board.over);
  const best = $derived(board.best);

  let flagMode = $state(false);
  let shown = $state(0);
  let held = false;
  let pressTimer: ReturnType<typeof setTimeout> | undefined;

  // The clock is presentation; its value is saved with each move.
  $effect(() => {
    if (game.status !== "playing") {
      shown = untrack(() => doc.current.seconds);
      return;
    }
    const base = untrack(() => doc.current.seconds);
    const started = Date.now();
    shown = base;
    const tick = setInterval(() => (shown = base + Math.floor((Date.now() - started) / 1000)), 500);
    return () => clearInterval(tick);
  });

  function dig(index: number) {
    if (over || game.cells[index] !== "h") return;
    doc.change((tx) => digCell(tx, game, index, Math.random, shown));
  }

  function flag(index: number) {
    if (over || game.cells[index] === "r") return;
    doc.change((tx) => toggleFlag(tx, game, index));
  }

  function tap(index: number) {
    if (held) { held = false; return; }
    if (flagMode) flag(index); else dig(index);
  }

  function press(index: number) {
    held = false;
    clearTimeout(pressTimer);
    pressTimer = setTimeout(() => { held = true; flag(index); }, 450);
  }
  const release = () => clearTimeout(pressTimer);

  function newGame(next: Level = level) {
    doc.change((tx) => startGame(tx, next));
  }

  function label(index: number): string {
    const cell = board.cells[index]!;
    const where = `row ${Math.floor(index / size.cols) + 1}, column ${(index % size.cols) + 1}`;
    if (cell.mark === "f") return `${where}: buoy`;
    if (cell.mark === "h") return `${where}: unknown water`;
    if (cell.mine) return `${where}: mine`;
    return `${where}: ${cell.near ? `${cell.near} mines nearby` : "clear"}`;
  }
</script>

<main class="sea" aria-label="Minesweeper">
  <article class="chart" data-reduced={prefersReducedMotion.current}>
    <header class="head">
      <p class="kicker">Harbor chart · clear the water</p>
      <h1>Buoy Sweep</h1>
      <div class="levels" role="group" aria-label="Board size">
        {#each Object.keys(LEVELS) as name}
          <Button.Root class="level" aria-pressed={level === name} onclick={() => name !== level && newGame(name as Level)}>{name}</Button.Root>
        {/each}
      </div>
    </header>

    <div class="readout" role="status">
      <span><b>{board.flagsLeft}</b> buoys left</span>
      <span class="state" data-status={game.status}>
        {#if game.status === "won"}Channel clear!{:else if game.status === "lost"}Hit a mine.{:else if game.status === "playing"}Sweeping…{:else}Tap any water to start{/if}
      </span>
      <span><b>{clock(shown)}</b>{#if best} · best {clock(best)}{/if}</span>
    </div>

    <div class="board" style:--cols={size.cols} data-over={over} role="grid" aria-label="Minefield">
      {#each board.cells as cell (cell.index)}
        {@const index = cell.index}
        <button
          class="cell"
          role="gridcell"
          data-open={cell.open}
          data-flag={cell.mark === "f"}
          data-mine={cell.boom}
          data-n={cell.near}
          aria-label={label(index)}
          disabled={over && !(game.status === "lost" && cell.mine)}
          onclick={() => tap(index)}
          onpointerdown={() => press(index)}
          onpointerup={release}
          onpointerleave={release}
          oncontextmenu={(event) => { event.preventDefault(); flag(index); }}
        >
          {#if cell.mark === "f"}<Flag size={16} strokeWidth={2.6} />
          {:else if cell.showMine}<Bomb size={16} strokeWidth={2.4} />
          {:else if cell.near}{cell.near}{/if}
        </button>
      {/each}
    </div>

    <footer class="foot">
      <Button.Root class="mode" aria-pressed={flagMode} onclick={() => (flagMode = !flagMode)} disabled={over}><Flag size={15} />Buoy mode</Button.Root>
      <Button.Root class="again" onclick={() => newGame()}><Rotate size={15} />{over ? "Play again" : "New board"}</Button.Root>
      <span class="record">{game.wins} won · {game.losses} lost</span>
    </footer>
  </article>
</main>
