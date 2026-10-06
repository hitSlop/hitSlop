<script lang="ts">
  import { untrack } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { Button } from "bits-ui";
  import Flag from "@lucide/svelte/icons/flag";
  import Bomb from "@lucide/svelte/icons/bomb";
  import Rotate from "@lucide/svelte/icons/rotate-ccw";
  import doc from "./schema";
  import { LEVELS, adjacent, clock, cleared, flagsLeft, hiddenBoard, layMines, reveal, type Level } from "./game";

  const game = $derived(doc.current);
  const level = $derived(game.level as Level);
  const size = $derived(LEVELS[level]);
  const over = $derived(game.status === "won" || game.status === "lost");
  const best = $derived(game.best[level]);

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
    const mines = game.mines || layMines(level, index);
    const result = reveal(level, game.cells, mines, index);
    const won = !result.boom && cleared(result.cells, mines);
    const seconds = game.status === "playing" ? shown : 0;
    doc.change((tx) => {
      tx.fields.mines.set(mines);
      tx.fields.cells.set(result.cells);
      tx.fields.seconds.set(seconds);
      tx.fields.status.set(result.boom ? "lost" : won ? "won" : "playing");
      if (result.boom) tx.fields.losses.increment();
      if (won) {
        tx.fields.wins.increment();
        if (!best || seconds < best) tx.fields.best[level].set(Math.max(seconds, 1));
      }
    });
  }

  function flag(index: number) {
    if (over) return;
    const mark = game.cells[index];
    if (mark === "r") return;
    doc.fields.cells.set(game.cells.slice(0, index) + (mark === "f" ? "h" : "f") + game.cells.slice(index + 1));
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
    doc.change((tx) => {
      tx.fields.level.set(next);
      tx.fields.status.set("ready");
      tx.fields.cells.set(hiddenBoard(next));
      tx.fields.mines.set("");
      tx.fields.seconds.set(0);
    });
  }

  function label(index: number): string {
    const mark = game.cells[index];
    const where = `row ${Math.floor(index / size.cols) + 1}, column ${(index % size.cols) + 1}`;
    if (mark === "f") return `${where}: buoy`;
    if (mark === "h") return `${where}: unknown water`;
    if (game.mines[index] === "1") return `${where}: mine`;
    const near = adjacent(level, game.mines, index);
    return `${where}: ${near ? `${near} mines nearby` : "clear"}`;
  }
</script>

<main class="sea" data-slop-selection="none" aria-label="Minesweeper">
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
      <span><b>{flagsLeft(level, game.cells)}</b> buoys left</span>
      <span class="state" data-status={game.status}>
        {#if game.status === "won"}Channel clear!{:else if game.status === "lost"}Hit a mine.{:else if game.status === "playing"}Sweeping…{:else}Tap any water to start{/if}
      </span>
      <span><b>{clock(shown)}</b>{#if best} · best {clock(best)}{/if}</span>
    </div>

    <div class="board" style:--cols={size.cols} data-over={over} role="grid" aria-label="Minefield">
      {#each game.cells as mark, index (index)}
        {@const near = mark === "r" && game.mines[index] !== "1" ? adjacent(level, game.mines, index) : 0}
        {@const mine = game.mines[index] === "1"}
        <button
          class="cell"
          role="gridcell"
          data-open={mark === "r"}
          data-flag={mark === "f"}
          data-mine={(mark === "r" && mine) || (game.status === "lost" && mine && mark !== "f")}
          data-n={near}
          aria-label={label(index)}
          disabled={over && !(game.status === "lost" && mine)}
          onclick={() => tap(index)}
          onpointerdown={() => press(index)}
          onpointerup={release}
          onpointerleave={release}
          oncontextmenu={(event) => { event.preventDefault(); flag(index); }}
        >
          {#if mark === "f"}<Flag size={16} strokeWidth={2.6} />
          {:else if mine && (mark === "r" || game.status === "lost")}<Bomb size={16} strokeWidth={2.4} />
          {:else if near}{near}{/if}
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
